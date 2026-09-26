use rekky_backend::{extraction::source_units, readable_source::validate};
use serde_json::{Value, json};
fn proposal(source: &str, replacements: &[&str]) -> Value {
    json!(
        source_units(source)
            .iter()
            .zip(replacements)
            .map(|(unit, text)| json!({"unit_id":unit.id,"corrections":[{"before":unit.text,"after":text}],"paragraph_start":false}))
            .collect::<Vec<_>>()
    )
}
#[test]
fn allows_light_grammar_punctuation_and_known_typos() {
    let raw = "i liked teh place. the tables was small but i didnt mind.";
    let edited = "I liked the place. The tables were small, but I didn't mind.";
    assert_eq!(
        validate(
            &proposal(
                raw,
                &[
                    "I liked the place.",
                    "The tables were small, but I didn't mind."
                ]
            ),
            raw,
            &[]
        )
        .as_deref(),
        Some(edited)
    );
}
#[test]
fn keeps_names_numbers_negation_uncertainty_and_meaning() {
    for (raw, bad) in [
        ("I liked Ravi.", "I liked Rani."),
        ("It cost 120 bucks.", "It cost 150 bucks."),
        ("It cost 1.20 bucks.", "It cost 120 bucks."),
        ("I did not like it.", "I did like it."),
        ("I didnt like it.", "I did like it."),
        ("Maybe it helps.", "It helps."),
        ("It was really really good.", "It was really good."),
        ("It was great.", "It was a treat."),
        ("I found it useful.", "Everyone will find it useful."),
    ] {
        assert_eq!(
            validate(&proposal(raw, &[bad]), raw, &[]),
            None,
            "{raw} -> {bad}"
        );
    }
    assert_eq!(
        validate(
            &proposal("i liked resturant.", &["I liked restaurant."]),
            "i liked resturant.",
            &["resturant".into()]
        )
        .as_deref(),
        None
    );
}
#[test]
fn preserves_mixed_language_instead_of_translating() {
    let raw = "yeh cafe really अच्छा है, maybe phir jaunga.";
    assert_eq!(
        validate(
            &proposal(raw, &["Yeh cafe really अच्छा है. Maybe phir jaunga."]),
            raw,
            &[]
        )
        .as_deref(),
        Some("Yeh cafe really अच्छा है. Maybe phir jaunga.")
    );
    assert_eq!(
        validate(
            &proposal(raw, &["This cafe is very good. I will return."]),
            raw,
            &[]
        ),
        None
    );
}
#[test]
fn requires_complete_ordered_units_and_falls_back_per_unit() {
    let raw = "i liked it. Maybe it helps.";
    assert_eq!(validate(&json!([]), raw, &[]), None);
    assert_eq!(validate(&json!(null), raw, &[]), None);
    assert_eq!(validate(&proposal(raw, &["I liked it."]), raw, &[]), None);
    let mut value = proposal(raw, &["I liked it.", "It helps everyone."]);
    value[1]["paragraph_start"] = json!(true);
    assert_eq!(
        validate(&value, raw, &[]).as_deref(),
        Some("I liked it.\n\nMaybe it helps.")
    );
    value[1]["unit_id"] = json!(1);
    assert_eq!(validate(&value, raw, &[]), None);
}

#[test]
fn rejects_ambiguous_overlapping_or_invented_correction_spans() {
    let raw = "i liked the tea and the coffee.";
    for corrections in [
        json!([{"before":"the","after":"a"}]),
        json!([{"before":"missing","after":"new"}]),
        json!([{"before":"the tea","after":"the tea"},{"before":"tea","after":"tea"}]),
        json!([{"before":"","after":"New fact"}]),
    ] {
        assert_eq!(
            validate(
                &json!([{"unit_id":1,"corrections":corrections,"paragraph_start":false}]),
                raw,
                &[]
            ),
            None
        );
    }
    let corrections = json!([{"unit_id":1,"corrections":[{"before":"i liked","after":"I liked"}],"paragraph_start":false}]);
    assert_eq!(
        validate(&corrections, raw, &[]).as_deref(),
        Some("I liked the tea and the coffee.")
    );
}

#[test]
fn does_not_remove_an_article_from_a_protected_name() {
    let raw = "The Rose was good.";
    assert_eq!(
        validate(
            &proposal(raw, &["Rose was good."]),
            raw,
            &["the".into(), "rose".into()]
        ),
        None
    );
}
