//! Explicit local maintenance command; never calls a provider or scans accounts.
//! Usage: classify_existing ITEM_UUID EXPECTED_REVISION TYPE_ID [FACET_ID ...]
use rekky_backend::{
    extraction::SourceUnit,
    taxonomy::{self, Assignment, ProposedClassification},
};
use serde_json::Value;
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        return Err("Expected item UUID, revision and category IDs".into());
    }
    let id = Uuid::parse_str(&args[0])?;
    let revision: i32 = args[1].parse()?;
    let pool = PgPool::connect(&std::env::var("DATABASE_URL")?).await?;
    let mut tx = pool.begin().await?;
    let row = sqlx::query("SELECT i.recommendation,s.support FROM knowledge_items i JOIN item_source_support s ON s.item_id=i.id WHERE i.id=$1 AND i.revision=$2 AND i.deleted_at IS NULL FOR UPDATE OF i,s")
        .bind(id).bind(revision).fetch_optional(&mut *tx).await?.ok_or("Item/evidence missing or revision changed")?;
    let mut rec: Value = row.get("recommendation");
    if rec["classification"].is_object() {
        return Err(
            "Already classified; preserve the existing assignment or user correction".into(),
        );
    }
    let mut support: Value = row.get("support");
    let units: Vec<SourceUnit> = serde_json::from_value(support["units"].clone())?;
    let mut proposal = ProposedClassification::default();
    for category_id in &args[2..] {
        let c = taxonomy::concept(category_id).ok_or("Unknown category ID")?;
        let (unit, phrase) = units
            .iter()
            .find_map(|u| {
                c.aliases.iter().find_map(|alias| {
                    let words = taxonomy::words(&u.text);
                    let needle = taxonomy::words(alias);
                    (!needle.is_empty() && words.windows(needle.len()).any(|w| w == needle))
                        .then_some((u, alias))
                })
            })
            .ok_or("No literal supporting phrase in this item's saved evidence")?;
        let assignment = Assignment {
            concept_id: c.id.clone(),
            source_phrase: phrase.clone(),
            evidence: vec![unit.id],
        };
        if c.dimension == "type" {
            proposal.types.push(assignment);
        } else {
            proposal.facets.push(assignment);
        }
    }
    let (classification, evidence) =
        taxonomy::validate(&proposal, rec["entity_kind"].as_str().unwrap_or(""), &units);
    let actual: Vec<_> = classification["types"]
        .as_array()
        .unwrap()
        .iter()
        .chain(classification["facets"].as_array().unwrap())
        .filter_map(|v| v["id"].as_str())
        .collect();
    if args[2..].iter().any(|id| !actual.contains(&id.as_str())) {
        return Err("Category was not supported; no change committed".into());
    }
    rec["classification"] = classification;
    rec["classification"]["origin"] = serde_json::json!("source_backfill");
    support["classification"] = evidence;
    sqlx::query("UPDATE knowledge_items SET recommendation=$1,revision=revision+1 WHERE id=$2")
        .bind(&rec)
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE item_source_support SET support=$1 WHERE item_id=$2")
        .bind(support)
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    println!(
        "Updated category to {} at revision {} (audience, source and copy unchanged)",
        rec["classification"]["display_label"],
        revision + 1
    );
    Ok(())
}
