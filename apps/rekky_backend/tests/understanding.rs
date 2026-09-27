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

#[test]
fn ratings_are_optional_and_cannot_claim_coverage_or_leak_private_quotes() {
    let (source, mut p) = sample();
    p["items"][0]["rating"] = json!({"mode":"inferred","stance":"good","spoken_value":null,"source_phrase":"I loved the noodles.","evidence":[2,3]});
    let (items, partial) = validate(decode(p.clone()), &source).unwrap();
    assert!(!partial);
    assert_eq!(items[0].recommendation["rating"]["origin"], "inferred");
    assert!(
        !items[0]
            .recommendation
            .to_string()
            .contains("source_phrase")
    );
    p["items"][0]["rating"]["evidence"] = json!([99]);
    let (items, partial) = validate(decode(p), &source).unwrap();
    assert!(!partial);
    assert!(items[0].recommendation["rating"].is_null());
}

#[test]
fn subject_and_retrieval_citations_cannot_mask_a_missing_experience() {
    let (source, mut p) = sample();
    p["items"][0]["subject_evidence"] = json!([1, 2, 3, 4]);
    p["items"][0]["summary"]["evidence"] = json!([1, 2]);
    p["items"][0]["observations"]
        .as_array_mut()
        .unwrap()
        .remove(2);
    p["items"][0]["use_cases"] = json!([{"text":"when I visited","evidence":[4]}]);
    assert!(validate(decode(p), &source).unwrap().1);
}

#[test]
fn service_coverage_is_distinct_from_an_airport_visit_or_address() {
    for (source, role, expected, partial) in [
        (
            "Mira Cabs provides taxis in Kochi.",
            "service_area",
            "service_area",
            false,
        ),
        ("Mira Cabs took me to Kochi.", "venue", "context", true),
        (
            "Mira Cabs repaired my vehicle in Kochi.",
            "service_area",
            "past_experience",
            true,
        ),
    ] {
        let p = json!({"items":[{"subject":"Mira Cabs","subject_evidence":[1],"entity_kind":"person_service","experience":"firsthand","summary":{"text":source,"evidence":[1]},"observations":[],"locations":[{"role":role,"text":"Kochi","evidence":[1]}],"use_cases":[]}],"ignored_unit_ids":[],"unresolved_unit_ids":[]});
        let (items, is_partial) = validate(decode(p), source).unwrap();
        assert_eq!(is_partial, partial);
        assert_eq!(items[0].recommendation["locations"][0]["role"], expected);
    }
}

#[test]
fn concise_account_does_not_need_duplicate_observations_for_coverage() {
    let p = json!({"items":[{"subject":"Mira","subject_evidence":[1],"entity_kind":"person_service","experience":"firsthand","summary":{"text":"Used for two days; a decent experience.","evidence":[2]},"observations":[{"kind":"price","text":"About 900 rupees for a ride, from memory.","evidence":[3]}],"locations":[],"use_cases":[]}],"ignored_unit_ids":[],"unresolved_unit_ids":[]});
    let (items, partial) = validate(decode(p), "I recommend Mira. I used her for two days and it was decent. I think a ride cost about 900 rupees.").unwrap();
    assert!(!partial);
    assert_eq!(
        items[0].recommendation["observations"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn single_account_projects_to_editable_prose_without_a_second_summary() {
    let p = json!({"items":[{"subject":"Mira","subject_evidence":[1],"entity_kind":"person_service","experience":"firsthand","account":[{"kind":"context","text":"Used for two days; a decent experience.","evidence":[2]},{"kind":"price","text":"About 900 rupees for a ride, from memory.","evidence":[3]}],"locations":[],"use_cases":[]}],"ignored_unit_ids":[],"unresolved_unit_ids":[]});
    let (items, partial) = validate(decode(p), "I recommend Mira. I used her for two days and it was decent. I think a ride cost about 900 rupees.").unwrap();
    assert!(!partial);
    let r = &items[0].recommendation;
    assert_eq!(r["summary"], "Used for two days; a decent experience.");
    assert_eq!(r["observations"].as_array().unwrap().len(), 1);
    assert!(!r.to_string().contains("evidence"));
    let schema = rekky_backend::extraction::schema();
    let properties = &schema["properties"]["items"]["items"]["properties"];
    assert!(properties.get("summary").is_none());
    assert!(properties.get("account").is_some());
}

#[test]
fn a_negative_opening_keeps_its_caution_signal_and_invented_amounts_fail() {
    let p = json!({"items":[{"subject":"Trail Mug","subject_evidence":[1],"entity_kind":"thing","experience":"firsthand","account":[{"kind":"caution","text":"The Trail Mug leaks when sideways.","evidence":[1]}],"locations":[],"use_cases":[]}],"ignored_unit_ids":[],"unresolved_unit_ids":[]});
    let (items, _) = validate(decode(p.clone()), "The Trail Mug leaks when sideways.").unwrap();
    assert_eq!(
        items[0].recommendation["observations"][0]["kind"],
        "caution"
    );
    let mut bad = p;
    bad["items"][0]["account"][0]["text"] = json!("The Trail Mug costs 900 rupees.");
    assert!(validate(decode(bad), "The Trail Mug leaks when sideways.").is_err());
}

#[test]
fn an_activity_can_have_a_venue_without_becoming_a_service_address() {
    let p = json!({"items":[{"subject":"weaving workshop at Cedar House","subject_evidence":[1],"entity_kind":"activity_event","experience":"firsthand","account":[{"kind":"context","text":"Attended a weaving workshop at Cedar House in May.","evidence":[1]}],"locations":[{"role":"venue","text":"Cedar House","evidence":[1]}],"use_cases":[]}],"ignored_unit_ids":[],"unresolved_unit_ids":[]});
    let (items, partial) = validate(
        decode(p),
        "I attended a weaving workshop at Cedar House in May.",
    )
    .unwrap();
    assert!(!partial);
    assert_eq!(items[0].recommendation["locations"][0]["role"], "venue");
}

#[test]
fn nearby_city_is_not_exact_practice_or_service_coverage() {
    let source = "Mira is based near Kochi and was helpful.";
    let proposal=serde_json::from_value(serde_json::json!({"items":[{"subject":"Mira","subject_evidence":[1],"entity_kind":"person_service","experience":"firsthand","summary":{"text":source,"evidence":[1]},"observations":[],"locations":[{"role":"practice","text":"Kochi","evidence":[1]}],"use_cases":[]}],"ignored_unit_ids":[],"unresolved_unit_ids":[]})).unwrap();
    let (items, _) = rekky_backend::extraction::validate(proposal, source).unwrap();
    assert_eq!(items[0].recommendation["locations"][0]["role"], "context");
}
