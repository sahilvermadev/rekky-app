use rekky_backend::extraction::{Proposal, preserve_unresolved, source_units, validate};
use serde_json::{Value, json};
fn sample() -> (String, Value) {
    ("I visited Lantern Cafe in Pune. I loved the noodles. Portions are small. A starter was about 180 when I visited. Um.".into(),json!({
        "items":[{"subject":"Lantern Cafe","subject_evidence":[1],"entity_kind":"place","experience":"firsthand",
          "summary":{"text":"You recommend Lantern Cafe for its noodles, with small portions worth keeping in mind.","evidence":[1,2,3]},
          "observations":[{"kind":"suggestion","text":"Try the noodles you enjoyed.","evidence":[2]},
            {"kind":"caution","text":"Portions are small.","evidence":[3]},
            {"kind":"price","text":"You recall a starter costing about 180 during your visit; current price unverified.","evidence":[4]}],
          "locations":[{"role":"venue","text":"Pune","evidence":[1]}],
          "use_cases":[{"text":"Noodles","evidence":[2]}]}],"ignored_unit_ids":[5],"unresolved_unit_ids":[]
    }))
}
fn decode(value: Value) -> Proposal {
    serde_json::from_value(value).unwrap()
}
#[test]
fn concise_recommendation_keeps_specifics_and_private_evidence_separate() {
    let (source, p) = sample();
    let (items, partial) = validate(decode(p), &source).unwrap();
    let item = &items[0];
    assert!(!partial);
    assert_eq!(item.recommendation["shelf"], "Places");
    assert!(item.body.contains("about 180"));
    assert!(item.body.contains("small"));
    assert!(!item.recommendation.to_string().contains("subject_evidence"));
    assert!(item.evidence.to_string().contains("subject_evidence"));
    assert_ne!(item.body, source);
}
#[test]
fn invalid_references_invented_numbers_and_locations_are_rejected() {
    let (source, base) = sample();
    for change in 0..3 {
        let mut p = base.clone();
        match change {
            0 => p["items"][0]["summary"]["evidence"] = json!([99]),
            1 => p["items"][0]["observations"][2]["text"] = json!("Costs 120 today."),
            _ => p["items"][0]["locations"][0]["text"] = json!("Mumbai"),
        }
        assert!(validate(decode(p), &source).is_err());
    }
}
#[test]
fn missed_material_or_falsely_ignored_caveat_is_partial_not_completed() {
    let (source, mut p) = sample();
    p["items"][0]["summary"]["evidence"] = json!([1, 2]);
    p["items"][0]["observations"]
        .as_array_mut()
        .unwrap()
        .remove(1);
    p["ignored_unit_ids"] = json!([3, 5]);
    let (_, partial) = validate(decode(p), &source).unwrap();
    assert!(partial);
}
#[test]
fn fallback_never_keeps_an_unsupported_model_claim() {
    let (source, mut p) = sample();
    p["items"][0]["summary"]["text"] = json!("Awarded 7 stars.");
    let p = decode(p);
    assert!(validate(p.clone(), &source).is_err());
    let (items, partial) = preserve_unresolved(p, &source).unwrap();
    assert!(partial);
    assert_eq!(items[0].body, source);
    assert!(items[0].recommendation.is_null());
}
#[test]
fn segmentation_preserves_unicode_names_abbreviations_and_decimals() {
    let s = "Dr. Neha works in Jaipur. लागत 180.50 थी। ठीक है!";
    let u = source_units(s);
    assert_eq!(u.len(), 3);
    assert_eq!(u[0].text, "Dr. Neha works in Jaipur.");
    assert!(u[1].text.contains("180.50"));
    for unit in u {
        assert_eq!(&s[unit.start..unit.end], unit.text);
    }
}
#[test]
fn empty_or_overflow_output_is_not_a_finished_recommendation() {
    let (source, mut p) = sample();
    p["items"] = json!([]);
    assert!(validate(decode(p), &source).is_err());
    let (source, mut p) = sample();
    p["unresolved_unit_ids"] = json!([4]);
    assert!(validate(decode(p), &source).unwrap().1);
}

#[test]
fn a_past_job_cannot_be_promoted_to_practice_or_service_coverage() {
    let source = "Ravi fixed my tap in Pune.";
    for role in ["practice", "service_area"] {
        let proposal = decode(
            json!({"items":[{"subject":"Ravi","subject_evidence":[1],"entity_kind":"person_service","experience":"firsthand",
        "summary":{"text":"Ravi fixed your tap in Pune.","evidence":[1]},"observations":[],"locations":[{"role":role,"text":"Pune","evidence":[1]}],"use_cases":[]}],"ignored_unit_ids":[],"unresolved_unit_ids":[]}),
        );
        let (items, partial) = validate(proposal, source).unwrap();
        assert!(partial);
        assert_eq!(
            items[0].recommendation["locations"][0]["role"],
            "past_experience"
        );
    }
}
#[test]
fn caveat_is_visible_even_when_model_groups_it_as_suitability() {
    let (source, mut p) = sample();
    p["items"][0]["observations"][1]["kind"] = json!("suitability");
    let (items, _) = validate(decode(p), &source).unwrap();
    assert_eq!(
        items[0].recommendation["observations"][1]["kind"],
        "caution"
    );
}
#[test]
fn an_unspoken_currency_is_not_added_to_a_spoken_amount() {
    let (source, mut p) = sample();
    p["items"][0]["observations"][2]["text"] = json!("About ₹180 per starter.");
    assert!(validate(decode(p), &source).is_err());
}

#[test]
fn unsupported_optional_use_cases_do_not_become_indexed_claims() {
    let (source, mut p) = sample();
    p["items"][0]["use_cases"] =
        json!([{"text":"Top-rated noodles for all occasions","evidence":[2]}]);
    let (items, _) = validate(decode(p), &source).unwrap();
    assert_eq!(items[0].recommendation["use_cases"], json!([]));
    assert!(!items[0].body.contains("Top-rated"));
}
