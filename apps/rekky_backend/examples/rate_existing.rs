//! Explicit, two-step, single-item maintenance: never scans or rewrites prose.
//! Prepare: rate_existing ITEM_UUID EXPECTED_REVISION /private/new-output.json
//! Apply:   rate_existing --apply /private/new-output.json
//! Preparation makes at most one understanding request. Review before applying.
use rekky_backend::{
    extraction::{OpenAiExtractor, TranscriptExtractor, source_units},
    ratings,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use std::{fs::OpenOptions, io::Write, os::unix::fs::OpenOptionsExt};
use uuid::Uuid;

const SNAPSHOT: &str = "SELECT i.subject,i.revision,i.recommendation,i.owner_id,s.id source_id,s.revision source_revision,s.content,p.generation FROM knowledge_items i JOIN source_texts s ON s.capture_id=i.capture_id AND s.owner_id=i.owner_id JOIN item_source_support e ON e.item_id=i.id AND e.source_id=s.id AND e.source_revision=s.revision JOIN transcript_extraction_permissions p ON p.account_id=i.owner_id WHERE i.id=$1 AND i.deleted_at IS NULL AND p.enabled AND p.disclosure_version=1 AND NOT (e.support ? 'superseded_by_user_revision') AND (SELECT count(*) FROM knowledge_items siblings WHERE siblings.capture_id=i.capture_id AND siblings.deleted_at IS NULL)=1 FOR UPDATE OF i,s,e,p";
fn digest(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}
fn name(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}
fn check(row: &sqlx::postgres::PgRow, revision: i32) -> Result<(), Box<dyn std::error::Error>> {
    let rec: Value = row.get("recommendation");
    if row.get::<i32, _>("revision") != revision
        || rec["origin"] == "user"
        || !rec["rating"].is_null()
    {
        return Err("Changed, owner-edited or already rated; no action taken".into());
    }
    Ok(())
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let args: Vec<_> = std::env::args().skip(1).collect();
    let pool = PgPool::connect(&std::env::var("DATABASE_URL")?).await?;
    if args.len() == 2 && args[0] == "--apply" {
        let saved: Value = serde_json::from_str(&std::fs::read_to_string(&args[1])?)?;
        let id = Uuid::parse_str(saved["id"].as_str().ok_or("Missing id")?)?;
        let revision = i32::try_from(saved["revision"].as_i64().ok_or("Missing revision")?)?;
        let mut tx = pool.begin().await?;
        let row = sqlx::query(SNAPSHOT)
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or("Source, permission or item unavailable")?;
        check(&row, revision)?;
        let source: String = row.get("content");
        if saved["source_id"] != row.get::<Uuid, _>("source_id").to_string()
            || saved["source_revision"] != row.get::<i32, _>("source_revision")
            || saved["permission_generation"] != row.get::<i64, _>("generation")
            || saved["source_hash"] != digest(&source)
        {
            return Err("Source or permission changed; no write".into());
        }
        let rec: Value = row.get("recommendation");
        let rating = ratings::validate(
            &saved["proposal"],
            rec["experience"].as_str().unwrap_or(""),
            &source_units(&source),
        );
        if rating.is_null() || rating != saved["rating"] {
            return Err("Rating no longer validates".into());
        }
        sqlx::query("UPDATE knowledge_items SET recommendation=jsonb_set(recommendation,'{rating}',$1),revision=revision+1 WHERE id=$2")
            .bind(&rating).bind(id).execute(&mut *tx).await?;
        sqlx::query("UPDATE item_source_support SET support=jsonb_set(support,'{rating_backfill}',$1) WHERE item_id=$2")
            .bind(json!({"model":rekky_backend::extraction::EXTRACTION_MODEL,"rubric_version":ratings::RUBRIC_VERSION,"proposal":saved["proposal"],"item_revision":revision+1,"source_revision":saved["source_revision"],"requested_at":saved["requested_at"]}))
            .bind(id).execute(&mut *tx).await?;
        tx.commit().await?;
        println!(
            "{}: {}/10 at revision {}",
            row.get::<String, _>("subject"),
            rating["value"],
            revision + 1
        );
    } else if args.len() == 3 {
        let id = Uuid::parse_str(&args[0])?;
        let revision: i32 = args[1].parse()?;
        // Fail before spending if the output already exists; private local evidence only.
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&args[2])?;
        let row = sqlx::query(SNAPSHOT)
            .bind(id)
            .fetch_optional(&pool)
            .await?
            .ok_or("Source, permission or item unavailable")?;
        check(&row, revision)?;
        let source: String = row.get("content");
        if source.chars().count() > 6000 {
            return Err("Source exceeds the understanding limit".into());
        }
        let subject: String = row.get("subject");
        let extractor = OpenAiExtractor::from_env();
        let proposed = extractor
            .extract(&source)
            .await
            .map_err(|_| "Understanding request failed; no retry")?;
        if proposed.items.len() != 1 || name(&proposed.items[0].subject) != name(&subject) {
            return Err("Ambiguous or changed subject; no write".into());
        }
        let rec: Value = row.get("recommendation");
        let proposal = &proposed.items[0].rating;
        let rating = ratings::validate(
            proposal,
            rec["experience"].as_str().unwrap_or(""),
            &source_units(&source),
        );
        if rating.is_null() {
            return Err("No supported overall rating; no write".into());
        }
        let saved = json!({"id":id,"revision":revision,"source_id":row.get::<Uuid,_>("source_id"),"source_revision":row.get::<i32,_>("source_revision"),"permission_generation":row.get::<i64,_>("generation"),"source_hash":digest(&source),"requested_at":chrono::Utc::now(),"proposal":proposal,"rating":rating});
        file.write_all(serde_json::to_string_pretty(&saved)?.as_bytes())?;
        file.sync_all()?;
        println!(
            "Prepared {}: {}/10; no database changes",
            subject, rating["value"]
        );
    } else {
        return Err("Usage: ITEM_UUID REVISION NEW_PRIVATE_FILE or --apply PRIVATE_FILE".into());
    }
    Ok(())
}
