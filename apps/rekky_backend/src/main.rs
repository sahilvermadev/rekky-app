use rekky_backend::{
    AppState, auth::OidcVerifier, extraction::OpenAiExtractor, migrate, router,
    voice::OpenAiTranscriber,
};
use sqlx::postgres::PgPoolOptions;
use std::{env, net::SocketAddr, sync::Arc};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let database_url = env::var("DATABASE_URL")?;
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await?;
    if env::args().nth(1).as_deref() == Some("migrate") {
        migrate::run(&pool).await?;
        return Ok(());
    }
    let host = env::var("HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let port: u16 = env::var("PORT").unwrap_or_else(|_| "3088".into()).parse()?;
    let address: SocketAddr = format!("{host}:{port}").parse()?;
    let verifier = Arc::new(OidcVerifier::from_env());
    let transcriber = Arc::new(OpenAiTranscriber::from_env());
    let extractor = Arc::new(OpenAiExtractor::from_env());
    let daily_account_limit: i64 = env::var("PILOT_DAILY_ACCOUNT_LIMIT")
        .unwrap_or_else(|_| "12".into())
        .parse()?;
    if !(1..=100).contains(&daily_account_limit) {
        return Err("PILOT_DAILY_ACCOUNT_LIMIT must be between 1 and 100".into());
    }
    let state = AppState {
        daily_account_limit,
        pool,
        verifier,
        transcriber,
        extractor,
        places: Arc::new(rekky_backend::places::GooglePlaces::from_env()),
    };
    let learning_pool = state.pool.clone();
    let learning_worker = tokio::spawn(async move {
        let learner = rekky_backend::category_learning::OpenAiCategoryLearner::from_env();
        loop {
            if rekky_backend::category_learning::process_one(&learning_pool, &learner)
                .await
                .is_err()
            {
                // Never log private candidate phrases or provider payloads.
                eprintln!("Category learning worker failed; retry remains bounded");
            }
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    });
    let listener = tokio::net::TcpListener::bind(address).await?;
    let worker_state = state.clone();
    let worker = tokio::spawn(async move {
        loop {
            if let Err(error) = rekky_backend::app::process_pending_voice(&worker_state, None).await
            {
                eprintln!("Voice worker database error: {error}");
            }
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    });
    println!("Rekky backend listening on {address}");
    axum::serve(listener, router(state))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    worker.abort();
    learning_worker.abort();
    Ok(())
}
