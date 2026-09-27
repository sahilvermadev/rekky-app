//! Synthetic generic phrases only; no account access or registry mutations.
use rekky_backend::{category_learning::*, taxonomy};
use serde_json::json;
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let learner = OpenAiCategoryLearner::from_env();
    let mut rows = vec![];
    let cases = if std::env::args().any(|a| a == "--final") {
        vec![
            ("clock repairer", "person_service"),
            ("puzzle box", "thing"),
            ("birdwatching walk", "activity_event"),
            ("cab provider", "person_service"),
            ("airport pickup", "person_service"),
            ("patient and helpful", "person_service"),
        ]
    } else if std::env::args().any(|a| a == "--heldout") {
        vec![
            ("violin repairer", "person_service"),
            ("patient and helpful", "person_service"),
        ]
    } else {
        vec![
            ("fountain pen restorer", "person_service"),
            ("cab provider", "person_service"),
            ("airport pickup", "person_service"),
        ]
    };
    for (term, kind) in cases {
        let start = std::time::Instant::now();
        let proposal = learner.propose(term, kind, taxonomy::vocabulary()).await;
        let output = match proposal {
            Ok(p) => {
                let check = check_definition(&p, term, kind, taxonomy::vocabulary());
                let review = if check.is_ok() {
                    learner
                        .review(term, kind, &p, taxonomy::vocabulary())
                        .await
                        .ok()
                } else {
                    None
                };
                json!({"proposal":p,"deterministic_result":check,"review":review})
            }
            Err(_) => json!({"error":"provider_failed"}),
        };
        rows.push(json!({"term":term,"entity_kind":kind,"elapsed_ms":start.elapsed().as_millis(),"output":output}));
    }
    println!("{}", serde_json::to_string_pretty(&rows)?);
    Ok(())
}
