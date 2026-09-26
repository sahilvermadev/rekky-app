use rekky_backend::{
    extraction::source_units,
    ratings::{self, Edit},
};
use serde_json::{Value, json};
fn proposed(mode: &str, stance: &str, value: Value, phrase: &str) -> Value {
    json!({"mode":mode,"stance":stance,"spoken_value":value,"source_phrase":phrase,"evidence":[1]})
}
#[test]
fn rubric_and_shared_wire_preserve_rating_provenance() {
    let wire: Value = serde_json::from_str(include_str!(
        "../../../contracts/rekky/v1/fixtures/ratings.json"
    ))
    .unwrap();
    let units = source_units("I liked Lantern Cafe.");
    for (stance, score) in [
        ("awful", 1),
        ("very_bad", 2),
        ("bad", 3),
        ("disappointing", 4),
        ("mixed", 5),
        ("okay", 6),
        ("good", 7),
        ("very_good", 8),
        ("excellent", 9),
        ("exceptional", 10),
    ] {
        let out = ratings::validate(
            &proposed("inferred", stance, Value::Null, "I liked Lantern Cafe."),
            "firsthand",
            &units,
        );
        assert_eq!(out["value"].as_f64(), Some(f64::from(score)));
        if score == 7 {
            assert_eq!(out, wire["inferred"]);
        }
    }
    let units = source_units("I give Lantern Cafe four and a half out of five.");
    assert_eq!(
        ratings::validate(
            &proposed("spoken", "none", json!(4.5), &units[0].text),
            "firsthand",
            &units
        ),
        wire["spoken"]
    );
}
#[test]
fn abstains_for_bad_support_hearsay_interest_unknown_stance_and_invalid_scales() {
    let units = source_units("I liked Lantern Cafe.");
    let good = proposed("inferred", "good", Value::Null, &units[0].text);
    for experience in ["secondhand", "interest", "unspecified"] {
        assert!(ratings::validate(&good, experience, &units).is_null());
    }
    for bad in [
        Value::Null,
        json!({}),
        proposed("inferred", "unknown", Value::Null, &units[0].text),
        proposed("inferred", "good", json!(4), &units[0].text),
        proposed("inferred", "good", Value::Null, "another subject is great"),
    ] {
        assert!(ratings::validate(&bad, "firsthand", &units).is_null());
    }
    let mut foreign = good;
    foreign["evidence"] = json!([2]);
    assert!(ratings::validate(&foreign, "firsthand", &units).is_null());
    for text in [
        "I paid 4 for coffee.",
        "A 4 star hotel.",
        "I give it 4 out of 20.",
    ] {
        let units = source_units(text);
        assert!(
            ratings::validate(
                &proposed("spoken", "none", json!(4), text),
                "firsthand",
                &units
            )
            .is_null()
        );
    }
    for (text, value) in [
        ("I give it 4.5/5.", 4.5),
        ("I give it four out of five.", 4.0),
        ("I give it 0/5.", 0.0),
    ] {
        assert_eq!(
            ratings::validate(
                &proposed("spoken", "none", json!(value), text),
                "firsthand",
                &source_units(text)
            )["value"],
            json!(value * 2.0)
        );
    }
}
#[test]
fn owner_can_override_remove_and_keep_without_relabelling_an_estimate() {
    let previous = json!({"entity_kind":"place","experience":"firsthand","summary":"Nice", "observations":[], "rating":{"value":7,"scale":10,"origin":"inferred","rubric_version":1}});
    let mut next = previous.clone();
    Edit::Keep.apply("Cafe", "Cafe", &previous, &mut next);
    assert_eq!(next["rating"], previous["rating"]);
    next["summary"] = json!("Disappointing");
    Edit::Keep.apply("Cafe", "Cafe", &previous, &mut next);
    assert!(next["rating"].is_null());
    Edit::Set { value: 2.5 }.apply("Cafe", "Cafe", &previous, &mut next);
    assert_eq!(
        next["rating"],
        json!({"value":2.5,"scale":10,"origin":"user"})
    );
    Edit::None.apply("Cafe", "Cafe", &previous, &mut next);
    assert!(next["rating"].is_null());
    for v in [-1.0, 10.1, f64::NAN] {
        assert!((Edit::Set { value: v }).check().is_err());
    }
    assert!(
        serde_json::from_value::<Edit>(json!({"mode":"set","value":4,"origin":"spoken"})).is_err()
    );
    let mut old_user = previous.clone();
    old_user["rating"]["origin"] = json!("user");
    Edit::Keep.apply("Cafe", "New subject", &old_user, &mut next);
    assert!(next["rating"].is_null());
}

#[test]
fn uncertainty_aspect_only_and_negated_scores_abstain_even_if_model_proposes_rating() {
    for source in [
        "I tried Cafe. I would not give it five out of five. I am not sure what score to give it.",
        "Food was four out of five, but I cannot give an overall rating yet.",
    ] {
        let units = source_units(source);
        let mut proposal = proposed("inferred", "mixed", Value::Null, &units[0].text);
        proposal["evidence"] = json!([1]);
        assert!(ratings::validate(&proposal, "firsthand", &units).is_null());
    }
    for text in [
        "The food was four out of five.",
        "I would not give it five out of five.",
    ] {
        assert!(
            ratings::validate(
                &proposed("spoken", "none", json!(4), text),
                "firsthand",
                &source_units(text)
            )
            .is_null()
        );
    }
}

#[test]
fn spoken_ten_point_values_are_not_doubled_and_original_scale_survives() {
    for (text, value) in [
        ("Overall I give it seven and a half out of ten.", 7.5),
        ("My rating is 9/10.", 9.0),
        ("Overall ten out of ten.", 10.0),
    ] {
        let score = ratings::validate(
            &proposed("spoken", "none", json!(value), text),
            "firsthand",
            &source_units(text),
        );
        assert_eq!(score["value"], json!(value));
        assert_eq!(score["scale"], 10);
        assert_eq!(score["spoken"], json!({"value":value,"scale":10.0}));
    }
}
