use async_trait::async_trait;
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode},
};
use rekky_backend::{
    AppState,
    auth::{IdentityVerifier, Provider, VerifyError, hash_token},
    extraction::{ExtractionError, Proposal, TranscriptExtractor, source_units},
    migrate, router,
    voice::{TranscriptionError, VoiceTranscriber},
};
use serde_json::{Value, json};
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::Notify;
use tower::ServiceExt;
use uuid::Uuid;

async fn categorized_item(
    t: &TestApp,
    token: &str,
    subject: &str,
    kind: &str,
    type_id: &str,
    facets: &[String],
) -> Value {
    let (status, saved) = t.call(Method::POST,"/v1/items",Some(token),
        Some(json!({"subject":subject,"body":"A saved experience in Pune.","visibility":"private"})),
        &[("idempotency-key",&Uuid::new_v4().to_string())]).await;
    assert_eq!(status, StatusCode::CREATED);
    let item = saved["item"].clone();
    let classification =
        rekky_backend::taxonomy::present(kind, &[type_id.to_owned()], facets, &[], "extracted")
            .unwrap();
    let rec = json!({"version":2,"entity_kind":kind,"shelf":"Test shelf","experience":"firsthand",
        "summary":"A saved experience in Pune.","observations":[],"locations":[],"use_cases":[],"classification":classification});
    sqlx::query("UPDATE knowledge_items SET recommendation=$1 WHERE id=$2")
        .bind(rec)
        .bind(Uuid::parse_str(item["id"].as_str().unwrap()).unwrap())
        .execute(&t.pool)
        .await
        .unwrap();
    item
}

#[tokio::test]
async fn categories_search_synonyms_parents_facets_and_exact_titles_without_cross_owner_results() {
    let Some(mut t) = TestApp::new().await else {
        return;
    };
    let (_, token) = t.sign_in("google", "valid-a").await;
    t.call(
        Method::POST,
        "/v1/me/visibility-disclosure",
        Some(&token),
        Some(json!({"accept":true})),
        &[],
    )
    .await;
    let gp = categorized_item(
        &t,
        &token,
        "Neha",
        "person_service",
        "service.general_doctor",
        &[],
    )
    .await;
    let doctor =
        categorized_item(&t, &token, "Mira", "person_service", "service.doctor", &[]).await;
    categorized_item(
        &t,
        &token,
        "Dental clinic",
        "person_service",
        "service.dentist",
        &[],
    )
    .await;
    let italian = categorized_item(
        &t,
        &token,
        "Lantern",
        "place",
        "place.restaurant",
        &["cuisine.italian".into()],
    )
    .await;
    categorized_item(
        &t,
        &token,
        "Lotus",
        "place",
        "place.restaurant",
        &["cuisine.thai".into()],
    )
    .await;
    categorized_item(&t, &token, "Harbour", "place", "place.bar", &[]).await;
    let film = categorized_item(&t, &token, "Doctor Who", "thing", "thing.film", &[]).await;
    for (query, expected) in [
        ("general physician", vec![gp["id"].clone()]),
        ("GP", vec![gp["id"].clone()]),
        ("doctors", vec![gp["id"].clone(), doctor["id"].clone()]),
        ("चिकित्सक", vec![gp["id"].clone(), doctor["id"].clone()]),
        ("Italian restaurants", vec![italian["id"].clone()]),
        ("Italian restaurants in Pune", vec![italian["id"].clone()]),
        ("Italian restaurants in Mumbai", vec![]),
    ] {
        let (status, response) = t
            .call(
                Method::POST,
                "/v1/ask",
                Some(&token),
                Some(json!({"question":query})),
                &[],
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{response}");
        let actual: Vec<_> = response["results"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["item_id"].clone())
            .collect();
        assert_eq!(actual.len(), expected.len(), "{query}: {actual:?}");
        assert!(expected.iter().all(|id| actual.contains(id)), "{query}");
    }
    let (_, exact) = t
        .call(
            Method::POST,
            "/v1/ask",
            Some(&token),
            Some(json!({"question":"Doctor Who"})),
            &[],
        )
        .await;
    assert_eq!(exact["results"][0]["item_id"], film["id"]);
    let (_, other) = t.sign_in("google", "valid-b").await;
    t.call(
        Method::POST,
        "/v1/me/visibility-disclosure",
        Some(&other),
        Some(json!({"accept":true})),
        &[],
    )
    .await;
    let (_, private) = t
        .call(
            Method::POST,
            "/v1/ask",
            Some(&other),
            Some(json!({"question":"physician"})),
            &[],
        )
        .await;
    assert_eq!(private["results"], json!([]));
    t.cleanup().await;
}

#[tokio::test]
async fn category_corrections_are_owner_revision_fenced_and_survive_source_deletion() {
    let Some(mut t) = TestApp::new().await else {
        return;
    };
    let (_, token) = t.sign_in("google", "valid-a").await;
    t.call(
        Method::POST,
        "/v1/me/visibility-disclosure",
        Some(&token),
        Some(json!({"accept":true})),
        &[],
    )
    .await;
    let item = categorized_item(&t, &token, "Neha", "person_service", "service.doctor", &[]).await;
    let path = format!("/v1/items/{}/classification", item["id"].as_str().unwrap());
    let input = json!({"types":["service.general_doctor"],"facets":[]});
    let (_, other) = t.sign_in("google", "valid-b").await;
    t.call(
        Method::POST,
        "/v1/me/visibility-disclosure",
        Some(&other),
        Some(json!({"accept":true})),
        &[],
    )
    .await;
    assert_eq!(
        t.call(
            Method::PATCH,
            &path,
            Some(&other),
            Some(input.clone()),
            &[("if-match", "1")]
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    for bad in [
        json!({"types":["place.bar"],"facets":[]}),
        json!({"types":["invented"],"facets":[]}),
        json!({"types":[],"facets":["cuisine.italian"]}),
    ] {
        assert_eq!(
            t.call(
                Method::PATCH,
                &path,
                Some(&token),
                Some(bad),
                &[("if-match", "1")]
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
    }
    let (status, edited) = t
        .call(
            Method::PATCH,
            &path,
            Some(&token),
            Some(input.clone()),
            &[("if-match", "1")],
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{edited}");
    assert_eq!(edited["item"]["revision"], 2);
    assert_eq!(edited["item"]["body"], item["body"]);
    assert_eq!(edited["item"]["visibility"], "private");
    assert_eq!(
        edited["item"]["recommendation"]["classification"]["origin"],
        "user"
    );
    assert_eq!(
        t.call(
            Method::PATCH,
            &path,
            Some(&token),
            Some(input),
            &[("if-match", "1")]
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let source_path = format!(
        "/v1/captures/{}/source",
        item["capture_id"].as_str().unwrap()
    );
    assert_eq!(
        t.call(
            Method::DELETE,
            &source_path,
            Some(&token),
            None,
            &[("if-match", "1")]
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    let (_, found) = t
        .call(
            Method::POST,
            "/v1/ask",
            Some(&token),
            Some(json!({"question":"general physician"})),
            &[],
        )
        .await;
    assert_eq!(found["results"][0]["item_id"], item["id"]);
    let delete_path = format!("/v1/items/{}", item["id"].as_str().unwrap());
    assert_eq!(
        t.call(
            Method::DELETE,
            &delete_path,
            Some(&token),
            None,
            &[("if-match", "2")]
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    let (_, found) = t
        .call(
            Method::POST,
            "/v1/ask",
            Some(&token),
            Some(json!({"question":"general physician"})),
            &[],
        )
        .await;
    assert_eq!(found["results"], json!([]));
    t.cleanup().await;
}

struct TestVerifier {
    marker: Uuid,
}
#[async_trait]
impl IdentityVerifier for TestVerifier {
    async fn verify(&self, provider: Provider, token: &str) -> Result<String, VerifyError> {
        if !matches!(token, "valid-a" | "valid-b") {
            return Err(VerifyError::Rejected);
        }
        Ok(format!("{}:{}:{token}", self.marker, provider.as_str()))
    }
}
struct TestPlaces;
#[async_trait]
impl rekky_backend::places::PlaceResolver for TestPlaces {
    fn available(&self) -> bool {
        true
    }
    async fn resolve(
        &self,
        _query: &rekky_backend::places::PlaceQuery,
    ) -> Option<rekky_backend::places::PlaceMatch> {
        Some(rekky_backend::places::PlaceMatch {
            place_id: "synthetic-place".into(),
            address: "12 Sample Road, Pune".into(),
        })
    }
}
struct TestTranscriber;
struct TestExtractor;
#[async_trait]
impl TranscriptExtractor for TestExtractor {
    fn available(&self) -> bool {
        true
    }
    async fn extract(&self, transcript: &str) -> Result<Proposal, ExtractionError> {
        let ids: Vec<_> = source_units(transcript).iter().map(|u| u.id).collect();
        Ok(serde_json::from_value(json!({
            "items":[{"subject":transcript.split_whitespace().next().unwrap_or_default(),
                "subject_evidence":ids,"entity_kind":"person_service","experience":"firsthand",
                "summary":{"text":transcript,"evidence":ids},"observations":[],"locations":[],"use_cases":[]}],
            "ignored_unit_ids":[],"unresolved_unit_ids":[],
            "readable_source":source_units(transcript).iter().map(|u|json!({
                "unit_id":u.id,"corrections":[{"before":u.text,"after":u.text.replace(".", "!")}],"paragraph_start":false
            })).collect::<Vec<_>>()
        })).unwrap())
    }
}
#[async_trait]
impl VoiceTranscriber for TestTranscriber {
    fn available(&self) -> bool {
        true
    }
    async fn transcribe(&self, _audio: Vec<u8>) -> Result<String, TranscriptionError> {
        Ok("Ravi fixed the kitchen tap on Tuesday.".to_owned())
    }
}
struct BlockingTranscriber {
    started: Notify,
    release: Notify,
}
struct FailingTranscriber;
#[async_trait]
impl VoiceTranscriber for FailingTranscriber {
    fn available(&self) -> bool {
        true
    }
    async fn transcribe(&self, _audio: Vec<u8>) -> Result<String, TranscriptionError> {
        Err(TranscriptionError::Failed)
    }
}
#[async_trait]
impl VoiceTranscriber for BlockingTranscriber {
    fn available(&self) -> bool {
        true
    }
    async fn transcribe(&self, _audio: Vec<u8>) -> Result<String, TranscriptionError> {
        self.started.notify_one();
        self.release.notified().await;
        Ok("Ravi fixed the kitchen tap on Tuesday.".to_owned())
    }
}
struct TestApp {
    pool: PgPool,
    app: Router,
    state: AppState,
    ids: Vec<Uuid>,
}
impl TestApp {
    async fn new() -> Option<Self> {
        Self::new_with_transcriber(Arc::new(TestTranscriber)).await
    }
    async fn new_with_transcriber(transcriber: Arc<dyn VoiceTranscriber>) -> Option<Self> {
        let url = std::env::var("DATABASE_URL").ok()?;
        let pool = PgPool::connect(&url).await.unwrap();
        migrate::run(&pool).await.unwrap();
        let verifier = Arc::new(TestVerifier {
            marker: Uuid::new_v4(),
        });
        let state = AppState {
            pool: pool.clone(),
            daily_account_limit: 12,
            ask_daily_account_limit: 30,
            verifier,
            transcriber,
            extractor: Arc::new(TestExtractor),
            places: Arc::new(TestPlaces),
            ask_model: Arc::new(rekky_backend::ask::OpenAiAsk::from_env()),
        };
        let app = router(state.clone());
        Some(Self {
            pool,
            app,
            state,
            ids: vec![],
        })
    }
    async fn call(
        &self,
        method: Method,
        path: &str,
        token: Option<&str>,
        body: Option<Value>,
        extra: &[(&str, &str)],
    ) -> (StatusCode, Value) {
        let mut builder = Request::builder().method(method).uri(path);
        if let Some(token) = token {
            builder = builder.header("authorization", format!("Bearer {token}"));
        }
        if body.is_some() {
            builder = builder.header("content-type", "application/json");
        }
        for (key, value) in extra {
            builder = builder.header(*key, *value);
        }
        let request = builder
            .body(Body::from(body.map(|v| v.to_string()).unwrap_or_default()))
            .unwrap();
        let response = self.app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let value = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap()
        };
        (status, value)
    }
    async fn call_audio(
        &self,
        path: &str,
        token: &str,
        audio: Vec<u8>,
        captured_ms: i64,
    ) -> (StatusCode, Value) {
        let request = Request::builder()
            .method(Method::POST)
            .uri(path)
            .header("authorization", format!("Bearer {token}"))
            .header("content-type", "audio/mp4")
            .header("x-captured-at-ms", captured_ms.to_string())
            .body(Body::from(audio))
            .unwrap();
        let response = self.app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap())
    }
    async fn sign_in(&mut self, provider: &str, id_token: &str) -> (Uuid, String) {
        let (status, body) = self
            .call(
                Method::POST,
                "/v1/session/exchange",
                None,
                Some(json!({"provider":provider,"id_token":id_token})),
                &[],
            )
            .await;
        assert_eq!(status, StatusCode::CREATED);
        let id = Uuid::parse_str(body["account"]["id"].as_str().unwrap()).unwrap();
        self.ids.push(id);
        (id, body["session"]["token"].as_str().unwrap().to_owned())
    }
    async fn cleanup(&self) {
        for id in &self.ids {
            sqlx::query("DELETE FROM accounts WHERE id=$1")
                .bind(id)
                .execute(&self.pool)
                .await
                .unwrap();
        }
    }
}
#[tokio::test]
async fn wire_fixtures() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../contracts/rekky/v1/fixtures/wire.json"
    ))
    .unwrap();
    assert_eq!(fixture["version"], 1);
    let names: Vec<&str> = fixture["examples"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["name"].as_str().unwrap())
        .collect();
    for name in [
        "session",
        "signed_out",
        "disclosure_required",
        "account",
        "saved_item",
        "private_override_ack",
        "items_page",
        "ask_result",
        "text_retained_audio_deleted",
        "source_deleted_item_retained",
        "idempotency_conflict",
        "processing_withdrawal_ack",
        "voice_permission",
        "voice_permission_ack",
        "voice_transcript_ready",
        "voice_permission_required",
        "transcript_extraction_permission",
        "voice_knowledge_saved",
        "structured_recommendation",
        "remember_queued",
        "remember_waiting_limit",
        "remember_saved",
        "remember_partial",
        "remember_cancelled",
    ] {
        assert!(names.contains(&name), "missing {name}");
    }
}
#[tokio::test]
async fn signed_in_text_memory_privacy_and_non_resurrection() {
    let Some(mut t) = TestApp::new().await else {
        return;
    };
    assert_eq!(
        t.call(Method::GET, "/v1/items", None, None, &[]).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        t.call(
            Method::POST,
            "/v1/session/exchange",
            None,
            Some(json!({"provider":"google","id_token":"bad"})),
            &[]
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    let (id_a, a) = t.sign_in("google", "valid-a").await;
    let (id_b, b) = t.sign_in("apple", "valid-b").await;
    let foreign_capture = Uuid::new_v4();
    sqlx::query("INSERT INTO captures(id,owner_id,kind,status,desired_visibility) VALUES ($1,$2,'typed','completed','friends')")
        .bind(foreign_capture).bind(id_a).execute(&t.pool).await.unwrap();
    assert!(sqlx::query("INSERT INTO source_texts(id,capture_id,owner_id,kind,content) VALUES ($1,$2,$3,'typed','private')")
        .bind(Uuid::new_v4()).bind(foreign_capture).bind(id_b).execute(&t.pool).await.is_err());
    assert_eq!(
        t.call(Method::GET, "/v1/items", Some(&a), None, &[])
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    for token in [&a, &b] {
        assert_eq!(
            t.call(
                Method::POST,
                "/v1/me/visibility-disclosure",
                Some(token),
                Some(json!({"accept":true})),
                &[]
            )
            .await
            .0,
            StatusCode::OK
        );
    }
    let key = Uuid::new_v4().to_string();
    let input =
        json!({"subject":"Raju","body":"Raju fixed our kitchen tap; ask Priya for his number."});
    let (status, saved) = t
        .call(
            Method::POST,
            "/v1/items",
            Some(&a),
            Some(input.clone()),
            &[("idempotency-key", &key)],
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(saved["item"]["visibility"], "friends");
    let item = saved["item"]["id"].as_str().unwrap();
    let capture = saved["item"]["capture_id"].as_str().unwrap();
    assert_eq!(
        t.call(Method::GET, "/v1/items", Some(&b), None, &[])
            .await
            .1["items"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    assert_eq!(
        t.call(
            Method::GET,
            &format!("/v1/captures/{capture}"),
            Some(&b),
            None,
            &[]
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        t.call(
            Method::GET,
            &format!("/v1/captures/{capture}"),
            Some(&a),
            None,
            &[]
        )
        .await
        .1["capture"]["source"]["text"],
        input["body"]
    );
    sqlx::query("INSERT INTO processing_permissions(account_id,enabled,generation,purpose,provider_ids,disclosure_version) VALUES ($1,true,1,'test',ARRAY['test'],1)")
        .bind(id_a).execute(&t.pool).await.unwrap();
    let (_, withdrawal) = t
        .call(
            Method::POST,
            "/v1/me/processing-withdrawal",
            Some(&a),
            Some(json!({})),
            &[],
        )
        .await;
    assert_eq!(
        withdrawal["processing"],
        json!({"enabled":false,"generation":2,"acknowledged":true})
    );
    assert_eq!(
        t.call(
            Method::GET,
            &format!("/v1/captures/{capture}"),
            Some(&a),
            None,
            &[]
        )
        .await
        .1["capture"]["source"]["text"],
        input["body"]
    );
    assert_eq!(
        t.call(Method::GET, "/v1/items", Some(&a), None, &[])
            .await
            .1["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        t.call(
            Method::POST,
            "/v1/items",
            Some(&a),
            Some(input.clone()),
            &[("idempotency-key", &key)]
        )
        .await
        .1["item"]["id"],
        item
    );
    assert_eq!(
        t.call(
            Method::POST,
            "/v1/items",
            Some(&a),
            Some(json!({"subject":"Other","body":input["body"]})),
            &[("idempotency-key", &key)]
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let (status, changed) = t
        .call(
            Method::PATCH,
            &format!("/v1/items/{item}"),
            Some(&a),
            Some(json!({"visibility":"private"})),
            &[("if-match", "1")],
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(changed["item"]["visibility"], "private");
    assert_eq!(changed["item"]["revision"], 2);
    assert_eq!(
        t.call(
            Method::POST,
            "/v1/items",
            Some(&a),
            Some(input.clone()),
            &[("idempotency-key", &key)]
        )
        .await
        .1["item"]["visibility"],
        "private"
    );
    assert_eq!(
        t.call(
            Method::PATCH,
            &format!("/v1/items/{item}"),
            Some(&a),
            Some(json!({"visibility":"friends"})),
            &[("if-match", "1")]
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let (_, ask) = t
        .call(
            Method::POST,
            "/v1/ask",
            Some(&a),
            Some(json!({"question":"Who fixed the kitchen tap?"})),
            &[],
        )
        .await;
    assert_eq!(ask["results"][0]["item_id"], item);
    assert_eq!(
        t.call(
            Method::POST,
            "/v1/ask",
            Some(&b),
            Some(json!({"question":"kitchen tap"})),
            &[]
        )
        .await
        .1["results"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    let (_,another) = t.call(Method::POST,"/v1/items",Some(&a),Some(json!({"subject":"Meera","body":"Meera teaches swimming, but I have not tried her class.","visibility":"private"})),&[("idempotency-key",&Uuid::new_v4().to_string())]).await;
    let another_id = another["item"]["id"].as_str().unwrap();
    let another_capture = another["item"]["capture_id"].as_str().unwrap();
    let source_path = format!("/v1/captures/{another_capture}/source");
    assert_eq!(
        t.call(
            Method::DELETE,
            &source_path,
            Some(&b),
            None,
            &[("if-match", "1")]
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        t.call(
            Method::DELETE,
            &source_path,
            Some(&a),
            None,
            &[("if-match", "2")]
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        t.call(
            Method::DELETE,
            &source_path,
            Some(&a),
            None,
            &[("if-match", "1")]
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert!(
        t.call(
            Method::GET,
            &format!("/v1/captures/{another_capture}"),
            Some(&a),
            None,
            &[]
        )
        .await
        .1["capture"]["source"]
            .is_null()
    );
    assert_eq!(
        t.call(
            Method::POST,
            "/v1/ask",
            Some(&a),
            Some(json!({"question":"swimming"})),
            &[]
        )
        .await
        .1["results"][0]["item_id"],
        another_id
    );
    assert_eq!(
        t.call(
            Method::DELETE,
            &format!("/v1/items/{item}"),
            Some(&a),
            None,
            &[("if-match", "1")]
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        t.call(
            Method::DELETE,
            &format!("/v1/items/{item}"),
            Some(&a),
            None,
            &[("if-match", "2")]
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        t.call(
            Method::GET,
            &format!("/v1/captures/{capture}"),
            Some(&a),
            None,
            &[]
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert!(
        sqlx::query("SELECT 1 FROM source_texts WHERE capture_id=$1")
            .bind(Uuid::parse_str(capture).unwrap())
            .fetch_optional(&t.pool)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        t.call(
            Method::POST,
            "/v1/items",
            Some(&a),
            Some(input),
            &[("idempotency-key", &key)]
        )
        .await
        .0,
        StatusCode::GONE
    );
    assert_eq!(
        t.call(
            Method::POST,
            "/v1/ask",
            Some(&a),
            Some(json!({"question":"kitchen tap"})),
            &[]
        )
        .await
        .1["results"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    assert_eq!(
        t.call(Method::DELETE, "/v1/session", Some(&a), None, &[])
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        t.call(Method::GET, "/v1/me", Some(&a), None, &[]).await.0,
        StatusCode::UNAUTHORIZED
    );
    t.cleanup().await;
}

#[tokio::test]
async fn voice_requires_separate_permission_and_retains_private_transcript() {
    let Some(mut t) = TestApp::new().await else {
        return;
    };
    let (owner_id, token) = t.sign_in("google", "valid-a").await;
    let (_, foreign_token) = t.sign_in("google", "valid-b").await;
    assert_eq!(
        t.call(
            Method::POST,
            "/v1/me/visibility-disclosure",
            Some(&token),
            Some(json!({"accept":true})),
            &[]
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        t.call(
            Method::POST,
            "/v1/me/visibility-disclosure",
            Some(&foreign_token),
            Some(json!({"accept":true})),
            &[]
        )
        .await
        .0,
        StatusCode::OK
    );
    let mut audio = vec![0_u8; 230_698];
    audio[4..8].copy_from_slice(b"ftyp");
    let path = "/v1/voice-drafts/abcdefghijklmnopabcdefghijklmnop/transcribe";
    let captured_ms = chrono::Utc::now().timestamp_millis();
    assert_eq!(
        t.call_audio(path, &token, audio.clone(), captured_ms)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        t.call(
            Method::POST,
            "/v1/me/voice-transcription-permission",
            Some(&token),
            Some(json!({"enabled":true,"disclosure_version":99})),
            &[]
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (status, permission) = t
        .call(
            Method::POST,
            "/v1/me/voice-transcription-permission",
            Some(&token),
            Some(json!({"enabled":true,"disclosure_version":1})),
            &[],
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(permission["voice_transcription"]["generation"], 1);
    let (status, saved) = t.call_audio(path, &token, audio.clone(), captured_ms).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(saved["capture"]["status"], "transcript_ready");
    assert_eq!(saved["server_audio_retained"], false);
    let capture_id = saved["capture"]["id"].as_str().unwrap();
    let (status, repeated) = t.call_audio(path, &token, audio, captured_ms).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(repeated["capture"]["id"], capture_id);
    assert_eq!(
        t.call(
            Method::GET,
            "/v1/voice-captures",
            Some(&foreign_token),
            None,
            &[]
        )
        .await
        .1["voice_captures"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    let (status, listed) = t
        .call(Method::GET, "/v1/voice-captures", Some(&token), None, &[])
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listed["voice_captures"][0]["id"], capture_id);
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT desired_visibility FROM captures WHERE id=$1 AND owner_id=$2",
        )
        .bind(Uuid::parse_str(capture_id).unwrap())
        .bind(owner_id)
        .fetch_one(&t.pool)
        .await
        .unwrap(),
        "friends"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM knowledge_items WHERE owner_id=$1")
            .bind(owner_id)
            .fetch_one(&t.pool)
            .await
            .unwrap(),
        0
    );
    let (status, withdrawn) = t
        .call(
            Method::POST,
            "/v1/me/voice-transcription-permission",
            Some(&token),
            Some(json!({"enabled":false})),
            &[],
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(withdrawn["voice_transcription"]["generation"], 2);
    assert_eq!(
        t.call(Method::GET, "/v1/voice-captures", Some(&token), None, &[])
            .await
            .1["voice_captures"][0]["id"],
        capture_id
    );
    let source_path = format!("/v1/captures/{capture_id}/source");
    assert_eq!(
        t.call(
            Method::DELETE,
            &source_path,
            Some(&foreign_token),
            None,
            &[("if-match", "1")],
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        t.call(
            Method::DELETE,
            &source_path,
            Some(&token),
            None,
            &[("if-match", "1")],
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert!(
        t.call(Method::GET, "/v1/voice-captures", Some(&token), None, &[])
            .await
            .1["voice_captures"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    t.cleanup().await;
}

#[tokio::test]
async fn voice_withdrawal_fences_in_flight_result() {
    let transcriber = Arc::new(BlockingTranscriber {
        started: Notify::new(),
        release: Notify::new(),
    });
    let Some(mut t) = TestApp::new_with_transcriber(transcriber.clone()).await else {
        return;
    };
    let (_, token) = t.sign_in("google", "valid-a").await;
    assert_eq!(
        t.call(
            Method::POST,
            "/v1/me/visibility-disclosure",
            Some(&token),
            Some(json!({"accept":true})),
            &[],
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        t.call(
            Method::POST,
            "/v1/me/voice-transcription-permission",
            Some(&token),
            Some(json!({"enabled":true,"disclosure_version":1})),
            &[],
        )
        .await
        .0,
        StatusCode::OK
    );
    let mut audio = vec![0_u8; 230_698];
    audio[4..8].copy_from_slice(b"ftyp");
    let path = "/v1/voice-drafts/fedcbafedcbafedcbafedcbafedcbafe/transcribe";
    let captured_ms = chrono::Utc::now().timestamp_millis();
    let started = transcriber.started.notified();
    let upload = t.call_audio(path, &token, audio.clone(), captured_ms);
    tokio::pin!(upload);
    tokio::select! {
        _ = started => {},
        result = &mut upload => panic!("upload ended before provider wait: {result:?}"),
    }
    let (status, _) = t
        .call(
            Method::POST,
            "/v1/me/voice-transcription-permission",
            Some(&token),
            Some(json!({"enabled":false})),
            &[],
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    transcriber.release.notify_one();
    let (status, response) = upload.await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(response["error"]["code"], "voice_permission_changed");
    assert_eq!(
        t.call(Method::GET, "/v1/voice-captures", Some(&token), None, &[])
            .await
            .1["voice_captures"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    t.call(
        Method::POST,
        "/v1/me/voice-transcription-permission",
        Some(&token),
        Some(json!({"enabled":true,"disclosure_version":1})),
        &[],
    )
    .await;
    assert_eq!(
        t.call_audio(path, &token, audio, captured_ms).await.0,
        StatusCode::CONFLICT
    );
    t.cleanup().await;
}

#[tokio::test]
async fn voice_budget_blocks_new_jobs_but_allows_idempotent_receipt_recovery() {
    let Some(mut t) = TestApp::new().await else {
        return;
    };
    let (_, token) = t.sign_in("google", "valid-a").await;
    t.call(
        Method::POST,
        "/v1/me/visibility-disclosure",
        Some(&token),
        Some(json!({"accept":true})),
        &[],
    )
    .await;
    t.call(
        Method::POST,
        "/v1/me/voice-transcription-permission",
        Some(&token),
        Some(json!({"enabled":true,"disclosure_version":1})),
        &[],
    )
    .await;
    let mut audio = vec![0_u8; 256];
    audio[4..8].copy_from_slice(b"ftyp");
    let captured_ms = chrono::Utc::now().timestamp_millis();
    for index in 0..12 {
        let path = format!("/v1/voice-drafts/{index:032}/transcribe");
        assert_eq!(
            t.call_audio(&path, &token, audio.clone(), captured_ms)
                .await
                .0,
            StatusCode::CREATED
        );
    }
    assert_eq!(
        t.call_audio(
            "/v1/voice-drafts/99999999999999999999999999999999/transcribe",
            &token,
            audio.clone(),
            captured_ms,
        )
        .await
        .0,
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(
        t.call_audio(
            "/v1/voice-drafts/00000000000000000000000000000000/transcribe",
            &token,
            audio,
            captured_ms,
        )
        .await
        .0,
        StatusCode::OK
    );
    t.cleanup().await;
}

#[tokio::test]
async fn voice_failed_draft_has_bounded_paid_retries() {
    let Some(mut t) = TestApp::new_with_transcriber(Arc::new(FailingTranscriber)).await else {
        return;
    };
    let (_, token) = t.sign_in("google", "valid-a").await;
    t.call(
        Method::POST,
        "/v1/me/visibility-disclosure",
        Some(&token),
        Some(json!({"accept":true})),
        &[],
    )
    .await;
    t.call(
        Method::POST,
        "/v1/me/voice-transcription-permission",
        Some(&token),
        Some(json!({"enabled":true,"disclosure_version":1})),
        &[],
    )
    .await;
    let mut audio = vec![0_u8; 256];
    audio[4..8].copy_from_slice(b"ftyp");
    let path = "/v1/voice-drafts/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/transcribe";
    let captured_ms = chrono::Utc::now().timestamp_millis();
    for _ in 0..3 {
        assert_eq!(
            t.call_audio(path, &token, audio.clone(), captured_ms)
                .await
                .0,
            StatusCode::BAD_GATEWAY
        );
    }
    assert_eq!(
        t.call_audio(path, &token, audio, captured_ms).await.0,
        StatusCode::TOO_MANY_REQUESTS
    );
    t.cleanup().await;
}

#[tokio::test]
async fn transcript_extraction_saves_private_grounded_item_for_own_ask() {
    let Some(mut t) = TestApp::new().await else {
        return;
    };
    let (owner_id, token) = t.sign_in("google", "valid-a").await;
    let (_, other) = t.sign_in("google", "valid-b").await;
    for token in [&token, &other] {
        assert_eq!(
            t.call(
                Method::POST,
                "/v1/me/visibility-disclosure",
                Some(token),
                Some(json!({"accept":true})),
                &[]
            )
            .await
            .0,
            StatusCode::OK
        );
    }
    let capture_id = Uuid::new_v4();
    let source = "Ravi fixed the kitchen tap. He was careful and explained the repair.";
    sqlx::query("INSERT INTO captures(id,owner_id,kind,status,desired_visibility) VALUES ($1,$2,'voice','transcript_ready','private')")
        .bind(capture_id).bind(owner_id).execute(&t.pool).await.unwrap();
    sqlx::query("INSERT INTO source_texts(id,capture_id,owner_id,kind,content) VALUES ($1,$2,$3,'transcript',$4)")
        .bind(Uuid::new_v4()).bind(capture_id).bind(owner_id).bind(source).execute(&t.pool).await.unwrap();
    let path = format!("/v1/voice-captures/{capture_id}/extract");
    assert_eq!(
        t.call(Method::POST, &path, Some(&token), None, &[]).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        t.call(
            Method::POST,
            "/v1/me/transcript-extraction-permission",
            Some(&token),
            Some(json!({"enabled":true,"disclosure_version":1})),
            &[]
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, result) = t.call(Method::POST, &path, Some(&token), None, &[]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["partial"], false);
    assert_eq!(result["items"].as_array().unwrap().len(), 1);
    assert_eq!(result["items"][0]["subject"], "Ravi");
    assert_eq!(result["items"][0]["body"], source);
    assert_eq!(result["items"][0]["visibility"], "private");
    assert_eq!(result["items"][0]["capture_id"], capture_id.to_string());
    assert_eq!(
        t.call(Method::POST, &path, Some(&token), None, &[]).await.1["items"][0]["id"],
        result["items"][0]["id"]
    );
    assert_eq!(
        t.call(Method::POST, &path, Some(&other), None, &[]).await.0,
        StatusCode::FORBIDDEN
    );
    let (_, found) = t
        .call(
            Method::POST,
            "/v1/ask",
            Some(&token),
            Some(json!({"question":"Who fixed our kitchen tap?"})),
            &[],
        )
        .await;
    assert_eq!(found["results"][0]["item_id"], result["items"][0]["id"]);
    let (_, hidden) = t
        .call(
            Method::POST,
            "/v1/ask",
            Some(&other),
            Some(json!({"question":"Who fixed our kitchen tap?"})),
            &[],
        )
        .await;
    assert!(hidden["results"].as_array().unwrap().is_empty());
    let (_, listed) = t
        .call(Method::GET, "/v1/voice-captures", Some(&token), None, &[])
        .await;
    assert_eq!(listed["voice_captures"][0]["item_count"], 1);
    assert_eq!(
        listed["voice_captures"][0]["extraction_status"],
        "completed"
    );
    t.cleanup().await;
}

#[tokio::test]
async fn deleting_one_voice_item_preserves_shared_source_until_last_item() {
    let Some(mut t) = TestApp::new().await else {
        return;
    };
    let (owner_id, token) = t.sign_in("google", "valid-a").await;
    assert_eq!(
        t.call(
            Method::POST,
            "/v1/me/visibility-disclosure",
            Some(&token),
            Some(json!({"accept":true})),
            &[]
        )
        .await
        .0,
        StatusCode::OK
    );
    let capture_id = Uuid::new_v4();
    sqlx::query("INSERT INTO captures(id,owner_id,kind,status,desired_visibility) VALUES ($1,$2,'voice','completed','private')")
        .bind(capture_id).bind(owner_id).execute(&t.pool).await.unwrap();
    sqlx::query("INSERT INTO source_texts(id,capture_id,owner_id,kind,content) VALUES ($1,$2,$3,'transcript','Ravi repaired the tap. Meera teaches swimming.')")
        .bind(Uuid::new_v4()).bind(capture_id).bind(owner_id).execute(&t.pool).await.unwrap();
    let mut item_ids = Vec::new();
    for subject in ["Ravi", "Meera"] {
        let item_id = Uuid::new_v4();
        sqlx::query("INSERT INTO knowledge_items(id,capture_id,owner_id,subject,body,visibility) VALUES ($1,$2,$3,$4,$5,'private')")
            .bind(item_id).bind(capture_id).bind(owner_id).bind(subject).bind(subject)
            .execute(&t.pool).await.unwrap();
        item_ids.push(item_id);
    }
    for (index, item_id) in item_ids.iter().enumerate() {
        let path = format!("/v1/items/{item_id}");
        assert_eq!(
            t.call(
                Method::DELETE,
                &path,
                Some(&token),
                None,
                &[("if-match", "1")]
            )
            .await
            .0,
            StatusCode::NO_CONTENT
        );
        let count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM source_texts WHERE capture_id=$1")
                .bind(capture_id)
                .fetch_one(&t.pool)
                .await
                .unwrap();
        assert_eq!(count, if index == 0 { 1 } else { 0 });
    }
    t.cleanup().await;
}

#[tokio::test]
async fn synthetic_ask_baseline() {
    let Some(mut t) = TestApp::new().await else {
        return;
    };
    let corpus: Value = serde_json::from_str(include_str!(
        "../../../docs/evaluation/ask_text_seed_v0.json"
    ))
    .unwrap();
    assert_eq!(corpus["version"], 0);
    assert_eq!(corpus["questions"].as_array().unwrap().len(), 10);
    let id = Uuid::new_v4();
    t.ids.push(id);
    let token = format!("test_{}", Uuid::new_v4().simple());
    sqlx::query("INSERT INTO accounts(id,disclosure_accepted_at) VALUES ($1,now())")
        .bind(id)
        .execute(&t.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO sessions(token_hash,account_id,expires_at) VALUES ($1,$2,now()+interval '1 hour')")
        .bind(hash_token(&token)).bind(id).execute(&t.pool).await.unwrap();
    let mut ids = std::collections::HashMap::new();
    for item in corpus["items"].as_array().unwrap() {
        let (status, response) = t
            .call(
                Method::POST,
                "/v1/items",
                Some(&token),
                Some(json!({"subject":item["subject"],"body":item["body"],"visibility":"private"})),
                &[("idempotency-key", &Uuid::new_v4().to_string())],
            )
            .await;
        assert_eq!(status, StatusCode::CREATED);
        ids.insert(
            item["key"].as_str().unwrap().to_owned(),
            response["item"]["id"].as_str().unwrap().to_owned(),
        );
    }
    let mut metrics = std::collections::BTreeMap::<String, (usize, usize, usize)>::new();
    let mut misses = Vec::new();
    let mut forbidden = Vec::new();
    for case in corpus["questions"].as_array().unwrap() {
        let (status, response) = t
            .call(
                Method::POST,
                "/v1/ask",
                Some(&token),
                Some(json!({"question":case["question"]})),
                &[],
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{}", case["key"]);
        let results = response["results"].as_array().unwrap();
        if case["key"] == "no_answer" {
            assert!(results.is_empty());
        }
        if case["key"] == "warning" {
            assert!(results.iter().any(|r| r["item_id"] == ids["kuuraku"]
                && r["body"].as_str().unwrap().contains("small tables")));
        }
        let shown: Vec<_> = results
            .iter()
            .take(5)
            .map(|r| r["item_id"].as_str().unwrap())
            .collect();
        let language = case["language"].as_str().unwrap().to_owned();
        let metric = metrics.entry(language).or_default();
        let expected = case["expected"].as_array().unwrap();
        if !expected.is_empty() {
            metric.1 += 1;
            if expected
                .iter()
                .any(|k| shown.contains(&ids[k.as_str().unwrap()].as_str()))
            {
                metric.0 += 1;
            } else {
                misses.push(case["key"].as_str().unwrap());
            }
        }
        if case["forbidden"]
            .as_array()
            .unwrap()
            .iter()
            .any(|k| shown.contains(&ids[k.as_str().unwrap()].as_str()))
        {
            metric.2 += 1;
            forbidden.push(case["key"].as_str().unwrap());
        }
    }
    println!("Synthetic lexical baseline: {metrics:?}; missed={misses:?}; forbidden={forbidden:?}");
    assert_eq!(metrics.values().map(|m| m.0).sum::<usize>(), 8);
    assert_eq!(metrics.values().map(|m| m.1).sum::<usize>(), 9);
    assert_eq!(forbidden.len(), 3);
    t.cleanup().await;
}

#[tokio::test]
async fn remember_finishes_without_a_session_and_purges_audio_atomically() {
    let Some(mut t) = TestApp::new().await else {
        return;
    };
    let (owner, token) = t.sign_in("google", "valid-a").await;
    let (_, other) = t.sign_in("google", "valid-b").await;
    for who in [&token, &other] {
        assert_eq!(
            t.call(
                Method::POST,
                "/v1/me/visibility-disclosure",
                Some(who),
                Some(json!({"accept":true})),
                &[]
            )
            .await
            .0,
            StatusCode::OK
        );
    }
    let mut audio = vec![0u8; 256];
    audio[4..8].copy_from_slice(b"ftyp");
    let path = "/v1/remember/automatic-recording-001";
    let now = chrono::Utc::now().timestamp_millis();
    assert_eq!(
        t.call_audio(path, &token, audio.clone(), now).await.0,
        StatusCode::FORBIDDEN
    );
    for permission in ["voice-transcription", "transcript-extraction"] {
        assert_eq!(
            t.call(
                Method::POST,
                &format!("/v1/me/{permission}-permission"),
                Some(&token),
                Some(json!({"enabled":true,"disclosure_version":1})),
                &[]
            )
            .await
            .0,
            StatusCode::OK
        );
    }
    let (status, receipt) = t.call_audio(path, &token, audio.clone(), now).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    assert_eq!(receipt["remember"]["status"], "queued");
    assert_eq!(receipt["remember"]["transcript_saved"], false);
    assert_eq!(
        t.call_audio(path, &token, audio.clone(), now).await.0,
        StatusCode::OK
    );
    let mut different = audio.clone();
    different[100] = 3;
    assert_eq!(
        t.call_audio(path, &token, different, now).await.0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        t.call(Method::GET, path, Some(&other), None, &[]).await.0,
        StatusCode::NOT_FOUND
    );
    // The worker has no session token and does not depend on the phone polling.
    assert_eq!(
        t.call(Method::DELETE, "/v1/session", Some(&token), None, &[])
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    rekky_backend::app::process_pending_voice(&t.state, Some(owner))
        .await
        .unwrap();
    let (status, audio_gone, capture_id): (String, bool, Uuid) = sqlx::query_as(
        "SELECT status,audio IS NULL,capture_id FROM voice_uploads WHERE account_id=$1",
    )
    .bind(owner)
    .fetch_one(&t.pool)
    .await
    .unwrap();
    assert_eq!(status, "transcribed");
    assert!(audio_gone);
    let count:i64=sqlx::query_scalar("SELECT count(*) FROM knowledge_items WHERE capture_id=$1 AND visibility='friends' AND deleted_at IS NULL")
        .bind(capture_id).fetch_one(&t.pool).await.unwrap();
    assert_eq!(count, 1);
    rekky_backend::app::process_pending_voice(&t.state, Some(owner))
        .await
        .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM knowledge_items WHERE capture_id=$1")
        .bind(capture_id)
        .fetch_one(&t.pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    let (_, new_token) = t.sign_in("google", "valid-a").await;
    let (_, receipt) = t.call(Method::GET, path, Some(&new_token), None, &[]).await;
    assert_eq!(receipt["remember"]["capture_status"], "completed");
    assert_eq!(receipt["remember"]["transcript_saved"], true);
    let (_, answer) = t
        .call(
            Method::POST,
            "/v1/ask",
            Some(&new_token),
            Some(json!({"question":"kitchen tap"})),
            &[],
        )
        .await;
    assert_eq!(answer["results"].as_array().unwrap().len(), 1);
    let item = &answer["results"][0];
    assert_eq!(item["visibility"], "friends");
    // A Friends marker does not create cross-account access ahead of friendships.
    assert_eq!(
        t.call(
            Method::GET,
            &format!("/v1/captures/{capture_id}"),
            Some(&other),
            None,
            &[]
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert!(
        t.call(
            Method::POST,
            "/v1/ask",
            Some(&other),
            Some(json!({"question":"kitchen tap"})),
            &[]
        )
        .await
        .1["results"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let item_path = format!("/v1/items/{}", item["item_id"].as_str().unwrap());
    let revision = item["revision"].to_string();
    let (status, _) = t
        .call(
            Method::PATCH,
            &item_path,
            Some(&new_token),
            Some(json!({"visibility":"private"})),
            &[("if-match", &revision)],
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    // Receipt recovery and repeated extraction must not reset an owner's choice.
    let (_, repeated) = t
        .call(
            Method::POST,
            &format!("/v1/voice-captures/{capture_id}/extract"),
            Some(&new_token),
            None,
            &[],
        )
        .await;
    assert_eq!(repeated["items"][0]["visibility"], "private");

    t.cleanup().await;
}

#[tokio::test]
async fn remember_withdrawal_purges_queued_audio_and_never_restarts_it() {
    let Some(mut t) = TestApp::new().await else {
        return;
    };
    let (owner, token) = t.sign_in("google", "valid-a").await;
    t.call(
        Method::POST,
        "/v1/me/visibility-disclosure",
        Some(&token),
        Some(json!({"accept":true})),
        &[],
    )
    .await;
    for permission in ["voice-transcription", "transcript-extraction"] {
        t.call(
            Method::POST,
            &format!("/v1/me/{permission}-permission"),
            Some(&token),
            Some(json!({"enabled":true,"disclosure_version":1})),
            &[],
        )
        .await;
    }
    let mut audio = vec![0u8; 256];
    audio[4..8].copy_from_slice(b"ftyp");
    let path = "/v1/remember/automatic-withdraw-001";
    assert_eq!(
        t.call_audio(path, &token, audio, chrono::Utc::now().timestamp_millis())
            .await
            .0,
        StatusCode::ACCEPTED
    );
    t.call(
        Method::POST,
        "/v1/me/transcript-extraction-permission",
        Some(&token),
        Some(json!({"enabled":false})),
        &[],
    )
    .await;
    t.call(
        Method::POST,
        "/v1/me/transcript-extraction-permission",
        Some(&token),
        Some(json!({"enabled":true,"disclosure_version":1})),
        &[],
    )
    .await;
    rekky_backend::app::process_pending_voice(&t.state, Some(owner))
        .await
        .unwrap();
    let (status, gone): (String, bool) =
        sqlx::query_as("SELECT status,audio IS NULL FROM voice_uploads WHERE account_id=$1")
            .bind(owner)
            .fetch_one(&t.pool)
            .await
            .unwrap();
    assert_eq!(status, "cancelled");
    assert!(gone);
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM voice_transcription_jobs WHERE account_id=$1")
            .bind(owner)
            .fetch_one(&t.pool)
            .await
            .unwrap();
    assert_eq!(count, 0);
    t.cleanup().await;
}

#[tokio::test]
async fn remember_cancellation_fences_delayed_upload_and_inflight_transcription() {
    let transcriber = Arc::new(BlockingTranscriber {
        started: Notify::new(),
        release: Notify::new(),
    });
    let Some(mut t) = TestApp::new_with_transcriber(transcriber.clone()).await else {
        return;
    };
    let (owner, token) = t.sign_in("google", "valid-a").await;
    t.call(
        Method::POST,
        "/v1/me/visibility-disclosure",
        Some(&token),
        Some(json!({"accept":true})),
        &[],
    )
    .await;
    for permission in ["voice-transcription", "transcript-extraction"] {
        t.call(
            Method::POST,
            &format!("/v1/me/{permission}-permission"),
            Some(&token),
            Some(json!({"enabled":true,"disclosure_version":1})),
            &[],
        )
        .await;
    }
    let mut audio = vec![0u8; 256];
    audio[4..8].copy_from_slice(b"ftyp");
    let now = chrono::Utc::now().timestamp_millis();
    let delayed = "/v1/remember/cancelled-before-upload-001";
    assert_eq!(
        t.call(Method::DELETE, delayed, Some(&token), None, &[])
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    let (_, receipt) = t.call_audio(delayed, &token, audio.clone(), now).await;
    assert_eq!(receipt["remember"]["status"], "cancelled");
    let path = "/v1/remember/cancelled-during-upload-001";
    assert_eq!(
        t.call_audio(path, &token, audio, now).await.0,
        StatusCode::ACCEPTED
    );
    let state = t.state.clone();
    let worker = tokio::spawn(async move {
        rekky_backend::app::process_pending_voice(&state, Some(owner))
            .await
            .unwrap();
    });
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        transcriber.started.notified(),
    )
    .await
    .unwrap();
    assert_eq!(
        t.call(Method::DELETE, path, Some(&token), None, &[])
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    transcriber.release.notify_one();
    tokio::time::timeout(std::time::Duration::from_secs(5), worker)
        .await
        .unwrap()
        .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM captures WHERE owner_id=$1")
        .bind(owner)
        .fetch_one(&t.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    let remaining:i64=sqlx::query_scalar("SELECT count(*) FROM voice_uploads WHERE account_id=$1 AND (audio IS NOT NULL OR status!='cancelled')").bind(owner).fetch_one(&t.pool).await.unwrap();
    assert_eq!(remaining, 0);
    t.cleanup().await;
}

struct BlockingExtractor {
    started: Notify,
    release: Notify,
}
#[async_trait]
impl TranscriptExtractor for BlockingExtractor {
    fn available(&self) -> bool {
        true
    }
    async fn extract(&self, transcript: &str) -> Result<Proposal, ExtractionError> {
        self.started.notify_one();
        self.release.notified().await;
        TestExtractor.extract(transcript).await
    }
}
async fn legacy_voice_item(t: &mut TestApp) -> (Uuid, String, String, String) {
    let (owner, token) = t.sign_in("google", "valid-a").await;
    t.call(
        Method::POST,
        "/v1/me/visibility-disclosure",
        Some(&token),
        Some(json!({"accept":true})),
        &[],
    )
    .await;
    t.call(
        Method::POST,
        "/v1/me/transcript-extraction-permission",
        Some(&token),
        Some(json!({"enabled":true,"disclosure_version":1})),
        &[],
    )
    .await;
    let (_,saved)=t.call(Method::POST,"/v1/items",Some(&token),Some(json!({"subject":"Ravi","body":"Ravi fixed the kitchen tap.","visibility":"private"})),&[("idempotency-key","legacy-item-0001")]).await;
    let item = saved["item"]["id"].as_str().unwrap().to_owned();
    let capture = saved["item"]["capture_id"].as_str().unwrap().to_owned();
    sqlx::query("UPDATE captures SET kind='voice' WHERE id=$1")
        .bind(Uuid::parse_str(&capture).unwrap())
        .execute(&t.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE source_texts SET kind='transcript' WHERE capture_id=$1")
        .bind(Uuid::parse_str(&capture).unwrap())
        .execute(&t.pool)
        .await
        .unwrap();
    (owner, token, item, capture)
}
#[tokio::test]
async fn explicit_refinement_preserves_identity_privacy_and_source_deletion_removes_support() {
    let Some(mut t) = TestApp::new().await else {
        return;
    };
    let (_owner, token, item, capture) = legacy_voice_item(&mut t).await;
    let path = format!("/v1/items/{item}/refine");
    let (status, result) = t
        .call(
            Method::POST,
            &path,
            Some(&token),
            None,
            &[("if-match", "1")],
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["items"][0]["id"], item);
    assert_eq!(result["items"][0]["visibility"], "private");
    assert_eq!(result["items"][0]["revision"], 2);
    assert_eq!(
        result["items"][0]["recommendation"]["experience"],
        "firsthand"
    );
    assert!(!result.to_string().contains("subject_evidence"));
    let capture_path = format!("/v1/captures/{capture}");
    let (_, source) = t
        .call(Method::GET, &capture_path, Some(&token), None, &[])
        .await;
    assert_eq!(
        source["capture"]["source"]["text"],
        "Ravi fixed the kitchen tap."
    );
    assert_eq!(
        source["capture"]["source"]["readable_text"],
        "Ravi fixed the kitchen tap!"
    );
    assert!(!result.to_string().contains("readable_text"));
    let (_, other_token) = t.sign_in("google", "valid-b").await;
    assert_ne!(
        t.call(Method::GET, &capture_path, Some(&other_token), None, &[])
            .await
            .0,
        StatusCode::OK
    );
    sqlx::query("UPDATE source_texts SET readable_source_revision=0 WHERE capture_id=$1")
        .bind(Uuid::parse_str(&capture).unwrap())
        .execute(&t.pool)
        .await
        .unwrap();
    let (_, stale) = t
        .call(Method::GET, &capture_path, Some(&token), None, &[])
        .await;
    assert!(stale["capture"]["source"]["readable_text"].is_null());
    sqlx::query("UPDATE source_texts SET readable_source_revision=revision WHERE capture_id=$1")
        .bind(Uuid::parse_str(&capture).unwrap())
        .execute(&t.pool)
        .await
        .unwrap();
    // Lost-response recovery is free and doesn't create a second item.
    assert_eq!(
        t.call(
            Method::POST,
            &path,
            Some(&token),
            None,
            &[("if-match", "1")]
        )
        .await
        .0,
        StatusCode::OK
    );
    let attempts: i32 =
        sqlx::query_scalar("SELECT attempts FROM recommendation_refinement_jobs WHERE item_id=$1")
            .bind(Uuid::parse_str(&item).unwrap())
            .fetch_one(&t.pool)
            .await
            .unwrap();
    assert_eq!(attempts, 1);
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM item_source_support WHERE item_id=$1")
            .bind(Uuid::parse_str(&item).unwrap())
            .fetch_one(&t.pool)
            .await
            .unwrap();
    assert_eq!(count, 1);
    assert_eq!(
        t.call(
            Method::DELETE,
            &format!("/v1/captures/{capture}/source"),
            Some(&token),
            None,
            &[("if-match", "1")]
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM item_source_support WHERE item_id=$1")
            .bind(Uuid::parse_str(&item).unwrap())
            .fetch_one(&t.pool)
            .await
            .unwrap();
    assert_eq!(count, 0);
    let (_, listed) = t
        .call(Method::GET, "/v1/items", Some(&token), None, &[])
        .await;
    assert_eq!(listed["items"].as_array().unwrap().len(), 1);
    t.cleanup().await;
}
#[tokio::test]
async fn refinement_rejects_late_results_after_source_delete_withdrawal_or_item_edit() {
    for action in ["source", "permission", "item", "content"] {
        let Some(mut t) = TestApp::new().await else {
            return;
        };
        let (_owner, token, item, capture) = legacy_voice_item(&mut t).await;
        let extractor = Arc::new(BlockingExtractor {
            started: Notify::new(),
            release: Notify::new(),
        });
        t.state.extractor = extractor.clone();
        t.app = router(t.state.clone());
        let app = t.app.clone();
        let path = format!("/v1/items/{item}/refine");
        let owned_token = token.clone();
        let request = tokio::spawn(async move {
            app.oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(path)
                    .header("authorization", format!("Bearer {owned_token}"))
                    .header("if-match", "1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
            .status()
        });
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            extractor.started.notified(),
        )
        .await
        .unwrap();
        match action {
            "content" => {
                let input: Value = serde_json::from_str(include_str!(
                    "../../../contracts/rekky/v1/fixtures/recommendation_edit.json"
                ))
                .unwrap();
                assert_eq!(
                    t.call(
                        Method::PATCH,
                        &format!("/v1/items/{item}/content"),
                        Some(&token),
                        Some(input),
                        &[("if-match", "1")]
                    )
                    .await
                    .0,
                    StatusCode::OK
                );
            }
            "source" => {
                t.call(
                    Method::DELETE,
                    &format!("/v1/captures/{capture}/source"),
                    Some(&token),
                    None,
                    &[("if-match", "1")],
                )
                .await;
            }
            "permission" => {
                t.call(
                    Method::POST,
                    "/v1/me/transcript-extraction-permission",
                    Some(&token),
                    Some(json!({"enabled":false})),
                    &[],
                )
                .await;
            }
            _ => {
                t.call(
                    Method::PATCH,
                    &format!("/v1/items/{item}"),
                    Some(&token),
                    Some(json!({"visibility":"friends"})),
                    &[("if-match", "1")],
                )
                .await;
            }
        }
        extractor.release.notify_one();
        assert_eq!(
            tokio::time::timeout(std::time::Duration::from_secs(5), request)
                .await
                .unwrap()
                .unwrap(),
            StatusCode::CONFLICT
        );
        let value: Option<Value> =
            sqlx::query_scalar("SELECT recommendation FROM knowledge_items WHERE id=$1")
                .bind(Uuid::parse_str(&item).unwrap())
                .fetch_one(&t.pool)
                .await
                .unwrap();
        if action == "content" {
            assert_eq!(value.unwrap()["origin"], "user");
        } else {
            assert!(value.is_none());
        }
        t.cleanup().await;
    }
}

#[tokio::test]
async fn full_edits_are_atomic_searchable_owner_fenced_and_preserve_sources() {
    let Some(mut t) = TestApp::new().await else {
        return;
    };
    let (_owner, token, item, capture) = legacy_voice_item(&mut t).await;
    let original_source: String =
        sqlx::query_scalar("SELECT content FROM source_texts WHERE capture_id=$1")
            .bind(Uuid::parse_str(&capture).unwrap())
            .fetch_one(&t.pool)
            .await
            .unwrap();
    let path = format!("/v1/items/{item}/content");
    let mut input: Value = serde_json::from_str(include_str!(
        "../../../contracts/rekky/v1/fixtures/recommendation_edit.json"
    ))
    .unwrap();
    input["rating"] = json!({"mode":"set","value":4.5});
    let (_, other) = t.sign_in("google", "valid-b").await;
    t.call(
        Method::POST,
        "/v1/me/visibility-disclosure",
        Some(&other),
        Some(json!({"accept":true})),
        &[],
    )
    .await;
    assert_eq!(
        t.call(
            Method::PATCH,
            &path,
            Some(&other),
            Some(input.clone()),
            &[("if-match", "1")]
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let mut invalid = input.clone();
    invalid["facets"] = json!(["invented"]);
    invalid["visibility"] = json!("friends");
    assert_eq!(
        t.call(
            Method::PATCH,
            &path,
            Some(&token),
            Some(invalid),
            &[("if-match", "1")]
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (status, edited) = t
        .call(
            Method::PATCH,
            &path,
            Some(&token),
            Some(input.clone()),
            &[("if-match", "1")],
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{edited}");
    assert_eq!(
        edited["item"]["recommendation"]["rating"],
        json!({"value":4.5,"scale":10,"origin":"user"})
    );
    assert_eq!(edited["item"]["revision"], 2);
    assert_eq!(edited["item"]["visibility"], "private");
    assert_eq!(edited["item"]["subject"], "Lantern Cafe");
    assert_eq!(edited["item"]["recommendation"]["attribution"], "Priya");
    assert_eq!(edited["item"]["recommendation"]["origin"], "user");
    assert_eq!(
        edited["item"]["recommendation"]["classification"]["origin"],
        "user"
    );
    assert!(!edited["item"]["body"].as_str().unwrap().contains("Tuesday"));
    let source: String = sqlx::query_scalar("SELECT content FROM source_texts WHERE capture_id=$1")
        .bind(Uuid::parse_str(&capture).unwrap())
        .fetch_one(&t.pool)
        .await
        .unwrap();
    assert_eq!(source, original_source);
    assert_eq!(
        t.call(
            Method::PATCH,
            &path,
            Some(&token),
            Some(input.clone()),
            &[("if-match", "1")]
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let (_, found) = t
        .call(
            Method::POST,
            "/v1/ask",
            Some(&token),
            Some(json!({"question":"Italian restaurant mushroom"})),
            &[],
        )
        .await;
    assert_eq!(found["results"][0]["item_id"], item);
    let (_, hidden) = t
        .call(
            Method::POST,
            "/v1/ask",
            Some(&other),
            Some(json!({"question":"mushroom"})),
            &[],
        )
        .await;
    assert!(hidden["results"].as_array().unwrap().is_empty());
    // A future pipeline version must still leave owner-authored replacements alone.
    sqlx::query("UPDATE knowledge_items SET recommendation=jsonb_set(recommendation,'{version}','1'::jsonb) WHERE id=$1")
        .bind(Uuid::parse_str(&item).unwrap()).execute(&t.pool).await.unwrap();
    assert_eq!(
        t.call(
            Method::POST,
            &format!("/v1/items/{item}/refine"),
            Some(&token),
            None,
            &[("if-match", "2")]
        )
        .await
        .0,
        StatusCode::OK
    );
    let jobs: i64 =
        sqlx::query_scalar("SELECT count(*) FROM recommendation_refinement_jobs WHERE item_id=$1")
            .bind(Uuid::parse_str(&item).unwrap())
            .fetch_one(&t.pool)
            .await
            .unwrap();
    assert_eq!(jobs, 0);
    let mut changed = input.clone();
    changed["subject"] = json!("Lantern Annex");
    assert_eq!(
        t.call(
            Method::PATCH,
            &path,
            Some(&token),
            Some(changed.clone()),
            &[("if-match", "2")]
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    changed["destination_confirmed"] = json!(true);
    changed["visibility"] = json!("friends");
    assert_eq!(
        t.call(
            Method::PATCH,
            &path,
            Some(&token),
            Some(changed),
            &[("if-match", "2")]
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        t.call(
            Method::DELETE,
            &format!("/v1/captures/{capture}/source"),
            Some(&token),
            None,
            &[("if-match", "1")]
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    let mut after_removal = input.clone();
    after_removal["destination"]["mode"] = json!("none");
    let (status, saved) = t
        .call(
            Method::PATCH,
            &path,
            Some(&token),
            Some(after_removal),
            &[("if-match", "3")],
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(saved["item"]["recommendation"]["destination"]["url"], "");
    t.call(
        Method::DELETE,
        &format!("/v1/items/{item}"),
        Some(&token),
        None,
        &[("if-match", "4")],
    )
    .await;
    assert_eq!(
        t.call(
            Method::PATCH,
            &path,
            Some(&token),
            Some(input),
            &[("if-match", "4")]
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    t.cleanup().await;
}

#[tokio::test]
async fn place_lookup_is_owner_scoped_ephemeral_and_budgeted() {
    let Some(mut t) = TestApp::new().await else {
        return;
    };
    let (owner, token) = t.sign_in("google", "valid-a").await;
    t.call(
        Method::POST,
        "/v1/me/visibility-disclosure",
        Some(&token),
        Some(json!({"accept":true})),
        &[],
    )
    .await;
    let item = categorized_item(&t, &token, "Cedar Cafe", "place", "place.cafe", &[]).await;
    let id = Uuid::parse_str(item["id"].as_str().unwrap()).unwrap();
    sqlx::query("UPDATE knowledge_items SET recommendation=jsonb_set(recommendation,'{locations}',$2) WHERE id=$1")
        .bind(id).bind(json!([{"role":"venue","text":"Pune"}])).execute(&t.pool).await.unwrap();
    let path = format!("/v1/items/{id}/place");
    let (status, result) = t.call(Method::POST, &path, Some(&token), None, &[]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["place"]["address"], "12 Sample Road, Pune");
    let (_, other) = t.sign_in("google", "valid-b").await;
    assert_ne!(
        t.call(Method::POST, &path, Some(&other), None, &[]).await.0,
        StatusCode::OK
    );
    let (_, retry) = t.call(Method::POST, &path, Some(&token), None, &[]).await;
    assert!(retry["place"].is_null());
    let rec: Value = sqlx::query_scalar("SELECT recommendation FROM knowledge_items WHERE id=$1")
        .bind(id)
        .fetch_one(&t.pool)
        .await
        .unwrap();
    assert!(!rec.to_string().contains("12 Sample Road"));
    sqlx::query(
        "UPDATE place_lookup_attempts SET created_at=now()-interval '1 minute' WHERE owner_id=$1",
    )
    .bind(owner)
    .execute(&t.pool)
    .await
    .unwrap();
    for _ in 0..19 {
        sqlx::query("INSERT INTO place_lookup_attempts(id,owner_id,item_id,created_at) VALUES($1,$2,$3,now()-interval '1 minute')").bind(Uuid::new_v4()).bind(owner).bind(id).execute(&t.pool).await.unwrap();
    }
    let (_, capped) = t.call(Method::POST, &path, Some(&token), None, &[]).await;
    assert!(capped["place"].is_null());
    // Explicit no-link overrides stop future lookup even if the provider is available.
    sqlx::query("DELETE FROM place_lookup_attempts WHERE owner_id=$1")
        .bind(owner)
        .execute(&t.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE knowledge_items SET recommendation=jsonb_set(recommendation,'{destination}',$2) WHERE id=$1").bind(id).bind(json!({"mode":"none"})).execute(&t.pool).await.unwrap();
    let (_, disabled) = t.call(Method::POST, &path, Some(&token), None, &[]).await;
    assert!(disabled["place"].is_null());
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM place_lookup_attempts WHERE owner_id=$1")
            .bind(owner)
            .fetch_one(&t.pool)
            .await
            .unwrap();
    assert_eq!(count, 0);
    t.cleanup().await;
}

struct BlockingPlaces {
    started: Notify,
    release: Notify,
}
#[async_trait]
impl rekky_backend::places::PlaceResolver for BlockingPlaces {
    fn available(&self) -> bool {
        true
    }
    async fn resolve(
        &self,
        query: &rekky_backend::places::PlaceQuery,
    ) -> Option<rekky_backend::places::PlaceMatch> {
        self.started.notify_one();
        self.release.notified().await;
        TestPlaces.resolve(query).await
    }
}
#[tokio::test]
async fn place_lookup_rejects_late_results_after_edit_or_logout() {
    for logout in [false, true] {
        let Some(mut t) = TestApp::new().await else {
            return;
        };
        let (_owner, token) = t.sign_in("google", "valid-a").await;
        t.call(
            Method::POST,
            "/v1/me/visibility-disclosure",
            Some(&token),
            Some(json!({"accept":true})),
            &[],
        )
        .await;
        let item = categorized_item(&t, &token, "Cedar Cafe", "place", "place.cafe", &[]).await;
        let id = Uuid::parse_str(item["id"].as_str().unwrap()).unwrap();
        sqlx::query("UPDATE knowledge_items SET recommendation=jsonb_set(recommendation,'{locations}',$2) WHERE id=$1").bind(id).bind(json!([{"role":"venue","text":"Pune"}])).execute(&t.pool).await.unwrap();
        let provider = Arc::new(BlockingPlaces {
            started: Notify::new(),
            release: Notify::new(),
        });
        t.state.places = provider.clone();
        t.app = router(t.state.clone());
        let app = t.app.clone();
        let auth = format!("Bearer {token}");
        let request = tokio::spawn(async move {
            app.oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(format!("/v1/items/{id}/place"))
                    .header("authorization", auth)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
        });
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            provider.started.notified(),
        )
        .await
        .unwrap();
        if logout {
            t.call(Method::DELETE, "/v1/session", Some(&token), None, &[])
                .await;
        } else {
            sqlx::query("UPDATE knowledge_items SET subject='Another branch',revision=revision+1 WHERE id=$1").bind(id).execute(&t.pool).await.unwrap();
        }
        provider.release.notify_one();
        let response = tokio::time::timeout(std::time::Duration::from_secs(5), request)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            response.status(),
            if logout {
                StatusCode::UNAUTHORIZED
            } else {
                StatusCode::CONFLICT
            }
        );
        t.cleanup().await;
    }
}

#[tokio::test]
async fn contact_attachment_is_owner_scoped_revision_fenced_and_follows_item_audience() {
    let Some(mut t) = TestApp::new().await else {
        return;
    };
    let (_, token) = t.sign_in("google", "valid-a").await;
    let (_, other) = t.sign_in("google", "valid-b").await;
    for auth in [&token, &other] {
        t.call(
            Method::POST,
            "/v1/me/visibility-disclosure",
            Some(auth),
            Some(json!({"accept":true})),
            &[],
        )
        .await;
    }
    let item = categorized_item(
        &t,
        &token,
        "Test Doctor",
        "person_service",
        "service.general_doctor",
        &[],
    )
    .await;
    let id = item["id"].as_str().unwrap();
    let path = format!("/v1/items/{id}/contact");
    let automatic = json!({"mode":"automatic","phone":"+91 98765 43210","generation":1});
    assert_eq!(
        t.call(
            Method::PATCH,
            &path,
            Some(&token),
            Some(automatic.clone()),
            &[("if-match", "1")]
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let (status, settings) = t
        .call(
            Method::POST,
            "/v1/me/contact-matching",
            Some(&token),
            Some(json!({"enabled":true})),
            &[],
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(settings["generation"], 1);
    assert_eq!(
        t.call(
            Method::PATCH,
            &path,
            Some(&other),
            Some(json!({"mode":"set","phone":"+919876543210"})),
            &[("if-match", "1")]
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        t.call(
            Method::PATCH,
            &path,
            Some(&token),
            Some(json!({"mode":"set","phone":"9876543210"})),
            &[("if-match", "1")]
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (status, attached) = t
        .call(
            Method::PATCH,
            &path,
            Some(&token),
            Some(automatic.clone()),
            &[("if-match", "1")],
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{attached}");
    assert_eq!(attached["item"]["revision"], 2);
    assert_eq!(
        attached["item"]["recommendation"]["contact"],
        json!({"phone":"+919876543210","origin":"contacts"})
    );
    assert_eq!(attached["item"]["visibility"], "private");
    assert!(!attached["item"]["body"].as_str().unwrap().contains("98765"));
    // Visibility changes carry the same snapshot without another phone-sharing field.
    let (_, shared) = t
        .call(
            Method::PATCH,
            &format!("/v1/items/{id}"),
            Some(&token),
            Some(json!({"visibility":"friends"})),
            &[("if-match", "2")],
        )
        .await;
    assert_eq!(
        shared["item"]["recommendation"]["contact"],
        attached["item"]["recommendation"]["contact"]
    );
    assert_eq!(shared["item"]["visibility"], "friends");
    // Friends metadata is not authorization for an unrelated account.
    let (_, outsider) = t
        .call(Method::GET, "/v1/items", Some(&other), None, &[])
        .await;
    assert!(!outsider.to_string().contains("9876543210"));
    assert_eq!(
        t.call(
            Method::PATCH,
            &path,
            Some(&token),
            Some(json!({"mode":"none"})),
            &[("if-match", "2")]
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let (status, removed) = t
        .call(
            Method::PATCH,
            &path,
            Some(&token),
            Some(json!({"mode":"none"})),
            &[("if-match", "3")],
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(removed["item"]["recommendation"].get("contact").is_none());
    assert_eq!(removed["item"]["recommendation"]["contact_matching"], "off");
    assert_eq!(
        t.call(
            Method::PATCH,
            &path,
            Some(&token),
            Some(automatic.clone()),
            &[("if-match", "4")]
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    // Disabling and re-enabling fences an earlier request's generation.
    t.call(
        Method::POST,
        "/v1/me/contact-matching",
        Some(&token),
        Some(json!({"enabled":false})),
        &[],
    )
    .await;
    t.call(
        Method::POST,
        "/v1/me/contact-matching",
        Some(&token),
        Some(json!({"enabled":true})),
        &[],
    )
    .await;
    let another = categorized_item(
        &t,
        &token,
        "Other Doctor",
        "person_service",
        "service.doctor",
        &[],
    )
    .await;
    let path2 = format!("/v1/items/{}/contact", another["id"].as_str().unwrap());
    assert_eq!(
        t.call(
            Method::PATCH,
            &path2,
            Some(&token),
            Some(automatic),
            &[("if-match", "1")]
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        t.call(
            Method::PATCH,
            &path2,
            Some(&token),
            Some(json!({"mode":"automatic","phone":"+919876543210","generation":3})),
            &[("if-match", "1")]
        )
        .await
        .0,
        StatusCode::OK
    );
    let place = categorized_item(&t, &token, "Test Cafe", "place", "place.cafe", &[]).await;
    assert_eq!(
        t.call(
            Method::PATCH,
            &format!("/v1/items/{}/contact", place["id"].as_str().unwrap()),
            Some(&token),
            Some(json!({"mode":"set","phone":"+919876543210"})),
            &[("if-match", "1")]
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let mut edit = json!({
        "subject":"Other Doctor", "visibility":"friends", "entity_kind":"person_service",
        "summary":"Helpful and practical.", "experience":"firsthand", "attribution":"",
        "observations":[], "locations":[], "use_cases":[], "types":["service.doctor"],
        "facets":[], "descriptors":[], "destination":{"mode":"none","url":"","label":""}
    });
    let content_path = format!("/v1/items/{}/content", another["id"].as_str().unwrap());
    // Omitted contact action from older clients preserves the snapshot.
    let (status, edited) = t
        .call(
            Method::PATCH,
            &content_path,
            Some(&token),
            Some(edit.clone()),
            &[("if-match", "2")],
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{edited}");
    assert_eq!(
        edited["item"]["recommendation"]["contact"]["phone"],
        "+919876543210"
    );
    edit["subject"] = json!("Different Doctor");
    let (status, renamed) = t
        .call(
            Method::PATCH,
            &content_path,
            Some(&token),
            Some(edit.clone()),
            &[("if-match", "3")],
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{renamed}");
    assert!(renamed["item"]["recommendation"].get("contact").is_none());
    assert_eq!(renamed["item"]["recommendation"]["contact_matching"], "off");
    // A full edit can deliberately attach a replacement without global matching.
    t.call(
        Method::POST,
        "/v1/me/contact-matching",
        Some(&token),
        Some(json!({"enabled":false})),
        &[],
    )
    .await;
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../contracts/rekky/v1/fixtures/contact.json"
    ))
    .unwrap();
    edit["contact"] = fixture["manual_attachment"].clone();
    let (status, replaced) = t
        .call(
            Method::PATCH,
            &content_path,
            Some(&token),
            Some(edit),
            &[("if-match", "4")],
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{replaced}");
    assert_eq!(
        replaced["item"]["recommendation"]["contact"]["phone"],
        fixture["saved_contact"]["phone"]
    );
    t.cleanup().await;
}

#[tokio::test]
async fn contact_label_backfill_cannot_replace_number_or_override_owner_label() {
    let Some(mut t) = TestApp::new().await else {
        return;
    };
    let (_, token) = t.sign_in("google", "valid-a").await;
    t.call(
        Method::POST,
        "/v1/me/visibility-disclosure",
        Some(&token),
        Some(json!({"accept":true})),
        &[],
    )
    .await;
    t.call(
        Method::POST,
        "/v1/me/contact-matching",
        Some(&token),
        Some(json!({"enabled":true})),
        &[],
    )
    .await;
    let item = categorized_item(
        &t,
        &token,
        "Test Doctor",
        "person_service",
        "service.doctor",
        &[],
    )
    .await;
    let path = format!("/v1/items/{}/contact", item["id"].as_str().unwrap());
    let (_, attached) = t
        .call(
            Method::PATCH,
            &path,
            Some(&token),
            Some(json!({"mode":"set","phone":"+12025550123"})),
            &[("if-match", "1")],
        )
        .await;
    let label = json!({"mode":"describe","phone":"+12025550123","saved_name":"Maya Rao Clinic","generation":1});
    let mut blank = label.clone();
    blank["saved_name"] = json!("");
    assert_eq!(
        t.call(
            Method::PATCH,
            &path,
            Some(&token),
            Some(blank),
            &[("if-match", "2")]
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let mut wrong = label.clone();
    wrong["phone"] = json!("+12025550124");
    assert_eq!(
        t.call(
            Method::PATCH,
            &path,
            Some(&token),
            Some(wrong),
            &[("if-match", "2")]
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let mut invalid = label.clone();
    invalid["saved_name"] = json!("a".repeat(201));
    assert_eq!(
        t.call(
            Method::PATCH,
            &path,
            Some(&token),
            Some(invalid),
            &[("if-match", "2")]
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (status, named) = t
        .call(
            Method::PATCH,
            &path,
            Some(&token),
            Some(label.clone()),
            &[("if-match", "2")],
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{named}");
    assert_eq!(
        named["item"]["recommendation"]["contact"],
        json!({"phone":"+12025550123","origin":"user","saved_name":"Maya Rao Clinic"})
    );
    assert_eq!(named["item"]["recommendation"]["contact_matching"], "off");
    assert_eq!(named["item"]["visibility"], attached["item"]["visibility"]);
    assert_eq!(
        t.call(
            Method::PATCH,
            &path,
            Some(&token),
            Some(label),
            &[("if-match", "3")]
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    t.cleanup().await;
}

struct IncompleteExtractor {
    fallback: bool,
}
#[async_trait]
impl TranscriptExtractor for IncompleteExtractor {
    fn available(&self) -> bool {
        true
    }
    async fn extract(&self, transcript: &str) -> Result<Proposal, ExtractionError> {
        let mut p = TestExtractor.extract(transcript).await?;
        if self.fallback {
            p.items[0].summary.text = "Invented cost of 99999 rupees.".into();
        } else {
            p.unresolved_unit_ids = vec![1];
        }
        Ok(p)
    }
}
#[tokio::test]
async fn voice_default_preserves_legacy_work_and_keeps_partial_and_fallback_private() {
    for scenario in 0..3 {
        let Some(mut t) = TestApp::new().await else {
            return;
        };
        if scenario > 0 {
            t.state.extractor = Arc::new(IncompleteExtractor {
                fallback: scenario == 2,
            });
            t.app = router(t.state.clone());
        }
        let (owner, token) = t.sign_in("google", "valid-a").await;
        t.call(
            Method::POST,
            "/v1/me/visibility-disclosure",
            Some(&token),
            Some(json!({"accept":true})),
            &[],
        )
        .await;
        for permission in ["voice-transcription", "transcript-extraction"] {
            t.call(
                Method::POST,
                &format!("/v1/me/{permission}-permission"),
                Some(&token),
                Some(json!({"enabled":true,"disclosure_version":1})),
                &[],
            )
            .await;
        }
        let mut audio = vec![0u8; 256];
        audio[4..8].copy_from_slice(b"ftyp");
        let path = "/v1/remember/privacy-regression-001";
        assert_eq!(
            t.call_audio(path, &token, audio, chrono::Utc::now().timestamp_millis())
                .await
                .0,
            StatusCode::ACCEPTED
        );
        if scenario == 0 {
            // Simulate an upload accepted under the previous Private promise.
            sqlx::query(
                "UPDATE voice_uploads SET desired_visibility='private' WHERE account_id=$1",
            )
            .bind(owner)
            .execute(&t.pool)
            .await
            .unwrap();
        }
        rekky_backend::app::process_pending_voice(&t.state, Some(owner))
            .await
            .unwrap();
        let (audience, desired, status):(String,String,String) = sqlx::query_as("SELECT k.visibility,c.desired_visibility,c.status FROM knowledge_items k JOIN captures c ON c.id=k.capture_id WHERE k.owner_id=$1").bind(owner).fetch_one(&t.pool).await.unwrap();
        assert_eq!(audience, "private");
        assert_eq!(desired, if scenario == 0 { "private" } else { "friends" });
        assert_eq!(
            status,
            if scenario == 0 {
                "completed"
            } else {
                "partial"
            }
        );
        t.cleanup().await;
    }
}
#[tokio::test]
async fn sharing_migration_preserves_existing_work_and_changes_only_new_defaults() {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        return;
    };
    let pool = PgPool::connect(&url).await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(732785)")
        .execute(&mut *tx)
        .await
        .unwrap();
    // Fixed test-only namespace, serialized across runs and removed by rollback.
    sqlx::raw_sql("CREATE SCHEMA sharing_migration_test; SET LOCAL search_path TO sharing_migration_test; CREATE TABLE voice_uploads(id int); CREATE TABLE voice_transcription_jobs(id int); INSERT INTO voice_uploads VALUES(1); INSERT INTO voice_transcription_jobs VALUES(1);").execute(&mut *tx).await.unwrap();
    sqlx::raw_sql(include_str!("../migrations/015_voice_sharing_default.sql"))
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::raw_sql("INSERT INTO voice_uploads(id) VALUES(2); INSERT INTO voice_transcription_jobs(id) VALUES(2);").execute(&mut *tx).await.unwrap();
    let actual: Vec<String> =
        sqlx::query_scalar("SELECT desired_visibility FROM voice_uploads ORDER BY id")
            .fetch_all(&mut *tx)
            .await
            .unwrap();
    assert_eq!(actual, ["private", "friends"]);
    let actual: Vec<String> =
        sqlx::query_scalar("SELECT desired_visibility FROM voice_transcription_jobs ORDER BY id")
            .fetch_all(&mut *tx)
            .await
            .unwrap();
    assert_eq!(actual, ["private", "friends"]);
    tx.rollback().await.unwrap();
}

struct UnfamiliarTypeExtractor;
#[async_trait]
impl TranscriptExtractor for UnfamiliarTypeExtractor {
    fn available(&self) -> bool {
        true
    }
    async fn extract(&self, transcript: &str) -> Result<Proposal, ExtractionError> {
        let mut p = TestExtractor.extract(transcript).await?;
        p.items[0].classification = json!({"types":[{"concept_id":"thing.product","source_phrase":"cafe","evidence":[1]}],"facets":[],"descriptors":[],"type_description":{"text":"caterer","evidence":[1]}});
        Ok(p)
    }
}
#[tokio::test]
async fn unfamiliar_type_survives_rejected_assignment_and_reaches_learning_queue() {
    let Some(mut t) = TestApp::new().await else {
        return;
    };
    t.state.extractor = Arc::new(UnfamiliarTypeExtractor);
    t.app = router(t.state.clone());
    let (owner, token) = t.sign_in("google", "valid-a").await;
    t.call(
        Method::POST,
        "/v1/me/visibility-disclosure",
        Some(&token),
        Some(json!({"accept":true})),
        &[],
    )
    .await;
    t.call(
        Method::POST,
        "/v1/me/transcript-extraction-permission",
        Some(&token),
        Some(json!({"enabled":true,"disclosure_version":1})),
        &[],
    )
    .await;
    let capture = Uuid::new_v4();
    sqlx::query("INSERT INTO captures(id,owner_id,kind,status,desired_visibility) VALUES($1,$2,'voice','transcript_ready','friends')").bind(capture).bind(owner).execute(&t.pool).await.unwrap();
    sqlx::query("INSERT INTO source_texts(id,capture_id,owner_id,kind,content) VALUES($1,$2,$3,'transcript','Nila is a caterer. She prepared a tasty lunch.')").bind(Uuid::new_v4()).bind(capture).bind(owner).execute(&t.pool).await.unwrap();
    let (status, result) = t
        .call(
            Method::POST,
            &format!("/v1/voice-captures/{capture}/extract"),
            Some(&token),
            None,
            &[],
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(
        result["items"][0]["recommendation"]["classification"]["display_label"],
        "caterer"
    );
    assert_eq!(result["items"][0]["visibility"], "friends");
    let key: String =
        sqlx::query_scalar("SELECT job_key FROM category_discoveries WHERE owner_id=$1")
            .bind(owner)
            .fetch_one(&t.pool)
            .await
            .unwrap();
    assert_eq!(
        key,
        rekky_backend::category_learning::job_key("person_service", "caterer")
    );
    t.cleanup().await;
}

#[tokio::test]
async fn remember_reports_quota_wait_and_resumes_without_retranscription() {
    for raise_limit in [false, true] {
        let Some(mut t) = TestApp::new().await else {
            return;
        };
        t.state.daily_account_limit = 2;
        t.app = router(t.state.clone());
        let (owner, token) = t.sign_in("google", "valid-a").await;
        t.call(
            Method::POST,
            "/v1/me/visibility-disclosure",
            Some(&token),
            Some(json!({"accept":true})),
            &[],
        )
        .await;
        for permission in ["voice-transcription", "transcript-extraction"] {
            t.call(
                Method::POST,
                &format!("/v1/me/{permission}-permission"),
                Some(&token),
                Some(json!({"enabled":true,"disclosure_version":1})),
                &[],
            )
            .await;
        }
        let mut audio = vec![0u8; 256];
        audio[4..8].copy_from_slice(b"ftyp");
        let now = chrono::Utc::now().timestamp_millis();
        let first = "/v1/remember/quota-first-recording";
        let second = "/v1/remember/quota-second-recording";
        assert_eq!(
            t.call_audio(first, &token, audio.clone(), now).await.0,
            StatusCode::ACCEPTED
        );
        rekky_backend::app::process_pending_voice(&t.state, Some(owner))
            .await
            .unwrap();
        sqlx::query("UPDATE transcript_extraction_jobs SET created_at=now()-interval '2 hours' WHERE account_id=$1").bind(owner).execute(&t.pool).await.unwrap();
        sqlx::query("INSERT INTO recommendation_refinement_jobs(item_id,account_id,attempt_id,status,permission_generation,source_revision,item_revision,lease_until,created_at) SELECT id,owner_id,$2,'completed',1,1,revision,now(),now()-interval '1 hour' FROM knowledge_items WHERE owner_id=$1")
            .bind(owner).bind(Uuid::new_v4()).execute(&t.pool).await.unwrap();
        assert_eq!(
            t.call_audio(second, &token, audio, now).await.0,
            StatusCode::ACCEPTED
        );
        rekky_backend::app::process_pending_voice(&t.state, Some(owner))
            .await
            .unwrap();
        let (_, receipt) = t.call(Method::GET, second, Some(&token), None, &[]).await;
        assert_eq!(receipt["remember"]["status"], "transcribed");
        assert_eq!(receipt["remember"]["transcript_saved"], true);
        assert_eq!(receipt["remember"]["waiting_reason"], "processing_limit");
        let retry =
            chrono::DateTime::parse_from_rfc3339(receipt["remember"]["retry_at"].as_str().unwrap())
                .unwrap();
        assert!((retry.with_timezone(&chrono::Utc) - chrono::Utc::now()).num_minutes() >= 1319);
        assert!((retry.with_timezone(&chrono::Utc) - chrono::Utc::now()).num_minutes() <= 1321);
        let capture = Uuid::parse_str(receipt["remember"]["capture_id"].as_str().unwrap()).unwrap();
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM transcript_extraction_jobs WHERE capture_id=$1",
        )
        .bind(capture)
        .fetch_one(&t.pool)
        .await
        .unwrap();
        assert_eq!(count, 0); // Waiting must not consume an attempt or call extraction.
        let audio_gone: bool =
            sqlx::query_scalar("SELECT audio IS NULL FROM voice_uploads WHERE capture_id=$1")
                .bind(capture)
                .fetch_one(&t.pool)
                .await
                .unwrap();
        assert!(audio_gone);
        // Lowering a limit must wait for the limit-th newest job, not the oldest.
        t.state.daily_account_limit = 1;
        t.app = router(t.state.clone());
        let (_, receipt) = t.call(Method::GET, second, Some(&token), None, &[]).await;
        let retry =
            chrono::DateTime::parse_from_rfc3339(receipt["remember"]["retry_at"].as_str().unwrap())
                .unwrap();
        assert!((retry.with_timezone(&chrono::Utc) - chrono::Utc::now()).num_minutes() >= 1379);
        t.state.daily_account_limit = if raise_limit { 3 } else { 2 };
        t.app = router(t.state.clone());
        if !raise_limit {
            sqlx::query("UPDATE transcript_extraction_jobs SET created_at=now()-interval '25 hours' WHERE account_id=$1").bind(owner).execute(&t.pool).await.unwrap();
        }
        sqlx::query("UPDATE captures SET auto_next_attempt_at=now() WHERE id=$1")
            .bind(capture)
            .execute(&t.pool)
            .await
            .unwrap();
        rekky_backend::app::process_pending_voice(&t.state, Some(owner))
            .await
            .unwrap();
        let (_, receipt) = t.call(Method::GET, second, Some(&token), None, &[]).await;
        assert_eq!(receipt["remember"]["capture_status"], "completed");
        assert!(receipt["remember"]["waiting_reason"].is_null());
        assert!(receipt["remember"]["retry_at"].is_null());
        let attempts:i32 = sqlx::query_scalar("SELECT attempts FROM voice_transcription_jobs WHERE account_id=$1 AND draft_id='quota-second-recording'").bind(owner).fetch_one(&t.pool).await.unwrap();
        assert_eq!(attempts, 1);
        t.cleanup().await;
    }
}

#[tokio::test]
async fn geography_enrichment_and_filters_preserve_roles_privacy_and_edit_fences() {
    let Some(mut t) = TestApp::new().await else {
        return;
    };
    let (owner, token) = t.sign_in("google", "valid-a").await;
    let (_, other) = t.sign_in("google", "valid-b").await;
    for who in [&token, &other] {
        t.call(
            Method::POST,
            "/v1/me/visibility-disclosure",
            Some(who),
            Some(json!({"accept":true})),
            &[],
        )
        .await;
    }
    // Dedicated synthetic geographic IDs; no public geocoder or private note is used.
    sqlx::query("INSERT INTO geographic_areas(id,name,label,country,feature,population,aliases,ancestors,hierarchy) VALUES('geonames:990000001','Testburg','Testburg','ZZ','PPL',0,ARRAY['testburg'],ARRAY['geonames:990000002'],'[]') ON CONFLICT DO NOTHING").execute(&t.pool).await.unwrap();
    sqlx::query("UPDATE geographic_catalog SET revision=1 WHERE singleton")
        .execute(&t.pool)
        .await
        .unwrap();
    let capture = Uuid::new_v4();
    sqlx::query("INSERT INTO captures(id,owner_id,kind,status,desired_visibility) VALUES($1,$2,'typed','completed','private')")
        .bind(capture)
        .bind(owner)
        .execute(&t.pool)
        .await
        .unwrap();
    let mut ids = vec![];
    for role in ["practice", "service_area", "past_experience", "context"] {
        let id = Uuid::new_v4();
        ids.push(id);
        let r = json!({"version":2,"entity_kind":"person_service","locations":[{"role":role,"text":"based right here in Testburg"}],"rating":{"value":8},"contact":{"phone":"+12025550123"}});
        sqlx::query("INSERT INTO knowledge_items(id,capture_id,owner_id,subject,body,visibility,recommendation) VALUES($1,$2,$3,'Test helper','Original body','private',$4)").bind(id).bind(capture).bind(owner).bind(r).execute(&t.pool).await.unwrap();
        assert!(
            rekky_backend::geography::process_one(&t.pool, Some(owner))
                .await
                .unwrap()
        );
    }
    assert!(
        !rekky_backend::geography::process_one(&t.pool, Some(owner))
            .await
            .unwrap()
    );
    // Catalog migration can run while an old worker is alive. Its projection
    // may carry the latest catalog revision yet still need the new rules.
    sqlx::query("UPDATE knowledge_items SET recommendation=recommendation - 'geography_rules_version' WHERE id=$1")
        .bind(ids[0]).execute(&t.pool).await.unwrap();
    assert!(
        rekky_backend::geography::process_one(&t.pool, Some(owner))
            .await
            .unwrap()
    );
    let rules_version: Value = sqlx::query_scalar(
        "SELECT recommendation->'geography_rules_version' FROM knowledge_items WHERE id=$1",
    )
    .bind(ids[0])
    .fetch_one(&t.pool)
    .await
    .unwrap();
    assert_eq!(rules_version, 2);
    assert!(
        !rekky_backend::geography::process_one(&t.pool, Some(owner))
            .await
            .unwrap()
    );
    // An older worker may have written the current catalog revision without
    // the new projection. Repair the missing derived format once, not forever.
    sqlx::query("UPDATE knowledge_items SET recommendation=recommendation #- '{locations,0,geography,browse}' WHERE id=$1").bind(ids[0]).execute(&t.pool).await.unwrap();
    assert!(
        rekky_backend::geography::process_one(&t.pool, Some(owner))
            .await
            .unwrap()
    );
    assert!(
        !rekky_backend::geography::process_one(&t.pool, Some(owner))
            .await
            .unwrap()
    );
    let path = "/v1/items?area_id=geonames:990000001";
    let (_, body) = t.call(Method::GET, path, Some(&token), None, &[]).await;
    assert_eq!(body["items"].as_array().unwrap().len(), 2); // No past trip/context as coverage.
    for item in body["items"].as_array().unwrap() {
        assert_eq!(item["body"], "Original body");
        assert_eq!(item["visibility"], "private");
        assert_eq!(item["recommendation"]["rating"]["value"], 8);
        assert_eq!(item["recommendation"]["contact"]["phone"], "+12025550123");
        assert_eq!(item["recommendation"]["locations"][0]["name"], "Testburg");
        assert_eq!(
            item["recommendation"]["locations"][0]["geography"]["browse"]["destination"]["id"],
            "geonames:990000001"
        );
        assert_eq!(
            item["recommendation"]["locations"][0]["text"],
            "based right here in Testburg"
        );
    }
    assert_eq!(
        t.call(Method::GET, path, Some(&other), None, &[]).await.1["items"],
        json!([])
    );
    assert_eq!(
        t.call(
            Method::GET,
            "/v1/items?area_id=geonames:990000002&location_role=service_area",
            Some(&token),
            None,
            &[]
        )
        .await
        .1["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        t.call(
            Method::GET,
            "/v1/items?area_id=geonames:990000001&location_role=past_experience",
            Some(&token),
            None,
            &[]
        )
        .await
        .1["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    // An owner edit invalidates the old index immediately, before background work.
    sqlx::query("UPDATE knowledge_items SET recommendation=jsonb_set(recommendation - 'geography_revision','{locations}',$2),revision=revision+1 WHERE id=$1").bind(ids[0]).bind(json!([{"role":"practice","text":"Unknown village"}])).execute(&t.pool).await.unwrap();
    assert_eq!(
        t.call(Method::GET, path, Some(&token), None, &[]).await.1["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    rekky_backend::geography::process_one(&t.pool, Some(owner))
        .await
        .unwrap();
    assert_eq!(
        t.call(Method::GET, path, Some(&token), None, &[]).await.1["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    // Soft-deleted items cannot appear through their residual derived index.
    sqlx::query("UPDATE knowledge_items SET deleted_at=now() WHERE id=$1")
        .bind(ids[1])
        .execute(&t.pool)
        .await
        .unwrap();
    assert_eq!(
        t.call(Method::GET, path, Some(&token), None, &[]).await.1["items"],
        json!([])
    );
    t.cleanup().await;
    sqlx::query("DELETE FROM geographic_areas WHERE id='geonames:990000001'")
        .execute(&t.pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn library_pins_are_owner_only_revisioned_and_do_not_edit_recommendations() {
    let Some(mut t) = TestApp::new().await else {
        return;
    };
    let (_, a) = t.sign_in("google", "valid-a").await;
    let (_, b) = t.sign_in("google", "valid-b").await;
    for token in [&a, &b] {
        t.call(
            Method::POST,
            "/v1/me/visibility-disclosure",
            Some(token),
            Some(json!({"accept":true})),
            &[],
        )
        .await;
    }
    let (_, saved) = t.call(Method::POST, "/v1/items", Some(&a),
        Some(json!({"subject":"Synthetic Library pin","body":"A useful saved note.","visibility":"friends"})),
        &[("idempotency-key", &Uuid::new_v4().to_string())]).await;
    let id = saved["item"]["id"].as_str().unwrap();
    let path = format!("/v1/items/{id}/pin");
    assert_eq!(saved["item"]["pinned"], false);
    assert_eq!(saved["item"]["pin_revision"], 1);
    assert_eq!(
        t.call(
            Method::PATCH,
            &path,
            None,
            Some(json!({"pinned":true})),
            &[("if-match", "1")]
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        t.call(
            Method::PATCH,
            &path,
            Some(&b),
            Some(json!({"pinned":true})),
            &[("if-match", "1")]
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let (status, pin) = t
        .call(
            Method::PATCH,
            &path,
            Some(&a),
            Some(json!({"pinned":true})),
            &[("if-match", "1")],
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(pin["pin"]["pinned"], true);
    assert_eq!(pin["pin"]["revision"], 2);
    let mut fixture: Value = serde_json::from_str(include_str!(
        "../../../contracts/rekky/v1/fixtures/library_pin.json"
    ))
    .unwrap();
    fixture["response"]["pin"]["item_id"] = json!(id);
    assert_eq!(pin, fixture["response"]);
    assert_eq!(
        t.call(
            Method::PATCH,
            &path,
            Some(&a),
            Some(json!({"pinned":false})),
            &[("if-match", "1")]
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let (_, list) = t.call(Method::GET, "/v1/items", Some(&a), None, &[]).await;
    let item = &list["items"][0];
    assert_eq!(item["pinned"], true);
    for field in [
        "revision",
        "body",
        "subject",
        "visibility",
        "recommendation",
    ] {
        assert_eq!(item[field], saved["item"][field]);
    }
    // A content/audience write must preserve the preference and its revision.
    let (status, edited) = t
        .call(
            Method::PATCH,
            &format!("/v1/items/{id}"),
            Some(&a),
            Some(json!({"visibility":"private"})),
            &[("if-match", "1")],
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(edited["item"]["pinned"], true);
    assert_eq!(edited["item"]["pin_revision"], 2);
    assert_eq!(
        t.call(
            Method::PATCH,
            &path,
            Some(&a),
            Some(json!({"pinned":false})),
            &[("if-match", "2")]
        )
        .await
        .0,
        StatusCode::OK
    );
    t.call(
        Method::DELETE,
        &format!("/v1/items/{id}"),
        Some(&a),
        None,
        &[("if-match", "2")],
    )
    .await;
    assert_eq!(
        t.call(
            Method::PATCH,
            &path,
            Some(&a),
            Some(json!({"pinned":true})),
            &[("if-match", "3")]
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    t.cleanup().await;
}

struct RepairingExtractor {
    success: bool,
    calls: std::sync::atomic::AtomicUsize,
    started: Option<Arc<Notify>>,
    release: Option<Arc<Notify>>,
}
#[async_trait]
impl TranscriptExtractor for RepairingExtractor {
    fn available(&self) -> bool {
        true
    }
    fn supports_repair(&self) -> bool {
        true
    }
    async fn extract(&self, _source: &str) -> Result<Proposal, ExtractionError> {
        Ok(serde_json::from_value(json!({"items":[
            {"subject":"Lantern Cafe","subject_evidence":[1],"entity_kind":"place","experience":"firsthand","account":[{"kind":"price","text":"Pizza cost 900 rupees.","evidence":[1]}],"locations":[],"use_cases":[]},
            {"subject":"Willow Cafe","subject_evidence":[2],"entity_kind":"place","experience":"firsthand","account":[{"kind":"praise","text":"Lovely noodles.","evidence":[2]}],"locations":[],"use_cases":[]}
        ],"ignored_unit_ids":[],"unresolved_unit_ids":[]})).unwrap())
    }
    async fn repair(
        &self,
        source: &str,
        _catalog: &rekky_backend::taxonomy::Vocabulary,
        _proposal: &Proposal,
        _issues: &[rekky_backend::understanding::Issue],
    ) -> Result<Proposal, ExtractionError> {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if let Some(started) = &self.started {
            started.notify_one();
        }
        if let Some(release) = &self.release {
            release.notified().await;
        }
        let mut proposal = self.extract(source).await?;
        if self.success {
            proposal.items[0].account[0].text = "Pizza cost 200 rupees.".into();
        }
        Ok(proposal)
    }
}
async fn assessment_capture(t: &mut TestApp) -> (Uuid, String, Uuid, Uuid) {
    let (owner, token) = t.sign_in("google", "valid-a").await;
    t.call(
        Method::POST,
        "/v1/me/visibility-disclosure",
        Some(&token),
        Some(json!({"accept":true})),
        &[],
    )
    .await;
    t.call(
        Method::POST,
        "/v1/me/transcript-extraction-permission",
        Some(&token),
        Some(json!({"enabled":true,"disclosure_version":1})),
        &[],
    )
    .await;
    let capture = Uuid::new_v4();
    let source = Uuid::new_v4();
    sqlx::query("INSERT INTO captures(id,owner_id,kind,status,desired_visibility) VALUES($1,$2,'voice','transcript_ready','friends')").bind(capture).bind(owner).execute(&t.pool).await.unwrap();
    sqlx::query("INSERT INTO source_texts(id,capture_id,owner_id,kind,content) VALUES($1,$2,$3,'transcript','Lantern Cafe had pizza for 200 rupees. Willow Cafe had lovely noodles.')").bind(source).bind(capture).bind(owner).execute(&t.pool).await.unwrap();
    (owner, token, capture, source)
}
#[tokio::test]
async fn assessment_repair_is_once_audited_and_sibling_visibility_is_independent() {
    for success in [true, false] {
        let Some(mut t) = TestApp::new().await else {
            return;
        };
        let extractor = Arc::new(RepairingExtractor {
            success,
            calls: 0.into(),
            started: None,
            release: None,
        });
        t.state.extractor = extractor.clone();
        t.app = router(t.state.clone());
        let (_owner, token, capture, source) = assessment_capture(&mut t).await;
        let path = format!("/v1/voice-captures/{capture}/extract");
        let (status, result) = t.call(Method::POST, &path, Some(&token), None, &[]).await;
        assert_eq!(status, StatusCode::OK, "{result}");
        assert_eq!(result["items"].as_array().unwrap().len(), 2);
        let first = result["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["subject"] == "Lantern Cafe")
            .unwrap();
        let second = result["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["subject"] == "Willow Cafe")
            .unwrap();
        assert_eq!(
            first["visibility"],
            if success { "friends" } else { "private" }
        );
        assert_eq!(first["needs_review"], !success);
        assert_eq!(second["visibility"], "friends");
        assert_eq!(second["needs_review"], false);
        assert!(!first["body"].as_str().unwrap().contains("900"));
        assert!(!first["body"].as_str().unwrap().contains("Willow"));
        let attempts: i32 = sqlx::query_scalar(
            "SELECT attempts FROM transcript_extraction_jobs WHERE capture_id=$1",
        )
        .bind(capture)
        .fetch_one(&t.pool)
        .await
        .unwrap();
        assert_eq!(attempts, 2);
        let audited:i64=sqlx::query_scalar("SELECT count(*) FROM understanding_attempts WHERE source_id=$1 AND candidate IS NOT NULL").bind(source).fetch_one(&t.pool).await.unwrap();
        assert_eq!(audited, 2);
        assert!(!result.to_string().contains("candidate"));
        assert!(!result.to_string().contains("subject_evidence"));
        t.call(Method::POST, &path, Some(&token), None, &[]).await;
        assert_eq!(extractor.calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        sqlx::query("DELETE FROM source_texts WHERE id=$1")
            .bind(source)
            .execute(&t.pool)
            .await
            .unwrap();
        let audited: i64 =
            sqlx::query_scalar("SELECT count(*) FROM understanding_attempts WHERE source_id=$1")
                .bind(source)
                .fetch_one(&t.pool)
                .await
                .unwrap();
        assert_eq!(audited, 0);
        t.cleanup().await;
    }
}
#[tokio::test]
async fn withdrawal_during_repair_cannot_commit_late_recommendations() {
    let Some(mut t) = TestApp::new().await else {
        return;
    };
    let started = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    t.state.extractor = Arc::new(RepairingExtractor {
        success: true,
        calls: 0.into(),
        started: Some(started.clone()),
        release: Some(release.clone()),
    });
    t.app = router(t.state.clone());
    let (owner, token, capture, _source) = assessment_capture(&mut t).await;
    let app = t.app.clone();
    let auth = token.clone();
    let task = tokio::spawn(async move {
        app.oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/v1/voice-captures/{capture}/extract"))
                .header("authorization", format!("Bearer {auth}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
    });
    started.notified().await;
    t.call(
        Method::POST,
        "/v1/me/transcript-extraction-permission",
        Some(&token),
        Some(json!({"enabled":false})),
        &[],
    )
    .await;
    release.notify_one();
    assert_eq!(task.await.unwrap().status(), StatusCode::CONFLICT);
    let saved: i64 = sqlx::query_scalar("SELECT count(*) FROM knowledge_items WHERE owner_id=$1")
        .bind(owner)
        .fetch_one(&t.pool)
        .await
        .unwrap();
    assert_eq!(saved, 0);
    t.cleanup().await;
}

struct ScriptedAsk {
    decisions: std::sync::atomic::AtomicUsize,
}

#[tokio::test]
async fn ask_views_are_complete_owner_scoped_and_revision_safe() {
    let Some(mut t) = TestApp::new().await else {
        return;
    };
    let (owner, token) = t.sign_in("google", "valid-a").await;
    t.call(
        Method::POST,
        "/v1/me/visibility-disclosure",
        Some(&token),
        Some(json!({"accept":true})),
        &[],
    )
    .await;
    let mut expected = Vec::new();
    for index in 0..23 {
        let item = categorized_item(
            &t,
            &token,
            &format!("Bar {index:02}"),
            "place",
            "place.bar",
            &[],
        )
        .await;
        expected.push(item["id"].as_str().unwrap().to_owned());
    }
    categorized_item(&t, &token, "A restaurant", "place", "place.restaurant", &[]).await;
    let (_, other) = t.sign_in("google", "valid-b").await;
    t.call(
        Method::POST,
        "/v1/me/visibility-disclosure",
        Some(&other),
        Some(json!({"accept":true})),
        &[],
    )
    .await;
    categorized_item(&t, &other, "Someone else's bar", "place", "place.bar", &[]).await;

    let (status, first) = t
        .call(
            Method::POST,
            "/v1/ask-ui/views",
            Some(&token),
            Some(json!({"kind":"place","category_ids":["place.bar"]})),
            &[],
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(first["total"], 23);
    assert_eq!(first["items"].as_array().unwrap().len(), 20);
    assert_eq!(first["next_offset"], 20);
    let id = first["view_id"].as_str().unwrap();
    let (status, second) = t
        .call(
            Method::GET,
            &format!("/v1/ask-ui/views/{id}?offset=20"),
            Some(&token),
            None,
            &[],
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{second}");
    assert_eq!(second["items"].as_array().unwrap().len(), 3);
    assert!(second["next_offset"].is_null());
    let all: std::collections::HashSet<_> = first["items"]
        .as_array()
        .unwrap()
        .iter()
        .chain(second["items"].as_array().unwrap())
        .map(|v| v["id"].as_str().unwrap())
        .collect();
    assert_eq!(all.len(), 23);
    assert!(expected.iter().all(|id| all.contains(id.as_str())));
    assert!(!all.contains("Someone else's bar"));
    assert_eq!(
        t.call(
            Method::GET,
            &format!("/v1/ask-ui/views/{id}"),
            Some(&other),
            None,
            &[]
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );

    sqlx::query("UPDATE knowledge_items SET subject='Edited bar',revision=revision+1 WHERE id=$1 AND owner_id=$2")
        .bind(Uuid::parse_str(&expected[0]).unwrap()).bind(owner).execute(&t.pool).await.unwrap();
    let (status, stale) = t
        .call(
            Method::GET,
            &format!("/v1/ask-ui/views/{id}"),
            Some(&token),
            None,
            &[],
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{stale}");
    assert_eq!(stale["error"]["code"], "view_stale");
    // A native area facet sends its stable ID back to the view API. It must
    // resolve by identity, not be treated as a human place name.
    let bar_id = Uuid::parse_str(&expected[0]).unwrap();
    sqlx::query("UPDATE knowledge_items SET recommendation=jsonb_set(recommendation,'{locations}',$3::jsonb,true),revision=revision+1 WHERE id=$1 AND owner_id=$2")
        .bind(bar_id).bind(owner).bind(json!([{"role":"venue","text":"Delhi","geography":{
            "status":"resolved","area_id":"geonames:1273294","filter_ids":["geonames:1273294"],
            "browse":{"destination":{"id":"geonames:1273294","label":"Delhi"}}
        }}])).execute(&t.pool).await.unwrap();
    let (status, delhi) = t
        .call(
            Method::POST,
            "/v1/ask-ui/views",
            Some(&token),
            Some(json!({"location":"geonames:1273294","kind":"place"})),
            &[],
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{delhi}");
    assert_eq!(delhi["title"], "Delhi");
    assert_eq!(delhi["total"], 1);
    assert_eq!(delhi["items"][0]["id"], expected[0]);
    let coarse = rekky_backend::ask_views::resolve_scope(&t.state, "Malviya Nagar, Delhi")
        .await
        .unwrap();
    assert_eq!(coarse["status"], "coarse");
    t.cleanup().await;
}

struct CollectionAsk;
#[async_trait]
impl rekky_backend::ask::AskModel for CollectionAsk {
    fn available(&self) -> bool {
        true
    }
    async fn decide(
        &self,
        context: Value,
        _last: bool,
    ) -> Result<rekky_backend::ask::Decision, rekky_backend::ask::AgentError> {
        let observations = context["tool_observations"].as_array().unwrap();
        let (name, arguments) = if observations.is_empty() {
            (
                "open_collection",
                json!({"title":"Bars","location":"","category_ids":["place.bar"],"kind":"place","query":"","source":"mine","location_role":"relevant","sort":"saved_newest"}),
            )
        } else {
            let id = &observations[0]["result"]["view_id"];
            (
                "present_answer",
                json!({"intent":"discovery","new_topic":false,"title":"Bars","reply":"Here are the saved bars.","view_ids":[id],"location":"","clarification":"","choices":[],"comparison":null,"results":[]}),
            )
        };
        Ok(rekky_backend::ask::Decision {
            output_items: vec![],
            calls: vec![rekky_backend::ask::ToolCall {
                call_id: String::new(),
                name: name.into(),
                arguments,
            }],
            input_tokens: 100,
            output_tokens: 100,
        })
    }
}

#[tokio::test]
async fn ask_agent_can_present_a_complete_native_collection() {
    let Some(mut t) = TestApp::new().await else {
        return;
    };
    t.state.ask_model = Arc::new(CollectionAsk);
    t.app = router(t.state.clone());
    let (owner, token) = t.sign_in("google", "valid-a").await;
    t.call(
        Method::POST,
        "/v1/me/visibility-disclosure",
        Some(&token),
        Some(json!({"accept":true})),
        &[],
    )
    .await;
    sqlx::query("INSERT INTO transcript_extraction_permissions(account_id,enabled,generation,disclosure_version) VALUES($1,true,1,1)")
        .bind(owner).execute(&t.pool).await.unwrap();
    categorized_item(&t, &token, "Bob's Bar", "place", "place.bar", &[]).await;
    categorized_item(&t, &token, "Another Bar", "place", "place.bar", &[]).await;
    categorized_item(&t, &token, "Dinner", "place", "place.restaurant", &[]).await;
    let (status, answer) = t
        .call(
            Method::POST,
            "/v1/ask/agent",
            Some(&token),
            Some(json!({"request_id":Uuid::new_v4(),"question":"Which bars have I saved?"})),
            &[],
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(answer["reply"], "Here are the saved bars.");
    assert_eq!(answer["views"].as_array().unwrap().len(), 1);
    assert_eq!(answer["views"][0]["total"], 2);
    assert!(
        answer["views"][0]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["subject"] == "Bob's Bar")
    );
    t.cleanup().await;
}

struct SparseCollectionAsk;
#[async_trait]
impl rekky_backend::ask::AskModel for SparseCollectionAsk {
    fn available(&self) -> bool {
        true
    }
    async fn decide(
        &self,
        context: Value,
        _last: bool,
    ) -> Result<rekky_backend::ask::Decision, rekky_backend::ask::AgentError> {
        let observations = context["tool_observations"].as_array().unwrap();
        let (name, arguments) = if observations.is_empty() {
            let category = if context["question"].as_str().unwrap().contains("bar") {
                "place.bar"
            } else {
                "place.cafe"
            };
            (
                "open_collection",
                json!({"title":"Saved places","location":"","category_ids":[category],"kind":"place","query":"","source":"mine","location_role":"relevant","sort":"saved_newest"}),
            )
        } else {
            let result = &observations[0]["result"];
            assert_eq!(result["view_id"], Value::Null);
            assert_eq!(result["presentation"], "direct_answer");
            let items = result["items"].as_array().unwrap();
            let cards: Vec<_> = items
                .iter()
                .map(|item| {
                    json!({
                        "item_id":item["item_id"],"section":"supported",
                        "reason":"This bar is in your saved recommendations.","caveat":"",
                        "evidence_ids":[item["evidence"][0]["id"]]
                    })
                })
                .collect();
            (
                "present_answer",
                json!({"intent":"discovery","new_topic":false,"title":"Saved places",
                    "reply":if items.is_empty() {"No saved cafe matched."} else {"I found one saved bar."},
                    "view_ids":[],"location":"","clarification":"","choices":[],"comparison":null,"results":cards}),
            )
        };
        Ok(rekky_backend::ask::Decision {
            output_items: vec![],
            calls: vec![rekky_backend::ask::ToolCall {
                call_id: String::new(),
                name: name.into(),
                arguments,
            }],
            input_tokens: 100,
            output_tokens: 100,
        })
    }
}

#[tokio::test]
async fn ask_agent_does_not_create_empty_or_single_item_collections() {
    let Some(mut t) = TestApp::new().await else {
        return;
    };
    t.state.ask_model = Arc::new(SparseCollectionAsk);
    t.app = router(t.state.clone());
    let (owner, token) = t.sign_in("google", "valid-a").await;
    t.call(
        Method::POST,
        "/v1/me/visibility-disclosure",
        Some(&token),
        Some(json!({"accept":true})),
        &[],
    )
    .await;
    sqlx::query("INSERT INTO transcript_extraction_permissions(account_id,enabled,generation,disclosure_version) VALUES($1,true,1,1)")
        .bind(owner).execute(&t.pool).await.unwrap();
    categorized_item(&t, &token, "Bob's Bar", "place", "place.bar", &[]).await;
    for (question, expected_cards) in [("Find a bar", 1), ("Find a cafe", 0)] {
        let (status, answer) = t
            .call(
                Method::POST,
                "/v1/ask/agent",
                Some(&token),
                Some(json!({"request_id":Uuid::new_v4(),"question":question})),
                &[],
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{answer}");
        assert!(answer["views"].as_array().unwrap().is_empty());
        assert_eq!(answer["results"].as_array().unwrap().len(), expected_cards);
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM ask_views WHERE owner_id=$1")
        .bind(owner)
        .fetch_one(&t.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    t.cleanup().await;
}
#[async_trait]
impl rekky_backend::ask::AskModel for ScriptedAsk {
    fn available(&self) -> bool {
        true
    }
    async fn decide(
        &self,
        context: Value,
        _last: bool,
    ) -> Result<rekky_backend::ask::Decision, rekky_backend::ask::AgentError> {
        self.decisions
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let history = context["tool_observations"].as_array().unwrap();
        let (name, arguments) = if history.is_empty() {
            (
                "search_knowledge",
                json!({"terms":["Lantern restaurant"],"location":"Delhi","page":0}),
            )
        } else {
            let items = history[0]["result"]["items"].as_array().unwrap();
            let results:Vec<_>=items.iter().map(|item|json!({"item_id":item["item_id"],"section":"supported","reason":"A saved meal with room to talk.","caveat":"","evidence_ids":[item["evidence"][0]["id"]]})).collect();
            (
                "present_answer",
                json!({"intent":"discovery","title":"Somewhere to talk","location":"","clarification":"","choices":[],"results":results}),
            )
        };
        Ok(rekky_backend::ask::Decision {
            output_items: vec![],
            calls: vec![rekky_backend::ask::ToolCall {
                call_id: String::new(),
                name: name.into(),
                arguments,
            }],
            input_tokens: 100,
            output_tokens: 100,
        })
    }
}

#[tokio::test]
async fn agentic_ask_is_owner_scoped_idempotent_and_revision_checked() {
    let Some(mut t) = TestApp::new().await else {
        return;
    };
    let model = Arc::new(ScriptedAsk {
        decisions: std::sync::atomic::AtomicUsize::new(0),
    });
    t.state.ask_model = model.clone();
    t.app = router(t.state.clone());
    let (account, token) = t.sign_in("google", "valid-a").await;
    let owner = account;
    t.call(
        Method::POST,
        "/v1/me/visibility-disclosure",
        Some(&token),
        Some(json!({"accept":true})),
        &[],
    )
    .await;
    sqlx::query("INSERT INTO transcript_extraction_permissions(account_id,enabled,generation,disclosure_version) VALUES($1,true,1,1)").bind(owner).execute(&t.pool).await.unwrap();
    let item = categorized_item(&t, &token, "Lantern", "place", "place.restaurant", &[]).await;
    let run = Uuid::new_v4();
    let request = json!({"request_id":run,"question":"somewhere to talk"});
    let (code, response) = t
        .call(
            Method::POST,
            "/v1/ask/agent",
            Some(&token),
            Some(request.clone()),
            &[],
        )
        .await;
    assert_eq!(code, StatusCode::OK, "{response}");
    assert_eq!(response["results"][0]["item"]["id"], item["id"]);
    assert_eq!(response["mode"], "agent");
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../contracts/rekky/v1/fixtures/ask_answer.json"
    ))
    .unwrap();
    for key in fixture.as_object().unwrap().keys() {
        assert!(
            response.get(key).is_some(),
            "Missing shared answer field: {key}"
        );
    }
    for key in fixture["results"][0].as_object().unwrap().keys() {
        assert!(
            response["results"][0].get(key).is_some(),
            "Missing shared result field: {key}"
        );
    }

    assert_eq!(model.decisions.load(std::sync::atomic::Ordering::SeqCst), 2);
    let (_, again) = t
        .call(
            Method::POST,
            "/v1/ask/agent",
            Some(&token),
            Some(request),
            &[],
        )
        .await;
    assert_eq!(again["results"], response["results"]);
    assert_eq!(model.decisions.load(std::sync::atomic::Ordering::SeqCst), 2);
    let (code, _) = t
        .call(
            Method::POST,
            "/v1/ask/agent",
            Some(&token),
            Some(json!({"request_id":run,"question":"different"})),
            &[],
        )
        .await;
    assert_eq!(code, StatusCode::CONFLICT);
    let (_, other) = t.sign_in("google", "valid-b").await;
    t.call(
        Method::POST,
        "/v1/me/visibility-disclosure",
        Some(&other),
        Some(json!({"accept":true})),
        &[],
    )
    .await;
    let (code, _) = t
        .call(
            Method::GET,
            &format!("/v1/ask/answers/{run}"),
            Some(&other),
            None,
            &[],
        )
        .await;
    assert_eq!(code, StatusCode::NOT_FOUND);
    let (code, _) = t
        .call(
            Method::GET,
            &format!("/v1/items/{}", item["id"].as_str().unwrap()),
            Some(&other),
            None,
            &[],
        )
        .await;
    assert_eq!(code, StatusCode::NOT_FOUND);
    sqlx::query("UPDATE knowledge_items SET revision=revision+1 WHERE id=$1")
        .bind(Uuid::parse_str(item["id"].as_str().unwrap()).unwrap())
        .execute(&t.pool)
        .await
        .unwrap();
    let (_, changed) = t
        .call(
            Method::GET,
            &format!("/v1/ask/answers/{run}"),
            Some(&token),
            None,
            &[],
        )
        .await;
    assert_eq!(changed["results"], json!([]));
    assert_eq!(changed["changed"], true);
    t.call(
        Method::POST,
        "/v1/me/processing-withdrawal",
        Some(&token),
        Some(json!({})),
        &[],
    )
    .await;
    let (code, _) = t
        .call(
            Method::GET,
            &format!("/v1/ask/answers/{run}"),
            Some(&token),
            None,
            &[],
        )
        .await;
    assert_eq!(code, StatusCode::FORBIDDEN);
    t.cleanup().await;
}

#[tokio::test]
async fn ask_account_allowance_is_configurable_without_erasing_usage() {
    let Some(mut t) = TestApp::new().await else {
        return;
    };
    let model = Arc::new(ScriptedAsk {
        decisions: std::sync::atomic::AtomicUsize::new(0),
    });
    t.state.ask_model = model.clone();
    t.state.ask_daily_account_limit = 1;
    t.app = router(t.state.clone());
    let (owner, token) = t.sign_in("google", "valid-a").await;
    t.call(
        Method::POST,
        "/v1/me/visibility-disclosure",
        Some(&token),
        Some(json!({"accept":true})),
        &[],
    )
    .await;
    sqlx::query("INSERT INTO transcript_extraction_permissions(account_id,enabled,generation,disclosure_version) VALUES($1,true,1,1)")
        .bind(owner)
        .execute(&t.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO ask_runs(id,owner_id,request_hash,status,permission_generation) VALUES($1,$2,'earlier','completed',1)")
        .bind(Uuid::new_v4())
        .bind(owner)
        .execute(&t.pool)
        .await
        .unwrap();

    let request = json!({"request_id":Uuid::new_v4(),"question":"a quiet meal"});
    let (status, limited) = t
        .call(
            Method::POST,
            "/v1/ask/agent",
            Some(&token),
            Some(request.clone()),
            &[],
        )
        .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(limited["error"]["code"], "ask_daily_limit");
    assert_eq!(model.decisions.load(std::sync::atomic::Ordering::SeqCst), 0);

    t.state.ask_daily_account_limit = 2;
    t.app = router(t.state.clone());
    let (status, response) = t
        .call(
            Method::POST,
            "/v1/ask/agent",
            Some(&token),
            Some(request),
            &[],
        )
        .await;
    assert_ne!(status, StatusCode::TOO_MANY_REQUESTS, "{response}");
    assert!(model.decisions.load(std::sync::atomic::Ordering::SeqCst) > 0);
    let runs: i64 = sqlx::query_scalar("SELECT count(*) FROM ask_runs WHERE owner_id=$1")
        .bind(owner)
        .fetch_one(&t.pool)
        .await
        .unwrap();
    assert_eq!(runs, 2);
    t.cleanup().await;
}

struct BlockingAsk {
    started: Arc<Notify>,
    release: Arc<Notify>,
    calls: std::sync::atomic::AtomicUsize,
}
#[async_trait]
impl rekky_backend::ask::AskModel for BlockingAsk {
    fn available(&self) -> bool {
        true
    }
    async fn decide(
        &self,
        _: Value,
        _: bool,
    ) -> Result<rekky_backend::ask::Decision, rekky_backend::ask::AgentError> {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.started.notify_one();
        self.release.notified().await;
        Ok(rekky_backend::ask::Decision {
            output_items: vec![],
            calls: vec![rekky_backend::ask::ToolCall {
                call_id: String::new(),
                name: "search_knowledge".into(),
                arguments: json!({"terms":[],"page":0}),
            }],
            input_tokens: 10,
            output_tokens: 10,
        })
    }
}
#[tokio::test]
async fn agentic_ask_cancellation_fences_late_work_and_pre_admission() {
    let Some(mut t) = TestApp::new().await else {
        return;
    };
    let model = Arc::new(BlockingAsk {
        started: Arc::new(Notify::new()),
        release: Arc::new(Notify::new()),
        calls: 0.into(),
    });
    t.state.ask_model = model.clone();
    t.app = router(t.state.clone());
    let (owner, token) = t.sign_in("google", "valid-a").await;
    t.call(
        Method::POST,
        "/v1/me/visibility-disclosure",
        Some(&token),
        Some(json!({"accept":true})),
        &[],
    )
    .await;
    sqlx::query("INSERT INTO transcript_extraction_permissions(account_id,enabled,generation,disclosure_version) VALUES($1,true,1,1)").bind(owner).execute(&t.pool).await.unwrap();
    let run = Uuid::new_v4();
    let app = t.app.clone();
    let auth = token.clone();
    let task = tokio::spawn(async move {
        app.oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/v1/ask/agent")
                .header("authorization", format!("Bearer {auth}"))
                .body(Body::from(
                    json!({"request_id":run,"question":"a quiet meal"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap()
    });
    tokio::time::timeout(std::time::Duration::from_secs(5), model.started.notified())
        .await
        .unwrap();
    let (status, _) = t
        .call(
            Method::DELETE,
            &format!("/v1/ask/answers/{run}"),
            Some(&token),
            None,
            &[],
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    model.release.notify_one();
    assert!(!task.await.unwrap().status().is_success());
    assert_eq!(model.calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    let receipt: (String, bool) =
        sqlx::query_as("SELECT status,result IS NULL FROM ask_runs WHERE id=$1")
            .bind(run)
            .fetch_one(&t.pool)
            .await
            .unwrap();
    assert_eq!(receipt, ("cancelled".into(), true));
    let early = Uuid::new_v4();
    t.call(
        Method::DELETE,
        &format!("/v1/ask/answers/{early}"),
        Some(&token),
        None,
        &[],
    )
    .await;
    let (status, _) = t
        .call(
            Method::POST,
            "/v1/ask/agent",
            Some(&token),
            Some(json!({"request_id":early,"question":"a quiet meal"})),
            &[],
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(model.calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    t.cleanup().await;
}

struct RecordingAsk {
    model: rekky_backend::ask::OpenAiAsk,
    trace: std::sync::Mutex<Vec<Value>>,
}
#[async_trait]
impl rekky_backend::ask::AskModel for RecordingAsk {
    fn available(&self) -> bool {
        self.model.available()
    }
    async fn decide(
        &self,
        context: Value,
        final_turn: bool,
    ) -> Result<rekky_backend::ask::Decision, rekky_backend::ask::AgentError> {
        let result = self.model.decide(context, final_turn).await;
        let trace = match &result {
            Ok(d) => {
                json!({"calls":d.calls.iter().map(|c| json!({"tool":c.name,"arguments":c.arguments})).collect::<Vec<_>>()})
            }
            Err(_) => json!({"error":"provider adapter failed"}),
        };
        self.trace.lock().unwrap().push(trace);
        result
    }
}

#[tokio::test]
#[ignore = "Explicit live-model probe with invented notes only; requires OPENAI_ASK_ENABLED and OPENAI_API_KEY"]
async fn live_ask_development_probe() {
    let mut t = TestApp::new()
        .await
        .expect("isolated DATABASE_URL required");
    assert!(
        t.state.ask_model.available(),
        "Live Ask adapter must be explicitly enabled"
    );
    let recording = Arc::new(RecordingAsk {
        model: rekky_backend::ask::OpenAiAsk::from_env(),
        trace: std::sync::Mutex::new(vec![]),
    });
    t.state.ask_model = recording.clone();
    t.app = router(t.state.clone());
    let (owner, token) = t.sign_in("google", "valid-a").await;
    t.call(
        Method::POST,
        "/v1/me/visibility-disclosure",
        Some(&token),
        Some(json!({"accept":true})),
        &[],
    )
    .await;
    sqlx::query("INSERT INTO transcript_extraction_permissions(account_id,enabled,generation,disclosure_version) VALUES($1,true,1,1)").bind(owner).execute(&t.pool).await.unwrap();
    let notes = [
        (
            "Lantern Garden",
            "place",
            "place.restaurant",
            "We had Italian pasta in Delhi. It was quiet enough to talk without raising our voices. The tables seat two people.",
            "Delhi",
        ),
        (
            "Lantern Live",
            "place",
            "place.restaurant",
            "The food was excellent in Delhi but the music was so loud we could not hear one another.",
            "Delhi",
        ),
        (
            "Pune Courtyard",
            "place",
            "place.restaurant",
            "A peaceful Italian dinner in Pune. Very easy to have a conversation.",
            "Pune",
        ),
        (
            "Mohan Hill Taxi",
            "person_service",
            "service.taxi",
            "Mohan drove us from Mussoorie to Landour on our last trip. He was punctual and drove carefully. I do not know his current service area.",
            "",
        ),
        (
            "Meera Home Catering",
            "person_service",
            "",
            "Meera catered a home lunch for 25 guests. That was good. I have no information about larger events.",
            "",
        ),
        (
            "River Loop",
            "activity",
            "",
            "I enjoyed walking this outdoor trail on a weekend. There were shady stretches and a steep climb at the end.",
            "",
        ),
        (
            "Boulder Room",
            "place",
            "",
            "An indoor climbing gym. The staff were helpful and beginners could try several levels.",
            "",
        ),
        (
            "Ravi Plumbing",
            "person_service",
            "",
            "Ravi repaired the kitchen tap. His work was good. He does not repair musical instruments.",
            "",
        ),
    ];
    for city in ["Delhi", "Pune"] {
        sqlx::query("INSERT INTO geographic_areas(id,name,label,country,feature,population,aliases,ancestors,hierarchy) VALUES($1,$2,$2,'IN','PPL',100,ARRAY[lower($2)],ARRAY[]::text[],'[]') ON CONFLICT(id) DO NOTHING")
            .bind(format!("ask-probe-{}",city.to_lowercase())).bind(city).execute(&t.pool).await.unwrap();
    }
    let mut ids = std::collections::HashMap::new();
    for (name, kind, category, body, city) in notes {
        let (_, saved) = t
            .call(
                Method::POST,
                "/v1/items",
                Some(&token),
                Some(json!({"subject":name,"body":body,"visibility":"private"})),
                &[("idempotency-key", &Uuid::new_v4().to_string())],
            )
            .await;
        let id = Uuid::parse_str(saved["item"]["id"].as_str().unwrap()).unwrap();
        ids.insert(name.to_string(), id);
        let mut rec = json!({"version":2,"entity_kind":kind,"shelf":"Test","experience":"firsthand","summary":body,"observations":[],"locations":if city.is_empty(){json!([])}else{json!([{"role":"venue","text":city,"name":city,"geography":{"status":"resolved","area_id":format!("ask-probe-{}",city.to_lowercase()),"filter_ids":[format!("ask-probe-{}",city.to_lowercase())]}}])},"use_cases":[],"classification":{"types":if category.is_empty(){json!([])}else{json!([{"id":category,"label":"Restaurant"}])}}});
        let mut connection = t.pool.acquire().await.unwrap();
        rekky_backend::geography::enrich(&mut connection, rec["locations"].as_array_mut().unwrap())
            .await
            .unwrap();
        drop(connection);
        sqlx::query("UPDATE knowledge_items SET recommendation=$1 WHERE id=$2")
            .bind(rec)
            .bind(id)
            .execute(&t.pool)
            .await
            .unwrap();
    }
    let queries = [
        (
            "Somewhere I can hear a friend over dinner",
            Some("Lantern Garden"),
            Some("Lantern Live"),
        ),
        (
            "वो ड्राइवर जिसने मसूरी से लंढौर पहुँचाया था",
            Some("Mohan Hill Taxi"),
            None,
        ),
        (
            "Koi quiet dinner ki jagah, Delhi mein",
            Some("Lantern Garden"),
            Some("Pune Courtyard"),
        ),
        ("Who repairs violins?", None, Some("Ravi Plumbing")),
        ("A caterer who can handle 60 guests", None, None),
        (
            "Something to do this weekend outdoors",
            Some("River Loop"),
            Some("Boulder Room"),
        ),
    ];
    let mut report = vec![];
    for (question, expected, excluded) in queries {
        let start = std::time::Instant::now();
        let (status, response) = t
            .call(
                Method::POST,
                "/v1/ask/agent",
                Some(&token),
                Some(json!({"request_id":Uuid::new_v4(),"question":question})),
                &[],
            )
            .await;
        let found = response["results"].as_array().cloned().unwrap_or_default();
        let hit = expected.is_none_or(|name| {
            found
                .iter()
                .any(|r| r["item"]["id"] == ids[name].to_string())
        });
        let forbidden = (question.contains("60 guests")
            && found.iter().any(|r| r["section"] == "supported"))
            || excluded.is_some_and(|name| {
                found
                    .iter()
                    .any(|r| r["item"]["id"] == ids[name].to_string())
            });
        let elapsed = start.elapsed().as_millis();
        println!(
            "probe status={status} expected_hit={hit} forbidden_supported={forbidden} elapsed_ms={elapsed}"
        );
        report.push(json!({"question":question,"status":status.as_u16(),"expected":expected,"excluded_supported":excluded,"hit":hit,"forbidden_supported":forbidden,"elapsed_ms":elapsed,"answer":response,"trace":std::mem::take(&mut *recording.trace.lock().unwrap())}));
    }
    let path = std::env::var("ASK_PROBE_OUTPUT").expect("ASK_PROBE_OUTPUT required");
    std::fs::write(path, serde_json::to_string_pretty(&report).unwrap()).unwrap();
    t.cleanup().await;
    sqlx::query("DELETE FROM geographic_areas WHERE id IN ('ask-probe-delhi','ask-probe-pune')")
        .execute(&t.pool)
        .await
        .unwrap();
    assert!(
        report
            .iter()
            .all(|v| v["status"] == 200 && v["hit"] == true && v["forbidden_supported"] == false),
        "Inspect the development report before enabling Ask"
    );
}

struct ContextAsk {
    seen: std::sync::Mutex<Vec<Value>>,
}
#[async_trait]
impl rekky_backend::ask::AskModel for ContextAsk {
    fn available(&self) -> bool {
        true
    }
    async fn decide(
        &self,
        context: Value,
        _: bool,
    ) -> Result<rekky_backend::ask::Decision, rekky_backend::ask::AgentError> {
        self.seen.lock().unwrap().push(context.clone());
        let previous = context["previous_results"].as_array().unwrap();
        let history = context["tool_observations"].as_array().unwrap();
        let (name, arguments) = if previous.is_empty() && history.is_empty() {
            ("search_knowledge", json!({"terms":["Lantern"],"page":0}))
        } else {
            let items = if previous.is_empty() {
                history[0]["result"]["items"].as_array().unwrap()
            } else {
                previous
            };
            (
                "present_answer",
                json!({"intent":"discovery","new_topic":context["question"]=="Unrelated new topic","title":"Useful options","location":"","clarification":"","choices":[],"results":items.iter().map(|i|json!({"item_id":i["item_id"],"section":"supported","reason":"A saved option.","caveat":"","evidence_ids":[i["evidence"][0]["id"]]})).collect::<Vec<_>>()}),
            )
        };
        Ok(rekky_backend::ask::Decision {
            output_items: vec![],
            calls: vec![rekky_backend::ask::ToolCall {
                call_id: String::new(),
                name: name.into(),
                arguments,
            }],
            input_tokens: 10,
            output_tokens: 10,
        })
    }
}
async fn ask_account(t: &mut TestApp) -> (Uuid, String) {
    let (id, token) = t.sign_in("google", "valid-a").await;
    t.call(
        Method::POST,
        "/v1/me/visibility-disclosure",
        Some(&token),
        Some(json!({"accept":true})),
        &[],
    )
    .await;
    t.call(
        Method::POST,
        "/v1/me/voice-transcription-permission",
        Some(&token),
        Some(json!({"enabled":true,"disclosure_version":1})),
        &[],
    )
    .await;
    t.call(
        Method::POST,
        "/v1/me/transcript-extraction-permission",
        Some(&token),
        Some(json!({"enabled":true,"disclosure_version":1})),
        &[],
    )
    .await;
    (id, token)
}
#[tokio::test]
async fn ask_followups_refresh_evidence_scope_references_and_preserve_constraints() {
    let Some(mut t) = TestApp::new().await else {
        return;
    };
    let model = Arc::new(ContextAsk {
        seen: std::sync::Mutex::new(vec![]),
    });
    t.state.ask_model = model.clone();
    t.app = router(t.state.clone());
    let (_owner, token) = ask_account(&mut t).await;
    let mut ids = vec![];
    for name in ["Lantern One", "Lantern Two"] {
        let (_, v) = t
            .call(
                Method::POST,
                "/v1/items",
                Some(&token),
                Some(
                    json!({"subject":name,"body":"A quiet place in Delhi.","visibility":"private"}),
                ),
                &[("idempotency-key", &Uuid::new_v4().to_string())],
            )
            .await;
        ids.push(v["item"]["id"].as_str().unwrap().to_owned());
    }
    let first = Uuid::new_v4();
    let (code, a) = t
        .call(
            Method::POST,
            "/v1/ask/agent",
            Some(&token),
            Some(json!({"request_id":first,"question":"quiet dinner in Delhi"})),
            &[],
        )
        .await;
    assert_eq!(code, StatusCode::OK, "{a}");
    assert_eq!(a["turn_count"], 1);
    // An edited recommendation is fetched afresh, not copied from the previous answer.
    sqlx::query(
        "UPDATE knowledge_items SET body='Tables for two only.',revision=revision+1 WHERE id=$1",
    )
    .bind(Uuid::parse_str(&ids[0]).unwrap())
    .execute(&t.pool)
    .await
    .unwrap();
    let second = Uuid::new_v4();
    let (code,a)=t.call(Method::POST,"/v1/ask/agent",Some(&token),Some(json!({"request_id":second,"question":"What about this one for six people?","scope_city":"Delhi","previous_request_id":first,"selected_item_ids":[ids[0]],"excluded_item_ids":[ids[1]]})),&[]).await;
    assert_eq!(code, StatusCode::OK, "{a}");
    assert_eq!(a["turn_count"], 2);
    assert_eq!(a["results"].as_array().unwrap().len(), 1);
    assert_eq!(a["results"][0]["item"]["id"], ids[0]);
    let context = model.seen.lock().unwrap().last().unwrap().clone();
    assert_eq!(context["scope_city"], "Delhi");
    assert_eq!(
        context["conversation"]["turns"],
        json!([
            "quiet dinner in Delhi",
            "What about this one for six people?"
        ])
    );
    assert_eq!(
        context["previous_results"][0]["evidence"][0]["text"],
        "Tables for two only."
    );
    assert_eq!(
        context["conversation"]["selected_item_ids"],
        json!([ids[0]])
    );
    let calls = model.seen.lock().unwrap().len();
    let (code,_)=t.call(Method::POST,"/v1/ask/agent",Some(&token),Some(json!({"request_id":Uuid::new_v4(),"question":"This one","previous_request_id":second,"selected_item_ids":[Uuid::new_v4()]})),&[]).await;
    assert_eq!(code, StatusCode::BAD_REQUEST);
    assert_eq!(calls, model.seen.lock().unwrap().len());
    let (_,a)=t.call(Method::POST,"/v1/ask/agent",Some(&token),Some(json!({"request_id":Uuid::new_v4(),"question":"Unrelated new topic","previous_request_id":second})),&[]).await;
    assert_eq!(a["turn_count"], 1);
    assert_eq!(a["excluded_item_ids"], json!([]));
    let (_, other) = t.sign_in("google", "valid-b").await;
    t.call(
        Method::POST,
        "/v1/me/visibility-disclosure",
        Some(&other),
        Some(json!({"accept":true})),
        &[],
    )
    .await;
    t.call(
        Method::POST,
        "/v1/me/transcript-extraction-permission",
        Some(&other),
        Some(json!({"enabled":true,"disclosure_version":1})),
        &[],
    )
    .await;
    let (code,_)=t.call(Method::POST,"/v1/ask/agent",Some(&other),Some(json!({"request_id":Uuid::new_v4(),"question":"Tell me more","previous_request_id":first})),&[]).await;
    assert_eq!(code, StatusCode::CONFLICT);
    sqlx::query("UPDATE ask_runs SET expires_at=now()-interval '1 second' WHERE id=$1")
        .bind(second)
        .execute(&t.pool)
        .await
        .unwrap();
    let (code,e)=t.call(Method::POST,"/v1/ask/agent",Some(&token),Some(json!({"request_id":Uuid::new_v4(),"question":"Tell me more","previous_request_id":second})),&[]).await;
    assert_eq!(code, StatusCode::CONFLICT);
    assert_eq!(e["error"]["code"], "ask_context_expired");
    sqlx::query("UPDATE knowledge_items SET deleted_at=now() WHERE id=$1")
        .bind(Uuid::parse_str(&ids[0]).unwrap())
        .execute(&t.pool)
        .await
        .unwrap();
    let (status,error)=t.call(Method::POST,"/v1/ask/agent",Some(&token),Some(json!({"request_id":Uuid::new_v4(),"previous_request_id":first,"question":"This one","selected_item_ids":[ids[0]]})),&[]).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(error["error"]["code"], "ask_reference_changed");
    t.cleanup().await;
}
fn question_audio() -> Vec<u8> {
    let mut audio = vec![0u8; 128];
    audio[0..4].copy_from_slice(&24u32.to_be_bytes());
    audio[4..8].copy_from_slice(b"ftyp");
    audio[24..28].copy_from_slice(&104u32.to_be_bytes());
    audio[28..32].copy_from_slice(b"moov");
    audio[32..36].copy_from_slice(&96u32.to_be_bytes());
    audio[36..40].copy_from_slice(b"mvhd");
    audio[52..56].copy_from_slice(&1000u32.to_be_bytes());
    audio[56..60].copy_from_slice(&5000u32.to_be_bytes());
    audio
}
#[tokio::test]
async fn ask_dictation_is_temporary_idempotent_and_never_creates_a_recommendation() {
    let Some(mut t) = TestApp::new().await else {
        return;
    };
    let (owner, token) = ask_account(&mut t).await;
    let id = Uuid::new_v4();
    let path = format!("/v1/ask/dictations/{id}");
    let (code, a) = t.call_audio(&path, &token, question_audio(), 0).await;
    assert_eq!(code, StatusCode::OK, "{a}");
    assert_eq!(a["text"], "Ravi fixed the kitchen tap on Tuesday.");
    let (code, b) = t.call_audio(&path, &token, question_audio(), 0).await;
    assert_eq!(code, StatusCode::OK);
    assert_eq!(a, b);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM captures WHERE owner_id=$1")
        .bind(owner)
        .fetch_one(&t.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    let (_, other) = t.sign_in("google", "valid-b").await;
    assert_eq!(
        t.call_audio(&path, &other, question_audio(), 0).await.0,
        StatusCode::FORBIDDEN
    );
    t.call(Method::DELETE, &path, Some(&token), None, &[]).await;
    let text: Option<String> =
        sqlx::query_scalar("SELECT transcript FROM ask_dictations WHERE id=$1")
            .bind(id)
            .fetch_one(&t.pool)
            .await
            .unwrap();
    assert!(text.is_none());
    assert_eq!(
        t.call_audio(&path, &token, question_audio(), 0).await.0,
        StatusCode::CONFLICT
    );
    let early = format!("/v1/ask/dictations/{}", Uuid::new_v4());
    t.call(Method::DELETE, &early, Some(&token), None, &[])
        .await;
    assert_eq!(
        t.call_audio(&early, &token, question_audio(), 0).await.0,
        StatusCode::CONFLICT
    );
    let mut long = question_audio();
    long[56..60].copy_from_slice(&120000u32.to_be_bytes());
    assert_eq!(
        t.call_audio(
            &format!("/v1/ask/dictations/{}", Uuid::new_v4()),
            &token,
            long,
            0
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let expiry_id = Uuid::new_v4();
    assert_eq!(
        t.call_audio(
            &format!("/v1/ask/dictations/{expiry_id}"),
            &token,
            question_audio(),
            0
        )
        .await
        .0,
        StatusCode::OK
    );
    sqlx::query("UPDATE ask_dictations SET expires_at=now()-interval '1 second' WHERE id=$1")
        .bind(expiry_id)
        .execute(&t.pool)
        .await
        .unwrap();
    rekky_backend::ask_voice::sweep(&t.pool).await.unwrap();
    let erased: bool =
        sqlx::query_scalar("SELECT transcript IS NULL FROM ask_dictations WHERE id=$1")
            .bind(expiry_id)
            .fetch_one(&t.pool)
            .await
            .unwrap();
    assert!(erased);
    t.cleanup().await;
}
#[tokio::test]
async fn ask_dictation_cancel_and_withdrawal_discard_late_transcript() {
    for withdraw in [false, true] {
        let transcriber = Arc::new(BlockingTranscriber {
            started: Notify::new(),
            release: Notify::new(),
        });
        let Some(mut t) = TestApp::new_with_transcriber(transcriber.clone()).await else {
            return;
        };
        let (_, token) = ask_account(&mut t).await;
        let id = Uuid::new_v4();
        let app = t.app.clone();
        let auth = token.clone();
        let task = tokio::spawn(async move {
            app.oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(format!("/v1/ask/dictations/{id}"))
                    .header("authorization", format!("Bearer {auth}"))
                    .header("content-type", "audio/mp4")
                    .body(Body::from(question_audio()))
                    .unwrap(),
            )
            .await
            .unwrap()
        });
        tokio::time::timeout(
            std::time::Duration::from_secs(3),
            transcriber.started.notified(),
        )
        .await
        .unwrap();
        if withdraw {
            t.call(
                Method::POST,
                "/v1/me/processing-withdrawal",
                Some(&token),
                Some(json!({})),
                &[],
            )
            .await;
        } else {
            t.call(
                Method::DELETE,
                &format!("/v1/ask/dictations/{id}"),
                Some(&token),
                None,
                &[],
            )
            .await;
        }
        transcriber.release.notify_one();
        assert!(!task.await.unwrap().status().is_success());
        let text: Option<String> =
            sqlx::query_scalar("SELECT transcript FROM ask_dictations WHERE id=$1")
                .bind(id)
                .fetch_one(&t.pool)
                .await
                .unwrap();
        assert!(text.is_none());
        t.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "Explicit synthetic live-model follow-up development probe"]
async fn live_ask_refinement_probe() {
    let mut t = TestApp::new()
        .await
        .expect("isolated test database required");
    assert!(t.state.ask_model.available());
    let model = Arc::new(RecordingAsk {
        model: rekky_backend::ask::OpenAiAsk::from_env(),
        trace: std::sync::Mutex::new(vec![]),
    });
    t.state.ask_model = model.clone();
    t.app = router(t.state.clone());
    let (_, token) = ask_account(&mut t).await;
    let mut ids = std::collections::HashMap::new();
    for (name, body) in [
        (
            "Lantern Two",
            "An Italian restaurant where we could talk easily. Very quiet. It can accommodate a maximum of two guests; no larger groups.",
        ),
        (
            "Maple Table",
            "We had an Italian dinner. Quiet enough to talk easily. Our group of eight fitted comfortably, with room for ten.",
        ),
        (
            "Loud Kitchen",
            "Italian dinner for eight was tasty but the music was so loud we could not hear each other.",
        ),
        (
            "Mohan Taxi",
            "Mohan drove us from Mussoorie to Landour. He was punctual. Current service area is unknown.",
        ),
    ] {
        let (_, v) = t
            .call(
                Method::POST,
                "/v1/items",
                Some(&token),
                Some(json!({"subject":name,"body":body,"visibility":"private"})),
                &[("idempotency-key", &Uuid::new_v4().to_string())],
            )
            .await;
        ids.insert(name, v["item"]["id"].clone());
    }
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    let inputs = [
        json!({"request_id":first,"question":"A quiet Italian dinner where we can talk"}),
        json!({"request_id":second,"previous_request_id":first,"question":"We will be eight people"}),
        json!({"request_id":Uuid::new_v4(),"previous_request_id":first,"question":"Would this one fit eight people?"}),
        json!({"request_id":Uuid::new_v4(),"previous_request_id":first,"question":"Would this one fit eight people?","selected_item_ids":[ids["Lantern Two"]]}),
        json!({"request_id":Uuid::new_v4(),"previous_request_id":second,"question":"Who was that taxi driver around Landour?"}),
    ];
    let mut report = vec![];
    for (i, input) in inputs.into_iter().enumerate() {
        let start = std::time::Instant::now();
        let (code, answer) = t
            .call(
                Method::POST,
                "/v1/ask/agent",
                Some(&token),
                Some(input.clone()),
                &[],
            )
            .await;
        let results = answer["results"].as_array().cloned().unwrap_or_default();
        let has = |name: &str| results.iter().any(|r| r["item"]["id"] == ids[name]);
        let supported = |name: &str| {
            results
                .iter()
                .any(|r| r["item"]["id"] == ids[name] && r["section"] == "supported")
        };
        let passed = code == StatusCode::OK
            && answer["mode"] == "agent"
            && match i {
                0 => has("Lantern Two") && has("Maple Table") && !has("Loud Kitchen"),
                1 => {
                    supported("Maple Table")
                        && !has("Lantern Two")
                        && !has("Loud Kitchen")
                        && answer["turn_count"] == 2
                }
                2 => !answer["clarification"]
                    .as_str()
                    .unwrap_or_default()
                    .is_empty(),
                3 => !supported("Lantern Two"),
                4 => {
                    has("Mohan Taxi")
                        && !has("Lantern Two")
                        && !has("Maple Table")
                        && answer["turn_count"] == 1
                }
                _ => false,
            };
        report.push(json!({"input":input,"passed":passed,"elapsed_ms":start.elapsed().as_millis(),"answer":answer,"trace":std::mem::take(&mut *model.trace.lock().unwrap())}));
        println!("Follow-up case {i}: passed={passed}");
    }
    std::fs::write(
        std::env::var("ASK_PROBE_OUTPUT").unwrap(),
        serde_json::to_string_pretty(&report).unwrap(),
    )
    .unwrap();
    t.cleanup().await;
    assert!(
        report.iter().all(|r| r["passed"] == true),
        "Inspect synthetic follow-up report"
    );
}

#[tokio::test]
#[ignore = "Explicit live transcription of a locally synthesized question only"]
async fn live_ask_synthetic_dictation_probe() {
    let transcriber = Arc::new(rekky_backend::voice::OpenAiTranscriber::from_env());
    assert!(transcriber.available());
    let mut t = TestApp::new_with_transcriber(transcriber)
        .await
        .expect("isolated DB required");
    let (owner, token) = ask_account(&mut t).await;
    let audio = std::fs::read(std::env::var("ASK_SYNTHETIC_AUDIO").unwrap()).unwrap();
    let id = Uuid::new_v4();
    let path = format!("/v1/ask/dictations/{id}");
    let start = std::time::Instant::now();
    let (code, response) = t.call_audio(&path, &token, audio, 0).await;
    let elapsed = start.elapsed().as_millis();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM captures WHERE owner_id=$1")
        .bind(owner)
        .fetch_one(&t.pool)
        .await
        .unwrap();
    t.call(Method::DELETE, &path, Some(&token), None, &[]).await;
    let erased: bool =
        sqlx::query_scalar("SELECT transcript IS NULL FROM ask_dictations WHERE id=$1")
            .bind(id)
            .fetch_one(&t.pool)
            .await
            .unwrap();
    std::fs::write(std::env::var("ASK_PROBE_OUTPUT").unwrap(),serde_json::to_string_pretty(&json!({"synthetic_utterance":"A quiet Italian dinner for eight people.","model":"gpt-transcribe","status":code.as_u16(),"response":response,"elapsed_ms":elapsed,"captures_created":count,"temporary_text_erased":erased})).unwrap()).unwrap();
    t.cleanup().await;
    assert_eq!(code, StatusCode::OK, "{response}");
    assert_eq!(count, 0);
    assert!(erased);
    let text = response["text"].as_str().unwrap().to_lowercase();
    assert!(text.contains("italian") && (text.contains("eight") || text.contains('8')));
}

struct ComparisonAsk;
#[async_trait]
impl rekky_backend::ask::AskModel for ComparisonAsk {
    fn available(&self) -> bool {
        true
    }
    async fn decide(
        &self,
        context: Value,
        final_turn: bool,
    ) -> Result<rekky_backend::ask::Decision, rekky_backend::ask::AgentError> {
        if !context["question"].as_str().unwrap().starts_with("Compare") {
            return ContextAsk {
                seen: std::sync::Mutex::new(vec![]),
            }
            .decide(context, final_turn)
            .await;
        }
        let items = context["previous_results"].as_array().unwrap();
        let selected = context["conversation"]["selected_item_ids"]
            .as_array()
            .unwrap();
        let participants: Vec<_> = items
            .iter()
            .filter(|item| selected.is_empty() || selected.contains(&item["item_id"]))
            .collect();
        let citations: Vec<_> = participants
            .iter()
            .map(|i| json!({"item_id":i["item_id"],"evidence_ids":[i["evidence"][0]["id"]]}))
            .collect();
        Ok(rekky_backend::ask::Decision {
            output_items: vec![],
            input_tokens: 10,
            output_tokens: 10,
            calls: vec![rekky_backend::ask::ToolCall {
                call_id: String::new(),
                name: "present_answer".into(),
                arguments: json!({"intent":"comparison","title":"Compare saved details","location":"","clarification":"","choices":[],"results":[],
              "comparison":{"item_ids":participants.iter().map(|i|i["item_id"].clone()).collect::<Vec<_>>(),
                "dimensions":[{"label":"Experience","cells":participants.iter().map(|i|json!({"item_id":i["item_id"],"text":i["evidence"][0]["text"],"evidence_ids":[i["evidence"][0]["id"]]})).collect::<Vec<_>>()},
                    {"label":"Price","cells":participants.iter().map(|i|json!({"item_id":i["item_id"],"text":"A made-up price without evidence","evidence_ids":[]})).collect::<Vec<_>>()}],
                "conclusion":"The notes describe quiet places; price is not saved.","citations":citations}}),
            }],
        })
    }
}
#[tokio::test]
async fn ask_comparison_replay_followup_and_source_change_invalidation() {
    let Some(mut t) = TestApp::new().await else {
        return;
    };
    t.state.ask_model = Arc::new(ComparisonAsk);
    t.app = router(t.state.clone());
    let (_, token) = ask_account(&mut t).await;
    let mut ids = vec![];
    for name in ["Lantern One", "Lantern Two"] {
        let (code, v) = t
            .call(
                Method::POST,
                "/v1/items",
                Some(&token),
                Some(json!({"subject":name,"body":"Quiet enough to talk.","visibility":"private"})),
                &[("idempotency-key", &Uuid::new_v4().to_string())],
            )
            .await;
        assert_eq!(code, StatusCode::CREATED, "{v}");
        ids.push(v["item"]["id"].clone());
    }
    let parent = Uuid::new_v4();
    let (code, a) = t
        .call(
            Method::POST,
            "/v1/ask/agent",
            Some(&token),
            Some(json!({"request_id":parent,"question":"Lantern dinner"})),
            &[],
        )
        .await;
    assert_eq!(code, StatusCode::OK, "{a}");
    let run = Uuid::new_v4();
    let input = json!({"request_id":run,"previous_request_id":parent,"question":"Compare these two","selected_item_ids":ids});
    let (code, a) = t
        .call(
            Method::POST,
            "/v1/ask/agent",
            Some(&token),
            Some(input.clone()),
            &[],
        )
        .await;
    assert_eq!(code, StatusCode::OK, "{a}");
    assert_eq!(a["comparison"]["items"].as_array().unwrap().len(), 2, "{a}");
    assert_eq!(a["results"], json!([]));
    assert_eq!(
        a["comparison"]["dimensions"][1]["cells"][0]["text"],
        "Not saved"
    );
    assert_eq!(
        a["comparison"]["dimensions"][0]["cells"][0]["evidence"][0]["text"],
        "Quiet enough to talk."
    );
    let (_, replay) = t
        .call(
            Method::POST,
            "/v1/ask/agent",
            Some(&token),
            Some(input),
            &[],
        )
        .await;
    assert_eq!(a, replay);
    // Comparison-only participants remain valid referents on subsequent turns.
    let (code,next)=t.call(Method::POST,"/v1/ask/agent",Some(&token),Some(json!({"request_id":Uuid::new_v4(),"previous_request_id":run,"question":"Tell me about this one","selected_item_ids":[ids[0]]})),&[]).await;
    assert_eq!(code, StatusCode::OK, "{next}");
    assert_eq!(next["selected_item_ids"], json!([ids[0]]));
    sqlx::query("UPDATE knowledge_items SET body='Now loud.',revision=revision+1 WHERE id=$1")
        .bind(Uuid::parse_str(ids[0].as_str().unwrap()).unwrap())
        .execute(&t.pool)
        .await
        .unwrap();
    let (_, fresh) = t
        .call(
            Method::GET,
            &format!("/v1/ask/answers/{run}"),
            Some(&token),
            None,
            &[],
        )
        .await;
    assert_eq!(fresh["changed"], true, "{fresh}");
    assert!(fresh["comparison"].is_null());
    // The next comparison uses current evidence instead of the old snapshot.
    let updated = Uuid::new_v4();
    let (code,fresh)=t.call(Method::POST,"/v1/ask/agent",Some(&token),Some(json!({"request_id":updated,"previous_request_id":run,"question":"Compare again","selected_item_ids":ids})),&[]).await;
    assert_eq!(code, StatusCode::OK, "{fresh}");
    assert!(
        fresh["comparison"]["dimensions"][0]["cells"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["text"] == "Now loud.")
    );
    sqlx::query("UPDATE knowledge_items SET deleted_at=now() WHERE id=$1")
        .bind(Uuid::parse_str(ids[1].as_str().unwrap()).unwrap())
        .execute(&t.pool)
        .await
        .unwrap();
    let (_, fresh) = t
        .call(
            Method::GET,
            &format!("/v1/ask/answers/{updated}"),
            Some(&token),
            None,
            &[],
        )
        .await;
    assert_eq!(fresh["changed"], true, "{fresh}");
    assert!(fresh["comparison"].is_null());
    t.cleanup().await;
}

#[tokio::test]
#[ignore = "Explicit synthetic live-model comparison development probe"]
async fn live_ask_comparison_probe() {
    let mut t = TestApp::new().await.expect("isolated test DB required");
    let model = Arc::new(RecordingAsk {
        model: rekky_backend::ask::OpenAiAsk::from_env(),
        trace: std::sync::Mutex::new(vec![]),
    });
    t.state.ask_model = model.clone();
    t.app = router(t.state.clone());
    let (_, token) = ask_account(&mut t).await;
    for (name, body) in [
        (
            "Lantern Two",
            "Italian dinner. Very quiet, but it can accommodate only two guests, never eight. Price not recorded. Personal estimated rating 10/10.",
        ),
        (
            "Maple Table",
            "Italian restaurant. Quiet enough to talk; our group of eight fitted comfortably last week. Personal estimated rating 7/10. Price not recorded.",
        ),
        (
            "Mohan Taxi",
            "Mohan was punctual on our Landour trip. His vehicle has four passenger seats. We paid 1200 rupees on that trip, not a current quote.",
        ),
        (
            "Pine Taxi",
            "Pine drove our group of six on a past trip. Driver was punctual. No fare recorded.",
        ),
        (
            "Thread Atelier",
            "They repaired the torn silk lining of my jacket neatly. I have never tried them for leather.",
        ),
        (
            "Patch Workshop",
            "They restored my leather bag neatly. I have no experience of their silk repairs.",
        ),
    ] {
        t.call(
            Method::POST,
            "/v1/items",
            Some(&token),
            Some(json!({"subject":name,"body":body,"visibility":"private"})),
            &[("idempotency-key", &Uuid::new_v4().to_string())],
        )
        .await;
    }
    for (name, body) in [
        (
            "Cedar Taxi",
            "Vehicle fits six passengers. It was clean on our trip.",
        ),
        (
            "Birch Taxi",
            "Helpful driver. Passenger capacity not recorded.",
        ),
    ] {
        t.call(
            Method::POST,
            "/v1/items",
            Some(&token),
            Some(json!({"subject":name,"body":body,"visibility":"private"})),
            &[("idempotency-key", &Uuid::new_v4().to_string())],
        )
        .await;
    }
    let mut report = vec![];
    for (index,question) in [
        "Compare Lantern Two and Maple Table for a quiet Italian dinner for eight. Include price. Which fits our group?",
        "Compare Mohan Taxi and Pine Taxi for six passengers. What about price?",
        "Thread Atelier aur Patch Workshop compare karo: meri leather bag repair ke liye. Silk ka experience leather skill nahi hai.",
        "Compare Mohan Taxi, Pine Taxi, Cedar Taxi and Birch Taxi for six passengers, price, punctuality and cleanliness.",
        "Compare these two. Which should I choose?",
    ].iter().enumerate() {
        let start=std::time::Instant::now();
        let (code,a)=t.call(Method::POST,"/v1/ask/agent",Some(&token),Some(json!({"request_id":Uuid::new_v4(),"question":question})),&[]).await;
        let c=&a["comparison"];
        let text=c["dimensions"].as_array().into_iter().flatten()
            .flat_map(|d|d["cells"].as_array().into_iter().flatten())
            .filter_map(|cell|cell["text"].as_str()).collect::<Vec<_>>().join(" ").to_lowercase();
        let unknown = ["not saved","not recorded","no fare recorded","unknown"].iter().any(|phrase|text.contains(phrase));
        let passed=code==StatusCode::OK && a["mode"]=="agent" && if index == 4 {
            c.is_null() && a["clarification"].as_str().is_some_and(|s|!s.is_empty())
        } else {
            a["intent"]=="comparison"
            && c["items"].as_array().is_some_and(|v|v.len()==if index==3 {4}else{2})
            && c["dimensions"].as_array().is_some_and(|v|!v.is_empty())
            && match index {
                0=>unknown && c["conclusion"].as_str().is_some_and(|s| s.contains("Maple")),
                1=>unknown && (text.contains("four") || text.contains("4 passenger")) && text.contains("past"),
                2=>text.contains("leather") && text.contains("silk"),
                3=>unknown && c["dimensions"].as_array().is_some_and(|ds| ds.iter().all(|d|d["cells"].as_array().is_some_and(|v|v.len()==4))),
                _=>false
            }
        };
        report.push(json!({"question":question,"passed":passed,"elapsed_ms":start.elapsed().as_millis(),"answer":a,"trace":std::mem::take(&mut *model.trace.lock().unwrap())}));
        println!("Comparison case {index}: passed={passed}");
    }
    std::fs::write(
        std::env::var("ASK_PROBE_OUTPUT").unwrap(),
        serde_json::to_string_pretty(&report).unwrap(),
    )
    .unwrap();
    t.cleanup().await;
    assert!(
        report.iter().all(|r| r["passed"] == true),
        "Inspect synthetic comparison report"
    );
}
