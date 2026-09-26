use sqlx::PgPool;

const MIGRATIONS: &[(&str, &str)] = &[
    (
        "001_foundation.sql",
        include_str!("../migrations/001_foundation.sql"),
    ),
    (
        "002_owner_consistency.sql",
        include_str!("../migrations/002_owner_consistency.sql"),
    ),
    (
        "003_processing_permission.sql",
        include_str!("../migrations/003_processing_permission.sql"),
    ),
    (
        "004_voice_transcription.sql",
        include_str!("../migrations/004_voice_transcription.sql"),
    ),
    (
        "005_voice_attempt_cap.sql",
        include_str!("../migrations/005_voice_attempt_cap.sql"),
    ),
    (
        "006_transcript_extraction.sql",
        include_str!("../migrations/006_transcript_extraction.sql"),
    ),
];

pub async fn run(pool: &PgPool) -> Result<(), sqlx::Error> {
    let mut transaction = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(732781)")
        .execute(&mut *transaction)
        .await?;
    sqlx::query("CREATE TABLE IF NOT EXISTS schema_migrations (name text PRIMARY KEY, applied_at timestamptz NOT NULL DEFAULT now())")
        .execute(&mut *transaction).await?;
    for (name, script) in MIGRATIONS {
        let exists = sqlx::query("SELECT 1 FROM schema_migrations WHERE name=$1")
            .bind(name)
            .fetch_optional(&mut *transaction)
            .await?
            .is_some();
        if !exists {
            sqlx::raw_sql(*script).execute(&mut *transaction).await?;
            sqlx::query("INSERT INTO schema_migrations(name) VALUES ($1)")
                .bind(name)
                .execute(&mut *transaction)
                .await?;
            println!("Applied {name}");
        }
    }
    transaction.commit().await?;
    Ok(())
}
