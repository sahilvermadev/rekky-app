use rekky_backend::{
    extraction::{Proposal, source_units, validate as understand},
    taxonomy::*,
};
use serde_json::{Value, json};
use std::collections::BTreeSet;

fn classification(types: Value, facets: Value, descriptors: Value) -> ProposedClassification {
    serde_json::from_value(json!({"types":types,"facets":facets,"descriptors":descriptors}))
        .unwrap()
}
fn a(id: &str, phrase: &str, evidence: usize) -> Value {
    json!({"concept_id":id,"source_phrase":phrase,"evidence":[evidence]})
}
#[test]
fn vocabulary_has_unique_ids_aliases_and_acyclic_compatible_parents() {
    let mut ids = BTreeSet::new();
    let mut aliases = BTreeSet::new();
    for c in &vocabulary().concepts {
        assert!(ids.insert(&c.id));
        assert!(!c.label.is_empty());
        for alias in &c.aliases {
            assert!(aliases.insert(words(alias)), "ambiguous alias: {alias}");
        }
        let mut parents = BTreeSet::new();
        let mut current = Some(c);
        while let Some(node) = current {
            assert!(parents.insert(&node.id), "cycle");
            current = node.parent_id.as_deref().map(|id| {
                let parent = concept(id).expect("valid parent");
                assert_eq!(parent.entity_kinds, node.entity_kinds);
                parent
            });
        }
    }
}
#[test]
fn synonymous_sources_share_one_label_and_broader_search_concept() {
    for phrase in [
        "general doctor",
        "general physician",
        "GP",
        "सामान्य चिकित्सक",
    ] {
        let source = format!("Neha is a {phrase}.");
        let p = classification(
            json!([a("service.general_doctor", phrase, 1)]),
            json!([]),
            json!([]),
        );
        let (value, evidence) = validate(&p, "person_service", &source_units(&source));
        assert_eq!(value["display_label"], "General doctor");
        assert!(
            value["search_ids"]
                .as_array()
                .unwrap()
                .contains(&json!("service.doctor"))
        );
        assert!(!value.to_string().contains("source_phrase"));
        assert!(evidence.to_string().contains("source_phrase"));
    }
}
#[test]
fn restaurant_and_cuisine_compose_without_creating_combination_categories() {
    let p = classification(
        json!([a("place.restaurant", "restaurant", 1)]),
        json!([a("cuisine.italian", "Italian", 2)]),
        json!([]),
    );
    let (value, _) = validate(
        &p,
        "place",
        &source_units("Lantern is a restaurant. It serves Italian food."),
    );
    assert_eq!(value["display_label"], "Italian restaurant");
    assert_eq!(value["types"].as_array().unwrap().len(), 1);
    assert_eq!(value["facets"][0]["id"], "cuisine.italian");
}

#[test]
fn registry_corrects_a_supported_cuisine_in_the_wrong_array_and_deduplicates_descriptions() {
    let p = classification(
        json!([
            a("place.restaurant", "restaurant", 1),
            a("cuisine.italian", "Italian food", 1)
        ]),
        json!([]),
        json!([
            {"text":"restaurant","evidence":[1]}, {"text":"Italian food","evidence":[1]}
        ]),
    );
    let value = validate(
        &p,
        "place",
        &source_units("Lantern is a restaurant serving Italian food."),
    )
    .0;
    assert_eq!(value["display_label"], "Italian restaurant");
    assert_eq!(value["types"].as_array().unwrap().len(), 1);
    assert_eq!(value["facets"][0]["id"], "cuisine.italian");
    assert_eq!(value["descriptors"], json!([]));
}
#[test]
fn specificity_invalid_ids_bad_evidence_and_negation_do_not_invent_types() {
    let p = classification(
        json!([
            a("service.general_doctor", "doctor", 1),
            a("invented.specialist", "doctor", 1),
            a("service.doctor", "doctor", 99),
            a("place.bar", "doctor", 1)
        ]),
        json!([]),
        json!([]),
    );
    let (value, _) = validate(&p, "person_service", &source_units("Neha is a doctor."));
    assert!(value["types"].as_array().unwrap().is_empty());
    let p = classification(
        json!([a("service.doctor", "doctor", 1)]),
        json!([]),
        json!([]),
    );
    for source in [
        "He is not a doctor.",
        "He is no longer a doctor.",
        "वह doctor नहीं है।",
    ] {
        assert!(
            validate(&p, "person_service", &source_units(source)).0["types"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }
}
#[test]
fn parent_child_selection_uses_specific_type_but_keeps_parent_searchable() {
    let p = classification(
        json!([
            a("service.doctor", "doctor", 1),
            a("service.general_doctor", "general doctor", 1)
        ]),
        json!([]),
        json!([]),
    );
    let value = validate(
        &p,
        "person_service",
        &source_units("Neha is a general doctor."),
    )
    .0;
    assert_eq!(value["display_label"], "General doctor");
    assert_eq!(value["types"].as_array().unwrap().len(), 1);
}
#[test]
fn unfamiliar_descriptions_survive_without_global_ids_or_invented_skills() {
    let p = classification(
        json!([]),
        json!([]),
        json!([
            {"text":"ceramic glaze consultant","evidence":[1]},
            {"text":"expert in all ceramics","evidence":[1]}
        ]),
    );
    let value = validate(
        &p,
        "person_service",
        &source_units("Ria is a ceramic glaze consultant."),
    )
    .0;
    assert_eq!(value["descriptors"], json!(["ceramic glaze consultant"]));
    assert!(value["types"].as_array().unwrap().is_empty());
    assert!(
        value["search_terms"]
            .as_str()
            .unwrap()
            .contains("ceramic glaze consultant")
    );
}
#[test]
fn category_queries_use_longest_aliases_and_retain_location_words() {
    let (ids, rest) = query_concepts("general physician in Dwarka");
    assert_eq!(ids, vec!["service.general_doctor"]);
    assert_eq!(rest, vec!["in", "dwarka"]);
    let (ids, _) = query_concepts("Italian restaurants in Pune");
    assert_eq!(ids, vec!["cuisine.italian", "place.restaurant"]);
    assert!(query_concepts("barometer").0.is_empty());
}
#[test]
fn optional_classification_never_discards_a_valid_memory_or_borrows_sibling_units() {
    let source = "Ria helped me. There is a bar nearby.";
    let mut proposal = json!({"items":[{"subject":"Ria","subject_evidence":[1],"entity_kind":"person_service","experience":"firsthand",
        "summary":{"text":"Ria helped me.","evidence":[1]},"observations":[],"locations":[],"use_cases":[],
        "classification":{"types":[],"facets":[],"descriptors":[{"text":"bar","evidence":[2]}]}}],
        "ignored_unit_ids":[],"unresolved_unit_ids":[2]});
    let parsed: Proposal = serde_json::from_value(proposal.clone()).unwrap();
    let (items, partial) = understand(parsed, source).unwrap();
    assert!(partial);
    assert_eq!(
        items[0].recommendation["classification"]["descriptors"],
        json!([])
    );
    proposal["items"][0]["classification"] = json!({"bad":"shape"});
    assert!(understand(serde_json::from_value(proposal).unwrap(), source).is_ok());
}
