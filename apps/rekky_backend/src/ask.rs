//! Bounded owner-only Ask agent. Tools expose saved evidence, never raw transcripts.
use crate::app::{ApiError, ApiResult, AppState, ok, owner, parse};
use async_trait::async_trait;
use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::Row;
use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};
use uuid::Uuid;

const MODEL: &str = "gpt-6-luna";
const MAX_DECISIONS: usize = 3;
const PAGE: usize = 8;

#[derive(Debug)]
pub struct AgentError;
#[derive(Clone, Debug)]
pub struct Decision {
    pub calls: Vec<ToolCall>,
    pub input_tokens: i64,
    pub output_tokens: i64,
}
#[derive(Clone, Debug)]
pub struct ToolCall {
    pub name: String,
    pub arguments: Value,
}
#[async_trait]
pub trait AskModel: Send + Sync {
    fn available(&self) -> bool;
    async fn decide(&self, context: Value, final_turn: bool) -> Result<Decision, AgentError>;
}
pub struct OpenAiAsk {
    key: Option<String>,
    enabled: bool,
    client: reqwest::Client,
}
impl OpenAiAsk {
    pub fn from_env() -> Self {
        Self {
            key: std::env::var("OPENAI_API_KEY")
                .ok()
                .filter(|s| !s.is_empty()),
            enabled: std::env::var("OPENAI_ASK_ENABLED").as_deref() == Ok("true"),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(18))
                .build()
                .expect("HTTP client"),
        }
    }
}
fn object(fields: Value) -> Value {
    let required: Vec<_> = fields
        .as_object()
        .expect("schema")
        .keys()
        .cloned()
        .collect();
    json!({"type":"object","properties":fields,"required":required,"additionalProperties":false})
}
fn strings() -> Value {
    json!({"type":"array","items":{"type":"string"}})
}
fn tool(name: &str, description: &str, parameters: Value) -> Value {
    json!({"type":"function","name":name,"description":description,"strict":true,"parameters":parameters})
}
fn tools() -> Vec<Value> {
    vec![
        tool(
            "search_knowledge",
            "Search saved evidence using tokenized OR alternatives; phrases are split into words. An empty terms list browses. Up to 8 terms, page 0..20.",
            object(json!({"terms":strings(),"page":{"type":"integer"}})),
        ),
        tool(
            "inspect_evidence",
            "Read current saved evidence for up to 8 previously returned item IDs.",
            object(json!({"item_ids":strings()})),
        ),
        tool(
            "present_answer",
            "Present all useful seen items with cited evidence, or a clarification. No made-up actions.",
            object(json!({
                "intent":{"type":"string","enum":["recall","discovery","comparison"]},"title":{"type":"string"},"location":{"type":"string"},
                "clarification":{"type":"string"},"choices":strings(),"new_topic":{"type":"boolean","description":"True only for a clearly unrelated new request; false for refinements and clarification replies."},
                "comparison":{"anyOf":[{"type":"null"},object(json!({
                    "item_ids":strings(),
                    "dimensions":{"type":"array","items":object(json!({"label":{"type":"string"},"cells":{"type":"array","items":object(json!({"item_id":{"type":"string"},"text":{"type":"string"},"evidence_ids":strings()}))}}))},
                    "conclusion":{"type":"string"},
                    "citations":{"type":"array","items":object(json!({"item_id":{"type":"string"},"evidence_ids":strings()}))}
                }))]},
                "results":{"type":"array","items":object(json!({"item_id":{"type":"string"},"section":{"type":"string","enum":["supported","worth_checking"],"description":"Suitability for the user request, NOT whether an exclusion statement has evidence. Include only viable options; omit irrelevant or contradicted candidates."},"reason":{"type":"string"},"caveat":{"type":"string"},"evidence_ids":strings()}))}
            })),
        ),
    ]
}
#[async_trait]
impl AskModel for OpenAiAsk {
    fn available(&self) -> bool {
        self.enabled && self.key.is_some()
    }
    async fn decide(&self, context: Value, final_turn: bool) -> Result<Decision, AgentError> {
        if !self.available() {
            return Err(AgentError);
        }
        let body = json!({"model":MODEL,"store":false,"reasoning":{"effort":"none"},"max_output_tokens":2000,
            "input":[{"role":"system","content":include_str!("../prompts/ask_v1.txt")},{"role":"user","content":context.to_string()}],
            "tools":tools(),"tool_choice":if final_turn {json!({"type":"function","name":"present_answer"})}else{json!("required")}});
        // Conservative token bound at <= one token per UTF-8 byte plus headroom.
        // 3 x (48k input at $0.10/M + 2k output at $0.50/M) <= $0.0174.
        if body.to_string().len() > 47_000 {
            return Err(AgentError);
        }
        let response = self
            .client
            .post("https://api.openai.com/v1/responses")
            .bearer_auth(self.key.as_ref().ok_or(AgentError)?)
            .json(&body)
            .send()
            .await
            .map_err(|_| AgentError)?;
        if !response.status().is_success() {
            return Err(AgentError);
        }
        let data: Value = response.json().await.map_err(|_| AgentError)?;
        if data["status"] != "completed" {
            return Err(AgentError);
        }
        let calls = data["output"]
            .as_array()
            .ok_or(AgentError)?
            .iter()
            .filter(|v| v["type"] == "function_call")
            .map(|v| {
                Ok(ToolCall {
                    name: v["name"].as_str().ok_or(AgentError)?.into(),
                    arguments: serde_json::from_str(v["arguments"].as_str().ok_or(AgentError)?)
                        .map_err(|_| AgentError)?,
                })
            })
            .collect::<Result<Vec<_>, AgentError>>()?;
        if calls.is_empty() || calls.len() > 4 {
            return Err(AgentError);
        }
        Ok(Decision {
            calls,
            input_tokens: data["usage"]["input_tokens"].as_i64().unwrap_or(0),
            output_tokens: data["usage"]["output_tokens"].as_i64().unwrap_or(0),
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Evidence {
    pub id: String,
    pub kind: String,
    pub text: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Candidate {
    pub item_id: Uuid,
    pub revision: i32,
    pub subject: String,
    pub entity_kind: String,
    pub evidence: Vec<Evidence>,
    pub locations: Value,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    request_id: Uuid,
    question: String,
    #[serde(default)]
    previous_request_id: Option<Uuid>,
    #[serde(default)]
    selected_item_ids: Vec<Uuid>,
    #[serde(default)]
    excluded_item_ids: Vec<Uuid>,
}
#[derive(Deserialize, Default)]
pub struct PageQuery {
    offset: Option<usize>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProposedResult {
    pub item_id: Uuid,
    pub section: String,
    pub reason: String,
    pub caveat: String,
    pub evidence_ids: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Answer {
    pub intent: String,
    #[serde(default)]
    pub new_topic: bool,
    pub title: String,
    pub location: String,
    pub clarification: String,
    pub choices: Vec<String>,
    pub results: Vec<ProposedResult>,
    #[serde(default)]
    pub comparison: Option<Comparison>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Comparison {
    pub item_ids: Vec<Uuid>,
    pub dimensions: Vec<Dimension>,
    pub conclusion: String,
    pub citations: Vec<Citation>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dimension {
    pub label: String,
    pub cells: Vec<ComparisonCell>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComparisonCell {
    pub item_id: Uuid,
    pub text: String,
    pub evidence_ids: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Citation {
    pub item_id: Uuid,
    pub evidence_ids: Vec<String>,
}
impl Answer {
    fn item_ids(&self) -> Vec<Uuid> {
        self.results
            .iter()
            .map(|r| r.item_id)
            .chain(
                self.comparison
                    .iter()
                    .flat_map(|c| c.item_ids.iter().copied()),
            )
            .collect()
    }
}
#[derive(Serialize, Deserialize)]
struct Snapshot {
    #[serde(default)]
    conversation: Conversation,
    answer: Answer,
    candidates: Vec<Candidate>,
    mode: String,
    #[serde(default)]
    location_geo: Value,
    #[serde(default)]
    search_incomplete: bool,
}
#[derive(Clone, Default, Serialize, Deserialize)]
struct Conversation {
    turns: Vec<String>,
    selected_item_ids: Vec<Uuid>,
    excluded_item_ids: Vec<Uuid>,
}
struct TurnContext {
    conversation: Conversation,
    previous_items: Vec<Candidate>,
    clarification: String,
}
async fn turn_context(
    state: &AppState,
    owner_id: Uuid,
    generation: i64,
    input: &Input,
) -> Result<TurnContext, ApiError> {
    let mut conversation = Conversation::default();
    let mut previous_items = vec![];
    let mut clarification = String::new();
    if let Some(id) = input.previous_request_id {
        let value:Option<Value>=sqlx::query_scalar("SELECT result FROM ask_runs WHERE id=$1 AND owner_id=$2 AND permission_generation=$3 AND status='completed' AND expires_at>now() AND result IS NOT NULL")
            .bind(id).bind(owner_id).bind(generation).fetch_optional(&state.pool).await?;
        let snapshot: Snapshot = value
            .and_then(|v| serde_json::from_value(v).ok())
            .ok_or_else(|| {
                ApiError::new(
                    StatusCode::CONFLICT,
                    "ask_context_expired",
                    "This conversation expired. Start a new question.",
                )
            })?;
        conversation = snapshot.conversation;
        if conversation.turns.len() >= 8 {
            return Err(ApiError::new(
                StatusCode::CONFLICT,
                "ask_context_full",
                "Start a new question to continue exploring.",
            ));
        }
        let allowed: HashSet<_> = snapshot.answer.item_ids().into_iter().collect();
        if input
            .selected_item_ids
            .iter()
            .chain(input.excluded_item_ids.iter())
            .any(|id| !allowed.contains(id))
        {
            return Err(ApiError::bad(
                "Choose a recommendation from the previous answer",
            ));
        }
        // Selection applies to the next turn only. Exclusions remain session-scoped.
        conversation.selected_item_ids = input.selected_item_ids.clone();
        conversation
            .excluded_item_ids
            .extend(&input.excluded_item_ids);
        conversation.excluded_item_ids.sort();
        conversation.excluded_item_ids.dedup();
        if conversation.excluded_item_ids.len() > 32 {
            return Err(ApiError::bad(
                "Start a new question to reset excluded options",
            ));
        }
        let mut ids = input.selected_item_ids.clone();
        ids.extend(snapshot.answer.item_ids());
        let mut used = HashSet::new();
        let mut changed = false;
        for id in ids {
            if !used.insert(id) || conversation.excluded_item_ids.contains(&id) {
                continue;
            }
            if let Some(item) = crate::app::owner_item(state, owner_id, id).await? {
                let revision = item["revision"].as_i64().unwrap_or(0) as i32;
                changed |= !snapshot
                    .candidates
                    .iter()
                    .any(|c| c.item_id == id && c.revision == revision);
                let rec = &item["recommendation"];
                previous_items.push(Candidate {
                    item_id: id,
                    revision,
                    subject: item["subject"].as_str().unwrap_or_default().into(),
                    entity_kind: rec["entity_kind"].as_str().unwrap_or("note").into(),
                    evidence: evidence(rec, item["body"].as_str().unwrap_or_default()),
                    locations: rec["locations"].clone(),
                });
            } else {
                changed = true;
            }
            if previous_items.len() >= 8 {
                break;
            }
        }
        if conversation
            .selected_item_ids
            .iter()
            .any(|id| !previous_items.iter().any(|c| c.item_id == *id))
        {
            return Err(ApiError::new(
                StatusCode::CONFLICT,
                "ask_reference_changed",
                "The selected recommendation is no longer available. Choose another or start a new question.",
            ));
        }
        if !changed {
            clarification = snapshot.answer.clarification;
        }
    } else if !input.selected_item_ids.is_empty() || !input.excluded_item_ids.is_empty() {
        return Err(ApiError::bad(
            "A previous answer is required for a selection",
        ));
    }
    conversation.turns.push(input.question.clone());
    Ok(TurnContext {
        conversation,
        previous_items,
        clarification,
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Search {
    terms: Vec<String>,
    page: usize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Inspect {
    item_ids: Vec<Uuid>,
}

fn bounded(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}
fn evidence(rec: &Value, body: &str) -> Vec<Evidence> {
    let mut result = vec![];
    for key in ["summary", "experience"] {
        if let Some(s) = rec[key].as_str().filter(|s| !s.is_empty()) {
            result.push(Evidence {
                id: key.into(),
                kind: key.into(),
                text: bounded(s, 800),
            });
        }
    }
    for key in ["observations", "use_cases", "locations"] {
        if let Some(values) = rec[key].as_array() {
            for (i, v) in values.iter().take(24).enumerate() {
                if let Some(s) = v["text"].as_str().or_else(|| v.as_str()) {
                    result.push(Evidence {
                        id: format!("{key}.{i}"),
                        kind: v["kind"]
                            .as_str()
                            .or_else(|| v["role"].as_str())
                            .unwrap_or(key)
                            .into(),
                        text: bounded(s, 600),
                    });
                }
            }
        }
    }
    if let Some(label) = rec["classification"]["display_label"].as_str() {
        result.push(Evidence {
            id: "category".into(),
            kind: "category".into(),
            text: bounded(label, 120),
        });
    }
    if result.is_empty() {
        result.push(Evidence {
            id: "body".into(),
            kind: "saved_note".into(),
            text: bounded(body, 2400),
        });
    }
    result
}
async fn search(
    state: &AppState,
    owner_id: Uuid,
    terms: &[String],
    page: usize,
) -> Result<(Vec<Candidate>, bool), ApiError> {
    if terms.len() > 8 || terms.iter().any(|s| s.chars().count() > 80) || page > 20 {
        return Err(ApiError::bad("Invalid search tool arguments"));
    }
    // Model-proposed phrases are recall clues, not mandatory exact substrings.
    // Retrieve broad token alternatives; the agent checks the complete evidence.
    let mut terms: Vec<_> = terms
        .iter()
        .flat_map(|s| s.split_whitespace())
        .map(|s| {
            s.trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase()
        })
        .filter(|s| s.chars().count() >= 2)
        .take(64)
        .collect();
    terms.sort();
    terms.dedup();
    let rows=sqlx::query("SELECT id,revision,subject,body,recommendation FROM knowledge_items WHERE owner_id=$1 AND deleted_at IS NULL AND (cardinality($2::text[])=0 OR EXISTS(SELECT 1 FROM unnest($2::text[]) t WHERE strpos(lower(subject||' '||body||' '||category_search),t)>0)) ORDER BY (SELECT coalesce(sum(CASE WHEN strpos(lower(subject),t)>0 THEN 4 ELSE 0 END + CASE WHEN strpos(lower(body||' '||category_search),t)>0 THEN 1 ELSE 0 END),0) FROM unnest($2::text[]) t) DESC,created_at DESC,id DESC LIMIT 9 OFFSET $3")
        .bind(owner_id).bind(&terms).bind((page*PAGE) as i64).fetch_all(&state.pool).await?;
    let more = rows.len() > PAGE;
    let candidates = rows
        .iter()
        .take(PAGE)
        .map(|r| {
            let rec = r
                .try_get::<Value, _>("recommendation")
                .unwrap_or(Value::Null);
            Candidate {
                item_id: r.get("id"),
                revision: r.get("revision"),
                subject: r.get("subject"),
                entity_kind: rec["entity_kind"].as_str().unwrap_or("note").into(),
                evidence: evidence(&rec, r.get::<String, _>("body").as_str()),
                locations: rec["locations"].clone(),
            }
        })
        .collect();
    Ok((candidates, more))
}
async fn permitted(state: &AppState, owner_id: Uuid) -> Result<Option<i64>, ApiError> {
    Ok(sqlx::query_scalar("SELECT generation FROM transcript_extraction_permissions p WHERE account_id=$1 AND enabled AND NOT EXISTS(SELECT 1 FROM processing_permissions g WHERE g.account_id=p.account_id AND NOT g.enabled)")
        .bind(owner_id).fetch_optional(&state.pool).await?)
}
fn fallback(question: &str, candidates: &HashMap<Uuid, Candidate>) -> Answer {
    let mut values: Vec<_> = candidates.values().collect();
    values.sort_by_key(|c| c.item_id);
    Answer {
        intent: "recall".into(),
        comparison: None,
        new_topic: false,
        title: "Saved matches".into(),
        location: String::new(),
        clarification: if values.is_empty() {
            "Try a name or a detail you remember.".into()
        } else {
            String::new()
        },
        choices: vec![],
        results: values
            .iter()
            .filter(|c| {
                question
                    .split_whitespace()
                    .any(|w| w.len() > 3 && c.subject.to_lowercase().contains(&w.to_lowercase()))
            })
            .map(|c| ProposedResult {
                item_id: c.item_id,
                section: "worth_checking".into(),
                reason: String::new(),
                caveat: "Relevance hasn’t been checked. Open the saved recommendation.".into(),
                evidence_ids: c.evidence.iter().take(1).map(|e| e.id.clone()).collect(),
            })
            .collect(),
    }
}

pub fn validate_answer(
    mut a: Answer,
    seen: &HashMap<Uuid, Candidate>,
) -> Result<Answer, AgentError> {
    if !["recall", "discovery", "comparison"].contains(&a.intent.as_str())
        || a.title.chars().count() > 100
        || a.clarification.chars().count() > 240
        || a.choices.len() > 3
        || a.choices.iter().any(|v| v.chars().count() > 80)
        || a.results.len() > 32
        || a.location.chars().count() > 100
    {
        return Err(AgentError);
    }
    if let Some(c) = &mut a.comparison {
        let participants: HashSet<_> = c.item_ids.iter().copied().collect();
        if a.intent != "comparison"
            || !(2..=4).contains(&c.item_ids.len())
            || participants.len() != c.item_ids.len()
            || participants.iter().any(|id| !seen.contains_key(id))
            || !(1..=4).contains(&c.dimensions.len())
            || c.conclusion.chars().count() > 400
            || c.citations.len() > 4
        {
            return Err(AgentError);
        }
        let supported = |item_id: &Uuid, ids: &[String]| {
            participants.contains(item_id)
                && (1..=3).contains(&ids.len())
                && seen.get(item_id).is_some_and(|item| {
                    ids.iter()
                        .all(|id| item.evidence.iter().any(|e| &e.id == id))
                })
        };
        let mut labels = HashSet::new();
        for dimension in &mut c.dimensions {
            if dimension.label.trim().is_empty()
                || dimension.label.chars().count() > 60
                || !labels.insert(dimension.label.trim().to_lowercase())
                || dimension.cells.len() != participants.len()
            {
                return Err(AgentError);
            }
            let mut ids = HashSet::new();
            for cell in &mut dimension.cells {
                if !participants.contains(&cell.item_id)
                    || !ids.insert(cell.item_id)
                    || cell.text.chars().count() > 240
                {
                    return Err(AgentError);
                }
                if cell.evidence_ids.is_empty() {
                    // Unknown values cannot carry unsupported generated claims.
                    cell.text = "Not saved".into();
                } else if cell.text.trim().is_empty()
                    || !supported(&cell.item_id, &cell.evidence_ids)
                {
                    return Err(AgentError);
                }
            }
            dimension
                .cells
                .sort_by_key(|cell| c.item_ids.iter().position(|id| *id == cell.item_id));
        }
        if c.citations
            .iter()
            .any(|r| !supported(&r.item_id, &r.evidence_ids))
        {
            return Err(AgentError);
        }
        // A comparative conclusion needs evidence from every participant.
        if !c.conclusion.is_empty()
            && participants
                .iter()
                .any(|id| !c.citations.iter().any(|r| &r.item_id == id))
        {
            return Err(AgentError);
        }
        a.results.clear();
        a.location.clear();
    } else if a.intent == "comparison" && a.clarification.trim().is_empty() {
        return Err(AgentError);
    }
    let mut ids = HashSet::new();
    a.results.retain(|r| {
        seen.get(&r.item_id).is_some_and(|c| {
            ["supported", "worth_checking"].contains(&r.section.as_str())
                && r.reason.chars().count() <= 400
                && r.caveat.chars().count() <= 300
                && (1..=3).contains(&r.evidence_ids.len())
                && r.evidence_ids
                    .iter()
                    .all(|id| c.evidence.iter().any(|e| &e.id == id))
                && ids.insert(r.item_id)
        })
    });
    Ok(a)
}

pub async fn run(State(state): State<AppState>, headers: HeaderMap, body: Bytes) -> ApiResult {
    let owner_id = owner(&state, &headers, true).await?;
    let mut input: Input = parse(&body)?;
    input.question = input.question.trim().into();
    if !(1..=500).contains(&input.question.chars().count()) {
        return Err(ApiError::bad("Write a question of up to 500 characters"));
    }
    let generation = permitted(&state, owner_id).await?.ok_or_else(|| {
        ApiError::new(
            StatusCode::FORBIDDEN,
            "processing_disabled",
            "Enable understanding in Voice processing to use intelligent Ask.",
        )
    })?;
    if input.selected_item_ids.len() > 4 || input.excluded_item_ids.len() > 8 {
        return Err(ApiError::bad("Too many selected recommendations"));
    }
    input.selected_item_ids.sort();
    input.selected_item_ids.dedup();
    input.excluded_item_ids.sort();
    input.excluded_item_ids.dedup();
    let request_hash = crate::auth::hash_token(&serde_json::to_string(&input).expect("input"));
    // Serialize admission across accounts: cap aggregate reserved spend, including unknown outcomes.
    let mut tx = state.pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(827713)")
        .execute(&mut *tx)
        .await?;
    if let Some(row) = sqlx::query("SELECT owner_id,request_hash,status FROM ask_runs WHERE id=$1")
        .bind(input.request_id)
        .fetch_optional(&mut *tx)
        .await?
    {
        if row.get::<Uuid, _>("owner_id") != owner_id
            || row.get::<String, _>("request_hash") != request_hash
        {
            return Err(ApiError::conflict(
                "Ask request changed. Start a new question.",
            ));
        }
        let status: String = row.get("status");
        tx.commit().await?;
        if status == "running" {
            return Err(ApiError::new(
                StatusCode::CONFLICT,
                "ask_running",
                "This question is still being answered. Try again shortly.",
            ));
        }
        return page_response(&state, owner_id, input.request_id, 0).await;
    }
    let context = turn_context(&state, owner_id, generation, &input).await?;
    if !state.ask_model.available() {
        return Err(ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "ask_unavailable",
            "Intelligent Ask is temporarily unavailable. Your Library is still available.",
        ));
    }
    let counts=sqlx::query("SELECT count(*) total,count(*) FILTER(WHERE owner_id=$1) personal,count(*) FILTER(WHERE owner_id=$1 AND status='running' AND created_at>now()-interval '1 minute') active FROM ask_runs WHERE created_at>now()-interval '24 hours' AND reserved_microusd>0").bind(owner_id).fetch_one(&mut *tx).await?;
    if counts.get::<i64, _>("total") >= 500
        || counts.get::<i64, _>("personal") >= 30
        || counts.get::<i64, _>("active") >= 1
    {
        return Err(ApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "ask_limit",
            "Ask is busy or its daily allowance is reached. Try again later.",
        ));
    }
    sqlx::query("UPDATE ask_runs SET result=NULL,status=CASE WHEN status='running' THEN 'failed' ELSE status END WHERE expires_at<now() AND (result IS NOT NULL OR status='running')").execute(&mut *tx).await?;
    sqlx::query("INSERT INTO ask_runs(id,owner_id,request_hash,status,permission_generation) VALUES($1,$2,$3,'running',$4)").bind(input.request_id).bind(owner_id).bind(request_hash).bind(generation).execute(&mut *tx).await?;
    tx.commit().await?;
    let result = tokio::time::timeout(
        Duration::from_secs(50),
        execute(
            &state,
            &headers,
            owner_id,
            generation,
            input.request_id,
            &input.question,
            context,
        ),
    )
    .await;
    let (snapshot, status) = match result {
        Ok(Ok(s)) => (Some(s), "completed"),
        _ => (None, "failed"),
    };
    // Do not persist private output after withdrawal or session revocation.
    if permitted(&state, owner_id).await? != Some(generation)
        || owner(&state, &headers, true).await.ok() != Some(owner_id)
    {
        sqlx::query("UPDATE ask_runs SET status='failed',result=NULL WHERE id=$1")
            .bind(input.request_id)
            .execute(&state.pool)
            .await?;
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "processing_changed",
            "Your processing permission or session changed. Please retry.",
        ));
    }
    sqlx::query("UPDATE ask_runs SET status=$2,result=$3 WHERE id=$1 AND status='running'")
        .bind(input.request_id)
        .bind(status)
        .bind(snapshot.map(|s| serde_json::to_value(s).expect("snapshot")))
        .execute(&state.pool)
        .await?;
    page_response(&state, owner_id, input.request_id, 0).await
}
async fn execute(
    state: &AppState,
    headers: &HeaderMap,
    owner_id: Uuid,
    generation: i64,
    run: Uuid,
    question: &str,
    turn: TurnContext,
) -> Result<Snapshot, ApiError> {
    let mut conversation = turn.conversation;
    let mut seen: HashMap<Uuid, Candidate> = turn
        .previous_items
        .iter()
        .map(|c| (c.item_id, c.clone()))
        .collect();
    let mut observations = vec![];
    let mut calls = HashSet::new();
    let mut reads = 0;
    let mut search_incomplete = false;
    for step in 0..MAX_DECISIONS {
        if owner(state, headers, true).await? != owner_id
            || permitted(state, owner_id).await? != Some(generation)
        {
            return Err(ApiError::bad("Processing permission changed"));
        }
        if !sqlx::query_scalar::<_, bool>("SELECT status='running' FROM ask_runs WHERE id=$1")
            .bind(run)
            .fetch_one(&state.pool)
            .await?
        {
            return Err(ApiError::conflict("Question cancelled"));
        }
        sqlx::query("UPDATE ask_runs SET decisions=decisions+1 WHERE id=$1")
            .bind(run)
            .execute(&state.pool)
            .await?;
        for candidate in seen.values() {
            let Some(current) = crate::app::owner_item(state, owner_id, candidate.item_id).await?
            else {
                return Err(ApiError::conflict("A recommendation changed. Ask again."));
            };
            if current["revision"].as_i64() != Some(candidate.revision as i64) {
                return Err(ApiError::conflict("A recommendation changed. Ask again."));
            }
        }
        let context = json!({"question":question,"conversation":conversation,"previous_results":turn.previous_items,"previous_clarification":turn.clarification,"tool_observations":observations,"decisions_left":MAX_DECISIONS-step,"read_tools_left":4-reads});
        let decision = match state
            .ask_model
            .decide(context, step == MAX_DECISIONS - 1)
            .await
        {
            Ok(v) => v,
            Err(_) => break,
        };
        sqlx::query("UPDATE ask_runs SET input_tokens=input_tokens+$2,output_tokens=output_tokens+$3 WHERE id=$1").bind(run).bind(decision.input_tokens.max(0)).bind(decision.output_tokens.max(0)).execute(&state.pool).await?;
        for call in decision.calls {
            if call.name == "present_answer" {
                if let Some(mut a) = serde_json::from_value::<Answer>(call.arguments)
                    .ok()
                    .and_then(|a| validate_answer(a, &seen).ok())
                {
                    if let Some(comparison) = &a.comparison {
                        let selected = &conversation.selected_item_ids;
                        if comparison
                            .item_ids
                            .iter()
                            .any(|id| conversation.excluded_item_ids.contains(id))
                            || (!selected.is_empty()
                                && (selected.len() != comparison.item_ids.len()
                                    || selected.iter().any(|id| !comparison.item_ids.contains(id))))
                        {
                            observations.push(json!({"error":"Compare exactly the explicitly selected IDs, never excluded items. Ask for clarification if fewer than two are identified."}));
                            continue;
                        }
                    }
                    if a.new_topic && conversation.selected_item_ids.is_empty() {
                        conversation.turns = vec![question.to_owned()];
                        conversation.excluded_item_ids.clear();
                    }
                    a.results
                        .retain(|r| !conversation.excluded_item_ids.contains(&r.item_id));
                    if a.intent == "recall" {
                        a.location.clear();
                    }
                    if !a.location.is_empty()
                        && !crate::geography::normalized(&conversation.turns.join(" "))
                            .contains(&crate::geography::normalized(&a.location))
                    {
                        observations.push(json!({"error":"Location must be an explicit phrase from the original question; otherwise leave it empty."}));
                        continue;
                    }
                    let mut locations = vec![json!({"text":a.location,"role":"context"})];
                    let mut connection = state.pool.acquire().await?;
                    crate::geography::enrich(&mut connection, &mut locations).await?;
                    return Ok(Snapshot {
                        conversation,
                        answer: a,
                        candidates: seen.into_values().collect(),
                        mode: "agent".into(),
                        location_geo: locations[0]["geography"].clone(),
                        search_incomplete,
                    });
                }
                observations.push(
                    json!({"error":"Invalid answer. Use only seen item IDs and evidence IDs."}),
                );
                continue;
            }
            if reads >= 4 {
                continue;
            }
            reads += 1;
            let signature = format!("{}:{}", call.name, call.arguments);
            if !calls.insert(signature) {
                observations.push(json!({"error":"Repeated unchanged tool call; inspect existing evidence or finish."}));
                continue;
            }
            if owner(state, headers, true).await? != owner_id
                || permitted(state, owner_id).await? != Some(generation)
            {
                return Err(ApiError::bad("Processing permission changed"));
            }
            let output = match call.name.as_str() {
                "search_knowledge" => {
                    if let Ok(s) = serde_json::from_value::<Search>(call.arguments.clone()) {
                        let (found, more) = search(state, owner_id, &s.terms, s.page).await?;
                        search_incomplete |= more;
                        for c in &found {
                            seen.insert(c.item_id, c.clone());
                        }
                        json!({"items":found,"next_page":if more {Some(s.page+1)}else{None}})
                    } else {
                        json!({"error":"Use terms and page"})
                    }
                }
                "inspect_evidence" => {
                    if let Ok(i) = serde_json::from_value::<Inspect>(call.arguments.clone()) {
                        json!({"items":i.item_ids.iter().take(8).filter_map(|id|seen.get(id)).collect::<Vec<_>>()})
                    } else {
                        json!({"error":"Use seen item_ids"})
                    }
                }
                _ => json!({"error":"Tool unavailable"}),
            };
            observations.push(json!({"tool":call.name,"arguments":call.arguments,"result":output}));
        }
    }
    seen.retain(|id, _| !conversation.excluded_item_ids.contains(id));
    Ok(Snapshot {
        conversation,
        answer: fallback(question, &seen),
        candidates: seen.into_values().collect(),
        mode: "limited".into(),
        location_geo: Value::Null,
        search_incomplete: true,
    })
}

pub async fn page(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Query(q): Query<PageQuery>,
) -> ApiResult {
    let owner_id = owner(&state, &headers, true).await?;
    page_response(&state, owner_id, id, q.offset.unwrap_or(0)).await
}
async fn page_response(state: &AppState, owner_id: Uuid, id: Uuid, offset: usize) -> ApiResult {
    if offset > 32 || !offset.is_multiple_of(PAGE) {
        return Err(ApiError::bad("Invalid answer page"));
    }
    let row=sqlx::query("SELECT result,permission_generation FROM ask_runs WHERE id=$1 AND owner_id=$2 AND expires_at>now()")
        .bind(id).bind(owner_id).fetch_optional(&state.pool).await?.ok_or_else(||ApiError::not_found("This answer expired. Ask again."))?;
    if permitted(state, owner_id).await? != Some(row.get("permission_generation")) {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "processing_changed",
            "Your processing settings changed. Ask again.",
        ));
    }
    let snapshot: Snapshot = serde_json::from_value(row.try_get("result").unwrap_or(Value::Null))
        .map_err(|_| {
        ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "ask_failed",
            "Couldn’t finish this answer. Try again.",
        )
    })?;
    let mut valid = vec![];
    let mut changed = false;
    for result in &snapshot.answer.results {
        let Some(candidate) = snapshot
            .candidates
            .iter()
            .find(|c| c.item_id == result.item_id)
        else {
            continue;
        };
        if let Some(item) = crate::app::owner_item(state, owner_id, result.item_id).await? {
            if item["revision"].as_i64() != Some(candidate.revision as i64) {
                changed = true;
                continue;
            }
            let mut section = result.section.clone();
            let mut caveat = result.caveat.clone();
            if snapshot.answer.intent == "discovery" && !snapshot.answer.location.is_empty() {
                let mut current_candidate = candidate.clone();
                current_candidate.locations = item["recommendation"]["locations"].clone();
                match location_match(&current_candidate, &snapshot.location_geo) {
                    LocationMatch::No => continue,
                    LocationMatch::Unknown => {
                        section = "worth_checking".into();
                        caveat = "Location or service area needs checking.".into();
                    }
                    LocationMatch::Yes => {}
                }
            }
            let support: Vec<_> = candidate
                .evidence
                .iter()
                .filter(|e| result.evidence_ids.contains(&e.id))
                .collect();
            valid.push(json!({"item":item,"section":section,"reason":result.reason,"caveat":caveat,"evidence":support}));
        } else {
            changed = true;
        }
    }
    let mut comparison = Value::Null;
    if let Some(c) = &snapshot.answer.comparison {
        let mut items = vec![];
        for item_id in &c.item_ids {
            let candidate = snapshot.candidates.iter().find(|v| &v.item_id == item_id);
            let item = crate::app::owner_item(state, owner_id, *item_id).await?;
            match (candidate, item) {
                (Some(saved), Some(item))
                    if item["revision"].as_i64() == Some(saved.revision as i64) =>
                {
                    items.push(item)
                }
                _ => changed = true,
            }
        }
        // Invalidate the whole comparison, including its conclusion, if any source changes.
        if !changed {
            let support = |id: Uuid, ids: &[String]| -> Vec<&Evidence> {
                snapshot
                    .candidates
                    .iter()
                    .find(|v| v.item_id == id)
                    .into_iter()
                    .flat_map(|v| v.evidence.iter())
                    .filter(|e| ids.contains(&e.id))
                    .collect()
            };
            comparison = json!({"items":items,"dimensions":c.dimensions.iter().map(|d| json!({
                "label":d.label,"cells":d.cells.iter().map(|cell| json!({"item_id":cell.item_id,
                    "text":cell.text,"evidence":support(cell.item_id, &cell.evidence_ids)})).collect::<Vec<_>>()
            })).collect::<Vec<_>>(),"conclusion":c.conclusion,
                "citations":c.citations.iter().map(|r| json!({"item_id":r.item_id,"evidence":support(r.item_id,&r.evidence_ids)})).collect::<Vec<_>>()});
        }
    }
    let next = if valid.len() > offset + PAGE {
        Some(offset + PAGE)
    } else {
        None
    };
    Ok(ok(
        json!({"version":1,"request_id":id,"question":snapshot.conversation.turns.last(),"turn_count":snapshot.conversation.turns.len(),"selected_item_ids":snapshot.conversation.selected_item_ids,"excluded_item_ids":snapshot.conversation.excluded_item_ids,"mode":snapshot.mode,"search_incomplete":snapshot.search_incomplete,"intent":snapshot.answer.intent,
        "title":if changed {"Saved recommendations"}else{&snapshot.answer.title},"clarification":if changed {""}else{&snapshot.answer.clarification},
        "choices":if changed {vec![]}else{snapshot.answer.choices},"location":snapshot.answer.location,"changed":changed,
        "comparison":comparison,"results":valid.into_iter().skip(offset).take(PAGE).collect::<Vec<_>>(),"next_offset":next}),
    ))
}
#[derive(PartialEq, Debug)]
enum LocationMatch {
    Yes,
    No,
    Unknown,
}
fn location_match(c: &Candidate, query: &Value) -> LocationMatch {
    let Some(area_id) = query["area_id"]
        .as_str()
        .filter(|_| query["status"] == "resolved")
    else {
        return LocationMatch::Unknown;
    };
    let roles: &[&str] = if c.entity_kind == "place" {
        &["venue"]
    } else {
        &["service_area", "practice"]
    };
    let Some(locations) = c.locations.as_array() else {
        return LocationMatch::Unknown;
    };
    let relevant: Vec<_> = locations
        .iter()
        .filter(|l| roles.contains(&l["role"].as_str().unwrap_or("")))
        .collect();
    let mut resolved = false;
    let mut unknown = relevant.is_empty();
    for location in relevant {
        let geography = &location["geography"];
        if geography["status"] != "resolved" {
            unknown = true;
            continue;
        }
        resolved = true;
        if geography["filter_ids"]
            .as_array()
            .is_some_and(|ids| ids.iter().any(|v| v.as_str() == Some(area_id)))
        {
            return LocationMatch::Yes;
        }
    }
    if resolved && !unknown {
        LocationMatch::No
    } else {
        LocationMatch::Unknown
    }
}

/// Expire answer content independently of client activity; retain only bounded usage receipts.
pub async fn sweep(pool: &sqlx::PgPool) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE ask_runs SET result=NULL,status=CASE WHEN status='running' THEN 'failed' ELSE status END WHERE expires_at<now() AND (result IS NOT NULL OR status='running')").execute(pool).await?;
    sqlx::query("UPDATE ask_runs SET status='failed',result=NULL WHERE status='running' AND created_at<now()-interval '1 minute'").execute(pool).await?;
    sqlx::query("DELETE FROM ask_runs WHERE created_at<now()-interval '30 days'")
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn cancel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> ApiResult {
    let owner_id = owner(&state, &headers, true).await?;
    let mut tx = state.pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(827713)")
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE ask_runs SET status='cancelled',result=NULL WHERE id=$1 AND owner_id=$2")
        .bind(id)
        .bind(owner_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO ask_runs(id,owner_id,request_hash,status,permission_generation,reserved_microusd) SELECT $1,$2,'cancelled','cancelled',0,0 WHERE (SELECT count(*) FROM ask_runs WHERE owner_id=$2 AND created_at>now()-interval '24 hours')<100 ON CONFLICT(id) DO NOTHING")
        .bind(id).bind(owner_id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(ok(json!({"cancelled":true})))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn candidate() -> Candidate {
        Candidate {
            item_id: Uuid::nil(),
            revision: 1,
            subject: "Lantern".into(),
            entity_kind: "place".into(),
            evidence: vec![Evidence {
                id: "summary".into(),
                kind: "summary".into(),
                text: "Quiet meal, but small tables.".into(),
            }],
            locations: json!([{"role":"venue","geography":{"status":"resolved","filter_ids":["pune","india"]}}]),
        }
    }
    #[test]
    fn geography_enforces_roles_and_unknowns() {
        let mut c = candidate();
        let pune = json!({"status":"resolved","area_id":"pune"});
        assert_eq!(location_match(&c, &pune), LocationMatch::Yes);
        assert_eq!(
            location_match(&c, &json!({"status":"resolved","area_id":"delhi"})),
            LocationMatch::No
        );
        c.locations[0]["role"] = json!("past_experience");
        assert_eq!(location_match(&c, &pune), LocationMatch::Unknown);
        c.entity_kind = "person_service".into();
        assert_eq!(location_match(&c, &pune), LocationMatch::Unknown);
        c.locations[0]["role"] = json!("service_area");
        assert_eq!(location_match(&c, &pune), LocationMatch::Yes);
        assert_eq!(location_match(&c, &Value::Null), LocationMatch::Unknown);
    }
    #[test]
    fn forged_ids_and_evidence_cannot_enter_an_answer() {
        let c = candidate();
        let seen = HashMap::from([(c.item_id, c.clone())]);
        let good = ProposedResult {
            item_id: c.item_id,
            section: "supported".into(),
            reason: "A quiet meal.".into(),
            caveat: "Small tables.".into(),
            evidence_ids: vec!["summary".into()],
        };
        let mut bad = good.clone();
        bad.item_id = Uuid::new_v4();
        let mut fake_evidence = good.clone();
        fake_evidence.evidence_ids = vec!["invented".into()];
        let a = Answer {
            intent: "discovery".into(),
            comparison: None,
            new_topic: false,
            title: "For dinner".into(),
            location: String::new(),
            clarification: String::new(),
            choices: vec![],
            results: vec![bad, fake_evidence, good],
        };
        // Invalid entries must not poison an otherwise valid entry of the same ID.
        assert_eq!(validate_answer(a, &seen).unwrap().results.len(), 1);
    }
    #[test]
    fn comparison_requires_same_participants_and_grounded_cells_and_conclusion() {
        let first = candidate();
        let mut second = candidate();
        second.item_id = Uuid::new_v4();
        let seen = HashMap::from([
            (first.item_id, first.clone()),
            (second.item_id, second.clone()),
        ]);
        let value = json!({"intent":"comparison","title":"A useful comparison","location":"", "clarification":"","choices":[],"results":[],
            "comparison":{"item_ids":[first.item_id,second.item_id],"dimensions":[{"label":"Price","cells":[
                {"item_id":first.item_id,"text":"Definitely cheap","evidence_ids":[]},
                {"item_id":second.item_id,"text":"Not saved","evidence_ids":[]}
            ]}],"conclusion":"","citations":[]}});
        let check = |v| validate_answer(serde_json::from_value(v).unwrap(), &seen);
        let valid = check(value.clone()).unwrap();
        assert_eq!(
            valid.comparison.unwrap().dimensions[0].cells[0].text,
            "Not saved"
        );
        let mut bad = value.clone();
        bad["comparison"]["dimensions"][0]["cells"][0]["evidence_ids"] = json!(["invented"]);
        assert!(check(bad).is_err());
        let mut bad = value.clone();
        bad["comparison"]["dimensions"][0]["cells"][1]["item_id"] = json!(first.item_id);
        assert!(check(bad).is_err());
        let mut bad = value.clone();
        bad["comparison"]["item_ids"][1] = json!(Uuid::new_v4());
        assert!(check(bad).is_err());
        let mut bad = value.clone();
        bad["comparison"]["conclusion"] = json!("A is better");
        assert!(check(bad).is_err());
        let mut bad = value;
        bad["comparison"]["dimensions"][0]["cells"]
            .as_array_mut()
            .unwrap()
            .pop();
        assert!(check(bad).is_err());
    }
    #[test]
    fn projections_exclude_raw_contact_fields_and_source_support() {
        let r = json!({"summary":"Reliable driver.","contact":{"phone":"+919876543210"},"source_support":{"raw":"private transcript"}});
        let text = serde_json::to_string(&evidence(&r, "unused")).unwrap();
        assert!(!text.contains("9876543210"));
        assert!(!text.contains("private transcript"));
    }
}
