use crate::auth::{
    IdentityVerifier, Provider, VerifyError, account_for_token, exchange_identity, hash_token,
};
use crate::extraction::{
    EXTRACTION_DISCLOSURE_VERSION, EXTRACTION_MODEL, TranscriptExtractor, preserve_unresolved,
    validate,
};
use crate::voice::{VOICE_DISCLOSURE_VERSION, VOICE_MODEL, VOICE_PROVIDER, VoiceTranscriber};
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
    pub transcriber: Arc<dyn VoiceTranscriber>,
    pub extractor: Arc<dyn TranscriptExtractor>,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/session/exchange", post(exchange))
        .route("/v1/session", axum::routing::delete(logout))
        .route("/v1/me", get(me))
        .route("/v1/me/visibility-disclosure", post(accept_disclosure))
        .route("/v1/me/processing-withdrawal", post(withdraw_processing))
        .route(
            "/v1/me/voice-transcription-permission",
            get(voice_permission).post(set_voice_permission),
        )
        .route("/v1/voice-captures", get(list_voice_captures))
        .route(
            "/v1/me/transcript-extraction-permission",
            get(extraction_permission).post(set_extraction_permission),
        )
        .route(
            "/v1/voice-captures/{id}/extract",
            post(extract_voice_capture),
        )
        .route(
            "/v1/voice-drafts/{id}/transcribe",
            post(transcribe_voice).layer(DefaultBodyLimit::max(5 * 1024 * 1024)),
        )
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
    let mut tx = state.pool.begin().await?;
    let row = sqlx::query("INSERT INTO processing_permissions(account_id,enabled,generation) VALUES ($1,false,1) ON CONFLICT (account_id) DO UPDATE SET enabled=false,generation=processing_permissions.generation+1,updated_at=now() RETURNING generation")
        .bind(id).fetch_one(&mut *tx).await?;
    sqlx::query("UPDATE voice_transcription_permissions SET enabled=false,generation=generation+1,updated_at=now() WHERE account_id=$1 AND enabled=true")
        .bind(id).execute(&mut *tx).await?;
    sqlx::query("UPDATE transcript_extraction_permissions SET enabled=false,generation=generation+1,updated_at=now() WHERE account_id=$1 AND enabled=true")
        .bind(id).execute(&mut *tx).await?;
    sqlx::query("UPDATE voice_transcription_jobs SET status='cancelled',updated_at=now() WHERE account_id=$1 AND status='processing'")
        .bind(id).execute(&mut *tx).await?;
    sqlx::query("UPDATE transcript_extraction_jobs SET status='cancelled',updated_at=now() WHERE account_id=$1 AND status='processing'")
        .bind(id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(ok(
        json!({"processing":{"enabled":false,"generation":row.try_get::<i64,_>("generation")?,"acknowledged":true}}),
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EmptyInput {}

async fn voice_permission(State(state): State<AppState>, headers: HeaderMap) -> ApiResult {
    let id = owner(&state, &headers, true).await?;
    let row = sqlx::query(
        "SELECT enabled,generation,disclosure_version FROM voice_transcription_permissions WHERE account_id=$1",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?;
    Ok(ok(json!({"voice_transcription":{
        "enabled":row.as_ref().map(|r| r.get::<bool,_>("enabled")).unwrap_or(false),
        "generation":row.as_ref().map(|r| r.get::<i64,_>("generation")).unwrap_or(0),
        "disclosure_version":row.as_ref().and_then(|r| r.get::<Option<i32>,_>("disclosure_version")),
        "provider":VOICE_PROVIDER,
        "model":VOICE_MODEL,
        "provider_available":state.transcriber.available()
    }})))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VoicePermissionInput {
    enabled: bool,
    disclosure_version: Option<i32>,
}

async fn set_voice_permission(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> ApiResult {
    let id = owner(&state, &headers, true).await?;
    let input: VoicePermissionInput = parse(&body)?;
    if input.enabled && input.disclosure_version != Some(VOICE_DISCLOSURE_VERSION) {
        return Err(ApiError::bad(
            "Current voice-processing disclosure is required",
        ));
    }
    if input.enabled && !state.transcriber.available() {
        return Err(ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "voice_unavailable",
            "Voice transcription is not configured",
        ));
    }
    let mut tx = state.pool.begin().await?;
    let disclosure_version = if input.enabled {
        Some(VOICE_DISCLOSURE_VERSION)
    } else {
        None
    };
    let row = sqlx::query(
        "INSERT INTO voice_transcription_permissions(account_id,enabled,generation,provider_id,disclosure_version) \
         VALUES ($1,$2,1,'openai',$3) \
         ON CONFLICT (account_id) DO UPDATE SET \
         generation=CASE WHEN voice_transcription_permissions.enabled IS DISTINCT FROM EXCLUDED.enabled \
           OR (EXCLUDED.enabled AND voice_transcription_permissions.disclosure_version IS DISTINCT FROM EXCLUDED.disclosure_version) \
           THEN voice_transcription_permissions.generation+1 ELSE voice_transcription_permissions.generation END, \
         enabled=EXCLUDED.enabled, \
         disclosure_version=CASE WHEN EXCLUDED.enabled THEN EXCLUDED.disclosure_version ELSE voice_transcription_permissions.disclosure_version END, \
         updated_at=now() RETURNING generation",
    )
    .bind(id)
    .bind(input.enabled)
    .bind(disclosure_version)
    .fetch_one(&mut *tx)
    .await?;
    if !input.enabled {
        sqlx::query("UPDATE voice_transcription_jobs SET status='cancelled',updated_at=now() WHERE account_id=$1 AND status='processing'")
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    let generation: i64 = row.try_get("generation")?;
    tx.commit().await?;
    Ok(ok(json!({"voice_transcription":{
        "enabled":input.enabled,
        "generation":generation,
        "disclosure_version":if input.enabled {Some(VOICE_DISCLOSURE_VERSION)} else {None},
        "provider":VOICE_PROVIDER,
        "acknowledged":true
    }})))
}

async fn list_voice_captures(State(state): State<AppState>, headers: HeaderMap) -> ApiResult {
    let id = owner(&state, &headers, true).await?;
    let rows = sqlx::query(
        "SELECT c.id,c.created_at,s.content,s.revision,j.status extraction_status,j.partial, \
         (SELECT count(*) FROM knowledge_items i WHERE i.capture_id=c.id AND i.owner_id=c.owner_id AND i.deleted_at IS NULL) item_count \
         FROM captures c JOIN source_texts s \
         ON s.capture_id=c.id AND s.owner_id=c.owner_id \
         LEFT JOIN transcript_extraction_jobs j ON j.capture_id=c.id AND j.account_id=c.owner_id \
         WHERE c.owner_id=$1 AND c.kind='voice' AND s.kind='transcript' \
         ORDER BY c.created_at DESC,c.id DESC LIMIT 50",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;
    let captures: Vec<Value> = rows
        .into_iter()
        .map(|row| {
            json!({
                "id":row.get::<Uuid,_>("id"),
                "created_at":iso(row.get::<DateTime<Utc>,_>("created_at")),
                "transcript":row.get::<String,_>("content"),
                "source_revision":row.get::<i32,_>("revision"),
                "item_count":row.get::<i64,_>("item_count"),
                "extraction_status":row.get::<Option<String>,_>("extraction_status"),
                "partial":row.get::<Option<bool>,_>("partial")
            })
        })
        .collect();
    Ok(ok(json!({"voice_captures":captures})))
}

async fn extraction_permission(State(state): State<AppState>, headers: HeaderMap) -> ApiResult {
    let id = owner(&state, &headers, true).await?;
    let row = sqlx::query("SELECT enabled,generation,disclosure_version FROM transcript_extraction_permissions WHERE account_id=$1")
        .bind(id).fetch_optional(&state.pool).await?;
    Ok(ok(json!({"transcript_extraction":{
        "enabled":row.as_ref().map(|r| r.get::<bool,_>("enabled")).unwrap_or(false),
        "generation":row.as_ref().map(|r| r.get::<i64,_>("generation")).unwrap_or(0),
        "disclosure_version":row.as_ref().and_then(|r| r.get::<Option<i32>,_>("disclosure_version")),
        "provider":"openai",
        "model":EXTRACTION_MODEL,
        "provider_available":state.extractor.available()
    }})))
}

async fn set_extraction_permission(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> ApiResult {
    let id = owner(&state, &headers, true).await?;
    let input: VoicePermissionInput = parse(&body)?;
    if input.enabled && input.disclosure_version != Some(EXTRACTION_DISCLOSURE_VERSION) {
        return Err(ApiError::bad(
            "Current transcript-processing disclosure is required",
        ));
    }
    if input.enabled && !state.extractor.available() {
        return Err(ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "extraction_unavailable",
            "Transcript extraction is not configured",
        ));
    }
    let mut tx = state.pool.begin().await?;
    let row = sqlx::query(
        "INSERT INTO transcript_extraction_permissions(account_id,enabled,generation,disclosure_version) VALUES ($1,$2,1,$3) \
         ON CONFLICT (account_id) DO UPDATE SET \
         generation=CASE WHEN transcript_extraction_permissions.enabled IS DISTINCT FROM EXCLUDED.enabled \
          OR (EXCLUDED.enabled AND transcript_extraction_permissions.disclosure_version IS DISTINCT FROM EXCLUDED.disclosure_version) \
          THEN transcript_extraction_permissions.generation+1 ELSE transcript_extraction_permissions.generation END, \
         enabled=EXCLUDED.enabled, \
         disclosure_version=CASE WHEN EXCLUDED.enabled THEN EXCLUDED.disclosure_version ELSE transcript_extraction_permissions.disclosure_version END, \
         updated_at=now() RETURNING generation")
        .bind(id).bind(input.enabled).bind(if input.enabled { Some(EXTRACTION_DISCLOSURE_VERSION) } else { None })
        .fetch_one(&mut *tx).await?;
    if !input.enabled {
        sqlx::query("UPDATE transcript_extraction_jobs SET status='cancelled',updated_at=now() WHERE account_id=$1 AND status='processing'")
            .bind(id).execute(&mut *tx).await?;
    }
    let generation: i64 = row.try_get("generation")?;
    tx.commit().await?;
    Ok(ok(
        json!({"transcript_extraction":{"enabled":input.enabled,"generation":generation,"acknowledged":true}}),
    ))
}

async fn extraction_items(
    pool: &PgPool,
    owner_id: Uuid,
    capture_id: Uuid,
    partial: bool,
) -> ApiResult {
    let items: Vec<ItemRow> = sqlx::query_as("SELECT id,capture_id,subject,body,visibility,revision,created_at FROM knowledge_items WHERE owner_id=$1 AND capture_id=$2 AND deleted_at IS NULL ORDER BY created_at,id")
        .bind(owner_id).bind(capture_id).fetch_all(pool).await?;
    Ok(ok(
        json!({"capture_id":capture_id,"items":items.iter().map(item_json).collect::<Vec<_>>(),"partial":partial}),
    ))
}

async fn fail_extraction_attempt(
    pool: &PgPool,
    owner_id: Uuid,
    capture_id: Uuid,
    attempt_id: Uuid,
) {
    let _ = sqlx::query("UPDATE transcript_extraction_jobs SET status='failed',updated_at=now() WHERE account_id=$1 AND capture_id=$2 AND attempt_id=$3 AND status='processing'")
        .bind(owner_id).bind(capture_id).bind(attempt_id).execute(pool).await;
}

async fn extract_voice_capture(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult {
    let owner_id = owner(&state, &headers, true).await?;
    let capture_id = uuid(&id)?;
    if !state.extractor.available() {
        return Err(ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "extraction_unavailable",
            "Transcript extraction is not configured",
        ));
    }
    let attempt_id = Uuid::new_v4();
    let mut tx = state.pool.begin().await?;
    let permission = sqlx::query("SELECT enabled,generation FROM transcript_extraction_permissions WHERE account_id=$1 FOR UPDATE")
        .bind(owner_id).fetch_optional(&mut *tx).await?;
    let generation = permission
        .as_ref()
        .filter(|r| r.get::<bool, _>("enabled"))
        .map(|r| r.get::<i64, _>("generation"))
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::FORBIDDEN,
                "extraction_permission_required",
                "Allow transcript processing before extracting knowledge",
            )
        })?;
    let withdrawn =
        sqlx::query("SELECT 1 FROM processing_permissions WHERE account_id=$1 AND enabled=false")
            .bind(owner_id)
            .fetch_optional(&mut *tx)
            .await?
            .is_some();
    if withdrawn {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "processing_withdrawn",
            "Processing was turned off",
        ));
    }
    let source = sqlx::query("SELECT s.content,s.revision FROM source_texts s JOIN captures c ON c.id=s.capture_id AND c.owner_id=s.owner_id WHERE c.id=$1 AND c.owner_id=$2 AND c.kind='voice' AND s.kind='transcript' FOR UPDATE OF s")
        .bind(capture_id).bind(owner_id).fetch_optional(&mut *tx).await?
        .ok_or_else(|| ApiError::not_found("Private transcript not found"))?;
    let transcript: String = source.try_get("content")?;
    if transcript.chars().count() > 6_000 {
        return Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "extraction_too_long",
            "Transcript is too long for this pilot; private text remains available",
        ));
    }
    let source_revision: i32 = source.try_get("revision")?;
    let job = sqlx::query("SELECT status,attempts,lease_until,partial,source_revision FROM transcript_extraction_jobs WHERE capture_id=$1 AND account_id=$2 FOR UPDATE")
        .bind(capture_id).bind(owner_id).fetch_optional(&mut *tx).await?;
    if let Some(job) = &job {
        let status: String = job.try_get("status")?;
        if status == "completed" {
            let partial = job.get::<Option<bool>, _>("partial").unwrap_or(false);
            tx.commit().await?;
            return extraction_items(&state.pool, owner_id, capture_id, partial).await;
        }
        if status == "cancelled" || job.get::<i32, _>("source_revision") != source_revision {
            return Err(ApiError::conflict(
                "Extraction source changed; review transcript",
            ));
        }
        if job.get::<i32, _>("attempts") >= 3 {
            return Err(ApiError::new(
                StatusCode::TOO_MANY_REQUESTS,
                "extraction_retry_limit",
                "Extraction retry limit reached",
            ));
        }
        if status == "processing" && job.get::<DateTime<Utc>, _>("lease_until") > Utc::now() {
            return Err(ApiError::conflict("Transcript is already being processed"));
        }
        sqlx::query("UPDATE transcript_extraction_jobs SET status='processing',attempt_id=$3,permission_generation=$4,attempts=attempts+1,lease_until=now()+interval '3 minutes',updated_at=now() WHERE capture_id=$1 AND account_id=$2")
            .bind(capture_id).bind(owner_id).bind(attempt_id).bind(generation).execute(&mut *tx).await?;
    } else {
        sqlx::query("SELECT pg_advisory_xact_lock(732783)")
            .execute(&mut *tx)
            .await?;
        let account_count: i64 = sqlx::query_scalar("SELECT count(*) FROM transcript_extraction_jobs WHERE account_id=$1 AND created_at>now()-interval '24 hours'")
            .bind(owner_id).fetch_one(&mut *tx).await?;
        let global_count: i64 = sqlx::query_scalar("SELECT count(*) FROM transcript_extraction_jobs WHERE created_at>now()-interval '24 hours'")
            .fetch_one(&mut *tx).await?;
        if account_count >= 12 || global_count >= 100 {
            return Err(ApiError::new(
                StatusCode::TOO_MANY_REQUESTS,
                "extraction_budget_reached",
                "Transcript processing limit reached",
            ));
        }
        sqlx::query("INSERT INTO transcript_extraction_jobs(capture_id,account_id,source_revision,permission_generation,attempt_id,status,lease_until) VALUES ($1,$2,$3,$4,$5,'processing',now()+interval '3 minutes')")
            .bind(capture_id).bind(owner_id).bind(source_revision).bind(generation).bind(attempt_id).execute(&mut *tx).await?;
    }
    tx.commit().await?;

    let proposal = match state.extractor.extract(&transcript).await {
        Ok(value) => value,
        Err(_) => {
            fail_extraction_attempt(&state.pool, owner_id, capture_id, attempt_id).await;
            return Err(ApiError::new(
                StatusCode::BAD_GATEWAY,
                "extraction_failed",
                "Knowledge extraction failed; private transcript remains",
            ));
        }
    };
    let (items, partial) = match validate(proposal.clone(), &transcript).or_else(|_| {
        preserve_unresolved(proposal, &transcript).ok_or(crate::extraction::ExtractionError::Failed)
    }) {
        Ok(value) => value,
        Err(_) => {
            fail_extraction_attempt(&state.pool, owner_id, capture_id, attempt_id).await;
            return Err(ApiError::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                "extraction_unusable",
                "No grounded recommendation was returned; private transcript remains",
            ));
        }
    };
    let mut tx = state.pool.begin().await?;
    let permission = sqlx::query("SELECT enabled,generation FROM transcript_extraction_permissions WHERE account_id=$1 FOR UPDATE")
        .bind(owner_id).fetch_one(&mut *tx).await?;
    let source = sqlx::query("SELECT revision FROM source_texts WHERE capture_id=$1 AND owner_id=$2 AND kind='transcript' FOR UPDATE")
        .bind(capture_id).bind(owner_id).fetch_optional(&mut *tx).await?;
    let job = sqlx::query("SELECT status,attempt_id FROM transcript_extraction_jobs WHERE capture_id=$1 AND account_id=$2 FOR UPDATE")
        .bind(capture_id).bind(owner_id).fetch_one(&mut *tx).await?;
    let withdrawn =
        sqlx::query("SELECT 1 FROM processing_permissions WHERE account_id=$1 AND enabled=false")
            .bind(owner_id)
            .fetch_optional(&mut *tx)
            .await?
            .is_some();
    if !permission.get::<bool, _>("enabled")
        || permission.get::<i64, _>("generation") != generation
        || withdrawn
        || source.as_ref().map(|r| r.get::<i32, _>("revision")) != Some(source_revision)
        || job.get::<String, _>("status") != "processing"
        || job.get::<Uuid, _>("attempt_id") != attempt_id
    {
        return Err(ApiError::conflict(
            "Processing permission or transcript changed before knowledge could be saved",
        ));
    }
    for item in items {
        sqlx::query("INSERT INTO knowledge_items(id,capture_id,owner_id,subject,body,visibility) VALUES ($1,$2,$3,$4,$5,'private')")
            .bind(Uuid::new_v4()).bind(capture_id).bind(owner_id).bind(item.subject).bind(item.body)
            .execute(&mut *tx).await?;
    }
    sqlx::query("UPDATE captures SET status=$2,revision=revision+1 WHERE id=$1 AND owner_id=$3")
        .bind(capture_id)
        .bind(if partial { "partial" } else { "completed" })
        .bind(owner_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE transcript_extraction_jobs SET status='completed',partial=$3,updated_at=now() WHERE capture_id=$1 AND account_id=$2")
        .bind(capture_id).bind(owner_id).bind(partial).execute(&mut *tx).await?;
    tx.commit().await?;
    extraction_items(&state.pool, owner_id, capture_id, partial).await
}

async fn voice_result(pool: &PgPool, owner_id: Uuid, capture_id: Uuid) -> ApiResult {
    let row = sqlx::query(
        "SELECT s.content FROM source_texts s JOIN captures c ON c.id=s.capture_id \
         WHERE c.id=$1 AND c.owner_id=$2 AND c.kind='voice' AND s.owner_id=$2 AND s.kind='transcript'",
    )
    .bind(capture_id)
    .bind(owner_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::not_found("Transcript was deleted"))?;
    Ok(ok(json!({"capture":{
        "id":capture_id,
        "status":"transcript_ready",
        "transcript":row.get::<String,_>("content")
    },"server_audio_retained":false})))
}

async fn fail_voice_attempt(pool: &PgPool, owner_id: Uuid, draft_id: &str, attempt_id: Uuid) {
    let _ = sqlx::query(
        "UPDATE voice_transcription_jobs SET status='failed',updated_at=now() \
         WHERE account_id=$1 AND draft_id=$2 AND attempt_id=$3 AND status='processing'",
    )
    .bind(owner_id)
    .bind(draft_id)
    .bind(attempt_id)
    .execute(pool)
    .await;
}

async fn transcribe_voice(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(draft_id): Path<String>,
    audio: Bytes,
) -> ApiResult {
    let owner_id = owner(&state, &headers, true).await?;
    if !(16..=64).contains(&draft_id.len())
        || !draft_id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    {
        return Err(ApiError::bad("Invalid voice draft ID"));
    }
    let captured_ms = header(&headers, "x-captured-at-ms")
        .and_then(|value| value.parse::<i64>().ok())
        .ok_or_else(|| ApiError::bad("Capture timestamp is required"))?;
    let now_ms = Utc::now().timestamp_millis();
    if captured_ms > now_ms + 5 * 60 * 1000 || captured_ms <= 0 {
        return Err(ApiError::bad("Invalid capture timestamp"));
    }
    if captured_ms < now_ms - 7 * 24 * 60 * 60 * 1000 {
        return Err(ApiError::new(
            StatusCode::GONE,
            "voice_expired",
            "Voice draft is no longer eligible for transcription",
        ));
    }
    if header(&headers, "content-type").and_then(|value| value.split(';').next())
        != Some("audio/mp4")
        || !(128..=5 * 1024 * 1024).contains(&audio.len())
        || audio.get(4..8) != Some(b"ftyp".as_slice())
    {
        return Err(ApiError::bad("A valid M4A recording is required"));
    }
    if !state.transcriber.available() {
        return Err(ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "voice_unavailable",
            "Voice transcription is not configured",
        ));
    }
    let audio_hash = hex::encode(Sha256::digest(&audio));
    let attempt_id = Uuid::new_v4();
    let mut tx = state.pool.begin().await?;
    let permission = sqlx::query(
        "SELECT enabled,generation FROM voice_transcription_permissions WHERE account_id=$1 FOR UPDATE",
    )
    .bind(owner_id)
    .fetch_optional(&mut *tx)
    .await?;
    let generation = permission
        .as_ref()
        .filter(|row| row.get::<bool, _>("enabled"))
        .map(|row| row.get::<i64, _>("generation"))
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::FORBIDDEN,
                "voice_permission_required",
                "Allow voice transcription before uploading audio",
            )
        })?;
    let withdrawn =
        sqlx::query("SELECT 1 FROM processing_permissions WHERE account_id=$1 AND enabled=false")
            .bind(owner_id)
            .fetch_optional(&mut *tx)
            .await?
            .is_some();
    if withdrawn {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "processing_withdrawn",
            "Processing was turned off",
        ));
    }
    let existing =
        sqlx::query("SELECT 1 FROM voice_transcription_jobs WHERE account_id=$1 AND draft_id=$2")
            .bind(owner_id)
            .bind(&draft_id)
            .fetch_optional(&mut *tx)
            .await?
            .is_some();
    if !existing {
        // Reserve a bounded pilot budget before any paid provider dispatch.
        sqlx::query("SELECT pg_advisory_xact_lock(732782)")
            .execute(&mut *tx)
            .await?;
        let account_count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM voice_transcription_jobs WHERE account_id=$1 AND created_at>now()-interval '24 hours'",
        )
        .bind(owner_id)
        .fetch_one(&mut *tx)
        .await?;
        let global_count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM voice_transcription_jobs WHERE created_at>now()-interval '24 hours'",
        )
        .fetch_one(&mut *tx)
        .await?;
        if account_count >= 12 || global_count >= 100 {
            return Err(ApiError::new(
                StatusCode::TOO_MANY_REQUESTS,
                "voice_budget_reached",
                "Voice processing limit reached; keep this draft and try later",
            ));
        }
    }
    sqlx::query(
        "INSERT INTO voice_transcription_jobs(account_id,draft_id,audio_sha256,permission_generation,attempt_id,status,lease_until) \
         VALUES ($1,$2,$3,$4,$5,'processing',now()+interval '3 minutes') ON CONFLICT DO NOTHING",
    )
    .bind(owner_id)
    .bind(&draft_id)
    .bind(&audio_hash)
    .bind(generation)
    .bind(attempt_id)
    .execute(&mut *tx)
    .await?;
    let job = sqlx::query(
        "SELECT audio_sha256,status,lease_until,capture_id,attempt_id,attempts FROM voice_transcription_jobs \
         WHERE account_id=$1 AND draft_id=$2 FOR UPDATE",
    )
    .bind(owner_id)
    .bind(&draft_id)
    .fetch_one(&mut *tx)
    .await?;
    if job.get::<String, _>("audio_sha256") != audio_hash {
        return Err(ApiError::conflict("Draft ID belongs to different audio"));
    }
    let status: String = job.try_get("status")?;
    if status == "completed" {
        let capture_id: Uuid = job
            .try_get::<Option<Uuid>, _>("capture_id")?
            .ok_or_else(|| {
                ApiError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "server_error",
                    "Capture is missing",
                )
            })?;
        tx.commit().await?;
        return voice_result(&state.pool, owner_id, capture_id).await;
    }
    if status == "cancelled" {
        return Err(ApiError::conflict("Withdrawn draft cannot be retried"));
    }
    if job.get::<Uuid, _>("attempt_id") != attempt_id {
        if job.get::<i32, _>("attempts") >= 3 {
            return Err(ApiError::new(
                StatusCode::TOO_MANY_REQUESTS,
                "voice_retry_limit",
                "Voice retry limit reached; keep or delete this draft",
            ));
        }
        if status == "processing" && job.get::<DateTime<Utc>, _>("lease_until") > Utc::now() {
            return Err(ApiError::new(
                StatusCode::CONFLICT,
                "voice_busy",
                "This draft is already being transcribed",
            ));
        }
        sqlx::query(
            "UPDATE voice_transcription_jobs SET status='processing',attempt_id=$3,permission_generation=$4,attempts=attempts+1, \
             lease_until=now()+interval '3 minutes',updated_at=now() WHERE account_id=$1 AND draft_id=$2",
        )
        .bind(owner_id)
        .bind(&draft_id)
        .bind(attempt_id)
        .bind(generation)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    let transcript = match state.transcriber.transcribe(audio.to_vec()).await {
        Ok(text) => text.trim().to_owned(),
        Err(_) => {
            fail_voice_attempt(&state.pool, owner_id, &draft_id, attempt_id).await;
            return Err(ApiError::new(
                StatusCode::BAD_GATEWAY,
                "transcription_failed",
                "Transcription failed; your audio remains on this device",
            ));
        }
    };
    if transcript.chars().count() > 20_000 || !transcript.chars().any(char::is_alphabetic) {
        fail_voice_attempt(&state.pool, owner_id, &draft_id, attempt_id).await;
        return Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "unusable_transcript",
            "No usable transcript was returned; your audio remains on this device",
        ));
    }

    let mut tx = state.pool.begin().await?;
    let permission = sqlx::query(
        "SELECT enabled,generation FROM voice_transcription_permissions WHERE account_id=$1 FOR UPDATE",
    )
    .bind(owner_id)
    .fetch_one(&mut *tx)
    .await?;
    let job = sqlx::query(
        "SELECT status,attempt_id FROM voice_transcription_jobs WHERE account_id=$1 AND draft_id=$2 FOR UPDATE",
    )
    .bind(owner_id)
    .bind(&draft_id)
    .fetch_one(&mut *tx)
    .await?;
    let withdrawn =
        sqlx::query("SELECT 1 FROM processing_permissions WHERE account_id=$1 AND enabled=false")
            .bind(owner_id)
            .fetch_optional(&mut *tx)
            .await?
            .is_some();
    if !permission.get::<bool, _>("enabled")
        || withdrawn
        || permission.get::<i64, _>("generation") != generation
        || job.get::<String, _>("status") != "processing"
        || job.get::<Uuid, _>("attempt_id") != attempt_id
    {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "voice_permission_changed",
            "Voice permission changed before transcription could be saved",
        ));
    }
    let capture_id = Uuid::new_v4();
    sqlx::query("INSERT INTO captures(id,owner_id,kind,status,desired_visibility) VALUES ($1,$2,'voice','transcript_ready','private')")
        .bind(capture_id).bind(owner_id).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO source_texts(id,capture_id,owner_id,kind,content) VALUES ($1,$2,$3,'transcript',$4)")
        .bind(Uuid::new_v4()).bind(capture_id).bind(owner_id).bind(&transcript)
        .execute(&mut *tx).await?;
    sqlx::query("UPDATE voice_transcription_jobs SET status='completed',capture_id=$3,updated_at=now() WHERE account_id=$1 AND draft_id=$2")
        .bind(owner_id).bind(&draft_id).bind(capture_id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(created(json!({"capture":{
        "id":capture_id,"status":"transcript_ready","transcript":transcript
    },"server_audio_retained":false})))
}

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
    let siblings: i64 = sqlx::query_scalar("SELECT count(*) FROM knowledge_items WHERE capture_id=$1 AND owner_id=$2 AND deleted_at IS NULL")
        .bind(capture_id).bind(owner_id).fetch_one(&mut *transaction).await?;
    if siblings == 0 {
        sqlx::query("DELETE FROM source_texts WHERE capture_id=$1 AND owner_id=$2")
            .bind(capture_id)
            .bind(owner_id)
            .execute(&mut *transaction)
            .await?;
    }
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
