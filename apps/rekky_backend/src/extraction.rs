use async_trait::async_trait;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::HashSet, env, time::Duration};

pub const EXTRACTION_DISCLOSURE_VERSION: i32 = 1;
pub const EXTRACTION_MODEL: &str = "gpt-4.1-mini";
pub const UNDERSTANDING_VERSION: i32 = 2;

#[derive(Debug)]
pub enum ExtractionError {
    Unavailable,
    Failed,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Claim {
    pub text: String,
    pub evidence: Vec<usize>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    pub kind: String,
    pub text: String,
    pub evidence: Vec<usize>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Location {
    pub role: String,
    pub text: String,
    pub evidence: Vec<usize>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProposedItem {
    pub subject: String,
    pub subject_evidence: Vec<usize>,
    pub entity_kind: String,
    pub experience: String,
    pub summary: Claim,
    pub observations: Vec<Observation>,
    pub locations: Vec<Location>,
    pub use_cases: Vec<Claim>,
    #[serde(default)]
    pub classification: Value,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Proposal {
    pub items: Vec<ProposedItem>,
    pub ignored_unit_ids: Vec<usize>,
    pub unresolved_unit_ids: Vec<usize>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SourceUnit {
    pub id: usize,
    pub text: String,
    pub start: usize,
    pub end: usize,
}

/// Stable UTF-8 byte spans supplied by the server, never invented by a model.
pub fn source_units(source: &str) -> Vec<SourceUnit> {
    let mut spans = Vec::new();
    let mut start = 0;
    for (i, c) in source.char_indices() {
        if !['.', '!', '?', '\n', '।'].contains(&c) {
            continue;
        }
        if c == '.' {
            let prefix = &source[start..i];
            let last = prefix
                .split_whitespace()
                .last()
                .unwrap_or("")
                .to_lowercase();
            let decimal = source[..i]
                .chars()
                .next_back()
                .is_some_and(|v| v.is_ascii_digit())
                && source[i + 1..]
                    .chars()
                    .next()
                    .is_some_and(|v| v.is_ascii_digit());
            if decimal || ["dr", "mr", "mrs", "ms", "st", "prof", "rs"].contains(&last.as_str()) {
                continue;
            }
        }
        let end = i + c.len_utf8();
        spans.push((start, end));
        start = end;
    }
    if start < source.len() {
        spans.push((start, source.len()));
    }
    spans
        .into_iter()
        .filter_map(|(start, end)| {
            let raw = &source[start..end];
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return None;
            }
            let start = start + raw.len() - raw.trim_start().len();
            Some((start, start + trimmed.len(), trimmed.to_owned()))
        })
        .enumerate()
        .map(|(index, (start, end, text))| SourceUnit {
            id: index + 1,
            text,
            start,
            end,
        })
        .collect()
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
            key: env::var("OPENAI_API_KEY").ok().filter(|v| !v.is_empty()),
            enabled: env::var("OPENAI_EXTRACTION_ENABLED").as_deref() == Ok("true"),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(75))
                .build()
                .expect("HTTP client should initialize"),
        }
    }
}
fn object(properties: Value) -> Value {
    let required: Vec<_> = properties.as_object().unwrap().keys().cloned().collect();
    json!({"type":"object","additionalProperties":false,"required":required,"properties":properties})
}
fn text_schema() -> Value {
    json!({"type":"string"})
}
fn evidence_schema() -> Value {
    json!({"type":"array","items":{"type":"integer"}})
}
fn array(items: Value) -> Value {
    json!({"type":"array","items":items})
}
fn choice(values: &[&str]) -> Value {
    json!({"type":"string","enum":values})
}
pub fn schema() -> Value {
    let claim = object(json!({"text":text_schema(),"evidence":evidence_schema()}));
    let assignment = |is_type| {
        object(
            json!({"concept_id":{"type":"string","enum":crate::taxonomy::vocabulary().concepts.iter().filter(|c| (c.dimension == "type") == is_type).map(|c|&c.id).collect::<Vec<_>>()},"source_phrase":text_schema(),"evidence":evidence_schema()}),
        )
    };
    object(json!({
        "items":array(object(json!({
            "subject":text_schema(),"subject_evidence":evidence_schema(),
            "entity_kind":choice(&["place","person_service","thing","activity_event","idea_tip"]),
            "experience":choice(&["firsthand","secondhand","interest","unspecified"]),
            "summary":claim,
            "observations":array(object(json!({"kind":choice(&["praise","suggestion","suitability","caution","price","context"]),"text":text_schema(),"evidence":evidence_schema()}))),
            "locations":array(object(json!({"role":choice(&["venue","practice","service_area","past_experience","context"]),"text":text_schema(),"evidence":evidence_schema()}))),
            "use_cases":array(claim.clone()),
            "classification":object(json!({"types":array(assignment(true)),"facets":array(assignment(false)),"descriptors":array(claim)}))
        }))),
        "ignored_unit_ids":evidence_schema(),"unresolved_unit_ids":evidence_schema()
    }))
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
        let response = self.client.post("https://api.openai.com/v1/responses")
            .bearer_auth(self.key.as_ref().ok_or(ExtractionError::Unavailable)?)
            .json(&json!({
                "model":EXTRACTION_MODEL,"store":false,"max_output_tokens":5500,
                "input":[
                    {"role":"system","content":format!("{}\nShared category vocabulary (use canonical IDs, not invented labels):\n{}",include_str!("../prompts/understanding_v2.txt"),serde_json::to_string(crate::taxonomy::vocabulary()).expect("vocabulary"))},
                    {"role":"user","content":json!({"transcript_units":source_units(transcript)}).to_string()}
                ],
                "text":{"format":{"type":"json_schema","name":"rekky_understanding_v2","strict":true,"schema":schema()}}
            })).send().await.map_err(|_| ExtractionError::Failed)?;
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
    /// Public-safe presentation fields: no full transcript or sibling evidence.
    pub recommendation: Value,
    /// Owner-only source support, never returned in item/search responses.
    pub evidence: Value,
}
fn words(text: &str) -> Vec<String> {
    Regex::new(r"[\p{L}\p{M}\p{N}]+")
        .unwrap()
        .find_iter(text)
        .map(|m| m.as_str().to_lowercase())
        .collect()
}
fn phrase_in(text: &str, source: &str) -> bool {
    let needle = words(text);
    let haystack = words(source);
    !needle.is_empty() && haystack.windows(needle.len()).any(|part| part == needle)
}
fn support(ids: &[usize], units: &[SourceUnit]) -> Result<String, ExtractionError> {
    if ids.is_empty() || ids.len() > 32 {
        return Err(ExtractionError::Failed);
    }
    ids.iter()
        .map(|id| {
            units
                .get(id.wrapping_sub(1))
                .map(|u| u.text.as_str())
                .ok_or(ExtractionError::Failed)
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|parts| parts.join(" "))
}
fn check_claim(
    text: &str,
    ids: &[usize],
    units: &[SourceUnit],
    limit: usize,
) -> Result<(), ExtractionError> {
    if text.trim().is_empty() || text.chars().count() > limit {
        return Err(ExtractionError::Failed);
    }
    let source = support(ids, units)?;
    let numbers = Regex::new(r"\p{N}+(?:[.,]\p{N}+)*").unwrap();
    let supported: HashSet<_> = numbers.find_iter(&source).map(|m| m.as_str()).collect();
    if numbers
        .find_iter(text)
        .any(|m| !supported.contains(m.as_str()))
    {
        return Err(ExtractionError::Failed);
    }
    if ['₹', '$', '€', '£']
        .iter()
        .any(|symbol| text.contains(*symbol) && !source.contains(*symbol))
    {
        return Err(ExtractionError::Failed);
    }
    // Displayed links must not be fabricated or become implied verified actions.
    let links = Regex::new(r"(?i)https?://\S+|www\.\S+|\S+@\S+").unwrap();
    if links.find_iter(text).any(|m| !source.contains(m.as_str())) {
        return Err(ExtractionError::Failed);
    }
    Ok(())
}
fn safely_ignored(unit: &SourceUnit, units: &[SourceUnit]) -> bool {
    let normalized = words(&unit.text);
    let fillers = [
        "um", "uh", "erm", "hmm", "okay", "ok", "you", "know", "i", "mean",
    ];
    (!normalized.is_empty() && normalized.iter().all(|w| fillers.contains(&w.as_str())))
        || units
            .iter()
            .any(|other| other.id < unit.id && words(&other.text) == normalized)
}

/// Checks source references, representation coverage and conservative literal
/// invariants. These checks do NOT prove semantic entailment of a paraphrase.
pub fn validate(
    proposal: Proposal,
    source: &str,
) -> Result<(Vec<ValidatedItem>, bool), ExtractionError> {
    let units = source_units(source);
    if proposal.items.is_empty() || proposal.items.len() > 5 || units.is_empty() {
        return Err(ExtractionError::Failed);
    }
    let mut accounted = HashSet::new();
    let mut partial = !proposal.unresolved_unit_ids.is_empty();
    for id in &proposal.unresolved_unit_ids {
        support(&[*id], &units)?;
    }
    for id in &proposal.ignored_unit_ids {
        let unit = units
            .get(id.wrapping_sub(1))
            .ok_or(ExtractionError::Failed)?;
        if safely_ignored(unit, &units) {
            accounted.insert(*id);
        } else {
            partial = true;
        }
    }
    let mut items = Vec::new();
    let practice_pattern =
        Regex::new(r"(?i)\b(practic(?:e|es|ing)|clinic|office|based|works from)\b").unwrap();
    let service_pattern =
        Regex::new(r"(?i)\b(serves?|service area|covers?|coverage|available in|travels? to)\b")
            .unwrap();
    let past_pattern = Regex::new(r"(?i)\b(fixed|repaired|visited|went|stayed)\b").unwrap();
    let caution_words=Regex::new(r"(?i)\b(not|never|but|avoid|slow|leaks?|leaking|unknown|uncertain|lekin|nahi|nahin)\b|too small|small (portions|plates|tables)|(portions|plates|tables) (are |were )?(too )?small").unwrap();
    for mut item in proposal.items {
        // Treat role labels and display groups as untrusted model proposals.
        // A past repair does not establish a professional's practice or coverage.
        for location in &mut item.locations {
            let cited = support(&location.evidence, &units)?;
            let explicit_practice = practice_pattern.is_match(&cited);
            let explicit_service = service_pattern.is_match(&cited);
            if (location.role == "practice" && !explicit_practice)
                || (location.role == "service_area" && !explicit_service)
            {
                location.role = if past_pattern.is_match(&cited) {
                    "past_experience"
                } else {
                    "context"
                }
                .to_owned();
                partial = true;
            }
            if let Some(rest) = location.text.strip_prefix(&item.subject) {
                let rest = rest.trim();
                let locality = rest
                    .strip_prefix("in ")
                    .or_else(|| rest.strip_prefix("at "))
                    .unwrap_or(rest);
                if !locality.is_empty() && phrase_in(locality, &cited) {
                    location.text = locality.to_owned();
                }
            }
        }

        for observation in &mut item.observations {
            if observation.kind != "price" && caution_words.is_match(&observation.text) {
                observation.kind = "caution".to_owned();
            }
        }
        let shelf = match item.entity_kind.as_str() {
            "place" => "Places",
            "person_service" => "People & services",
            "thing" => "Things",
            "activity_event" => "Activities & events",
            "idea_tip" => "Ideas & tips",
            _ => return Err(ExtractionError::Failed),
        };
        if !["firsthand", "secondhand", "interest", "unspecified"]
            .contains(&item.experience.as_str())
            || item.subject.trim().is_empty()
            || item.subject.chars().count() > 120
            || item.observations.len() > 16
            || item.locations.len() > 8
            || item.use_cases.len() > 8
        {
            return Err(ExtractionError::Failed);
        }
        if !phrase_in(&item.subject, &support(&item.subject_evidence, &units)?) {
            return Err(ExtractionError::Failed);
        }
        check_claim(&item.summary.text, &item.summary.evidence, &units, 280)?;
        let mut ids: HashSet<usize> = item.subject_evidence.iter().copied().collect();
        // A broad summary citation must not count as coverage of every detail
        // in a long note. Observations and typed fields account for those units.
        if item.summary.evidence.len() <= 2 {
            ids.extend(&item.summary.evidence);
        }
        let mut body = vec![item.summary.text.trim().to_owned()];
        for observation in &item.observations {
            if ![
                "praise",
                "suggestion",
                "suitability",
                "caution",
                "price",
                "context",
            ]
            .contains(&observation.kind.as_str())
            {
                return Err(ExtractionError::Failed);
            }
            check_claim(&observation.text, &observation.evidence, &units, 400)?;
            ids.extend(&observation.evidence);
            body.push(observation.text.trim().to_owned());
        }
        for location in &item.locations {
            if ![
                "venue",
                "practice",
                "service_area",
                "past_experience",
                "context",
            ]
            .contains(&location.role.as_str())
            {
                return Err(ExtractionError::Failed);
            }
            check_claim(&location.text, &location.evidence, &units, 160)?;
            if !phrase_in(&location.text, &support(&location.evidence, &units)?) {
                return Err(ExtractionError::Failed);
            }
            // A venue/location classification is still a model interpretation;
            // never silently promote it to a confirmed service area or identity.
            ids.extend(&location.evidence);
            body.push(location.text.trim().to_owned());
        }
        // Optional retrieval phrases are source phrases in this pilot. Drop
        // model-written advice/rankings here rather than indexing new claims.
        item.use_cases.retain(|claim| {
            support(&claim.evidence, &units).is_ok_and(|cited| phrase_in(&claim.text, &cited))
        });
        for use_case in &item.use_cases {
            check_claim(&use_case.text, &use_case.evidence, &units, 100)?;
            ids.extend(&use_case.evidence);
            body.push(use_case.text.trim().to_owned());
        }
        accounted.extend(&ids);
        ids.extend(&item.summary.evidence);
        let classification_proposal =
            serde_json::from_value(item.classification.clone()).unwrap_or_default();
        // Classification can only cite this item's already represented units;
        // it cannot claim coverage or borrow uncited sibling source material.
        let item_units: Vec<_> = units
            .iter()
            .filter(|u| ids.contains(&u.id))
            .cloned()
            .collect();
        let (classification, classification_support) =
            crate::taxonomy::validate(&classification_proposal, &item.entity_kind, &item_units);
        let evidence = json!({"pipeline_version":UNDERSTANDING_VERSION,"proposal":item,
            "units":item_units,"classification":classification_support});
        let recommendation = json!({
            "version":UNDERSTANDING_VERSION,"entity_kind":item.entity_kind,"shelf":shelf,
            "experience":item.experience,"summary":item.summary.text,
            "observations":item.observations.iter().map(|o|json!({"kind":o.kind,"text":o.text})).collect::<Vec<_>>(),
            "locations":item.locations.iter().map(|l|json!({"role":l.role,"text":l.text})).collect::<Vec<_>>(),
            "use_cases":item.use_cases.iter().map(|c|c.text.clone()).collect::<Vec<_>>(),
            "classification":classification
        });
        items.push(ValidatedItem {
            subject: item.subject.trim().to_owned(),
            body: body.join("\n"),
            recommendation,
            evidence,
        });
    }
    partial |= units.iter().any(|u| !accounted.contains(&u.id));
    Ok((items, partial))
}

pub fn preserve_unresolved(proposal: Proposal, source: &str) -> Option<(Vec<ValidatedItem>, bool)> {
    if source.trim().is_empty() || source.chars().count() > 6000 {
        return None;
    }
    let subject = proposal.items.iter().find_map(|i| {
        let subject = i.subject.trim();
        ((3..=120).contains(&subject.chars().count()) && phrase_in(subject, source))
            .then(|| subject.to_owned())
    })?;
    Some((
        vec![ValidatedItem {
            subject,
            body: source.to_owned(),
            recommendation: Value::Null,
            evidence: json!({"pipeline_version":UNDERSTANDING_VERSION,"fallback":"source_only"}),
        }],
        true,
    ))
}
