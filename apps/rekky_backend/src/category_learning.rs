//! Background vocabulary learning. No raw note or model-authored executable policy.
use crate::taxonomy::{self, Concept, Vocabulary};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgPool, Row};
use std::{collections::BTreeSet, env, time::Duration};
use uuid::Uuid;

pub const FOCUS: &[&str] = &[
    "work_performed",
    "experience_duration",
    "price_basis",
    "compatibility",
    "limitations",
    "service_area",
    "venue",
    "specific_variant",
    "accessibility",
    "attribution",
];

pub async fn catalog(pool: &PgPool) -> Result<Vocabulary, sqlx::Error> {
    let row = sqlx::query("SELECT revision, (SELECT coalesce(jsonb_agg(definition ORDER BY id),'[]') FROM learned_categories WHERE active) concepts, (SELECT coalesce(jsonb_agg(jsonb_build_object('phrase',phrase,'concept_id',concept_id) ORDER BY normalized),'[]') FROM learned_category_aliases WHERE active) aliases FROM category_registry_state WHERE id")
        .fetch_one(pool).await?;
    let mut catalog = taxonomy::vocabulary().clone();
    catalog.version = catalog.version * 1_000_000 + row.get::<i32, _>("revision");
    let learned: Value = row.get("concepts");
    for value in learned.as_array().into_iter().flatten() {
        if let Ok(c) = serde_json::from_value::<Concept>(value.clone()) {
            catalog.concepts.push(c);
        }
    }
    let aliases: Value = row.get("aliases");
    for value in aliases.as_array().into_iter().flatten() {
        if let Some(c) = catalog
            .concepts
            .iter_mut()
            .find(|c| value["concept_id"] == c.id)
            && let Some(phrase) = value["phrase"].as_str()
            && !c.aliases.iter().any(|a| normalize(a) == normalize(phrase))
        {
            c.aliases.push(phrase.to_owned());
        }
    }
    Ok(catalog)
}
pub fn normalize(text: &str) -> String {
    taxonomy::words(text).join(" ")
}
pub fn job_key(kind: &str, phrase: &str) -> String {
    hex::encode(Sha256::digest(format!(
        "learning-v1:{kind}:{}",
        normalize(phrase)
    )))
}
fn plain_term(text: &str) -> bool {
    let count = taxonomy::words(text).len();
    (1..=6).contains(&count)
        && text.chars().count() <= 60
        && text
            .chars()
            .all(|c| c.is_alphabetic() || c.is_whitespace() || ['-', '\''].contains(&c))
        && ![
            "ignore",
            "instruction",
            "system",
            "prompt",
            "assistant",
            "http",
        ]
        .iter()
        .any(|w| taxonomy::words(text).iter().any(|t| t == w))
}
fn generic_text(text: &str, max: usize) -> bool {
    !text.trim().is_empty()
        && text.chars().count() <= max
        && !text.chars().any(|c| c.is_numeric() || c.is_control())
        && !["http", "www.", "@", "ignore previous", "system prompt"]
            .iter()
            .any(|x| text.to_lowercase().contains(x))
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Definition {
    pub decision: String,
    pub existing_id: String,
    pub label: String,
    pub definition: String,
    pub parent_id: String,
    pub aliases: Vec<String>,
    pub evidence_focus: Vec<String>,
    pub positive_examples: Vec<String>,
    pub negative_examples: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Review {
    pub approve: bool,
    pub generic_no_private_identity: bool,
    pub correct_meaning: bool,
    pub equivalent_aliases: bool,
    pub no_existing_duplicate: bool,
    pub compatible_kind_and_parent: bool,
    pub examples_correct: bool,
    pub safe_optional_focus: bool,
}
impl Review {
    pub fn accepted(&self) -> bool {
        self.approve
            && self.generic_no_private_identity
            && self.correct_meaning
            && self.equivalent_aliases
            && self.no_existing_duplicate
            && self.compatible_kind_and_parent
            && self.examples_correct
            && self.safe_optional_focus
    }
}
/// Deterministic checks complement the separate semantic review; neither proves
/// all semantics. A catalog revision change requires re-evaluation, not blind publication.
pub fn check_definition(
    d: &Definition,
    term: &str,
    kind: &str,
    catalog: &Vocabulary,
) -> Result<String, &'static str> {
    if d.decision == "defer" {
        return Err("provisional");
    }
    if ![
        "place",
        "person_service",
        "thing",
        "activity_event",
        "idea_tip",
    ]
    .contains(&kind)
        || !plain_term(term)
        || !plain_term(&d.label)
        || !generic_text(&d.definition, 400)
        || d.aliases.is_empty()
        || !d.parent_id.is_empty()
        || d.aliases.len() > 2
        || d.aliases
            .iter()
            .any(|a| normalize(a) != normalize(term) && normalize(a) != normalize(&d.label))
        || !d.aliases.iter().all(|a| plain_term(a))
        || !d.aliases.iter().any(|a| normalize(a) == normalize(term))
        || d.evidence_focus.len() > 5
        || !d.evidence_focus.iter().all(|f| FOCUS.contains(&f.as_str()))
        || d.positive_examples.len() != 2
        || d.negative_examples.len() != 2
        || !d
            .positive_examples
            .iter()
            .chain(&d.negative_examples)
            .all(|s| generic_text(s, 240))
    {
        return Err("invalid_definition");
    }
    let positive: BTreeSet<_> = d.positive_examples.iter().map(|s| normalize(s)).collect();
    if positive.len() != 2
        || d.negative_examples
            .iter()
            .any(|s| positive.contains(&normalize(s)))
    {
        return Err("invalid_examples");
    }
    let id = match d.decision.as_str() {
        "alias" => {
            let c = taxonomy::concept_in(catalog, &d.existing_id).ok_or("unknown_existing_type")?;
            if c.dimension != "type"
                || !c.entity_kinds.contains(&kind.to_owned())
                || d.label != c.label
            {
                return Err("incompatible_type");
            }
            c.id.clone()
        }
        "create" if d.existing_id.is_empty() => {
            // Learning adds a flat type within a known shelf. It cannot invent
            // a hierarchy, broaden the encountered term or mint extra synonyms.
            if normalize(&d.label) != normalize(term) {
                return Err("broadened_label");
            }
            format!("learned.{}", &job_key(kind, &d.label)[..24])
        }
        _ => return Err("provisional"),
    };
    for alias in d.aliases.iter().chain(std::iter::once(&d.label)) {
        if catalog.concepts.iter().any(|c| {
            c.id != id
                && (normalize(&c.label) == normalize(alias)
                    || c.aliases.iter().any(|a| normalize(a) == normalize(alias)))
        }) {
            return Err("alias_collision");
        }
    }
    Ok(id)
}

#[async_trait]
pub trait CategoryLearner: Send + Sync {
    fn available(&self) -> bool;
    async fn propose(&self, term: &str, kind: &str, catalog: &Vocabulary)
    -> Result<Definition, ()>;
    async fn review(
        &self,
        term: &str,
        kind: &str,
        proposal: &Definition,
        catalog: &Vocabulary,
    ) -> Result<Review, ()>;
}
pub struct OpenAiCategoryLearner {
    client: reqwest::Client,
    key: Option<String>,
    enabled: bool,
}
fn object(properties: Value) -> Value {
    let required: Vec<_> = properties.as_object().unwrap().keys().cloned().collect();
    json!({"type":"object","additionalProperties":false,"required":required,"properties":properties})
}
fn string() -> Value {
    json!({"type":"string"})
}
impl Default for OpenAiCategoryLearner {
    fn default() -> Self {
        Self::from_env()
    }
}
impl OpenAiCategoryLearner {
    pub fn from_env() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(40))
                .build()
                .expect("client"),
            key: env::var("OPENAI_API_KEY").ok().filter(|s| !s.is_empty()),
            enabled: env::var("CATEGORY_LEARNING_ENABLED").as_deref() == Ok("true"),
        }
    }
    async fn call(&self, instructions: &str, input: Value, schema: Value) -> Result<Value, ()> {
        if !self.available() {
            return Err(());
        }
        let response = self.client.post("https://api.openai.com/v1/responses").bearer_auth(self.key.as_ref().ok_or(())?)
            .json(&json!({"model":"gpt-4.1-mini","store":false,"max_output_tokens":1800,
                "input":[{"role":"system","content":instructions},{"role":"user","content":input.to_string()}],
                "text":{"format":{"type":"json_schema","name":"category_learning_v1","strict":true,"schema":schema}}}))
            .send().await.map_err(|_| ())?;
        if !response.status().is_success() {
            return Err(());
        }
        let value: Value = response.json().await.map_err(|_| ())?;
        if value["status"] != "completed" {
            return Err(());
        }
        let text = value["output"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|o| o["content"].as_array().into_iter().flatten())
            .find(|c| c["type"] == "output_text")
            .and_then(|c| c["text"].as_str())
            .ok_or(())?;
        serde_json::from_str(text).map_err(|_| ())
    }
}
#[async_trait]
impl CategoryLearner for OpenAiCategoryLearner {
    fn available(&self) -> bool {
        self.enabled && self.key.is_some()
    }
    async fn propose(
        &self,
        term: &str,
        kind: &str,
        catalog: &Vocabulary,
    ) -> Result<Definition, ()> {
        let schema = object(
            json!({"decision":{"type":"string","enum":["create","alias","defer"]},"existing_id":string(),"label":string(),"definition":string(),"parent_id":{"type":"string","enum":[""]},"evidence_focus":{"type":"array","items":{"type":"string","enum":FOCUS}},"positive_examples":{"type":"array","items":string(),"maxItems":2},"negative_examples":{"type":"array","items":string(),"maxItems":2}}),
        );
        let mut value = self
            .call(
                include_str!("../prompts/category_proposal_v1.txt"),
                json!({"term":term,"entity_kind":kind,"catalog":catalog}),
                schema,
            )
            .await?;
        // The encountered phrase is the only proposed alias. Do not ask a model
        // to reproduce it or invent a list; semantic equivalence is reviewed next.
        value["aliases"] = json!([term]);
        serde_json::from_value(value).map_err(|_| ())
    }
    async fn review(
        &self,
        term: &str,
        kind: &str,
        proposal: &Definition,
        catalog: &Vocabulary,
    ) -> Result<Review, ()> {
        let mut properties = serde_json::Map::new();
        for field in [
            "approve",
            "generic_no_private_identity",
            "correct_meaning",
            "equivalent_aliases",
            "no_existing_duplicate",
            "compatible_kind_and_parent",
            "examples_correct",
            "safe_optional_focus",
        ] {
            properties.insert(field.into(), json!({"type":"boolean"}));
        }
        serde_json::from_value(
            self.call(
                include_str!("../prompts/category_review_v1.txt"),
                json!({"term":term,"entity_kind":kind,"proposal":proposal,"catalog":catalog}),
                object(Value::Object(properties)),
            )
            .await?,
        )
        .map_err(|_| ())
    }
}

pub async fn enqueue(conn: &mut PgConnection, item_id: Uuid) -> Result<(), sqlx::Error> {
    let row = sqlx::query("SELECT k.owner_id,k.recommendation,s.source_id,s.source_revision,p.generation FROM knowledge_items k JOIN item_source_support s ON s.item_id=k.id JOIN transcript_extraction_permissions p ON p.account_id=k.owner_id AND p.enabled WHERE k.id=$1 AND k.deleted_at IS NULL AND coalesce(k.recommendation->>'origin','extracted') <> 'user'")
        .bind(item_id).fetch_optional(&mut *conn).await?;
    let Some(row) = row else {
        return Ok(());
    };
    let rec: Value = row.get("recommendation");
    let Some(term) = rec["classification"]["descriptive_type"].as_str() else {
        return Ok(());
    };
    if rec["classification"]["origin"] == "user"
        || !plain_term(term)
        || rec["classification"]["types"]
            .as_array()
            .is_none_or(|a| !a.is_empty())
    {
        return Ok(());
    }
    let owner: Uuid = row.get("owner_id");
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM category_discoveries WHERE owner_id=$1")
            .bind(owner)
            .fetch_one(&mut *conn)
            .await?;
    if count >= 30 {
        return Ok(());
    }
    let key = job_key(rec["entity_kind"].as_str().unwrap_or(""), term);
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM category_learning_jobs WHERE key=$1)")
            .bind(&key)
            .fetch_one(&mut *conn)
            .await?;
    let queued: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM category_learning_jobs j WHERE status IN ('pending','processing') AND EXISTS(SELECT 1 FROM category_discoveries d WHERE d.job_key=j.key)",
    )
    .fetch_one(&mut *conn)
    .await?;
    if !exists && queued >= 1000 {
        return Ok(());
    }
    sqlx::query("INSERT INTO category_learning_jobs(key) VALUES($1) ON CONFLICT DO NOTHING")
        .bind(&key)
        .execute(&mut *conn)
        .await?;
    let cached: bool = sqlx::query_scalar(
        "SELECT status IN ('provisional','failed') FROM category_learning_jobs WHERE key=$1",
    )
    .bind(&key)
    .fetch_one(&mut *conn)
    .await?;
    if cached {
        return Ok(());
    }
    sqlx::query("INSERT INTO category_discoveries(item_id,owner_id,source_id,source_revision,permission_generation,classification,job_key) VALUES($1,$2,$3,$4,$5,$6,$7) ON CONFLICT(item_id) DO NOTHING")
        .bind(item_id).bind(owner).bind(row.get::<Uuid,_>("source_id")).bind(row.get::<i32,_>("source_revision")).bind(row.get::<i64,_>("generation")).bind(&rec["classification"]).bind(key).execute(conn).await?;
    Ok(())
}
#[derive(sqlx::FromRow, Debug)]
struct Candidate {
    item_id: Uuid,
    owner_id: Uuid,
    job_key: String,
    term: String,
    kind: String,
    recommendation: Value,
}
// Item classification and source/permission generation are fences. Contact-only
// changes may advance item revision without making a category result stale.
const ELIGIBLE: &str = "SELECT d.item_id,d.owner_id,d.job_key,k.recommendation #>> '{classification,descriptive_type}' AS term,k.recommendation->>'entity_kind' AS kind,k.recommendation FROM category_discoveries d JOIN knowledge_items k ON k.id=d.item_id JOIN source_texts s ON s.id=d.source_id JOIN transcript_extraction_permissions p ON p.account_id=d.owner_id WHERE d.item_id=$1 AND k.deleted_at IS NULL AND s.revision=d.source_revision AND p.enabled AND p.generation=d.permission_generation AND k.recommendation->'classification'=d.classification AND coalesce(k.recommendation->>'origin','extracted') <> 'user' AND NOT EXISTS(SELECT 1 FROM processing_permissions x WHERE x.account_id=d.owner_id AND NOT x.enabled) FOR UPDATE OF p,k,s,d";

async fn apply_ready(pool: &PgPool) -> Result<(), sqlx::Error> {
    let ids: Vec<Uuid> = sqlx::query_scalar("SELECT d.item_id FROM category_discoveries d JOIN category_learning_jobs j ON j.key=d.job_key WHERE j.status='accepted' ORDER BY d.created_at LIMIT 20").fetch_all(pool).await?;
    for id in ids {
        let catalog = catalog(pool).await?;
        let mut tx = pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock(732799)")
            .execute(&mut *tx)
            .await?;
        let revision: i32 =
            sqlx::query_scalar("SELECT revision FROM category_registry_state WHERE id")
                .fetch_one(&mut *tx)
                .await?;
        if taxonomy::vocabulary().version * 1_000_000 + revision != catalog.version {
            continue;
        }
        if let Some(c) = sqlx::query_as::<_, Candidate>(ELIGIBLE)
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
        {
            let target: Option<String> = sqlx::query_scalar(
                "SELECT concept_id FROM category_learning_jobs WHERE key=$1 AND status='accepted'",
            )
            .bind(&c.job_key)
            .fetch_optional(&mut *tx)
            .await?
            .flatten();
            if let Some(target) = target.filter(|id| taxonomy::concept_in(&catalog, id).is_some()) {
                let descriptors: Vec<String> = c.recommendation["classification"]["descriptors"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect();
                let facets: Vec<String> = c.recommendation["classification"]["facets"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|f| f["id"].as_str())
                    .map(str::to_owned)
                    .collect();
                if let Some(mut classification) = taxonomy::present_in(
                    &catalog,
                    &c.kind,
                    std::slice::from_ref(&target),
                    &facets,
                    &descriptors,
                    "learned",
                ) {
                    classification["descriptive_type"] = json!(c.term);
                    sqlx::query("UPDATE knowledge_items SET recommendation=jsonb_set(recommendation,'{classification}',$2),revision=revision+1 WHERE id=$1").bind(id).bind(classification).execute(&mut *tx).await?;
                    sqlx::query("UPDATE item_source_support SET support=support || jsonb_build_object('category_learning',jsonb_build_object('concept_id',$2::text,'catalog_version',$3::int,'source_phrase',$4::text)) WHERE item_id=$1")
                        .bind(id).bind(target).bind(catalog.version).bind(&c.term).execute(&mut *tx).await?;
                }
            }
        }
        sqlx::query("DELETE FROM category_discoveries WHERE item_id=$1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
    }
    Ok(())
}

/// One bounded job; call from a separate worker so provider latency never holds
/// up recording/transcription work. Accepted/provisional outcomes are cached.
pub async fn process_one(
    pool: &PgPool,
    learner: &dyn CategoryLearner,
) -> Result<bool, sqlx::Error> {
    if !learner.available() {
        return Ok(false);
    }
    apply_ready(pool).await?;
    sqlx::query("DELETE FROM category_discoveries d USING category_learning_jobs j WHERE j.key=d.job_key AND j.status IN ('provisional','failed')").execute(pool).await?;
    // Remove private pointers after deletion, edits or permission withdrawal.
    sqlx::query("DELETE FROM category_discoveries d USING knowledge_items k,transcript_extraction_permissions p WHERE k.id=d.item_id AND p.account_id=d.owner_id AND (k.deleted_at IS NOT NULL OR NOT p.enabled OR p.generation<>d.permission_generation OR k.recommendation->'classification' IS DISTINCT FROM d.classification OR coalesce(k.recommendation->>'origin','extracted')='user' OR EXISTS(SELECT 1 FROM processing_permissions x WHERE x.account_id=d.owner_id AND NOT x.enabled))").execute(pool).await?;
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(732798)")
        .execute(&mut *tx)
        .await?;
    // Serialize lease recovery with publication; both lock item/job rows.
    sqlx::query("SELECT pg_advisory_xact_lock(732799)")
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE category_learning_jobs SET status=CASE WHEN attempts<2 THEN 'pending' ELSE 'failed' END,reason='lease_expired',retry_at=now()+interval '15 minutes' WHERE status='processing' AND lease_until<now()").execute(&mut *tx).await?;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM category_learning_attempts WHERE created_at>now()-interval '24 hours'").fetch_one(&mut *tx).await?;
    if count >= 12 {
        tx.commit().await?;
        return Ok(false);
    }
    let id: Option<Uuid> = sqlx::query_scalar("SELECT d.item_id FROM category_discoveries d JOIN category_learning_jobs j ON j.key=d.job_key WHERE j.status='pending' AND j.retry_at<=now() AND j.attempts<2 AND (SELECT count(*) FROM category_learning_attempts a WHERE a.owner_id=d.owner_id AND a.created_at>now()-interval '24 hours')<4 ORDER BY d.created_at LIMIT 1").fetch_optional(&mut *tx).await?;
    let Some(id) = id else {
        tx.commit().await?;
        return Ok(false);
    };
    let Some(candidate) = sqlx::query_as::<_, Candidate>(ELIGIBLE)
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
    else {
        sqlx::query("DELETE FROM category_discoveries WHERE item_id=$1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        return Ok(false);
    };
    let attempt = Uuid::new_v4();
    sqlx::query("UPDATE category_learning_jobs SET status='processing',attempts=attempts+1,attempt_id=$2,lease_until=now()+interval '3 minutes' WHERE key=$1").bind(&candidate.job_key).bind(attempt).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO category_learning_attempts(id,job_key,owner_id) VALUES($1,$2,$3)")
        .bind(attempt)
        .bind(&candidate.job_key)
        .bind(candidate.owner_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    let snapshot = catalog(pool).await?;
    // Bound registry-review context/cost. Existing recognition still works when
    // the pilot cap is reached; unknown records retain their descriptive label.
    let outcome = if snapshot.concepts.len() >= 500 {
        Err("registry_capacity")
    } else {
        match learner
            .propose(&candidate.term, &candidate.kind, &snapshot)
            .await
        {
            Err(_) => Err("provider_failed"),
            Ok(d) => match check_definition(&d, &candidate.term, &candidate.kind, &snapshot) {
                Err(reason) => Err(reason),
                Ok(_)
                    if sqlx::query_as::<_, Candidate>(ELIGIBLE)
                        .bind(candidate.item_id)
                        .fetch_optional(pool)
                        .await?
                        .is_none() =>
                {
                    Err("source_or_permission_changed")
                }
                Ok(target) => match learner
                    .review(&candidate.term, &candidate.kind, &d, &snapshot)
                    .await
                {
                    Err(_) => Err("provider_failed"),
                    Ok(review) if review.accepted() => Ok((d, target)),
                    Ok(_) => Err("review_deferred"),
                },
            },
        }
    };
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(732799)")
        .execute(&mut *tx)
        .await?;
    let eligible = sqlx::query_as::<_, Candidate>(ELIGIBLE)
        .bind(candidate.item_id)
        .fetch_optional(&mut *tx)
        .await?
        .is_some();
    let owns: bool = sqlx::query_scalar("SELECT status='processing' AND attempt_id=$2 AND lease_until>now() FROM category_learning_jobs WHERE key=$1 FOR UPDATE").bind(&candidate.job_key).bind(attempt).fetch_one(&mut *tx).await?;
    if !owns {
        return Ok(true);
    }
    let revision: i32 =
        sqlx::query_scalar("SELECT revision FROM category_registry_state WHERE id FOR UPDATE")
            .fetch_one(&mut *tx)
            .await?;
    let outcome = if !eligible {
        Err("source_or_permission_changed")
    } else if taxonomy::vocabulary().version * 1_000_000 + revision != snapshot.version {
        Err("catalog_changed")
    } else {
        outcome
    };
    let outcome = match outcome {
        Ok((d, target)) => {
            let aliases: Vec<String> = d
                .aliases
                .iter()
                .chain(std::iter::once(&d.label))
                .map(|a| normalize(a))
                .collect();
            let reserved: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM learned_category_aliases WHERE normalized=ANY($1) AND (concept_id<>$2 OR NOT active)) OR EXISTS(SELECT 1 FROM learned_categories WHERE id=$2 AND NOT active)").bind(aliases).bind(&target).fetch_one(&mut *tx).await?;
            if reserved {
                Err("reserved_alias")
            } else {
                Ok((d, target))
            }
        }
        other => other,
    };
    match outcome {
        Ok((d, target)) => {
            sqlx::query("INSERT INTO category_registry_events(registry_revision,action,concept_id,receipt) VALUES($1,$2,$3,$4)")
                .bind(revision+1).bind(&d.decision).bind(&target)
                .bind(json!({"learning_version":1,"model":"gpt-4.1-mini","proposal":d,"independent_review_all_checks_passed":true}))
                .execute(&mut *tx).await?;
            if d.decision == "create" {
                let c = Concept {
                    id: target.clone(),
                    label: d.label.clone(),
                    dimension: "type".into(),
                    dimension_label: "Type".into(),
                    entity_kinds: vec![candidate.kind],
                    parent_id: if d.parent_id.is_empty() {
                        None
                    } else {
                        Some(d.parent_id)
                    },
                    aliases: vec![],
                    definition: d.definition,
                    evidence_focus: d.evidence_focus,
                };
                sqlx::query("INSERT INTO learned_categories(id,definition) VALUES($1,$2)")
                    .bind(&target)
                    .bind(json!(c))
                    .execute(&mut *tx)
                    .await?;
            }
            let mut seen = BTreeSet::new();
            for phrase in d.aliases.iter().chain(std::iter::once(&d.label)) {
                if !seen.insert(normalize(phrase)) {
                    continue;
                }
                sqlx::query("INSERT INTO learned_category_aliases(normalized,phrase,concept_id) VALUES($1,$2,$3) ON CONFLICT(normalized) DO UPDATE SET active=true WHERE learned_category_aliases.concept_id=EXCLUDED.concept_id")
                    .bind(normalize(phrase)).bind(phrase).bind(&target).execute(&mut *tx).await?;
            }
            sqlx::query("UPDATE category_registry_state SET revision=revision+1 WHERE id")
                .execute(&mut *tx)
                .await?;
            sqlx::query("UPDATE category_learning_jobs SET status='accepted',concept_id=$2,reason=NULL WHERE key=$1").bind(&candidate.job_key).bind(target).execute(&mut *tx).await?;
        }
        Err(reason) => {
            sqlx::query("UPDATE category_learning_jobs SET status=CASE WHEN $2 IN ('provider_failed','catalog_changed') AND attempts<2 THEN 'pending' WHEN $2='provider_failed' THEN 'failed' ELSE 'provisional' END,reason=$2,retry_at=now()+interval '15 minutes' WHERE key=$1")
                .bind(&candidate.job_key).bind(reason).execute(&mut *tx).await?;
        }
    }
    tx.commit().await?;
    apply_ready(pool).await?;
    Ok(true)
}

/// Operator-only rollback: stop future assignments and alias reuse. Existing
/// recommendation snapshots and owner corrections are never silently rewritten.
pub async fn suspend(pool: &PgPool, id: &str) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(732799)")
        .execute(&mut *tx)
        .await?;
    let c: Option<Value> = sqlx::query_scalar(
        "SELECT definition FROM learned_categories WHERE id=$1 AND active FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(c) = c else {
        return Err(sqlx::Error::Protocol(
            "Active learned category not found".into(),
        ));
    };
    let children:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM learned_categories WHERE active AND definition->>'parent_id'=$1)").bind(id).fetch_one(&mut *tx).await?;
    if children {
        return Err(sqlx::Error::Protocol(
            "Suspend learned children first".into(),
        ));
    }
    sqlx::query("UPDATE learned_categories SET active=false,revision=revision+1 WHERE id=$1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE learned_category_aliases SET active=false WHERE concept_id=$1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE category_learning_jobs SET status='provisional',reason='category_suspended' WHERE concept_id=$1").bind(id).execute(&mut *tx).await?;
    let revision: i32 = sqlx::query_scalar(
        "UPDATE category_registry_state SET revision=revision+1 WHERE id RETURNING revision",
    )
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query("INSERT INTO category_registry_events(registry_revision,action,concept_id,receipt) VALUES($1,'suspend',$2,$3)").bind(revision).bind(id).bind(c).execute(&mut *tx).await?;
    tx.commit().await
}

/// Allow an owner to retain a retired category already on this item while
/// editing other fields. It is never offered to new items or new extraction.
pub async fn include_stored_types(
    conn: &mut PgConnection,
    catalog: &mut Vocabulary,
    previous: &Value,
) -> Result<(), sqlx::Error> {
    let retired: Vec<Value> =
        sqlx::query_scalar("SELECT definition FROM learned_categories WHERE NOT active")
            .fetch_all(conn)
            .await?;
    let mut full = catalog.clone();
    full.concepts.extend(
        retired
            .into_iter()
            .filter_map(|c| serde_json::from_value::<Concept>(c).ok()),
    );
    let mut ids = BTreeSet::new();
    for id in previous["classification"]["types"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| c["id"].as_str())
    {
        ids.extend(taxonomy::ancestors_in(&full, id));
    }
    for id in ids {
        if taxonomy::concept_in(catalog, &id).is_none()
            && let Some(c) = taxonomy::concept_in(&full, &id)
        {
            catalog.concepts.push(c.clone());
        }
    }
    Ok(())
}

/// Revoke a learned synonym (including one pointing to a seed concept) without
/// deactivating that concept or silently changing existing recommendation text.
pub async fn suspend_alias(pool: &PgPool, phrase: &str) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(732799)")
        .execute(&mut *tx)
        .await?;
    let target:Option<String>=sqlx::query_scalar("UPDATE learned_category_aliases SET active=false WHERE normalized=$1 AND active RETURNING concept_id").bind(normalize(phrase)).fetch_optional(&mut *tx).await?;
    let Some(target) = target else {
        return Err(sqlx::Error::Protocol(
            "Active learned alias not found".into(),
        ));
    };
    let keys: Vec<_> = [
        "place",
        "person_service",
        "thing",
        "activity_event",
        "idea_tip",
    ]
    .iter()
    .map(|k| job_key(k, phrase))
    .collect();
    sqlx::query("UPDATE category_learning_jobs SET status='provisional',reason='alias_suspended' WHERE key=ANY($1)").bind(keys).execute(&mut *tx).await?;
    let revision: i32 = sqlx::query_scalar(
        "UPDATE category_registry_state SET revision=revision+1 WHERE id RETURNING revision",
    )
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query("INSERT INTO category_registry_events(registry_revision,action,concept_id,receipt) VALUES($1,'suspend',$2,$3)").bind(revision).bind(target).bind(json!({"alias":normalize(phrase)})).execute(&mut *tx).await?;
    tx.commit().await
}
