use async_trait::async_trait;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{env, time::Duration};

pub const EXTRACTION_DISCLOSURE_VERSION: i32 = 1;
pub const EXTRACTION_MODEL: &str = "gpt-4.1-mini";

#[derive(Debug)]
pub enum ExtractionError {
    Unavailable,
    Failed,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProposedItem {
    pub subject: String,
    pub evidence: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Proposal {
    pub items: Vec<ProposedItem>,
}

#[async_trait]
pub trait TranscriptExtractor: Send + Sync {
    fn available(&self) -> bool;
    async fn extract(&self, transcript: &str) -> Result<Proposal, ExtractionError>;
}

pub struct OpenAiExtractor {
    key: Option<String>,
    enabled: bool,
    client: reqwest::Client,
}

impl OpenAiExtractor {
    pub fn from_env() -> Self {
        Self {
            key: env::var("OPENAI_API_KEY")
                .ok()
                .filter(|value| !value.is_empty()),
            enabled: env::var("OPENAI_EXTRACTION_ENABLED").as_deref() == Ok("true"),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(75))
                .build()
                .expect("HTTP client should initialize"),
        }
    }
}

fn schema() -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":["items"],
        "properties":{
            "items":{
                "type":"array",
                "items":{
                    "type":"object",
                    "additionalProperties":false,
                    "required":["subject","evidence"],
                    "properties":{
                        "subject":{"type":"string"},
                        "evidence":{"type":"array","items":{"type":"string"}}
                    }
                }
            }
        }
    })
}

#[async_trait]
impl TranscriptExtractor for OpenAiExtractor {
    fn available(&self) -> bool {
        self.enabled && self.key.is_some()
    }

    async fn extract(&self, transcript: &str) -> Result<Proposal, ExtractionError> {
        if !self.available() {
            return Err(ExtractionError::Unavailable);
        }
        let response = self
            .client
            .post("https://api.openai.com/v1/responses")
            .bearer_auth(self.key.as_ref().ok_or(ExtractionError::Unavailable)?)
            .json(&json!({
                "model":EXTRACTION_MODEL,
                "store":false,
                "max_output_tokens":2000,
                "input":[
                    {"role":"system","content":"Extract useful knowledge from a private voice transcript. Return at most five separate subjects. For each subject, give the shortest exact source phrase that names it and one or more contiguous evidence passages from the transcript containing every material claim, caveat, and example about it. Copy each passage word-for-word from the source; punctuation and whitespace may differ, but no word may be added, removed, or changed. If unsure, copy a longer complete passage instead of paraphrasing or using ellipses. Do not invent or infer a location or create a subject absent from the source. Separate distinct people, places, things, and tips. If nothing useful is present, return an empty items array. Return JSON matching the schema."},
                    {"role":"user","content":transcript}
                ],
                "text":{"format":{"type":"json_schema","name":"rekky_extraction_v1","strict":true,"schema":schema()}}
            }))
            .send()
            .await
            .map_err(|_| ExtractionError::Failed)?;
        if !response.status().is_success() {
            return Err(ExtractionError::Failed);
        }
        let payload: Value = response.json().await.map_err(|_| ExtractionError::Failed)?;
        if payload["status"] != "completed" {
            return Err(ExtractionError::Failed);
        }
        let text = payload["output"]
            .as_array()
            .and_then(|items| {
                items.iter().find_map(|item| {
                    item["content"]
                        .as_array()
                        .and_then(|parts| parts.iter().find(|part| part["type"] == "output_text"))
                        .and_then(|part| part["text"].as_str())
                })
            })
            .ok_or(ExtractionError::Failed)?;
        serde_json::from_str(text).map_err(|_| ExtractionError::Failed)
    }
}

#[derive(Clone, Debug)]
pub struct ValidatedItem {
    pub subject: String,
    pub body: String,
}

fn evidence_span(source: &str, quote: &str) -> Option<(usize, usize)> {
    if let Some(start) = source.find(quote) {
        return Some((start, start + quote.len()));
    }
    let words = Regex::new(r"[\p{L}\p{M}\p{N}]+").expect("word regex");
    let source_tokens: Vec<_> = words
        .find_iter(source)
        .map(|m| (m.as_str().to_lowercase(), m.start(), m.end()))
        .collect();
    let quote_tokens: Vec<_> = words
        .find_iter(quote)
        .map(|m| m.as_str().to_lowercase())
        .collect();
    if quote_tokens.is_empty() {
        return None;
    }
    source_tokens
        .windows(quote_tokens.len())
        .find(|window| window.iter().map(|token| &token.0).eq(&quote_tokens))
        .map(|window| (window.first().unwrap().1, window.last().unwrap().2))
}

pub fn validate(
    proposal: Proposal,
    source: &str,
) -> Result<(Vec<ValidatedItem>, bool), ExtractionError> {
    if proposal.items.is_empty() || proposal.items.len() > 5 {
        return Err(ExtractionError::Failed);
    }
    let mut items = Vec::with_capacity(proposal.items.len());
    let mut spans = Vec::new();
    for proposed in proposal.items {
        let subject = proposed.subject.trim();
        if subject.is_empty()
            || subject.chars().count() > 120
            || proposed.evidence.is_empty()
            || proposed.evidence.len() > 8
        {
            return Err(ExtractionError::Failed);
        }
        let mut quotes = Vec::new();
        for quote in proposed.evidence {
            let quote = quote.trim();
            if quote.chars().count() < 3 {
                return Err(ExtractionError::Failed);
            }
            let (start, end) = evidence_span(source, quote).ok_or(ExtractionError::Failed)?;
            spans.push((start, end));
            quotes.push(source[start..end].to_owned());
        }
        let body = quotes.join(" ");
        if body.chars().count() > 20_000 || !body.to_lowercase().contains(&subject.to_lowercase()) {
            return Err(ExtractionError::Failed);
        }
        items.push(ValidatedItem {
            subject: subject.to_owned(),
            body,
        });
    }
    let mut covered = vec![false; source.len()];
    for (start, end) in spans {
        covered[start..end].fill(true);
    }
    let material_bytes = source
        .char_indices()
        .filter(|(_, c)| c.is_alphanumeric())
        .count();
    let covered_material = source
        .char_indices()
        .filter(|(i, c)| c.is_alphanumeric() && covered[*i])
        .count();
    let partial = material_bytes > 0 && covered_material * 100 < material_bytes * 80;
    Ok((items, partial))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grounded_quotes_are_saved_and_omissions_mark_partial() {
        let source = "Ravi fixed the tap carefully. Meera teaches swimming near us.";
        let proposal = Proposal {
            items: vec![ProposedItem {
                subject: "Ravi".into(),
                evidence: vec!["Ravi fixed the tap carefully.".into()],
            }],
        };
        let (items, partial) = validate(proposal, source).unwrap();
        assert_eq!(items[0].body, "Ravi fixed the tap carefully.");
        assert!(partial);
    }

    #[test]
    fn fabricated_evidence_or_subject_is_rejected() {
        let source = "Ravi fixed the tap.";
        let proposal = Proposal {
            items: vec![ProposedItem {
                subject: "Ravi".into(),
                evidence: vec!["Ravi fixed the tap for free.".into()],
            }],
        };
        assert!(validate(proposal, source).is_err());
        let proposal = Proposal {
            items: vec![ProposedItem {
                subject: "Meera".into(),
                evidence: vec![source.into()],
            }],
        };
        assert!(validate(proposal, source).is_err());
    }

    #[test]
    fn punctuation_variations_recover_the_original_words_only() {
        let source = "Ravi fixed the kitchen tap—carefully.";
        let proposal = Proposal {
            items: vec![ProposedItem {
                subject: "Ravi".into(),
                evidence: vec!["ravi fixed the kitchen tap - carefully".into()],
            }],
        };
        let (items, _) = validate(proposal, source).unwrap();
        assert_eq!(items[0].body, "Ravi fixed the kitchen tap—carefully");
    }
}
