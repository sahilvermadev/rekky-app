//! Field-local assessment. Evidence references establish provenance, not semantic
//! entailment. Unresolved material remains private and eligible for bounded repair.
use crate::extraction::{Observation, Proposal, SourceUnit, ValidatedItem, source_units};
use crate::taxonomy::Vocabulary;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashSet;

pub const ASSESSMENT_VERSION: i32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Issue {
    pub code: String,
    pub item: Option<usize>,
    pub field: String,
    pub units: Vec<usize>,
    pub material: bool,
}

#[derive(Clone, Debug)]
pub struct Assessment {
    pub items: Vec<ValidatedItem>,
    pub issues: Vec<Issue>,
}
impl Assessment {
    pub fn partial(&self) -> bool {
        self.issues.iter().any(|i| i.material)
    }
}

fn issue(
    issues: &mut Vec<Issue>,
    item: Option<usize>,
    field: &str,
    code: &str,
    units: &[usize],
    material: bool,
) {
    issues.push(Issue {
        code: code.into(),
        item,
        field: field.into(),
        units: units.to_vec(),
        material,
    });
}
pub(crate) fn words(text: &str) -> Vec<String> {
    Regex::new(r"[\p{L}\p{M}\p{N}]+")
        .unwrap()
        .find_iter(text)
        .map(|m| m.as_str().to_lowercase())
        .collect()
}
pub(crate) fn phrase(text: &str, source: &str) -> bool {
    let needle = words(text);
    !needle.is_empty() && words(source).windows(needle.len()).any(|w| w == needle)
}
fn cited(ids: &[usize], units: &[SourceUnit]) -> Option<String> {
    if ids.is_empty() || ids.len() > 32 {
        return None;
    }
    ids.iter()
        .map(|id| units.iter().find(|u| u.id == *id).map(|u| u.text.clone()))
        .collect::<Option<Vec<_>>>()
        .map(|s| s.join(" "))
}

// A small deterministic equivalence set, not a general language/amount parser.
// Unknown forms remain literal. In particular "bucks" never supplies a currency.
fn numbers(text: &str) -> HashSet<String> {
    let mut result: HashSet<String> = Regex::new(r"\p{N}+(?:[.,]\p{N}+)*")
        .unwrap()
        .find_iter(text)
        .map(|m| m.as_str().into())
        .collect();
    let tokens = words(text);
    let small = [
        "zero",
        "one",
        "two",
        "three",
        "four",
        "five",
        "six",
        "seven",
        "eight",
        "nine",
        "ten",
        "eleven",
        "twelve",
        "thirteen",
        "fourteen",
        "fifteen",
        "sixteen",
        "seventeen",
        "eighteen",
        "nineteen",
    ];
    let tens = [
        ("twenty", 20),
        ("thirty", 30),
        ("forty", 40),
        ("fifty", 50),
        ("sixty", 60),
        ("seventy", 70),
        ("eighty", 80),
        ("ninety", 90),
    ];
    let mut group = 0u64;
    let mut total = 0u64;
    let mut active = false;
    let mut previous_small = false;
    for token in tokens.iter().map(String::as_str).chain(std::iter::once("")) {
        let value = small
            .iter()
            .position(|v| *v == token)
            .map(|v| v as u64)
            .or_else(|| tens.iter().find(|(s, _)| *s == token).map(|(_, v)| *v));
        if let Some(value) = value {
            // Do not treat a spoken sequence such as "one two" as addition.
            if previous_small && value < 20 {
                result.insert((total + group).to_string());
                group = 0;
                total = 0;
            }
            group += value;
            active = true;
            previous_small = value < 20;
        } else if token == "hundred" && active && group > 0 && group < 100 {
            group *= 100;
            previous_small = false;
        } else if token == "thousand" && active && group > 0 && group < 1000 {
            total += group * 1000;
            group = 0;
            previous_small = false;
        } else {
            if active {
                result.insert((total + group).to_string());
            }
            group = 0;
            total = 0;
            active = false;
            previous_small = false;
        }
    }
    result
}
fn claim_error(
    text: &str,
    evidence: &[usize],
    units: &[SourceUnit],
    limit: usize,
) -> Option<&'static str> {
    if text.trim().is_empty() || text.chars().count() > limit {
        return Some("presentation_length");
    }
    let Some(source) = cited(evidence, units) else {
        return Some("invalid_evidence");
    };
    let actual = numbers(&source);
    if numbers(text).iter().any(|n| !actual.contains(n)) {
        return Some("unsupported_amount");
    }
    for symbol in ['₹', '$', '€', '£'] {
        let explicit = symbol == '₹'
            && ["rupee", "rupees", "inr", "रुपये", "रुपए"]
                .iter()
                .any(|s| phrase(s, &source));
        if text.contains(symbol) && !source.contains(symbol) && !explicit {
            return Some("unsupported_currency");
        }
    }
    let links = Regex::new(r"(?i)https?://\S+|www\.\S+|\S+@\S+").unwrap();
    if links.find_iter(text).any(|m| !source.contains(m.as_str())) {
        return Some("unsupported_link");
    }
    None
}
fn location_supported(text: &str, source: &str) -> bool {
    // Reordered independently mentioned locality components are not invented
    // names. Geographic resolution separately determines their relationship.
    phrase(text, source)
        || (text.contains(',') && text.split(',').all(|p| phrase(p.trim(), source)))
}
fn signals(text: &str) -> (bool, bool) {
    // Narrow escalation signals, never an entailment verdict. Unknown details
    // and untried experiences are not negative reviews. Examine equivalent
    // uncertainty wording and the complete account supporting the same units.
    let knowledge = Regex::new(r"(?i)\b(do not know|don't know|don’t know|not sure|not tried|haven't tried|haven’t tried|have not tried|don't remember|do not remember|yaad nahi|yaad nahin|pata nahi|pata nahin)\b").unwrap();
    let opinion = knowledge.replace_all(text, "");
    let caution = Regex::new(r"(?i)\b(not|never|avoid|slow|leaks?|leaking|broken|disappointing|rushed|disliked|dislike|bad|poor)\b|small portions|too small|नहीं|\b(nahi|nahin)\b").unwrap().is_match(&opinion);
    let uncertainty = knowledge.is_match(text) || Regex::new(r"(?i)\b(maybe|perhaps|probably|about|roughly|around|unsure|uncertain|unknown|untried)\b|i think|from memory|लगभग|शायद|\b(shayad|lagbhag)\b").unwrap().is_match(text);
    (caution, uncertainty)
}
fn filler(unit: &SourceUnit, units: &[SourceUnit]) -> bool {
    let w = words(&unit.text);
    (!w.is_empty()
        && w.iter().all(|w| {
            [
                "um", "uh", "erm", "hmm", "okay", "ok", "you", "know", "i", "mean",
            ]
            .contains(&w.as_str())
        }))
        || units
            .iter()
            .any(|other| other.id < unit.id && words(&other.text) == w)
}
fn identity_only(unit: &SourceUnit, subject: &str) -> bool {
    let t = words(&unit.text);
    let n = words(subject);
    let Some(start) = t.windows(n.len().max(1)).position(|w| w == n) else {
        return false;
    };
    t[..start]
        .iter()
        .chain(t[start + n.len()..].iter())
        .all(|w| {
            [
                "i",
                "want",
                "to",
                "recommend",
                "the",
                "name",
                "of",
                "this",
                "is",
                "called",
                "it",
                "a",
                "an",
                "service",
            ]
            .contains(&w.as_str())
        })
}

// Pure identity/category/locality sentences can be represented by their fields;
// metadata does not cover a sentence containing praise, timing or qualifications.
fn metadata_only(
    unit: &SourceUnit,
    subject: &str,
    locations: &[Value],
    classification: &Value,
) -> bool {
    let mut remaining = words(&unit.text);
    let mut phrases = vec![subject.to_owned()];
    for location in locations {
        if let Some(text) = location["text"].as_str() {
            phrases.extend(text.split(',').map(str::to_owned));
        }
    }
    if let Some(text) = classification["type_description"]["text"].as_str() {
        phrases.push(text.into());
    }
    if let Some(assignments) = classification["assignments"].as_array() {
        phrases.extend(
            assignments
                .iter()
                .filter_map(|a| a["source_phrase"].as_str().map(str::to_owned)),
        );
    }
    for phrase in phrases {
        let phrase = words(&phrase);
        if phrase.is_empty() {
            continue;
        }
        while let Some(start) = remaining.windows(phrase.len()).position(|w| w == phrase) {
            remaining.drain(start..start + phrase.len());
        }
    }
    remaining.iter().all(|w| {
        [
            "i",
            "want",
            "to",
            "recommend",
            "the",
            "name",
            "of",
            "this",
            "is",
            "called",
            "it",
            "a",
            "an",
            "service",
            "he",
            "she",
            "they",
            "in",
            "at",
            "and",
            "practices",
            "practice",
            "based",
            "works",
            "from",
            "serves",
        ]
        .contains(&w.as_str())
    })
}

pub fn assess(proposal: &Proposal, source: &str, catalog: &Vocabulary) -> Assessment {
    let units = source_units(source);
    let mut issues = Vec::new();
    let mut result = Vec::new();
    let mut covered = HashSet::new();
    let mut assigned = HashSet::new();
    if proposal.items.is_empty() || proposal.items.len() > 5 || units.is_empty() {
        issue(&mut issues, None, "items", "invalid_subjects", &[], true);
        return Assessment {
            items: result,
            issues,
        };
    }
    for u in &units {
        if filler(u, &units) {
            covered.insert(u.id);
        }
    }
    let practice_pattern =
        Regex::new(r"(?i)\b(practic(?:e|es|ing)|clinic|office|based|works from)\b").unwrap();
    let service_pattern=Regex::new(r"(?i)\b(serves?|service area|covers?|coverage|available in|travels? to|provides? [\p{L} -]{1,50} in|operates? in)\b").unwrap();
    for (index, item) in proposal.items.iter().enumerate() {
        let first_issue = issues.len();
        let mut item_covered = covered.clone();
        let subject_source = cited(&item.subject_evidence, &units);
        if item.subject.trim().is_empty()
            || item.subject.chars().count() > 120
            || !subject_source.is_some_and(|s| phrase(&item.subject, &s))
        {
            issue(
                &mut issues,
                Some(index),
                "subject",
                "subject_unresolved",
                &item.subject_evidence,
                true,
            );
            continue;
        }
        let account = if item.account.is_empty() {
            let mut a = vec![Observation {
                kind: "context".into(),
                text: item.summary.text.clone(),
                evidence: item.summary.evidence.clone(),
            }];
            a.extend(item.observations.clone());
            a
        } else {
            item.account.clone()
        };
        let refs: HashSet<usize> = item
            .subject_evidence
            .iter()
            .chain(account.iter().flat_map(|p| p.evidence.iter()))
            .chain(item.locations.iter().flat_map(|l| l.evidence.iter()))
            .copied()
            .chain(
                item.classification["type_description"]["evidence"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|v| v.as_u64().map(|v| v as usize)),
            )
            .collect();
        // Never recover a sibling's passage beneath this item's title. Shared
        // sentences require review rather than claiming safe source separation.
        let own: Vec<SourceUnit> = units
            .iter()
            .filter(|u| {
                let ours = refs.contains(&u.id);
                let names_other = proposal
                    .items
                    .iter()
                    .enumerate()
                    .any(|(i, p)| i != index && phrase(&p.subject, &u.text));
                if ours && names_other {
                    issue(
                        &mut issues,
                        Some(index),
                        "source",
                        "subject_boundary",
                        &[u.id],
                        true,
                    );
                }
                ours && !names_other
            })
            .cloned()
            .collect();
        assigned.extend(own.iter().map(|u| u.id));
        let classification_proposal =
            serde_json::from_value(item.classification.clone()).unwrap_or_default();
        let kinds = [
            "place",
            "person_service",
            "thing",
            "activity_event",
            "idea_tip",
        ];
        let compatible: Vec<_> = kinds
            .iter()
            .filter(|kind| {
                let (value, _) =
                    crate::taxonomy::validate_in(catalog, &classification_proposal, kind, &own);
                value["types"]
                    .as_array()
                    .is_some_and(|types| !types.is_empty())
            })
            .copied()
            .collect();
        let entity_kind = if !kinds.contains(&item.entity_kind.as_str()) && compatible.len() == 1 {
            compatible[0]
        } else {
            item.entity_kind.as_str()
        };
        if entity_kind != item.entity_kind {
            issue(
                &mut issues,
                Some(index),
                "entity_kind",
                "category_normalized",
                &item.subject_evidence,
                false,
            );
        }
        let (classification, classification_support) =
            crate::taxonomy::validate_in(catalog, &classification_proposal, entity_kind, &own);
        if item.classification["types"]
            .as_array()
            .is_some_and(|v| !v.is_empty())
            && classification["types"]
                .as_array()
                .is_none_or(|v| v.is_empty())
        {
            issue(
                &mut issues,
                Some(index),
                "classification",
                "category_omitted",
                &[],
                false,
            );
        }

        for u in &own {
            if identity_only(u, &item.subject) {
                item_covered.insert(u.id);
            }
        }
        let mut paragraphs: Vec<Observation> = Vec::new();
        for (pidx, p) in account.iter().enumerate() {
            let field = format!("account.{pidx}");
            let failure = claim_error(
                &p.text,
                &p.evidence,
                &own,
                if pidx == 0 { 420 } else { 400 },
            );
            if let Some(code) = failure {
                issue(&mut issues, Some(index), &field, code, &p.evidence, true);
                // Preserve the exact, item-local meaning while awaiting repair.
                // Invalid/out-of-item references never become a source excerpt.
                if let Some(text) = cited(&p.evidence, &own) {
                    paragraphs.push(Observation {
                        kind: if signals(&text).0 {
                            "caution"
                        } else {
                            "context"
                        }
                        .into(),
                        text,
                        evidence: p.evidence.clone(),
                    });
                    item_covered.extend(p.evidence.iter().copied());
                }
                continue;
            }
            let source = cited(&p.evidence, &own).unwrap();
            let (warning, uncertain) = signals(&source);
            let related_account = account
                .iter()
                .enumerate()
                .filter(|(other_index, other)| {
                    other.evidence.iter().any(|id| p.evidence.contains(id))
                        && claim_error(
                            &other.text,
                            &other.evidence,
                            &own,
                            if *other_index == 0 { 420 } else { 400 },
                        )
                        .is_none()
                })
                .map(|(_, other)| other.text.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            let (kept_warning, kept_uncertainty) = signals(&related_account);
            // Narrow risk triggers, not a semantic correctness guarantee. The
            // unchanged passage protects qualifiers until targeted repair.
            if (warning && !kept_warning) || (uncertain && !kept_uncertainty) {
                issue(
                    &mut issues,
                    Some(index),
                    &field,
                    if warning && !kept_warning {
                        "possible_missing_caveat"
                    } else {
                        "possible_missing_qualifier"
                    },
                    &p.evidence,
                    true,
                );
                paragraphs.push(Observation {
                    kind: if warning { "caution" } else { "context" }.into(),
                    text: source,
                    evidence: p.evidence.clone(),
                });
            } else {
                let mut p = p.clone();
                if signals(&p.text).0 {
                    p.kind = "caution".into();
                }
                if ![
                    "praise",
                    "suggestion",
                    "suitability",
                    "caution",
                    "price",
                    "context",
                ]
                .contains(&p.kind.as_str())
                {
                    p.kind = "context".into();
                }
                paragraphs.push(p);
            }
            item_covered.extend(p.evidence.iter().copied());
        }
        if paragraphs.len() > 17 {
            issue(
                &mut issues,
                Some(index),
                "account",
                "presentation_length",
                &[],
                true,
            );
        }
        let mut locations = Vec::new();
        for (lidx, l) in item.locations.iter().enumerate() {
            let field = format!("locations.{lidx}");
            let Some(s) = cited(&l.evidence, &own) else {
                issue(
                    &mut issues,
                    Some(index),
                    &field,
                    "location_unresolved",
                    &l.evidence,
                    true,
                );
                continue;
            };
            let text = l.text.trim();
            if text.chars().count() > 160 || !location_supported(text, &s) {
                issue(
                    &mut issues,
                    Some(index),
                    &field,
                    "location_unresolved",
                    &l.evidence,
                    true,
                );
                continue;
            }
            let explicit_type = classification_support["type_description"]["text"]
                .as_str()
                .unwrap_or("");
            let nominal_base = !explicit_type.is_empty()
                && ["is a", "is an"]
                    .iter()
                    .any(|prefix| phrase(&format!("{prefix} {explicit_type} in {text}"), &s));
            let practice = practice_pattern.is_match(&s) || nominal_base;
            let service = service_pattern.is_match(&s);
            let approximate = Regex::new(&format!(
                r"(?i)\b(?:near|around|outside|outskirts of)\s+{}\b",
                regex::escape(text)
            ))
            .unwrap()
            .is_match(&s);
            let mut role = l.role.as_str();
            if ![
                "venue",
                "practice",
                "service_area",
                "past_experience",
                "context",
            ]
            .contains(&role)
                || (role == "venue" && !["place", "activity_event"].contains(&entity_kind))
                || (role == "practice" && !practice)
                || (role == "service_area" && !service)
                || (approximate && ["venue", "practice", "service_area"].contains(&role))
            {
                issue(
                    &mut issues,
                    Some(index),
                    &field,
                    "location_role_uncertain",
                    &l.evidence,
                    true,
                );
                role = "context";
            }
            locations.push(json!({"role":role,"text":text}));
            // Location metadata alone cannot account for the opinion in a sentence.
        }
        for u in &own {
            if metadata_only(u, &item.subject, &locations, &classification_support) {
                item_covered.insert(u.id);
            }
        }
        let item_issues: Vec<usize> = proposal
            .unresolved_unit_ids
            .iter()
            .copied()
            .filter(|id| own.iter().any(|u| u.id == *id))
            .collect();
        if !item_issues.is_empty() {
            issue(
                &mut issues,
                Some(index),
                "source",
                "unresolved_meaning",
                &item_issues,
                true,
            );
        }
        let missing: Vec<_> = own
            .iter()
            .filter(|u| !item_covered.contains(&u.id))
            .map(|u| u.id)
            .collect();
        if !missing.is_empty() {
            issue(
                &mut issues,
                Some(index),
                "account",
                "unrepresented_source",
                &missing,
                true,
            );
            for u in own.iter().filter(|u| missing.contains(&u.id)) {
                paragraphs.push(Observation {
                    kind: if signals(&u.text).0 {
                        "caution"
                    } else {
                        "context"
                    }
                    .into(),
                    text: u.text.clone(),
                    evidence: vec![u.id],
                });
            }
        }
        if paragraphs.is_empty() {
            // Identity survives an invalid summary; the account is still private.
            if let Some(text) = cited(&item.subject_evidence, &own) {
                paragraphs.push(Observation {
                    kind: "context".into(),
                    text,
                    evidence: item.subject_evidence.clone(),
                });
            } else {
                continue;
            }
        }
        paragraphs.dedup_by(|a, b| words(&a.text) == words(&b.text));
        if paragraphs
            .iter()
            .map(|p| p.text.chars().count())
            .sum::<usize>()
            > 14000
        {
            issue(
                &mut issues,
                Some(index),
                "account",
                "presentation_length",
                &[],
                true,
            );
            paragraphs = own
                .iter()
                .map(|u| Observation {
                    kind: if signals(&u.text).0 {
                        "caution"
                    } else {
                        "context"
                    }
                    .into(),
                    text: u.text.clone(),
                    evidence: vec![u.id],
                })
                .collect();
        }
        let shelf = match entity_kind {
            "place" => "Places",
            "person_service" => "People & services",
            "thing" => "Things",
            "activity_event" => "Activities & events",
            "idea_tip" => "Ideas & tips",
            _ => "Notes",
        };
        if shelf == "Notes" {
            issue(
                &mut issues,
                Some(index),
                "entity_kind",
                "category_unresolved",
                &item.subject_evidence,
                true,
            );
        }
        let experience = if ["firsthand", "secondhand", "interest", "unspecified"]
            .contains(&item.experience.as_str())
        {
            item.experience.as_str()
        } else {
            "unspecified"
        };
        let rating = crate::ratings::validate(&item.rating, experience, &own);
        if item
            .rating
            .get("mode")
            .and_then(Value::as_str)
            .is_some_and(|m| m != "none")
            && rating.is_null()
        {
            issue(
                &mut issues,
                Some(index),
                "rating",
                "rating_omitted",
                &[],
                false,
            );
        }
        let use_cases: Vec<_> = item
            .use_cases
            .iter()
            .filter(|c| {
                cited(&c.evidence, &own).is_some_and(|s| phrase(&c.text, &s))
                    && claim_error(&c.text, &c.evidence, &own, 100).is_none()
            })
            .map(|c| c.text.clone())
            .collect();
        let mut observations: Vec<Value> = paragraphs
            .iter()
            .skip(1)
            .map(|p| json!({"kind":p.kind,"text":p.text}))
            .collect();
        if paragraphs[0].kind == "caution" {
            observations.insert(0, json!({"kind":"caution","text":paragraphs[0].text}));
        }
        let current = &issues[first_issue..];
        let review = current.iter().any(|i| i.material);
        let codes: HashSet<_> = current
            .iter()
            .filter(|i| i.material)
            .map(|i| i.code.clone())
            .collect();
        let mut codes: Vec<_> = codes.into_iter().collect();
        codes.sort();
        let mut body = paragraphs
            .iter()
            .map(|p| p.text.clone())
            .collect::<Vec<_>>();
        body.extend(
            locations
                .iter()
                .filter_map(|l| l["text"].as_str().map(str::to_owned)),
        );
        body.extend(use_cases.clone());
        result.push(ValidatedItem{subject:item.subject.trim().into(),body:body.join("\n"),recommendation:json!({
            "version":2,"entity_kind":entity_kind,"shelf":shelf,"experience":experience,
            "summary":paragraphs[0].text,"observations":observations,"locations":locations,"use_cases":use_cases,"classification":classification,"rating":rating,
            "quality":{"version":ASSESSMENT_VERSION,"needs_review":review,"issues":codes}
        }),evidence:json!({"pipeline_version":2,"editorial_version":2,"assessment_version":ASSESSMENT_VERSION,"proposal":item,"units":own,"classification":classification_support,"issues":current,"proposal_index":index})});
    }
    let unassigned: Vec<_> = units
        .iter()
        .filter(|u| !assigned.contains(&u.id) && !covered.contains(&u.id))
        .map(|u| u.id)
        .collect();
    if !unassigned.is_empty() || result.len() != proposal.items.len() {
        issue(
            &mut issues,
            None,
            "source",
            "subject_boundary",
            &unassigned,
            true,
        );
        // Unassigned material might qualify any sibling. Keep all held until
        // boundaries are resolved, without destroying any item's structure.
        for item in &mut result {
            item.recommendation["quality"]["needs_review"] = json!(true);
            item.recommendation["quality"]["issues"]
                .as_array_mut()
                .unwrap()
                .push(json!("subject_boundary"));
        }
    }
    Assessment {
        items: result,
        issues,
    }
}

/// Apply only repaired items whose grounded identity stayed the same. Other
/// items retain their original proposal; a repair cannot silently drop siblings.
pub fn merge_repair(original: &Proposal, repaired: Proposal, assessment: &Assessment) -> Proposal {
    let mut merged = original.clone();
    for (index, item) in original.items.iter().enumerate() {
        if !assessment
            .issues
            .iter()
            .any(|i| i.material && (i.item == Some(index) || i.item.is_none()))
        {
            continue;
        }
        let matches: Vec<_> = repaired
            .items
            .iter()
            .filter(|r| {
                words(&r.subject) == words(&item.subject)
                    && r.subject_evidence == item.subject_evidence
            })
            .collect();
        if matches.len() == 1 {
            let repaired = matches[0];
            let relevant: Vec<_> = assessment
                .issues
                .iter()
                .filter(|i| i.material && (i.item == Some(index) || i.item.is_none()))
                .collect();
            if relevant
                .iter()
                .any(|i| i.field.starts_with("account") || i.field == "source")
            {
                merged.items[index].account = repaired.account.clone();
                merged.items[index].summary = repaired.summary.clone();
                merged.items[index].observations = repaired.observations.clone();
                merged.items[index].rating = repaired.rating.clone();
            }
            if relevant.iter().any(|i| i.field.starts_with("locations")) {
                merged.items[index].locations = repaired.locations.clone();
            }
            if relevant.iter().any(|i| i.field == "entity_kind") {
                merged.items[index].entity_kind = repaired.entity_kind.clone();
            }
        }
    }
    let missing: HashSet<usize> = assessment
        .issues
        .iter()
        .filter(|i| {
            i.material
                && [
                    "subject_boundary",
                    "unrepresented_source",
                    "unresolved_meaning",
                ]
                .contains(&i.code.as_str())
        })
        .flat_map(|i| i.units.iter().copied())
        .collect();
    let original_names: HashSet<_> = original.items.iter().map(|i| words(&i.subject)).collect();
    for item in &repaired.items {
        if merged.items.len() >= 5 {
            break;
        }
        if !original_names.contains(&words(&item.subject))
            && !item.subject_evidence.is_empty()
            && item.subject_evidence.iter().all(|id| missing.contains(id))
            && !merged
                .items
                .iter()
                .any(|i| words(&i.subject) == words(&item.subject))
        {
            merged.items.push(item.clone());
        }
    }
    // A repair still has to represent all source material; clearing these model
    // hints alone does not satisfy assessment's independent coverage checks.
    merged.unresolved_unit_ids = repaired.unresolved_unit_ids;
    merged.ignored_unit_ids = repaired.ignored_unit_ids;
    merged
}

/// Additional grounded subjects can resolve unassigned source. Reassessment
/// must preserve every already accepted subject and reduce material issues.
pub fn improves(original: &Assessment, candidate: &Assessment) -> bool {
    candidate.issues.iter().filter(|i| i.material).count()
        < original.issues.iter().filter(|i| i.material).count()
        && original.items.iter().all(|old| {
            candidate
                .items
                .iter()
                .any(|new| words(&new.subject) == words(&old.subject))
        })
}
