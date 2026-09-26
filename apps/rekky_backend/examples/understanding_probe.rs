//! Explicit synthetic-only provider probe. Never loads account/user content.
use rekky_backend::extraction::{OpenAiExtractor, TranscriptExtractor, validate};
use serde_json::{Value, json};
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let seed = if std::env::args().any(|arg| arg == "--taxonomy") {
        include_str!("../../../docs/evaluation/taxonomy_seed_v1.json")
    } else {
        include_str!("../../../docs/evaluation/understanding_v2_seed.json")
    };
    let cases: Vec<Value> = serde_json::from_str(seed)?;
    let extractor = OpenAiExtractor::from_env();
    let mut report = Vec::new();
    for case in cases {
        let start = std::time::Instant::now();
        let result = extractor
            .extract(case["transcript"].as_str().unwrap())
            .await;
        let output = match result {
            Ok(proposal) => {
                match validate(proposal.clone(), case["transcript"].as_str().unwrap()) {
                    Ok((items, partial)) => {
                        json!({"status":"valid","partial":partial,"items":items.iter().map(|i|json!({"subject":i.subject,"recommendation":i.recommendation,"proposed_classification":i.evidence["proposal"]["classification"]})).collect::<Vec<_>>()})
                    }
                    Err(_) => json!({"status":"validation_failed","synthetic_proposal":proposal}),
                }
            }
            Err(_) => json!({"status":"provider_failed"}),
        };
        report.push(
            json!({"id":case["id"],"elapsed_ms":start.elapsed().as_millis(),"output":output}),
        );
    }
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
