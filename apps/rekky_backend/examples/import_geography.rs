//! Import the prepared, licensed gazetteer, atomically publishing its revision.
use sqlx::postgres::PgPoolOptions;
use std::io::{BufRead, BufReader};
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let path = std::env::args().nth(1).ok_or("Pass prepared areas.jsonl")?;
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&std::env::var("DATABASE_URL")?)
        .await?;
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(732789)")
        .execute(&mut *tx)
        .await?;
    sqlx::query("CREATE TEMP TABLE geographic_import_seen(id text PRIMARY KEY) ON COMMIT DROP")
        .execute(&mut *tx)
        .await?;
    let mut batch = Vec::new();
    let mut count = 0;
    for line in BufReader::new(std::fs::File::open(path)?)
        .lines()
        .chain(std::iter::once(Ok(String::new())))
    {
        let line = line?;
        if !line.is_empty() {
            batch.push(serde_json::from_str::<rekky_backend::geography::Area>(
                &line,
            )?);
        }
        if batch.len() >= 1000 || line.is_empty() {
            sqlx::query("INSERT INTO geographic_areas SELECT id,name,label,country,feature,population,aliases,ancestors,hierarchy FROM jsonb_to_recordset($1) AS a(id text,name text,label text,country text,feature text,population bigint,aliases text[],ancestors text[],hierarchy jsonb) ON CONFLICT(id) DO UPDATE SET name=excluded.name,label=excluded.label,country=excluded.country,feature=excluded.feature,population=excluded.population,aliases=excluded.aliases,ancestors=excluded.ancestors,hierarchy=excluded.hierarchy")
                .bind(serde_json::to_value(&batch)?).execute(&mut *tx).await?;
            sqlx::query("INSERT INTO geographic_import_seen SELECT unnest($1::text[]) ON CONFLICT DO NOTHING")
                .bind(batch.iter().map(|a| a.id.clone()).collect::<Vec<_>>()).execute(&mut *tx).await?;
            count += batch.len();
            batch.clear();
        }
    }
    if count == 0 {
        return Err("Empty gazetteer".into());
    }
    sqlx::query(
        "UPDATE geographic_catalog SET revision=revision+1,imported_at=now() WHERE singleton",
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    println!("Imported {count} geographic areas; local enrichment will resume.");
    Ok(())
}
