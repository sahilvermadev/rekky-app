//! Versioned concepts are shared data; the model proposes assignments, not labels.
use crate::extraction::SourceUnit;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::BTreeSet, sync::OnceLock};

#[derive(Debug, Deserialize, Serialize)]
pub struct Vocabulary {
    pub version: i32,
    pub concepts: Vec<Concept>,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct Concept {
    pub id: String,
    pub label: String,
    pub dimension: String,
    pub dimension_label: String,
    pub entity_kinds: Vec<String>,
    pub parent_id: Option<String>,
    pub aliases: Vec<String>,
}
pub fn vocabulary() -> &'static Vocabulary {
    static V: OnceLock<Vocabulary> = OnceLock::new();
    V.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../contracts/rekky/v1/taxonomy/vocabulary.json"
        ))
        .expect("checked vocabulary")
    })
}
pub fn concept(id: &str) -> Option<&'static Concept> {
    vocabulary().concepts.iter().find(|c| c.id == id)
}
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProposedClassification {
    pub types: Vec<Assignment>,
    pub facets: Vec<Assignment>,
    pub descriptors: Vec<Descriptor>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Assignment {
    pub concept_id: String,
    pub source_phrase: String,
    pub evidence: Vec<usize>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Descriptor {
    pub text: String,
    pub evidence: Vec<usize>,
}

pub fn words(text: &str) -> Vec<String> {
    static WORDS: OnceLock<Regex> = OnceLock::new();
    WORDS
        .get_or_init(|| Regex::new(r"[\p{L}\p{M}\p{N}]+").unwrap())
        .find_iter(text)
        .map(|m| m.as_str().to_lowercase())
        .collect()
}
fn contains_phrase(text: &str, phrase: &str) -> bool {
    let a = words(text);
    let b = words(phrase);
    !b.is_empty() && a.windows(b.len()).any(|w| w == b)
}
fn supported(text: &str, ids: &[usize], units: &[SourceUnit]) -> bool {
    !text.trim().is_empty()
        && text.chars().count() <= 100
        && !ids.is_empty()
        && ids.len() <= 8
        && ids.iter().all(|id| units.iter().any(|u| u.id == *id))
        && ids.iter().any(|id| {
            units
                .iter()
                .any(|u| u.id == *id && contains_phrase(&u.text, text))
        })
}
fn compatible(c: &Concept, kind: &str) -> bool {
    c.entity_kinds.iter().any(|k| k == kind)
}
fn accepted(a: &Assignment, kind: &str, dimension: &str, units: &[SourceUnit]) -> bool {
    concept(&a.concept_id).is_some_and(|c| {
        compatible(c, kind) && (if dimension == "type" { c.dimension == "type" } else { c.dimension != "type" })
        && supported(&a.source_phrase, &a.evidence, units)
        && c.aliases.iter().any(|alias| words(alias) == words(&a.source_phrase))
        // Reject explicit negation rather than letting a label reverse the note.
        && !a.evidence.iter().any(|id| units.iter().any(|u| u.id == *id && {
            let text = words(&u.text).join(" ");
            let phrase = words(&a.source_phrase).join(" ");
            [format!("not a {phrase}"), format!("not an {phrase}"),
             format!("not {phrase}"), format!("no longer a {phrase}"),
             format!("{phrase} नहीं"), format!("{phrase} nahi")]
                .iter().any(|negative| text.contains(negative))
        }))
    })
}

/// Optional taxonomy failures never discard otherwise useful saved knowledge.
/// Evidence references/alias checks bound assignments; they do not prove meaning.
pub fn validate(
    proposal: &ProposedClassification,
    kind: &str,
    units: &[SourceUnit],
) -> (Value, Value) {
    let mut types = Vec::new();
    let mut facets = Vec::new();
    let mut support = Vec::new();
    for a in proposal.types.iter().chain(&proposal.facets).take(16) {
        let Some(c) = concept(&a.concept_id) else {
            continue;
        };
        // The registry owns dimensions too. A supported cuisine accidentally
        // placed in the model's types array is still a cuisine, never a type.
        let (dimension, ids) = if c.dimension == "type" {
            ("type", &mut types)
        } else {
            ("facet", &mut facets)
        };
        if accepted(a, kind, dimension, units) && !ids.contains(&a.concept_id) {
            ids.push(a.concept_id.clone());
            support.push(json!(a));
        }
    }
    // The most specific of a selected parent/child pair carries the label.
    let all = types.clone();
    types.retain(|id| {
        !all.iter()
            .any(|other| other != id && ancestors(other).contains(id))
    });
    types.truncate(3);
    facets.truncate(4);
    let descriptors: Vec<String> = proposal
        .descriptors
        .iter()
        .take(4)
        .filter(|d| supported(&d.text, &d.evidence, units))
        .filter(|d| {
            !types
                .iter()
                .chain(&facets)
                .filter_map(|id| concept(id))
                .any(|c| c.aliases.iter().any(|alias| words(alias) == words(&d.text)))
        })
        .map(|d| d.text.trim().to_owned())
        .collect();
    let presentation =
        present(kind, &types, &facets, &descriptors, "extracted").expect("validated concepts");
    (
        presentation,
        json!({"vocabulary_version":vocabulary().version,
        "assignments":support,"descriptors":proposal.descriptors.iter()
            .filter(|d| descriptors.contains(&d.text.trim().to_owned())).collect::<Vec<_>>()}),
    )
}

pub fn ancestors(id: &str) -> BTreeSet<String> {
    let mut result = BTreeSet::new();
    let mut current = concept(id);
    while let Some(c) = current {
        if !result.insert(c.id.clone()) {
            break;
        }
        current = c.parent_id.as_deref().and_then(concept);
    }
    result
}

pub fn present(
    kind: &str,
    types: &[String],
    facets: &[String],
    descriptors: &[String],
    origin: &str,
) -> Option<Value> {
    if types.len() > 3 || facets.len() > 4 || descriptors.len() > 4 {
        return None;
    }
    let mut seen = BTreeSet::new();
    for (ids, dimension) in [(types, "type"), (facets, "facet")] {
        for id in ids {
            let c = concept(id)?;
            if !compatible(c, kind)
                || ((dimension == "type") != (c.dimension == "type"))
                || !seen.insert(id)
            {
                return None;
            }
        }
    }
    let types: Vec<String> = types
        .iter()
        .filter(|id| {
            !types
                .iter()
                .any(|other| other != *id && ancestors(other).contains(*id))
        })
        .cloned()
        .collect();
    let refs = |ids: &[String]| {
        ids.iter()
            .map(|id| {
                let c = concept(id).unwrap();
                json!({"id":c.id,"label":c.label,"dimension":c.dimension,"dimension_label":c.dimension_label})
            })
            .collect::<Vec<_>>()
    };
    let mut search_ids = BTreeSet::new();
    for id in types.iter().chain(facets) {
        search_ids.extend(ancestors(id));
    }
    let mut terms = BTreeSet::new();
    for id in &search_ids {
        let c = concept(id).unwrap();
        terms.insert(c.label.clone());
        terms.extend(c.aliases.clone());
    }
    terms.extend(descriptors.iter().cloned());
    let mut display = types
        .first()
        .and_then(|id| concept(id))
        .map(|c| c.label.clone());
    let cuisines: Vec<_> = facets
        .iter()
        .filter_map(|id| concept(id))
        .filter(|c| c.dimension == "cuisine")
        .collect();
    if types.first().map(String::as_str) == Some("place.restaurant") && cuisines.len() == 1 {
        display = Some(format!("{} restaurant", cuisines[0].label));
    }
    Some(
        json!({"vocabulary_version":vocabulary().version,"origin":origin,
        "types":refs(&types),"facets":refs(facets),"descriptors":descriptors,
        "display_label":display,"search_ids":search_ids,
        "search_terms":terms.into_iter().collect::<Vec<_>>().join(" ")}),
    )
}

/// Longest aliases first: "general physician" is one specific concept, not
/// unrelated matches for "general" and "physician". Query qualifiers remain.
pub fn query_concepts(question: &str) -> (Vec<String>, Vec<String>) {
    let tokens = words(question);
    let mut used = vec![false; tokens.len()];
    let mut ids = BTreeSet::new();
    let mut aliases: Vec<_> = vocabulary()
        .concepts
        .iter()
        .flat_map(|c| c.aliases.iter().map(move |a| (words(a), c.id.clone())))
        .collect();
    aliases.sort_by_key(|(a, _)| std::cmp::Reverse(a.len()));
    for (alias, id) in aliases {
        if alias.is_empty() || alias.len() > tokens.len() {
            continue;
        }
        for i in 0..=tokens.len() - alias.len() {
            if tokens[i..i + alias.len()] == alias && used[i..i + alias.len()].iter().all(|v| !v) {
                ids.insert(id.clone());
                used[i..i + alias.len()].fill(true);
            }
        }
    }
    (
        ids.into_iter().collect(),
        tokens
            .into_iter()
            .enumerate()
            .filter(|(i, _)| !used[*i])
            .map(|(_, v)| v)
            .collect(),
    )
}
