use rekky_backend::{
    extraction::Proposal,
    taxonomy,
    understanding::{assess, merge_repair},
};
use serde_json::{Value, json};

fn proposal(items: Vec<Value>) -> Proposal {
    serde_json::from_value(json!({"items":items,"ignored_unit_ids":[],"unresolved_unit_ids":[]}))
        .unwrap()
}
fn item(subject: &str, unit: usize, text: &str) -> Value {
    json!({"subject":subject,"subject_evidence":[unit],"entity_kind":"place","experience":"firsthand","account":[{"kind":"praise","text":text,"evidence":[unit]}],"locations":[],"use_cases":[]})
}
fn review(v: &Value) -> bool {
    v["quality"]["needs_review"].as_bool().unwrap()
}

#[test]
fn equivalent_locations_and_numbers_do_not_destroy_recommendations() {
    let source = "I visited Lantern Cafe in Delhi, in Malviya Nagar and paid seven hundred rupees for pizza.";
    let mut i = item("Lantern Cafe", 1, "Paid 700 rupees for pizza.");
    i["locations"] = json!([{"role":"venue","text":"Malviya Nagar, Delhi","evidence":[1]}]);
    let a = assess(&proposal(vec![i]), source, taxonomy::vocabulary());
    assert!(!a.partial(), "{:?}", a.issues);
    assert_eq!(
        a.items[0].recommendation["locations"][0]["text"],
        "Malviya Nagar, Delhi"
    );
}

#[test]
fn one_invalid_price_keeps_category_and_valid_sibling_separate() {
    let source = "Lantern Cafe is a restaurant; I loved its pizza and paid 200 rupees. Willow Cafe had lovely noodles.";
    let mut first = item("Lantern Cafe", 1, "Loved the pizza; paid 900 rupees.");
    first["classification"] = json!({"types":[],"facets":[],"descriptors":[],"type_description":{"text":"restaurant","evidence":[1]}});
    let a = assess(
        &proposal(vec![first, item("Willow Cafe", 2, "Lovely noodles.")]),
        source,
        taxonomy::vocabulary(),
    );
    assert_eq!(a.items.len(), 2);
    assert!(review(&a.items[0].recommendation));
    assert_eq!(
        a.items[0].recommendation["classification"]["display_label"],
        "restaurant"
    );
    assert_eq!(a.items[0].recommendation["entity_kind"], "place");
    assert!(!a.items[0].body.contains("Willow"));
    assert!(!a.items[0].body.contains("900"));
    assert!(!review(&a.items[1].recommendation));
    assert!(!a.items[1].body.contains("Lantern"));
}

#[test]
fn caveat_omission_and_reversed_negation_remain_material_issues() {
    for source in [
        "I liked the pizza at Lantern Cafe but the service was painfully slow.",
        "I did not like the pizza at Lantern Cafe.",
    ] {
        let a = assess(
            &proposal(vec![item("Lantern Cafe", 1, "Loved the pizza.")]),
            source,
            taxonomy::vocabulary(),
        );
        assert!(a.partial());
        assert!(a.issues.iter().any(|i| i.code == "possible_missing_caveat"));
        assert_eq!(a.items[0].recommendation["summary"], source);
    }
}

#[test]
fn ambiguity_is_not_removed_and_currency_is_not_invented() {
    for (source, account, code) in [
        (
            "I think pizza at Lantern Cafe cost about 700 bucks.",
            "Pizza cost 700.",
            "possible_missing_qualifier",
        ),
        (
            "Pizza at Lantern Cafe cost 700 bucks.",
            "Pizza cost ₹700.",
            "unsupported_currency",
        ),
    ] {
        let a = assess(
            &proposal(vec![item("Lantern Cafe", 1, account)]),
            source,
            taxonomy::vocabulary(),
        );
        assert!(a.issues.iter().any(|i| i.code == code));
        assert!(a.partial());
    }
}

#[test]
fn compress_repetition_without_artificial_citation_limit() {
    let source =
        "I visited Lantern Cafe. I loved the pizza. The pizza was really good. It was fantastic.";
    let mut i = item("Lantern Cafe", 1, "Loved the pizza.");
    i["account"][0]["evidence"] = json!([1, 2, 3, 4]);
    let a = assess(&proposal(vec![i]), source, taxonomy::vocabulary());
    assert!(!a.partial());
    assert!(
        a.items[0].recommendation["observations"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn bad_optional_rating_does_not_block_a_useful_recommendation() {
    let mut i = item("Lantern Cafe", 1, "Loved the pizza.");
    i["rating"] = json!({"mode":"inferred","stance":"excellent","source_phrase":"Everyone loves it","evidence":[99]});
    let a = assess(
        &proposal(vec![i]),
        "I loved the pizza at Lantern Cafe.",
        taxonomy::vocabulary(),
    );
    assert!(!a.partial());
    assert!(a.items[0].recommendation["rating"].is_null());
}

#[test]
fn invalid_evidence_never_copies_a_siblings_source() {
    let source = "Lantern Cafe had good pizza. Willow Cafe was expensive.";
    let mut i = item("Lantern Cafe", 1, "Paid 500 for noodles.");
    i["account"][0]["evidence"] = json!([2]);
    let a = assess(
        &proposal(vec![i, item("Willow Cafe", 2, "It was expensive.")]),
        source,
        taxonomy::vocabulary(),
    );
    assert!(a.partial());
    assert!(!a.items[0].body.contains("Willow"));
}

#[test]
fn repair_cannot_change_other_items_or_accepted_classification() {
    let source = "Lantern Cafe had pizza for 200 rupees. Willow Cafe had lovely noodles.";
    let mut first = item("Lantern Cafe", 1, "Pizza cost 900 rupees.");
    first["classification"] = json!({"type_description":{"text":"pizza","evidence":[1]}});
    let original = proposal(vec![first, item("Willow Cafe", 2, "Lovely noodles.")]);
    let assessment = assess(&original, source, taxonomy::vocabulary());
    let repaired = proposal(vec![
        item("Lantern Cafe", 1, "Pizza cost 200 rupees."),
        item("Willow Cafe", 2, "Changed unrelated text."),
    ]);
    let merged = merge_repair(&original, repaired, &assessment);
    assert_eq!(
        merged.items[0].classification,
        original.items[0].classification
    );
    assert_eq!(
        merged.items[1].account[0].text,
        original.items[1].account[0].text
    );
    assert!(!assess(&merged, source, taxonomy::vocabulary()).partial());
}

#[test]
fn unfamiliar_type_survives_a_prose_problem() {
    let mut i = item("Mira", 1, "Restoration cost 9999.");
    i["entity_kind"] = json!("person_service");
    i["classification"] = json!({"type_description":{"text":"fountain pen restorer","evidence":[1]},"types":[],"facets":[],"descriptors":[]});
    let a = assess(
        &proposal(vec![i]),
        "Mira is a fountain pen restorer; restoration cost 200.",
        taxonomy::vocabulary(),
    );
    assert_eq!(
        a.items[0].recommendation["classification"]["descriptive_type"],
        "fountain pen restorer"
    );
}

#[test]
fn caveat_can_live_in_another_paragraph_using_the_same_source_unit() {
    let source = "I liked the pizza at Lantern Cafe but the service was slow.";
    let mut i = item("Lantern Cafe", 1, "Liked the pizza.");
    i["account"]
        .as_array_mut()
        .unwrap()
        .push(json!({"kind":"caution","text":"The service was slow.","evidence":[1]}));
    let a = assess(&proposal(vec![i]), source, taxonomy::vocabulary());
    assert!(!a.partial(), "{:?}", a.issues);
    assert_eq!(a.items[0].recommendation["summary"], "Liked the pizza.");
    assert_eq!(
        a.items[0].recommendation["observations"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn equivalent_unknown_wording_is_not_a_lost_caveat_or_a_negative_review() {
    let mut i = item("Dr Meena", 1, "Her qualifications are unknown.");
    i["entity_kind"] = json!("person_service");
    let a = assess(
        &proposal(vec![i]),
        "I do not know Dr Meena's qualifications.",
        taxonomy::vocabulary(),
    );
    assert!(!a.partial(), "{:?}", a.issues);
    assert!(
        a.items[0].recommendation["observations"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn invented_number_words_are_detected_as_well_as_digits() {
    let a = assess(
        &proposal(vec![item(
            "Lantern Cafe",
            1,
            "Pizza cost nine hundred rupees.",
        )]),
        "Pizza at Lantern Cafe cost 200 rupees.",
        taxonomy::vocabulary(),
    );
    assert!(a.issues.iter().any(|i| i.code == "unsupported_amount"));
    assert_eq!(a.items[0].recommendation["entity_kind"], "place");
}

#[test]
fn supported_restaurant_type_recovers_an_unknown_shelf() {
    let mut i = item("Lantern Cafe", 1, "Loved the pizza.");
    i["entity_kind"] = json!("unspecified");
    i["classification"] = json!({"types":[{"concept_id":"place.restaurant","source_phrase":"restaurant","evidence":[1]}],"facets":[],"descriptors":[]});
    let a = assess(
        &proposal(vec![i]),
        "Lantern Cafe is a restaurant and I loved the pizza.",
        taxonomy::vocabulary(),
    );
    assert!(!a.partial(), "{:?}", a.issues);
    assert_eq!(a.items[0].recommendation["entity_kind"], "place");
    assert_eq!(
        a.items[0].recommendation["classification"]["display_label"],
        "Restaurant"
    );
}

#[test]
fn unassigned_subjects_are_not_dumped_under_the_only_proposed_name() {
    let source = "Lantern Cafe had lovely pizza. Ravi fixed my tap, but it leaked again.";
    let original = proposal(vec![item("Lantern Cafe", 1, "Lovely pizza.")]);
    let first = assess(&original, source, taxonomy::vocabulary());
    assert!(first.partial());
    assert!(!first.items[0].body.contains("Ravi"));
    let mut repair_item = item("Ravi", 2, "Fixed my tap, but it leaked again.");
    repair_item["entity_kind"] = json!("person_service");
    let repaired = proposal(vec![item("Lantern Cafe", 1, "Lovely pizza."), repair_item]);
    let merged = merge_repair(&original, repaired, &first);
    let result = assess(&merged, source, taxonomy::vocabulary());
    assert!(rekky_backend::understanding::improves(&first, &result));
    assert_eq!(result.items.len(), 2);
    assert!(!result.partial(), "{:?}", result.issues);
    assert!(!result.items[0].body.contains("Ravi"));
}

#[test]
fn unremembered_price_is_uncertainty_rather_than_negative_sentiment() {
    let mut i = item(
        "Lantern Cafe",
        1,
        "Lantern Cafe ka price shayad 80 tha; exact yaad nahi.",
    );
    i["account"][0]["text"] =
        json!("The price at Lantern Cafe may have been 80; I don't remember exactly.");
    let a = assess(
        &proposal(vec![i]),
        "Lantern Cafe ka price shayad 80 tha; exact yaad nahi.",
        taxonomy::vocabulary(),
    );
    assert!(!a.partial(), "{:?}", a.issues);
}

#[test]
fn metadata_can_cover_a_pure_identity_and_location_sentence() {
    let mut i = item("Dr Meena", 1, "She listens carefully.");
    i["account"][0]["evidence"] = json!([2]);
    i["entity_kind"] = json!("person_service");
    i["locations"] = json!([{"role":"practice","text":"Jaipur","evidence":[1]}]);
    let a = assess(
        &proposal(vec![i]),
        "Dr Meena practices in Jaipur. She listens carefully.",
        taxonomy::vocabulary(),
    );
    assert!(!a.partial(), "{:?}", a.issues);
    assert_eq!(
        a.items[0].recommendation["summary"],
        "She listens carefully."
    );
}

#[test]
fn a_source_stated_provider_base_does_not_require_specific_category_rules() {
    let mut i = item("Mira", 1, "Mira is a fountain pen restorer in Kochi.");
    i["entity_kind"] = json!("person_service");
    i["classification"] = json!({"types":[],"facets":[],"descriptors":[],"type_description":{"text":"fountain pen restorer","evidence":[1]}});
    i["locations"] = json!([{"role":"practice","text":"Kochi","evidence":[1]}]);
    let a = assess(
        &proposal(vec![i]),
        "Mira is a fountain pen restorer in Kochi.",
        taxonomy::vocabulary(),
    );
    assert!(!a.partial(), "{:?}", a.issues);
    assert_eq!(
        a.items[0].recommendation["locations"][0]["role"],
        "practice"
    );
}
