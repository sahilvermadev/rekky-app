//! Explicit operator repair of one omitted type; no transcript provider call.
//! Only source-backed, unclassified, unedited items with current evidence qualify.
use rekky_backend::{
    category_learning,
    extraction::SourceUnit,
    taxonomy::{self, Descriptor, ProposedClassification},
};
use serde_json::{Value, json};
use sqlx::{PgPool, Row};
use uuid::Uuid;
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 4 {
        return Err(
            "Usage: repair_missing_type ITEM_ID EXPECTED_REVISION SOURCE_TYPE_PHRASE UNIT_ID"
                .into(),
        );
    }
    let id = Uuid::parse_str(&args[0])?;
    let revision: i32 = args[1].parse()?;
    let unit: usize = args[3].parse()?;
    let pool = PgPool::connect(&std::env::var("DATABASE_URL")?).await?;
    let catalog = category_learning::catalog(&pool).await?;
    let mut tx = pool.begin().await?;
    let row=sqlx::query("SELECT k.recommendation,s.support FROM knowledge_items k JOIN item_source_support s ON s.item_id=k.id JOIN source_texts t ON t.id=s.source_id AND t.revision=s.source_revision JOIN transcript_extraction_permissions p ON p.account_id=k.owner_id AND p.enabled WHERE k.id=$1 AND k.revision=$2 AND k.deleted_at IS NULL AND NOT EXISTS(SELECT 1 FROM processing_permissions x WHERE x.account_id=k.owner_id AND NOT x.enabled) FOR UPDATE OF p,k,t,s")
        .bind(id).bind(revision).fetch_optional(&mut *tx).await?.ok_or("Missing/stale item or processing disabled")?;
    let mut rec: Value = row.get("recommendation");
    let mut support: Value = row.get("support");
    if rec["origin"] == "user"
        || rec["classification"]["origin"] == "user"
        || rec["classification"]["types"]
            .as_array()
            .is_none_or(|a| !a.is_empty())
        || rec["classification"]["descriptive_type"]
            .as_str()
            .is_some_and(|s| !s.is_empty())
    {
        return Err("Preserve existing owner/category choices".into());
    }
    let units: Vec<SourceUnit> = serde_json::from_value(support["units"].clone())?;
    let mut proposal: ProposedClassification =
        serde_json::from_value(support["proposal"]["classification"].clone())?;
    proposal.type_description = Some(Descriptor {
        text: args[2].clone(),
        evidence: vec![unit],
    });
    let (classification, evidence) = taxonomy::validate_in(
        &catalog,
        &proposal,
        rec["entity_kind"].as_str().ok_or("Missing kind")?,
        &units,
    );
    if classification["descriptive_type"] != args[2] {
        return Err("Type phrase is not supported by this item's evidence".into());
    }
    rec["classification"] = classification;
    support["classification"] = evidence;
    support["category_repair"] =
        json!({"reason":"missing_type_description","source_phrase":args[2],"unit_id":unit});
    sqlx::query("UPDATE knowledge_items SET recommendation=$2,revision=revision+1 WHERE id=$1")
        .bind(id)
        .bind(rec)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE item_source_support SET support=$2 WHERE item_id=$1")
        .bind(id)
        .bind(support)
        .execute(&mut *tx)
        .await?;
    category_learning::enqueue(&mut tx, id).await?;
    tx.commit().await?;
    println!(
        "Repaired the missing source-backed type and queued normal learning; content and audience preserved."
    );
    Ok(())
}
