//! Operator-only maintenance. Does not print candidate phrases or private notes.
use rekky_backend::category_learning;
use sqlx::PgPool;
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3 || !["suspend", "suspend-alias"].contains(&args[1].as_str()) {
        return Err(
            "Usage: category_admin suspend LEARNED_CATEGORY_ID | suspend-alias PHRASE".into(),
        );
    }
    let pool = PgPool::connect(&std::env::var("DATABASE_URL")?).await?;
    if args[1] == "suspend" {
        category_learning::suspend(&pool, &args[2]).await?;
    } else {
        category_learning::suspend_alias(&pool, &args[2]).await?;
    }
    println!("Category suspended for future use; existing recommendations retained.");
    Ok(())
}
