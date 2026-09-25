use crate::auth::{
    IdentityVerifier, Provider, VerifyError, account_for_token, exchange_identity, hash_token,
};
use axum::{
    Json, Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, SecondsFormat, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgPool, Row};
use std::{
    collections::HashSet,
    sync::{Arc, OnceLock},
};
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub verifier: Arc<dyn IdentityVerifier>,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/session/exchange", post(exchange))
        .route("/v1/session", axum::routing::delete(logout))
        .route("/v1/me", get(me))
        .route("/v1/me/visibility-disclosure", post(accept_disclosure))
        .route("/v1/me/processing-withdrawal", post(withdraw_processing))
        .route("/v1/items", post(save_item).get(list_items))
        .route(
            "/v1/items/{id}",
            axum::routing::patch(change_visibility).delete(delete_item),
        )
        .route("/v1/captures/{id}", get(capture))
        .route(
            "/v1/captures/{id}/source",
            axum::routing::delete(delete_source),
        )
        .route("/v1/ask", post(ask))
        .layer(DefaultBodyLimit::max(100 * 1024))
        .with_state(state)
}

type ApiResult = Result<Response, ApiError>;

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: &'static str,
}
impl ApiError {
    fn new(status: StatusCode, code: &'static str, message: &'static str) -> Self {
        Self {
            status,
            code,
            message,
        }
    }
    fn bad(message: &'static str) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid_request", message)
    }
    fn cursor() -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            "invalid_cursor",
            "Invalid page cursor",
        )
    }
    fn not_found(message: &'static str) -> Self {
        Self::new(StatusCode::NOT_FOUND, "not_found", message)
    }
    fn conflict(message: &'static str) -> Self {
        Self::new(StatusCode::CONFLICT, "revision_conflict", message)
    }
}
impl From<sqlx::Error> for ApiError {
    fn from(cause: sqlx::Error) -> Self {
        eprintln!("Database request failed: {cause}");
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "server_error",
            "Request could not be completed",
        )
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(json!({"error":{"code":self.code,"message":self.message}})),
        )
            .into_response()
    }
}
fn ok(value: Value) -> Response {
    Json(value).into_response()
}
fn created(value: Value) -> Response {
    (StatusCode::CREATED, Json(value)).into_response()
}
fn no_content() -> Response {
    StatusCode::NO_CONTENT.into_response()
}
fn parse<T: DeserializeOwned>(body: &Bytes) -> Result<T, ApiError> {
    serde_json::from_slice(body).map_err(|_| ApiError::bad("Invalid request body"))
}
fn header<'a>(headers: &'a HeaderMap, name: &'static str) -> Option<&'a str> {
    headers.get(name).and_then(|v| v.to_str().ok())
}
fn uuid(value: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(value).map_err(|_| ApiError::bad("Invalid ID"))
}
fn revision(headers: &HeaderMap) -> Result<i32, ApiError> {
    header(headers, "if-match")
        .and_then(|v| v.parse::<i32>().ok())
        .filter(|v| *v > 0)
        .ok_or_else(|| ApiError::bad("If-Match revision is required"))
}
async fn owner(state: &AppState, headers: &HeaderMap, disclosure: bool) -> Result<Uuid, ApiError> {
    let bearer = header(headers, "authorization").and_then(|v| v.strip_prefix("Bearer "));
    let token = bearer
        .filter(|v| {
            (32..=128).contains(&v.len())
                && v.bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
        })
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "Sign in is required",
            )
        })?;
    let account = account_for_token(&state.pool, token)
        .await?
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "Session is invalid or expired",
            )
        })?;
    if disclosure {
        let accepted = sqlx::query(
            "SELECT 1 FROM accounts WHERE id=$1 AND disclosure_accepted_at IS NOT NULL",
        )
        .bind(account)
        .fetch_optional(&state.pool)
        .await?
        .is_some();
        if !accepted {
            return Err(ApiError::new(
                StatusCode::FORBIDDEN,
                "visibility_disclosure_required",
                "Accept the visibility disclosure before using Rekky",
            ));
        }
    }
    Ok(account)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExchangeInput {
    provider: String,
    id_token: String,
}
async fn health() -> Response {
    ok(json!({"status":"ok"}))
}
async fn exchange(State(state): State<AppState>, body: Bytes) -> ApiResult {
    let input: ExchangeInput = parse(&body)?;
    let provider =
        Provider::parse(&input.provider).ok_or_else(|| ApiError::bad("Invalid sign-in request"))?;
    if input.id_token.is_empty() || input.id_token.len() > 12000 {
        return Err(ApiError::bad("Invalid sign-in request"));
    }
    let subject = match state.verifier.verify(provider, &input.id_token).await {
        Ok(subject) if !subject.is_empty() => subject,
        Err(VerifyError::Unconfigured) => {
            return Err(ApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "identity_unavailable",
                "Identity provider is not configured",
            ));
        }
        _ => {
            return Err(ApiError::new(
                StatusCode::UNAUTHORIZED,
                "invalid_identity",
                "Identity token was not accepted",
            ));
        }
    };
    let (id, token, expires_at) = exchange_identity(&state.pool, provider, &subject).await?;
    Ok(created(
        json!({"session":{"token":token,"expires_at":iso(expires_at)},"account":{"id":id}}),
    ))
}
async fn me(State(state): State<AppState>, headers: HeaderMap) -> ApiResult {
    let id = owner(&state, &headers, false).await?;
    let row = sqlx::query("SELECT a.disclosure_accepted_at,COALESCE(p.enabled,false) processing_enabled,COALESCE(p.generation,0) processing_generation FROM accounts a LEFT JOIN processing_permissions p ON p.account_id=a.id WHERE a.id=$1")
        .bind(id).fetch_one(&state.pool).await?;
    let accepted: Option<DateTime<Utc>> = row.try_get("disclosure_accepted_at")?;
    Ok(ok(
        json!({"account":{"id":id,"visibility_disclosure_accepted":accepted.is_some(),"processing_enabled":row.try_get::<bool,_>("processing_enabled")?,"processing_generation":row.try_get::<i64,_>("processing_generation")?}}),
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AcceptInput {
    accept: bool,
}
async fn accept_disclosure(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> ApiResult {
    let id = owner(&state, &headers, false).await?;
    let input: AcceptInput = parse(&body)?;
    if !input.accept {
        return Err(ApiError::bad(
            "Accept the visibility disclosure to continue",
        ));
    }
    sqlx::query("UPDATE accounts SET disclosure_accepted_at=COALESCE(disclosure_accepted_at,now()) WHERE id=$1")
        .bind(id).execute(&state.pool).await?;
    Ok(ok(
        json!({"account":{"id":id,"visibility_disclosure_accepted":true}}),
    ))
}
async fn logout(State(state): State<AppState>, headers: HeaderMap) -> ApiResult {
    owner(&state, &headers, false).await?;
    let token = header(&headers, "authorization")
        .unwrap()
        .strip_prefix("Bearer ")
        .unwrap();
    sqlx::query("DELETE FROM sessions WHERE token_hash=$1")
        .bind(hash_token(token))
        .execute(&state.pool)
        .await?;
    Ok(no_content())
}
async fn withdraw_processing(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> ApiResult {
    let id = owner(&state, &headers, false).await?;
    if !body.is_empty() && parse::<EmptyInput>(&body).is_err() {
        return Err(ApiError::bad("Invalid withdrawal request"));
    }
    let row = sqlx::query("INSERT INTO processing_permissions(account_id,enabled,generation) VALUES ($1,false,1) ON CONFLICT (account_id) DO UPDATE SET enabled=false,generation=processing_permissions.generation+1,updated_at=now() RETURNING generation")
        .bind(id).fetch_one(&state.pool).await?;
    Ok(ok(
        json!({"processing":{"enabled":false,"generation":row.try_get::<i64,_>("generation")?,"acknowledged":true}}),
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EmptyInput {}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum Visibility {
    Friends,
    Private,
}
impl Visibility {
    fn as_str(self) -> &'static str {
        match self {
            Self::Friends => "friends",
            Self::Private => "private",
        }
    }
}
fn friends() -> Visibility {
    Visibility::Friends
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ItemInput {
    subject: String,
    body: String,
    #[serde(default = "friends")]
    visibility: Visibility,
}
impl ItemInput {
    fn normalized(mut self) -> Result<Self, ApiError> {
        self.subject = self.subject.trim().to_owned();
        self.body = self.body.trim().to_owned();
        if !(1..=120).contains(&self.subject.chars().count())
            || !(1..=20000).contains(&self.body.chars().count())
        {
            return Err(ApiError::bad("Valid item and Idempotency-Key are required"));
        }
        Ok(self)
    }
}
#[derive(FromRow, Serialize)]
struct ItemRow {
    id: Uuid,
    capture_id: Uuid,
    subject: String,
    body: String,
    visibility: String,
    revision: i32,
    created_at: DateTime<Utc>,
}
fn item_json(row: &ItemRow) -> Value {
    json!({"id":row.id,"capture_id":row.capture_id,"subject":row.subject,"body":row.body,"visibility":row.visibility,"revision":row.revision,"created_at":iso(row.created_at)})
}
fn iso(date: DateTime<Utc>) -> String {
    date.to_rfc3339_opts(SecondsFormat::Micros, true)
}
fn hash(value: &[u8]) -> String {
    hex::encode(Sha256::digest(value))
}
fn idempotency_key(headers: &HeaderMap) -> Result<&str, ApiError> {
    header(headers, "idempotency-key")
        .filter(|v| {
            (8..=128).contains(&v.len())
                && v.bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
        })
        .ok_or_else(|| ApiError::bad("Valid item and Idempotency-Key are required"))
}
async fn save_item(State(state): State<AppState>, headers: HeaderMap, body: Bytes) -> ApiResult {
    let owner_id = owner(&state, &headers, true).await?;
    let key = idempotency_key(&headers)?;
    let input = parse::<ItemInput>(&body)?.normalized()?;
    let payload_hash = hash(&serde_json::to_vec(&input).expect("serializable item"));
    let mut transaction = state.pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("{owner_id}:manual_item:{key}"))
        .execute(&mut *transaction)
        .await?;
    let prior = sqlx::query("SELECT payload_hash,response FROM idempotency_records WHERE account_id=$1 AND operation='manual_item' AND request_key=$2")
        .bind(owner_id).bind(key).fetch_optional(&mut *transaction).await?;
    if let Some(record) = prior {
        let prior_hash: String = record.try_get("payload_hash")?;
        if prior_hash != payload_hash {
            return Err(ApiError::new(
                StatusCode::CONFLICT,
                "idempotency_conflict",
                "This key was used for another request",
            ));
        }
        let response: Value = record.try_get("response")?;
        let item_id: Uuid = response["item"]["id"]
            .as_str()
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| ApiError::from(sqlx::Error::RowNotFound))?;
        let live: Option<ItemRow> = sqlx::query_as("SELECT id,capture_id,subject,body,visibility,revision,created_at FROM knowledge_items WHERE id=$1 AND owner_id=$2 AND deleted_at IS NULL")
            .bind(item_id).bind(owner_id).fetch_optional(&mut *transaction).await?;
        transaction.commit().await?;
        return match live {
            Some(item) => Ok(ok(json!({"item":item_json(&item)}))),
            None => Err(ApiError::new(
                StatusCode::GONE,
                "item_deleted",
                "The previously saved item was deleted",
            )),
        };
    }
    let capture_id = Uuid::new_v4();
    let source_id = Uuid::new_v4();
    let item_id = Uuid::new_v4();
    sqlx::query("INSERT INTO captures(id,owner_id,kind,status,desired_visibility) VALUES ($1,$2,'typed','completed',$3)")
        .bind(capture_id).bind(owner_id).bind(input.visibility.as_str()).execute(&mut *transaction).await?;
    sqlx::query("INSERT INTO source_texts(id,capture_id,owner_id,kind,content) VALUES ($1,$2,$3,'typed',$4)")
        .bind(source_id).bind(capture_id).bind(owner_id).bind(&input.body).execute(&mut *transaction).await?;
    let item: ItemRow = sqlx::query_as("INSERT INTO knowledge_items(id,capture_id,owner_id,subject,body,visibility) VALUES ($1,$2,$3,$4,$5,$6) RETURNING id,capture_id,subject,body,visibility,revision,created_at")
        .bind(item_id).bind(capture_id).bind(owner_id).bind(&input.subject).bind(&input.body).bind(input.visibility.as_str())
        .fetch_one(&mut *transaction).await?;
    let response = json!({"item":item_json(&item)});
    sqlx::query("INSERT INTO idempotency_records(account_id,operation,request_key,payload_hash,response) VALUES ($1,'manual_item',$2,$3,$4)")
        .bind(owner_id).bind(key).bind(payload_hash).bind(&response).execute(&mut *transaction).await?;
    transaction.commit().await?;
    Ok(created(response))
}

#[derive(Deserialize)]
struct ListQuery {
    cursor: Option<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ItemCursor {
    created_at: DateTime<Utc>,
    id: Uuid,
}
fn decode<T: DeserializeOwned>(value: &str) -> Result<T, ApiError> {
    if value.len() > 500 {
        return Err(ApiError::cursor());
    }
    let bytes = URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|_| ApiError::cursor())?;
    serde_json::from_slice(&bytes).map_err(|_| ApiError::cursor())
}
fn encode<T: Serialize>(value: &T) -> String {
    URL_SAFE_NO_PAD.encode(serde_json::to_vec(value).expect("cursor"))
}
async fn list_items(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ListQuery>,
) -> ApiResult {
    let owner_id = owner(&state, &headers, true).await?;
    let cursor: Option<ItemCursor> = query.cursor.as_deref().map(decode).transpose()?;
    let rows: Vec<ItemRow> = sqlx::query_as("SELECT id,capture_id,subject,body,visibility,revision,created_at FROM knowledge_items WHERE owner_id=$1 AND deleted_at IS NULL AND ($2::timestamptz IS NULL OR (created_at,id)<($2::timestamptz,$3::uuid)) ORDER BY created_at DESC,id DESC LIMIT 21")
        .bind(owner_id).bind(cursor.as_ref().map(|c| c.created_at)).bind(cursor.as_ref().map(|c| c.id))
        .fetch_all(&state.pool).await?;
    let next_cursor = if rows.len() > 20 {
        rows.get(19).map(|r| {
            encode(&ItemCursor {
                created_at: r.created_at,
                id: r.id,
            })
        })
    } else {
        None
    };
    Ok(ok(
        json!({"items":rows.iter().take(20).map(item_json).collect::<Vec<_>>(),"next_cursor":next_cursor}),
    ))
}

async fn capture(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult {
    let owner_id = owner(&state, &headers, true).await?;
    let id = uuid(&id)?;
    let row = sqlx::query("SELECT c.id,c.status,c.revision,s.kind source_kind,s.content source_content,s.revision source_revision FROM captures c LEFT JOIN source_texts s ON s.capture_id=c.id AND s.owner_id=c.owner_id WHERE c.id=$1 AND c.owner_id=$2 AND EXISTS (SELECT 1 FROM knowledge_items i WHERE i.capture_id=c.id AND i.owner_id=c.owner_id AND i.deleted_at IS NULL)")
        .bind(id).bind(owner_id).fetch_optional(&state.pool).await?
        .ok_or_else(|| ApiError::not_found("Capture not found"))?;
    let content: Option<String> = row.try_get("source_content")?;
    let source = match content {
        Some(text) => {
            json!({"kind":row.try_get::<Option<String>,_>("source_kind")?,"text":text,"revision":row.try_get::<Option<i32>,_>("source_revision")?})
        }
        None => Value::Null,
    };
    Ok(ok(
        json!({"capture":{"id":id,"status":row.try_get::<String,_>("status")?,"revision":row.try_get::<i32,_>("revision")?,"source":source}}),
    ))
}
async fn delete_source(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult {
    let owner_id = owner(&state, &headers, true).await?;
    let id = uuid(&id)?;
    let revision = revision(&headers)?;
    let result =
        sqlx::query("DELETE FROM source_texts WHERE capture_id=$1 AND owner_id=$2 AND revision=$3")
            .bind(id)
            .bind(owner_id)
            .bind(revision)
            .execute(&state.pool)
            .await?;
    if result.rows_affected() == 0 {
        let exists = sqlx::query("SELECT 1 FROM source_texts WHERE capture_id=$1 AND owner_id=$2")
            .bind(id)
            .bind(owner_id)
            .fetch_optional(&state.pool)
            .await?
            .is_some();
        return Err(if exists {
            ApiError::conflict("Source changed; refresh before deleting")
        } else {
            ApiError::not_found("Source not found")
        });
    }
    Ok(no_content())
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VisibilityInput {
    visibility: Visibility,
}
async fn change_visibility(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Bytes,
) -> ApiResult {
    let owner_id = owner(&state, &headers, true).await?;
    let id = uuid(&id)?;
    let revision = revision(&headers)?;
    let input: VisibilityInput = parse(&body)?;
    let updated: Option<ItemRow> = sqlx::query_as("UPDATE knowledge_items SET visibility=$1,revision=revision+1 WHERE id=$2 AND owner_id=$3 AND revision=$4 AND deleted_at IS NULL RETURNING id,capture_id,subject,body,visibility,revision,created_at")
        .bind(input.visibility.as_str()).bind(id).bind(owner_id).bind(revision).fetch_optional(&state.pool).await?;
    if let Some(item) = updated {
        return Ok(ok(json!({"item":item_json(&item)})));
    }
    let exists = sqlx::query(
        "SELECT 1 FROM knowledge_items WHERE id=$1 AND owner_id=$2 AND deleted_at IS NULL",
    )
    .bind(id)
    .bind(owner_id)
    .fetch_optional(&state.pool)
    .await?
    .is_some();
    Err(if exists {
        ApiError::conflict("Item changed; refresh before editing")
    } else {
        ApiError::not_found("Item not found")
    })
}
async fn delete_item(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult {
    let owner_id = owner(&state, &headers, true).await?;
    let id = uuid(&id)?;
    let revision = revision(&headers)?;
    let mut transaction = state.pool.begin().await?;
    let row = sqlx::query("SELECT capture_id,revision FROM knowledge_items WHERE id=$1 AND owner_id=$2 AND deleted_at IS NULL FOR UPDATE")
        .bind(id).bind(owner_id).fetch_optional(&mut *transaction).await?
        .ok_or_else(|| ApiError::not_found("Item not found"))?;
    if row.try_get::<i32, _>("revision")? != revision {
        return Err(ApiError::conflict("Item changed; refresh before deleting"));
    }
    let capture_id: Uuid = row.try_get("capture_id")?;
    sqlx::query("UPDATE knowledge_items SET deleted_at=now(),revision=revision+1 WHERE id=$1")
        .bind(id)
        .execute(&mut *transaction)
        .await?;
    sqlx::query("DELETE FROM source_texts WHERE capture_id=$1 AND owner_id=$2")
        .bind(capture_id)
        .bind(owner_id)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    Ok(no_content())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AskInput {
    question: String,
    cursor: Option<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AskCursor {
    offset: i64,
    question_hash: String,
}
#[derive(FromRow)]
struct AskRow {
    id: Uuid,
    subject: String,
    body: String,
    visibility: String,
    revision: i32,
}
async fn ask(State(state): State<AppState>, headers: HeaderMap, body: Bytes) -> ApiResult {
    let owner_id = owner(&state, &headers, true).await?;
    let mut input: AskInput = parse(&body)?;
    input.question = input.question.trim().to_owned();
    if !(1..=500).contains(&input.question.chars().count())
        || input.cursor.as_ref().is_some_and(|v| v.len() > 500)
    {
        return Err(ApiError::bad("Invalid question"));
    }
    static WORDS: OnceLock<Regex> = OnceLock::new();
    let words = WORDS.get_or_init(|| Regex::new(r"[\p{L}\p{M}\p{N}]+").expect("word regex"));
    let stop: HashSet<&'static str> = [
        "who",
        "what",
        "where",
        "which",
        "did",
        "does",
        "do",
        "have",
        "has",
        "any",
        "the",
        "our",
        "my",
        "your",
        "can",
        "is",
        "in",
        "for",
        "me",
        "to",
        "a",
        "an",
        "recommendation",
        "recommendations",
    ]
    .into();
    let lower = input.question.to_lowercase();
    let terms: Vec<_> = words
        .find_iter(&lower)
        .map(|m| m.as_str())
        .filter(|term| term.chars().count() >= 2 && !stop.contains(term))
        .take(8)
        .collect();
    if terms.is_empty() {
        return Ok(ok(
            json!({"scope":"own","results":[],"answer":null,"next_cursor":null}),
        ));
    }
    let question_hash = hash(input.question.as_bytes());
    let offset = if let Some(cursor) = input.cursor.as_deref() {
        let parsed: AskCursor = decode(cursor)?;
        if parsed.offset < 0 || parsed.offset > 100000 || parsed.question_hash != question_hash {
            return Err(ApiError::cursor());
        }
        parsed.offset
    } else {
        0
    };
    let search_terms = terms
        .iter()
        .map(|term| format!("{term}:*"))
        .collect::<Vec<_>>()
        .join(" | ");
    let rows: Vec<AskRow> = sqlx::query_as("SELECT id,subject,body,visibility,revision FROM knowledge_items WHERE owner_id=$1 AND deleted_at IS NULL AND to_tsvector('simple',subject||' '||body) @@ to_tsquery('simple',$2) ORDER BY ts_rank_cd(to_tsvector('simple',subject||' '||body),to_tsquery('simple',$2)) DESC,created_at DESC,id DESC LIMIT 21 OFFSET $3")
        .bind(owner_id).bind(search_terms).bind(offset).fetch_all(&state.pool).await?;
    let results: Vec<_> = rows.iter().take(20).map(|r| json!({"item_id":r.id,"subject":r.subject,"body":r.body,"visibility":r.visibility,"revision":r.revision,"evidence_item_id":r.id})).collect();
    let next_cursor = if rows.len() > 20 {
        Some(encode(&AskCursor {
            offset: offset + 20,
            question_hash,
        }))
    } else {
        None
    };
    Ok(ok(
        json!({"scope":"own","results":results,"answer":null,"next_cursor":next_cursor}),
    ))
}
