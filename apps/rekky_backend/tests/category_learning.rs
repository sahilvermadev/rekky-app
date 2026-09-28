use async_trait::async_trait;
use rekky_backend::{
    category_learning::*,
    migrate,
    taxonomy::{self, Vocabulary},
};
use serde_json::{Value, json};
use sqlx::{PgPool, Row};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use uuid::Uuid;

fn definition() -> Definition {
    serde_json::from_value(json!({"decision":"create","existing_id":"","label":"Fountain pen restorer","definition":"A service that repairs and restores fountain pens.","parent_id":"","aliases":["fountain pen restorer"],"evidence_focus":["work_performed","limitations"],"positive_examples":["Restores damaged fountain pen nibs.","Repairs filling mechanisms in fountain pens."],"negative_examples":["Sells new pens without doing repairs.","Restores wooden desks rather than pens."]})).unwrap()
}
fn review() -> Review {
    serde_json::from_value(json!({"approve":true,"generic_no_private_identity":true,"correct_meaning":true,"equivalent_aliases":true,"no_existing_duplicate":true,"compatible_kind_and_parent":true,"examples_correct":true,"safe_optional_focus":true})).unwrap()
}
#[test]
fn registry_validation_rejects_related_aliases_conflicts_and_executable_guidance() {
    let v = taxonomy::vocabulary();
    let d = definition();
    assert!(check_definition(&d, "fountain pen restorer", "person_service", v).is_ok());
    for mutate in 0..7 {
        let mut d = d.clone();
        match mutate {
            0 => d.aliases.push("doctor".into()),
            1 => d.evidence_focus.push("send_contacts".into()),
            2 => d.definition = "Visit https://example.com for details".into(),
            3 => d.parent_id = "place.restaurant".into(),
            4 => d.aliases = vec!["pen restoration specialist".into()],
            5 => d.aliases.push("writing instrument repairer".into()),
            _ => d.negative_examples[0] = d.positive_examples[0].clone(),
        }
        assert!(check_definition(&d, "fountain pen restorer", "person_service", v).is_err());
    }
    let mut r = review();
    r.equivalent_aliases = false;
    assert!(!r.accepted());
    assert_eq!(
        job_key("person_service", "Fountain Pen Restorer"),
        job_key("person_service", "fountain   pen restorer")
    );
    assert_ne!(
        job_key("thing", "fountain pen restorer"),
        job_key("person_service", "fountain pen restorer")
    );
}
struct Learner {
    proposals: AtomicUsize,
    reviews: AtomicUsize,
    reject: bool,
    block: Option<(Arc<tokio::sync::Notify>, Arc<tokio::sync::Notify>)>,
    fail: bool,
    proposal: Option<Definition>,
}
impl Learner {
    fn good() -> Self {
        Self {
            proposals: AtomicUsize::new(0),
            reviews: AtomicUsize::new(0),
            reject: false,
            block: None,
            fail: false,
            proposal: None,
        }
    }
}
#[async_trait]
impl CategoryLearner for Learner {
    fn available(&self) -> bool {
        true
    }
    async fn propose(&self, _: &str, _: &str, _: &Vocabulary) -> Result<Definition, ()> {
        self.proposals.fetch_add(1, Ordering::SeqCst);
        if let Some((start, release)) = &self.block {
            start.notify_one();
            release.notified().await;
        }
        if self.fail {
            Err(())
        } else {
            Ok(self.proposal.clone().unwrap_or_else(definition))
        }
    }
    async fn review(&self, _: &str, _: &str, _: &Definition, _: &Vocabulary) -> Result<Review, ()> {
        self.reviews.fetch_add(1, Ordering::SeqCst);
        let mut r = review();
        if self.reject {
            r.approve = false;
        }
        Ok(r)
    }
}
async fn reset(pool: &PgPool) {
    sqlx::query("DELETE FROM category_registry_events")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM category_discoveries")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM category_learning_attempts")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM category_learning_jobs")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM learned_category_aliases")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM learned_categories")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("UPDATE category_registry_state SET revision=0")
        .execute(pool)
        .await
        .unwrap();
}
async fn seed(pool: &PgPool, owner: Uuid) -> Uuid {
    let capture = Uuid::new_v4();
    let source = Uuid::new_v4();
    let item = Uuid::new_v4();
    sqlx::query("INSERT INTO captures(id,owner_id,kind,status,desired_visibility) VALUES($1,$2,'voice','completed','private')").bind(capture).bind(owner).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO source_texts(id,capture_id,owner_id,kind,content) VALUES($1,$2,$3,'transcript','An invented fountain pen restorer repaired a pen.')").bind(source).bind(capture).bind(owner).execute(pool).await.unwrap();
    let mut c = taxonomy::present("person_service", &[], &[], &[], "extracted").unwrap();
    c["descriptive_type"] = json!("fountain pen restorer");
    c["display_label"] = json!("fountain pen restorer");
    let rec = json!({"version":2,"entity_kind":"person_service","summary":"A useful nib repair.","shelf":"People & services","experience":"firsthand","observations":[],"locations":[],"use_cases":[],"classification":c,"rating":{"value":7,"scale":10,"origin":"inferred"},"contact":{"phone":"+12025550123","saved_name":"Synthetic contact"}});
    sqlx::query("INSERT INTO knowledge_items(id,capture_id,owner_id,subject,body,visibility,recommendation) VALUES($1,$2,$3,'Synthetic restorer','A useful nib repair.','private',$4)").bind(item).bind(capture).bind(owner).bind(rec).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO item_source_support(item_id,source_id,source_revision,pipeline_version,support) VALUES($1,$2,1,2,'{}')").bind(item).bind(source).execute(pool).await.unwrap();
    let mut conn = pool.acquire().await.unwrap();
    enqueue(&mut conn, item).await.unwrap();
    item
}
// One sequential DB test owns a separate database; never truncates the app or
// the other integration test database. Caller must explicitly supply its URL.
#[tokio::test]
async fn background_learning_deduplicates_reuses_and_fences_results() {
    let Ok(url) = std::env::var("CATEGORY_TEST_DATABASE_URL") else {
        return;
    };
    assert!(
        url.ends_with("/rekky_category_learning_test"),
        "dedicated database required"
    );
    let pool = PgPool::connect(&url).await.unwrap();
    migrate::run(&pool).await.unwrap();
    reset(&pool).await;
    let owner = Uuid::new_v4();
    sqlx::query("INSERT INTO accounts(id) VALUES($1)")
        .bind(owner)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO transcript_extraction_permissions(account_id,enabled,generation,disclosure_version) VALUES($1,true,1,1)").bind(owner).execute(&pool).await.unwrap();
    let a = seed(&pool, owner).await;
    let b = seed(&pool, owner).await;
    let before: Value =
        sqlx::query_scalar("SELECT recommendation FROM knowledge_items WHERE id=$1")
            .bind(a)
            .fetch_one(&pool)
            .await
            .unwrap();
    let learner = Learner::good();
    let (r1, r2) = tokio::join!(process_one(&pool, &learner), process_one(&pool, &learner));
    r1.unwrap();
    r2.unwrap();
    assert_eq!(learner.proposals.load(Ordering::SeqCst), 1);
    assert_eq!(learner.reviews.load(Ordering::SeqCst), 1);
    for item in [a, b] {
        let row = sqlx::query(
            "SELECT recommendation,visibility,revision FROM knowledge_items WHERE id=$1",
        )
        .bind(item)
        .fetch_one(&pool)
        .await
        .unwrap();
        let after: Value = row.get("recommendation");
        assert_eq!(
            after["classification"]["display_label"],
            "Fountain pen restorer"
        );
        for field in ["summary", "contact", "rating"] {
            assert_eq!(after[field], before[field]);
        }
        assert_eq!(row.get::<String, _>("visibility"), "private");
        assert_eq!(row.get::<i32, _>("revision"), 2);
    }
    let c = catalog(&pool).await.unwrap();
    let id = c
        .concepts
        .iter()
        .find(|c| c.label == "Fountain pen restorer")
        .unwrap()
        .id
        .clone();
    assert_eq!(
        taxonomy::query_concepts_in(&c, "fountain pen restorer").0,
        vec![id.clone()]
    );
    let prompt = taxonomy::for_source(&c, "I used a fountain pen restorer.");
    assert!(taxonomy::concept_in(&prompt, &id).is_some());
    assert!(taxonomy::concept_in(&taxonomy::for_source(&c, "a cafe"), &id).is_none());
    let next = seed(&pool, owner).await;
    assert!(!process_one(&pool, &learner).await.unwrap());
    assert_eq!(learner.proposals.load(Ordering::SeqCst), 1);
    let r: Value = sqlx::query_scalar("SELECT recommendation FROM knowledge_items WHERE id=$1")
        .bind(next)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(r["classification"]["types"][0]["id"], id);

    // Exercise the signed-in API with the learned registry, not only helpers.
    // These read-only routes never invoke any provider object.
    let token = Uuid::new_v4().to_string();
    sqlx::query("UPDATE accounts SET disclosure_accepted_at=now() WHERE id=$1")
        .bind(owner)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO sessions(token_hash,account_id,expires_at) VALUES($1,$2,now()+interval '1 hour')").bind(rekky_backend::auth::hash_token(&token)).bind(owner).execute(&pool).await.unwrap();
    let app = rekky_backend::router(rekky_backend::AppState {
        pool: pool.clone(),
        daily_account_limit: 12,
        ask_daily_account_limit: 30,
        verifier: Arc::new(rekky_backend::auth::OidcVerifier::from_env()),
        transcriber: Arc::new(rekky_backend::voice::OpenAiTranscriber::from_env()),
        extractor: Arc::new(rekky_backend::extraction::OpenAiExtractor::from_env()),
        places: Arc::new(rekky_backend::places::GooglePlaces::from_env()),
        ask_model: Arc::new(rekky_backend::ask::OpenAiAsk::from_env()),
    });
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use tower::ServiceExt;
    for (method, path, body) in [
        ("GET", "/v1/taxonomy", ""),
        ("POST", "/v1/ask", r#"{"question":"fountain pen restorer"}"#),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .header("authorization", format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let value: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 100_000).await.unwrap())
                .unwrap();
        if method == "GET" {
            assert!(
                value["concepts"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|c| c["id"] == id)
            );
            assert!(!value.to_string().contains("Synthetic contact"));
        } else {
            assert_eq!(value["results"].as_array().unwrap().len(), 3);
        }
    }
    // Learned IDs work in extraction, wire presentation and owner editing.
    let proposal=serde_json::from_value(json!({"items":[{"subject":"pen restorer","subject_evidence":[1],"entity_kind":"person_service","experience":"firsthand","account":[{"kind":"context","text":"Used a fountain pen restorer.","evidence":[1]}],"locations":[],"use_cases":[],"classification":{"types":[{"concept_id":id,"source_phrase":"fountain pen restorer","evidence":[1]}],"facets":[],"descriptors":[]}}],"ignored_unit_ids":[],"unresolved_unit_ids":[]})).unwrap();
    let (validated, partial) = rekky_backend::extraction::validate_with_catalog(
        proposal,
        "I used a fountain pen restorer.",
        &c,
    )
    .unwrap();
    assert!(!partial);
    assert_eq!(
        validated[0].recommendation["classification"]["types"][0]["id"],
        id
    );
    let mut input: Value = serde_json::from_str(include_str!(
        "../../../contracts/rekky/v1/fixtures/recommendation_edit.json"
    ))
    .unwrap();
    input["entity_kind"] = json!("person_service");
    input["types"] = json!([id]);
    input["facets"] = json!([]);
    let input: rekky_backend::editing::EditInput = serde_json::from_value(input).unwrap();
    assert_eq!(
        input.build_with_catalog(&c).unwrap().0["classification"]["display_label"],
        "Fountain pen restorer"
    );
    suspend(&pool, &id).await.unwrap();
    let mut after = catalog(&pool).await.unwrap();
    assert!(taxonomy::concept_in(&after, &id).is_none());
    let mut conn = pool.acquire().await.unwrap();
    include_stored_types(&mut conn, &mut after, &r)
        .await
        .unwrap();
    drop(conn);
    assert!(input.build_with_catalog(&after).is_ok());
    let unchanged: Value =
        sqlx::query_scalar("SELECT recommendation FROM knowledge_items WHERE id=$1")
            .bind(next)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(r, unchanged);
    // A reviewed synonym reuses a seed ID, and revocation removes future reuse.
    reset(&pool).await;
    let item = seed(&pool, owner).await;
    sqlx::query("DELETE FROM category_discoveries WHERE item_id=$1")
        .bind(item)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE knowledge_items SET recommendation=jsonb_set(recommendation,'{classification,descriptive_type}','\"cab provider\"') WHERE id=$1").bind(item).execute(&pool).await.unwrap();
    enqueue(&mut pool.acquire().await.unwrap(), item)
        .await
        .unwrap();
    let mut alias = Learner::good();
    let mut d = definition();
    d.decision = "alias".into();
    d.existing_id = "service.taxi".into();
    d.label = "Taxi service".into();
    d.aliases = vec!["cab provider".into()];
    d.definition = "A provider of taxi rides.".into();
    alias.proposal = Some(d);
    process_one(&pool, &alias).await.unwrap();
    let active = catalog(&pool).await.unwrap();
    assert_eq!(
        taxonomy::query_concepts_in(&active, "cab provider").0,
        vec!["service.taxi"]
    );
    assert_eq!(active.concepts.len(), taxonomy::vocabulary().concepts.len());
    suspend_alias(&pool, "cab provider").await.unwrap();
    assert!(
        !taxonomy::concept_in(&catalog(&pool).await.unwrap(), "service.taxi")
            .unwrap()
            .aliases
            .contains(&"cab provider".into())
    );
    // Rejections are cached, not silently retried on every capture.
    reset(&pool).await;
    seed(&pool, owner).await;
    let mut reject = Learner::good();
    reject.reject = true;
    assert!(process_one(&pool, &reject).await.unwrap());
    seed(&pool, owner).await;
    assert!(!process_one(&pool, &reject).await.unwrap());
    assert_eq!(reject.proposals.load(Ordering::SeqCst), 1);
    // Owner correction, deletion, source removal and opt-out fence late publication.
    for change in 0..4 {
        reset(&pool).await;
        sqlx::query("UPDATE transcript_extraction_permissions SET enabled=true,generation=generation+1 WHERE account_id=$1").bind(owner).execute(&pool).await.unwrap();
        let item = seed(&pool, owner).await;
        let start = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let mut slow = Learner::good();
        slow.block = Some((start.clone(), release.clone()));
        let slow = Arc::new(slow);
        let run = slow.clone();
        let p = pool.clone();
        let task = tokio::spawn(async move { process_one(&p, run.as_ref()).await });
        start.notified().await;
        match change {
            0 => {
                sqlx::query("UPDATE knowledge_items SET recommendation=jsonb_set(recommendation,'{origin}','\"user\"'),revision=revision+1 WHERE id=$1").bind(item).execute(&pool).await.unwrap();
            }
            1 => {
                sqlx::query(
                    "UPDATE knowledge_items SET deleted_at=now(),revision=revision+1 WHERE id=$1",
                )
                .bind(item)
                .execute(&pool)
                .await
                .unwrap();
            }
            2 => {
                sqlx::query("DELETE FROM source_texts WHERE capture_id=(SELECT capture_id FROM knowledge_items WHERE id=$1)").bind(item).execute(&pool).await.unwrap();
            }
            _ => {
                sqlx::query("UPDATE transcript_extraction_permissions SET enabled=false,generation=generation+1 WHERE account_id=$1").bind(owner).execute(&pool).await.unwrap();
            }
        }
        release.notify_one();
        task.await.unwrap().unwrap();
        assert_eq!(slow.reviews.load(Ordering::SeqCst), 0);
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM learned_categories")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }
    reset(&pool).await;
    sqlx::query("UPDATE transcript_extraction_permissions SET enabled=true,generation=generation+1 WHERE account_id=$1").bind(owner).execute(&pool).await.unwrap();
    seed(&pool, owner).await;
    let mut failed = Learner::good();
    failed.fail = true;
    assert!(process_one(&pool, &failed).await.unwrap());
    assert!(!process_one(&pool, &failed).await.unwrap());
    sqlx::query("UPDATE category_learning_jobs SET retry_at=now()")
        .execute(&pool)
        .await
        .unwrap();
    assert!(process_one(&pool, &failed).await.unwrap());
    assert!(!process_one(&pool, &failed).await.unwrap());
    assert_eq!(failed.proposals.load(Ordering::SeqCst), 2);
    // Reservations enforce per-account and global limits without provider work.
    for (reservations, same_owner) in [(4, true), (12, false)] {
        reset(&pool).await;
        seed(&pool, owner).await;
        let key = job_key("person_service", "fountain pen restorer");
        for _ in 0..reservations {
            sqlx::query(
                "INSERT INTO category_learning_attempts(id,job_key,owner_id) VALUES($1,$2,$3)",
            )
            .bind(Uuid::new_v4())
            .bind(&key)
            .bind(if same_owner { Some(owner) } else { None })
            .execute(&pool)
            .await
            .unwrap();
        }
        let limited = Learner::good();
        assert!(!process_one(&pool, &limited).await.unwrap());
        assert_eq!(limited.proposals.load(Ordering::SeqCst), 0);
    }
    // An expired final lease is terminal; a crashed worker cannot spend forever.
    reset(&pool).await;
    seed(&pool, owner).await;
    sqlx::query("UPDATE category_learning_jobs SET status='processing',attempts=2,attempt_id=$1,lease_until=now()-interval '1 minute'").bind(Uuid::new_v4()).execute(&pool).await.unwrap();
    assert!(!process_one(&pool, &Learner::good()).await.unwrap());
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM category_learning_jobs")
            .fetch_one(&pool)
            .await
            .unwrap(),
        "failed"
    );
    reset(&pool).await;
    sqlx::query("DELETE FROM accounts WHERE id=$1")
        .bind(owner)
        .execute(&pool)
        .await
        .unwrap();
}
