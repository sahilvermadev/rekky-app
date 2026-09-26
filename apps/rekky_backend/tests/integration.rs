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
            "ignored_unit_ids":[],"unresolved_unit_ids":[]
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
            verifier,
            transcriber,
            extractor: Arc::new(TestExtractor),
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
        "private"
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
    let count:i64=sqlx::query_scalar("SELECT count(*) FROM knowledge_items WHERE capture_id=$1 AND visibility='private' AND deleted_at IS NULL")
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
    let input: Value = serde_json::from_str(include_str!(
        "../../../contracts/rekky/v1/fixtures/recommendation_edit.json"
    ))
    .unwrap();
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
