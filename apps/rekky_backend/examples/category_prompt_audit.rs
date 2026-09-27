//! Controlled synthetic ablation. Never reads the app database or user notes.
//! Pass the pre-change prompt file; baseline schema is reconstructed explicitly.
use rekky_backend::{
    extraction::{self, Proposal},
    taxonomy,
};
use serde_json::{Value, json};
use std::{
    fs,
    time::{Duration, Instant},
};
fn old_schema(free_phrase: bool) -> Value {
    let mut s = extraction::schema();
    for (field, is_type) in [("types", true), ("facets", false)] {
        let cs: Vec<_> = taxonomy::vocabulary()
            .concepts
            .iter()
            .filter(|c| (c.dimension == "type") == is_type)
            .collect();
        let phrase = if free_phrase {
            json!({"type":"string"})
        } else {
            json!({"type":"string","enum":cs.iter().flat_map(|c|c.aliases.iter()).collect::<Vec<_>>()})
        };
        s["properties"]["items"]["items"]["properties"]["classification"]["properties"][field] = json!({"type":"array","items":{"type":"object","additionalProperties":false,"required":["concept_id","evidence","source_phrase"],"properties":{"concept_id":{"type":"string","enum":cs.iter().map(|c|&c.id).collect::<Vec<_>>()},"evidence":{"type":"array","items":{"type":"integer"}},"source_phrase":phrase}}});
    }
    s
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let baseline = fs::read_to_string(std::env::args().nth(1).ok_or("Pass baseline prompt path")?)?;
    let final_prompt = include_str!("../prompts/understanding_v2.txt");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(75))
        .build()?;
    let key = std::env::var("OPENAI_API_KEY")?;
    let cases = [
        (
            "caterer",
            "Asha is a caterer. She catered a birthday lunch for me. The guests enjoyed the food. She can cook several cuisines. Her prices seemed reasonable. She is based in Pune.",
        ),
        (
            "bookbinder",
            "I used Dev, a bookbinder. He repaired my old notebook neatly. The cover feels sturdy.",
        ),
    ];
    let mut report = vec![];
    for repeat in 0..2 {
        for (name, source) in cases {
            for arm in [
                "baseline",
                "free_source_phrase",
                "prompt_only",
                "schema_only",
                "combined",
            ] {
                let revised_prompt = ["prompt_only", "combined"].contains(&arm);
                let wrapper = if revised_prompt {
                    "Known category IDs and aliases. Use these only when supported; unfamiliar explicit types belong in type_description with types=[], never in a guessed ID:"
                } else {
                    "Shared category vocabulary (use canonical IDs, not invented labels):"
                };
                let prompt = if revised_prompt {
                    final_prompt
                } else {
                    baseline.as_str()
                };
                let schema = match arm {
                    "schema_only" | "combined" => {
                        extraction::schema_for_source(taxonomy::vocabulary(), source)
                    }
                    "free_source_phrase" => old_schema(true),
                    _ => old_schema(false),
                };
                let request = json!({"model":extraction::EXTRACTION_MODEL,"store":false,"max_output_tokens":5500,"input":[{"role":"system","content":format!("{prompt}\n{wrapper}\n{}",serde_json::to_string(taxonomy::vocabulary())?)},{"role":"user","content":json!({"transcript_units":extraction::source_units(source)}).to_string()}],"text":{"format":{"type":"json_schema","name":"rekky_understanding_v2","strict":true,"schema":schema}}});
                let start = Instant::now();
                let response = client
                    .post("https://api.openai.com/v1/responses")
                    .bearer_auth(&key)
                    .json(&request)
                    .send()
                    .await?;
                let http = response.status().as_u16();
                let payload: Value = response.json().await?;
                let raw = payload["output"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .flat_map(|o| o["content"].as_array().into_iter().flatten())
                    .find(|c| c["type"] == "output_text")
                    .and_then(|c| c["text"].as_str());
                let parsed = raw.and_then(|t| serde_json::from_str::<Proposal>(t).ok());
                let output = match parsed {
                    Some(p) => {
                        let classes:Vec<_>=p.items.iter().map(|i|json!({"subject":i.subject,"kind":i.entity_kind,"classification":i.classification})).collect();
                        let checked = extraction::validate(p, source).ok();
                        json!({"proposals":classes,"partial":checked.as_ref().map(|x|x.1),"saved_classifications":checked.map(|x|x.0.into_iter().map(|i|i.recommendation["classification"].clone()).collect::<Vec<_>>())})
                    }
                    None => {
                        json!({"error_type":payload["error"]["type"],"error_code":payload["error"]["code"],"status":payload["status"]})
                    }
                };
                report.push(json!({"case":name,"repeat":repeat,"arm":arm,"elapsed_ms":start.elapsed().as_millis(),"http":http,"returned_model":payload["model"],"usage":payload["usage"],"output":output}));
                eprintln!("Finished {name} {arm} repeat {repeat}");
            }
        }
    }
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"baseline_commit":"5ab18a5","cases":cases,"runs":report})
        )?
    );
    Ok(())
}
