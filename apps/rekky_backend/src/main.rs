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
    let state = AppState {
        pool,
        verifier,
        transcriber,
        extractor,
    };
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
    Ok(())
}
