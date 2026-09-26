//! A rating describes this author's experience, never public reputation.
use crate::extraction::SourceUnit;
use regex::Regex;
use serde::Deserialize;
use serde_json::{Value, json};

pub const RUBRIC_VERSION: i32 = 1;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Proposal {
    mode: String,
    stance: String,
    spoken_value: Option<f64>,
    source_phrase: String,
    evidence: Vec<usize>,
}

/// Optional enrichment: malformed/unsupported scores never block saving a note.
/// These checks establish shape and source locality, not semantic correctness.
pub fn validate(proposal: &Value, experience: &str, units: &[SourceUnit]) -> Value {
    let Ok(p) = serde_json::from_value::<Proposal>(proposal.clone()) else {
        return Value::Null;
    };
    if !["firsthand", "unspecified"].contains(&experience)
        || p.evidence.is_empty()
        || p.source_phrase.trim().is_empty()
        || p.source_phrase.chars().count() > 600
        || p.evidence
            .iter()
            .any(|id| !units.iter().any(|u| u.id == *id))
        || !units
            .iter()
            .any(|u| p.evidence.contains(&u.id) && u.text.contains(&p.source_phrase))
    {
        return Value::Null;
    }
    let source = units
        .iter()
        .map(|u| u.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    // Treat an explicit inability/refusal to rate as abstention, not neutrality.
    let unsettled = Regex::new(r"(?i)(?:not sure|unsure|cannot|can't|can’t|could not|don't know|do not know|not ready|haven't decided|have not decided)[^.?!]{0,90}\b(?:rating|rate|score)\b|\b(?:rating|score)\b[^.?!]{0,40}(?:undecided|uncertain|not decided)").unwrap();
    if unsettled.is_match(&source) {
        return Value::Null;
    }
    let mut spoken = Value::Null;
    let value = match p.mode.as_str() {
        "inferred" if experience == "firsthand" && p.spoken_value.is_none() => {
            // If a score was discussed but was not a valid overall spoken
            // score, do not silently substitute an inferred one.
            if Regex::new(r"(?i)\bout of (?:\d+|five|ten)\b|\d\s*/\s*\d")
                .unwrap()
                .is_match(&source)
            {
                return Value::Null;
            }
            match p.stance.as_str() {
                "awful" => 1.0,
                "very_bad" => 2.0,
                "bad" => 3.0,
                "disappointing" => 4.0,
                "mixed" => 5.0,
                "okay" => 6.0,
                "good" => 7.0,
                "very_good" => 8.0,
                "excellent" => 9.0,
                "exceptional" => 10.0,
                _ => return Value::Null,
            }
        }
        "spoken" => {
            let Some(value) = p.spoken_value.filter(|v| valid_value(*v)) else {
                return Value::Null;
            };
            // Normalize only explicit /5 or /10 self-ratings. Never turn prices,
            // hotel categories or aspect-only ratings into an overall score.
            // Whether this is an overall self-rating is judged by the model.
            if !Regex::new(
                r"(?i)\b(?:overall|i (?:would )?(?:give|rate|rated|gave)|my (?:rating|score))\b",
            )
            .unwrap()
            .is_match(&p.source_phrase)
                || Regex::new(r"(?i)\b(?:not|never|wouldn't|wouldn’t)\b")
                    .unwrap()
                    .is_match(&p.source_phrase)
            {
                return Value::Null;
            }
            let pattern = Regex::new(r"(?i)\b(10|[0-9](?:\.\d{1,2})?|zero|one|two|three|four|five|six|seven|eight|nine|ten|one and a half|two and a half|three and a half|four and a half|five and a half|six and a half|seven and a half|eight and a half|nine and a half)\s*(?:/\s*|out of )(5|five|10|ten)\b").unwrap();
            let scale = pattern.captures_iter(&p.source_phrase).find_map(|c| {
                let text = c[1].to_lowercase();
                let score = spoken_number(&text)?;
                let scale = match c[2].to_lowercase().as_str() {
                    "5" | "five" => 5.0,
                    _ => 10.0,
                };
                (score == value && score <= scale).then_some(scale)
            });
            let Some(scale) = scale else {
                return Value::Null;
            };
            spoken = json!({"value":value,"scale":scale});
            value * (10.0 / scale)
        }
        _ => return Value::Null,
    };
    let mut result =
        json!({"value":value,"scale":10,"origin":p.mode,"rubric_version":RUBRIC_VERSION});
    if !spoken.is_null() {
        result["spoken"] = spoken;
    }
    result
}

fn spoken_number(text: &str) -> Option<f64> {
    if let Some(whole) = text.strip_suffix(" and a half") {
        return spoken_number(whole).map(|n| n + 0.5);
    }
    match text {
        "zero" => Some(0.0),
        "one" => Some(1.0),
        "two" => Some(2.0),
        "three" => Some(3.0),
        "four" => Some(4.0),
        "five" => Some(5.0),
        "six" => Some(6.0),
        "seven" => Some(7.0),
        "eight" => Some(8.0),
        "nine" => Some(9.0),
        "ten" => Some(10.0),
        _ => text.parse().ok(),
    }
}

pub fn valid_value(value: f64) -> bool {
    value.is_finite() && (0.0..=10.0).contains(&value)
}

#[derive(Debug, Default, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum Edit {
    #[default]
    Keep,
    None,
    Set {
        value: f64,
    },
}
impl Edit {
    pub fn check(&self) -> Result<(), &'static str> {
        if let Self::Set { value } = self
            && !valid_value(*value)
        {
            return Err("Choose a rating between 0 and 10");
        }
        Ok(())
    }
    pub fn apply(&self, previous_subject: &str, subject: &str, previous: &Value, next: &mut Value) {
        next["rating"] = match self {
            Self::None => Value::Null,
            Self::Set { value } => json!({"value":value,"scale":10,"origin":"user"}),
            Self::Keep => {
                let rating = &previous["rating"];
                // Identity/experience edits invalidate a carried score. Opinion
                // edits also invalidate an automatic score, never an owner-set one.
                let identity_changed = previous_subject != subject
                    || previous["entity_kind"] != next["entity_kind"]
                    || previous["experience"] != next["experience"];
                let opinion_changed =
                    ["summary", "observations", "attribution"]
                        .iter()
                        .any(|key| {
                            previous[*key] != next[*key]
                                && !(previous[*key].is_null() && next[*key] == "")
                        });
                if identity_changed || (rating["origin"] != "user" && opinion_changed) {
                    Value::Null
                } else {
                    rating.clone()
                }
            }
        };
    }
}
