//! Question dictation is temporary and cannot create captures, sources or recommendations.
use crate::app::{ApiError, ApiResult, AppState, ok, owner};
use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::Row;
use std::time::Duration;
use uuid::Uuid;

async fn permission(state: &AppState, id: Uuid) -> Result<i64, ApiError> {
    sqlx::query_scalar("SELECT generation FROM voice_transcription_permissions p WHERE account_id=$1 AND enabled AND NOT EXISTS(SELECT 1 FROM processing_permissions g WHERE g.account_id=p.account_id AND NOT g.enabled)")
        .bind(id).fetch_optional(&state.pool).await?.ok_or_else(||ApiError::new(StatusCode::FORBIDDEN,"voice_permission_required","Turn on voice processing in settings to speak a question."))
}

// Validate the bounded M4A container duration before spending a transcription call.
// Only the app's single AAC recording format is supported; never accept a client duration header.
fn duration_ms(bytes: &[u8]) -> Option<u64> {
    fn boxes(bytes: &[u8], depth: u8) -> Option<u64> {
        if depth > 4 {
            return None;
        }
        let mut offset = 0;
        let mut duration = None;
        while offset < bytes.len() {
            let head = bytes.get(offset..offset + 8)?;
            let short_size = u32::from_be_bytes(head[0..4].try_into().ok()?);
            let (size, header_size) = match short_size {
                0 => (bytes.len() - offset, 8),
                1 => (
                    usize::try_from(u64::from_be_bytes(
                        bytes.get(offset + 8..offset + 16)?.try_into().ok()?,
                    ))
                    .ok()?,
                    16,
                ),
                n => (n as usize, 8),
            };
            if size < header_size {
                return None;
            }
            let end = offset.checked_add(size)?;
            let payload = bytes.get(offset + header_size..end)?;
            match &head[4..8] {
                b"moov" | b"trak" | b"mdia" => {
                    if let Some(value) = boxes(payload, depth + 1) {
                        duration = Some(duration.unwrap_or(0).max(value));
                    }
                }
                b"mvhd" | b"mdhd" => {
                    let version = *payload.first()?;
                    let (scale, units) = match version {
                        0 => (
                            u32::from_be_bytes(payload.get(12..16)?.try_into().ok()?) as u64,
                            u32::from_be_bytes(payload.get(16..20)?.try_into().ok()?) as u64,
                        ),
                        1 => (
                            u32::from_be_bytes(payload.get(20..24)?.try_into().ok()?) as u64,
                            u64::from_be_bytes(payload.get(24..32)?.try_into().ok()?),
                        ),
                        _ => return None,
                    };
                    if scale == 0 {
                        return None;
                    }
                    duration = Some(duration.unwrap_or(0).max(units.checked_mul(1000)? / scale));
                }
                _ => {}
            }
            offset = end;
        }
        duration
    }
    boxes(bytes, 0)
}

pub async fn transcribe(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    audio: Bytes,
) -> ApiResult {
    let account = owner(&state, &headers, true).await?;
    let generation = permission(&state, account).await?;
    if headers.get("content-type").and_then(|v| v.to_str().ok()) != Some("audio/mp4")
        || !(128..=600_000).contains(&audio.len())
        || audio.get(4..8) != Some(b"ftyp".as_slice())
        || !duration_ms(&audio).is_some_and(|d| (300..=61_000).contains(&d))
    {
        return Err(ApiError::bad("Record a question of up to one minute."));
    }
    let hash = hex::encode(Sha256::digest(&audio));
    let mut tx = state.pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(827714)")
        .execute(&mut *tx)
        .await?;
    if let Some(row)=sqlx::query("SELECT owner_id,audio_hash,status,transcript,permission_generation,expires_at>now() valid FROM ask_dictations WHERE id=$1").bind(id).fetch_optional(&mut *tx).await? {
        if row.get::<Uuid,_>("owner_id")!=account || row.get::<String,_>("audio_hash")!=hash { return Err(ApiError::conflict("This recording is no longer available. Record again.")); }
        if row.get::<String,_>("status")=="completed" && row.get::<bool,_>("valid") && row.get::<i64,_>("permission_generation")==generation {
            return Ok(ok(json!({"request_id":id,"text":row.get::<Option<String>,_>("transcript")})));
        }
        return Err(ApiError::new(StatusCode::CONFLICT,"dictation_unavailable","This recording is still processing or is no longer available. Type your question or record again."));
    }
    if !state.transcriber.available() {
        return Err(ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "voice_unavailable",
            "Voice is unavailable. You can still type your question.",
        ));
    }
    let row=sqlx::query("SELECT count(*) total,count(*) FILTER(WHERE owner_id=$1) personal,count(*) FILTER(WHERE owner_id=$1 AND status='running' AND created_at>now()-interval '45 seconds') active FROM ask_dictations WHERE charged AND created_at>now()-interval '24 hours'").bind(account).fetch_one(&mut *tx).await?;
    if row.get::<i64, _>("total") >= 300
        || row.get::<i64, _>("personal") >= 30
        || row.get::<i64, _>("active") >= 1
    {
        return Err(ApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "dictation_limit",
            "Voice questions are busy or today's allowance is reached. You can still type.",
        ));
    }
    sqlx::query("INSERT INTO ask_dictations(id,owner_id,audio_hash,status,permission_generation) VALUES($1,$2,$3,'running',$4)").bind(id).bind(account).bind(hash).bind(generation).execute(&mut *tx).await?;
    tx.commit().await?;
    let result = tokio::time::timeout(
        Duration::from_secs(35),
        state.transcriber.transcribe(audio.to_vec()),
    )
    .await;
    drop(audio);
    if owner(&state, &headers, true).await.ok() != Some(account)
        || permission(&state, account).await.ok() != Some(generation)
    {
        sqlx::query("UPDATE ask_dictations SET status='cancelled',transcript=NULL WHERE id=$1")
            .bind(id)
            .execute(&state.pool)
            .await?;
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "processing_changed",
            "Voice processing changed. Type your question or try again.",
        ));
    }
    let text = match result {
        Ok(Ok(text)) if (1..=500).contains(&text.trim().chars().count()) => {
            Some(text.trim().to_owned())
        }
        _ => None,
    };
    let status = if text.is_some() {
        "completed"
    } else {
        "failed"
    };
    let updated = sqlx::query(
        "UPDATE ask_dictations SET status=$2,transcript=$3 WHERE id=$1 AND status='running'",
    )
    .bind(id)
    .bind(status)
    .bind(&text)
    .execute(&state.pool)
    .await?
    .rows_affected();
    if updated == 0 || text.is_none() {
        return Err(ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "dictation_failed",
            "Couldn’t transcribe this question. Try a shorter recording or type it.",
        ));
    }
    Ok(ok(json!({"request_id":id,"text":text})))
}

pub async fn cancel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> ApiResult {
    let account = owner(&state, &headers, true).await?;
    let mut tx = state.pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(827714)")
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "UPDATE ask_dictations SET status='cancelled',transcript=NULL WHERE id=$1 AND owner_id=$2",
    )
    .bind(id)
    .bind(account)
    .execute(&mut *tx)
    .await?;
    sqlx::query("INSERT INTO ask_dictations(id,owner_id,audio_hash,status,permission_generation,charged) SELECT $1,$2,'cancelled','cancelled',0,false WHERE (SELECT count(*) FROM ask_dictations WHERE owner_id=$2 AND created_at>now()-interval '24 hours')<100 ON CONFLICT(id) DO NOTHING").bind(id).bind(account).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(ok(json!({"cancelled":true})))
}
pub async fn sweep(pool: &sqlx::PgPool) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE ask_dictations SET transcript=NULL,status=CASE WHEN status='running' THEN 'failed' ELSE status END WHERE (transcript IS NOT NULL OR status='running') AND (expires_at<now() OR (status='running' AND created_at<now()-interval '45 seconds') OR NOT EXISTS(SELECT 1 FROM voice_transcription_permissions p WHERE p.account_id=ask_dictations.owner_id AND p.enabled AND p.generation=ask_dictations.permission_generation))").execute(pool).await?;
    sqlx::query("DELETE FROM ask_dictations WHERE created_at<now()-interval '30 days'")
        .execute(pool)
        .await?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duration_rejects_missing_truncated_and_long_headers() {
        assert_eq!(duration_ms(b"not audio"), None);
        let mut b = vec![0u8; 36];
        b[0..4].copy_from_slice(&36u32.to_be_bytes());
        b[4..8].copy_from_slice(b"mvhd");
        b[20..24].copy_from_slice(&1000u32.to_be_bytes());
        b[24..28].copy_from_slice(&60000u32.to_be_bytes());
        assert_eq!(duration_ms(&b), Some(60000));
        assert_eq!(duration_ms(&b[..30]), None);
        b[20..24].fill(0);
        assert_eq!(duration_ms(&b), None);
    }
}
