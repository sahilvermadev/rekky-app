//! Conservative, optional copy editing for private display. Never extraction input.
use crate::{extraction::source_units, taxonomy::words};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EditedUnit {
    unit_id: usize,
    corrections: Vec<Correction>,
    paragraph_start: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Correction {
    before: String,
    after: String,
}
fn apply_corrections(raw: &str, corrections: &[Correction]) -> Option<String> {
    if corrections.len() > 30 {
        return None;
    }
    let mut spans = Vec::new();
    for correction in corrections {
        if correction.before.is_empty() || correction.after.len() > correction.before.len() * 2 + 40
        {
            return None;
        }
        let matches: Vec<_> = raw.match_indices(&correction.before).collect();
        if matches.len() != 1 {
            return None;
        }
        let start = matches[0].0;
        spans.push((
            start,
            start + correction.before.len(),
            correction.after.as_str(),
        ));
    }
    spans.sort_by_key(|(start, _, _)| *start);
    let mut result = String::new();
    let mut offset = 0;
    for (start, end, after) in spans {
        if start < offset {
            return None;
        }
        result.push_str(&raw[offset..start]);
        result.push_str(after);
        offset = end;
    }
    result.push_str(&raw[offset..]);
    Some(result)
}

fn protected(word: &str) -> bool {
    word.chars().any(char::is_numeric)
        || [
            "not",
            "no",
            "never",
            "without",
            "only",
            "if",
            "unless",
            "maybe",
            "might",
            "could",
            "perhaps",
            "very",
            "really",
            "always",
            "sometimes",
            "every",
            "all",
            "some",
            "i",
            "we",
            "you",
            "he",
            "she",
            "they",
            "my",
            "our",
            "your",
            "his",
            "her",
            "their",
            "नहीं",
            "नही",
            "मत",
            "शायद",
            "अगर",
            "सिर्फ",
            "nahi",
            "nahin",
            "mat",
            "shayad",
            "agar",
            "sirf",
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
            "hundred",
            "thousand",
        ]
        .contains(&word)
}
fn replacement(a: &str, b: &str, protected_names: &[String]) -> bool {
    if a == b {
        return true;
    }
    if [
        ("teh", "the"),
        ("adn", "and"),
        ("thier", "their"),
        ("reccomend", "recommend"),
        ("recomend", "recommend"),
        ("definately", "definitely"),
        ("resturant", "restaurant"),
        ("delicous", "delicious"),
    ]
    .contains(&(a, b))
        && !protected_names.iter().any(|n| n == a || n == b)
    {
        return true;
    }
    if protected(a) || protected(b) || protected_names.iter().any(|n| n == a || n == b) {
        return false;
    }
    if [
        ("is", "are"),
        ("was", "were"),
        ("has", "have"),
        ("a", "an"),
        ("hai", "hain"),
        ("है", "हैं"),
    ]
    .iter()
    .any(|(x, y)| (a == *x && b == *y) || (a == *y && b == *x))
    {
        return true;
    }
    false
}
fn edit_words(text: &str) -> Vec<String> {
    let mut normalized = text.replace('’', "'");
    for (raw, punctuated) in [
        ("didnt", "didn't"),
        ("dont", "don't"),
        ("doesnt", "doesn't"),
        ("isnt", "isn't"),
        ("wasnt", "wasn't"),
        ("werent", "weren't"),
        ("couldnt", "couldn't"),
        ("wouldnt", "wouldn't"),
        ("shouldnt", "shouldn't"),
    ] {
        let pattern = regex::Regex::new(&format!(r"(?i)\b{raw}\b")).unwrap();
        normalized = pattern.replace_all(&normalized, punctuated).into_owned();
    }
    words(&normalized)
}
fn minor_edit(raw: &str, edited: &str, names: &[String]) -> bool {
    if edited.is_empty() || edited.chars().count() > raw.chars().count() * 2 + 40 {
        return false;
    }
    let a = edit_words(raw);
    let b = edit_words(edited);
    if a.len() > 600 || b.len() > a.len() + 8 {
        return raw == edited;
    }
    // Keep digits, currency and URLs verbatim, including decimal punctuation.
    let anchors = regex::Regex::new(r"https?://\S+|[\p{N}]+(?:[.,:/-][\p{N}]+)*|[$₹€£%]").unwrap();
    if anchors
        .find_iter(raw)
        .map(|m| m.as_str())
        .collect::<Vec<_>>()
        != anchors
            .find_iter(edited)
            .map(|m| m.as_str())
            .collect::<Vec<_>>()
    {
        return false;
    }
    let mut names = names.to_vec();
    // Unknown capitalized words may be names; do not correct them by guessing.
    for token in raw.split_whitespace() {
        let token = token.trim_matches(|c: char| !c.is_alphanumeric());
        if token.chars().next().is_some_and(char::is_uppercase)
            && ![
                "I", "The", "A", "An", "It", "This", "That", "They", "We", "You", "So", "But",
                "And", "He", "She",
            ]
            .contains(&token)
        {
            names.extend(words(token));
        }
    }
    let inf = 10_000;
    let mut row = vec![inf; b.len() + 1];
    row[0] = 0;
    for (j, word) in b.iter().enumerate() {
        if ["a", "an", "the"].contains(&word.as_str()) && !names.contains(word) {
            row[j + 1] = row[j] + 1;
        }
    }
    for (i, word) in a.iter().enumerate() {
        let removable = (["a", "an", "the"].contains(&word.as_str()) && !names.contains(word))
            || (i > 0 && a[i - 1] == *word && ["i", "a", "an", "the"].contains(&word.as_str()));
        let mut next = vec![inf; b.len() + 1];
        if removable {
            next[0] = row[0] + 1;
        }
        for (j, out) in b.iter().enumerate() {
            if replacement(word, out, &names) {
                next[j + 1] = row[j] + usize::from(word != out);
            }
            if removable {
                next[j + 1] = next[j + 1].min(row[j + 1] + 1);
            }
            if ["a", "an", "the"].contains(&out.as_str()) && !names.contains(out) {
                next[j + 1] = next[j + 1].min(next[j] + 1);
            }
        }
        row = next;
    }
    row[b.len()] <= (a.len() / 6).max(1)
}

/// All source units must be present, once, in order. A questionable unit keeps
/// its raw wording; malformed/missing output simply has no edited display.
pub fn validate(value: &Value, source: &str, names: &[String]) -> Option<String> {
    let edited: Vec<EditedUnit> = serde_json::from_value(value.clone()).ok()?;
    let units = source_units(source);
    if units.is_empty() || edited.len() != units.len() {
        return None;
    }
    let mut result = String::new();
    for (index, (input, output)) in units.iter().zip(&edited).enumerate() {
        if input.id != output.unit_id {
            return None;
        }
        let corrected = apply_corrections(&input.text, &output.corrections)
            .unwrap_or_else(|| input.text.clone());
        let text = corrected.trim();
        let text = if minor_edit(&input.text, text, names) {
            text
        } else {
            &input.text
        };
        if index > 0 {
            result.push_str(if output.paragraph_start { "\n\n" } else { " " });
        }
        result.push_str(text);
    }
    (result.trim() != source.trim()).then_some(result)
}

pub fn from_proposal(proposal: &crate::extraction::Proposal, source: &str) -> Option<String> {
    let names = proposal
        .items
        .iter()
        .flat_map(|item| {
            std::iter::once(item.subject.as_str())
                .chain(item.locations.iter().map(|l| l.text.as_str()))
        })
        .flat_map(words)
        .collect::<Vec<_>>();
    validate(&proposal.readable_source, source, &names)
}
