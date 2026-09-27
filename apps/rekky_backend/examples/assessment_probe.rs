//! Synthetic-only development probe; does not read user data or create items.
use rekky_backend::{
    extraction::{ExtractionError, OpenAiExtractor, Proposal, TranscriptExtractor},
    taxonomy,
    understanding::{assess, merge_repair},
};
use serde_json::{Value, json};
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let mut cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../docs/evaluation/understanding_v2_seed.json"
    ))?;
    cases.extend([
        json!({"id":"restaurant_normalization","transcript":"I ate at Harbor Oven, a pizza restaurant in Mysuru, in Lakshmipuram. I loved the mushroom pizza and the friendly staff. I think I paid about seven hundred rupees for two pizzas. A great spot for a quiet date, although the upstairs room is not wheelchair accessible."}),
        json!({"id":"unfamiliar_restorer","transcript":"Mira is a fountain pen restorer in Kochi. She repaired the nib on my old pen last month and it writes smoothly now. I paid 400 rupees. She took three weeks, so I would avoid her for an urgent repair."})
    ]);
    let args: Vec<_> = std::env::args().collect();
    let replay: Option<Value> = if args.get(1).map(String::as_str) == Some("--replay") {
        Some(serde_json::from_str(&std::fs::read_to_string(
            args.get(2).ok_or("Missing replay file")?,
        )?)?)
    } else {
        None
    };
    let extractor = OpenAiExtractor::from_env();
    let mut results = Vec::new();
    for case in cases {
        if let Ok(only) = std::env::var("PROBE_CASES")
            && !only.split(',').any(|id| Some(id) == case["id"].as_str())
        {
            continue;
        }
        let source = case["transcript"].as_str().unwrap();
        let context = taxonomy::for_source(taxonomy::vocabulary(), source);
        let start = std::time::Instant::now();
        let saved = replay
            .as_ref()
            .and_then(|r| r["results"].as_array())
            .and_then(|rows| rows.iter().find(|r| r["id"] == case["id"]));
        let response: Result<Proposal, ExtractionError> = if let Some(row) = saved {
            serde_json::from_value(row["initial_proposal"].clone())
                .map_err(|_| ExtractionError::InvalidJson)
        } else if replay.is_some() {
            Err(ExtractionError::Unavailable)
        } else {
            extractor.extract_with_catalog(source, &context).await
        };
        match response {
            Ok(p) => {
                let initial = assess(&p, source, taxonomy::vocabulary());
                let mut outcome = initial.clone();
                let mut repair_count = 0;
                let mut repair_proposal = Value::Null;
                if initial.partial() {
                    repair_count = 1;
                    let repaired = if let Some(row) = saved {
                        serde_json::from_value(row["repair_proposal"].clone())
                            .map_err(|_| ExtractionError::InvalidJson)
                    } else if replay.is_some() {
                        Err(ExtractionError::Unavailable)
                    } else {
                        extractor
                            .repair(source, &context, &p, &initial.issues)
                            .await
                    };
                    if let Ok(r) = repaired {
                        repair_proposal = serde_json::to_value(&r)?;
                        let merged = merge_repair(&p, r, &initial);
                        let candidate = assess(&merged, source, taxonomy::vocabulary());
                        if rekky_backend::understanding::improves(&outcome, &candidate) {
                            outcome = candidate;
                        }
                    }
                }
                results.push(json!({"id":case["id"],"elapsed_ms":start.elapsed().as_millis(),"initial_proposal":p,"repair_proposal":repair_proposal,"initial_issues":initial.issues,"repair_count":repair_count,"partial":outcome.partial(),"issues":outcome.issues,"items":outcome.items.iter().map(|i|json!({"subject":i.subject,"recommendation":i.recommendation})).collect::<Vec<_>>() }));
            }
            Err(e) => results.push(json!({"id":case["id"],"error":e.code()})),
        }
    }
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"scope":"Synthetic development probe, not held-out or audio evaluation", "offline_replay":replay.is_some(), "results":results})
        )?
    );
    Ok(())
}
