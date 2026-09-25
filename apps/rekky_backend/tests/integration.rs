use async_trait::async_trait;
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode},
};
use rekky_backend::{
    AppState,
    auth::{IdentityVerifier, Provider, VerifyError, hash_token},
    migrate, router,
};
use serde_json::{Value, json};
use sqlx::PgPool;
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

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
struct TestApp {
    pool: PgPool,
    app: Router,
    ids: Vec<Uuid>,
}
impl TestApp {
    async fn new() -> Option<Self> {
        let url = std::env::var("DATABASE_URL").ok()?;
        let pool = PgPool::connect(&url).await.unwrap();
        migrate::run(&pool).await.unwrap();
        let verifier = Arc::new(TestVerifier {
            marker: Uuid::new_v4(),
        });
        let app = router(AppState {
            pool: pool.clone(),
            verifier,
        });
        Some(Self {
            pool,
            app,
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
