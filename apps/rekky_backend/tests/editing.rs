use rekky_backend::editing::EditInput;
use serde_json::{Value, json};
fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../contracts/rekky/v1/fixtures/recommendation_edit.json"
    ))
    .unwrap()
}
#[test]
fn shared_edit_contract_preserves_owner_words_and_builds_search_projection() {
    let input: EditInput = serde_json::from_value(fixture()).unwrap();
    let (rec, body) = input.normalized().unwrap().build().unwrap();
    assert_eq!(rec["summary"], fixture()["summary"]);
    assert_eq!(rec["observations"][1]["kind"], "caution");
    assert!(body.contains("Priya"));
    assert!(!body.contains("https://"));
    assert_eq!(rec["classification"]["display_label"], "Italian restaurant");
}
#[test]
fn rejects_unsafe_links_bad_categories_and_overlong_content() {
    for url in [
        "javascript:alert(1)",
        "file:///tmp/x",
        "http://example.com",
        "https://user:password@example.com",
        "not a link",
    ] {
        let mut v = fixture();
        v["destination"]["url"] = json!(url);
        assert!(
            serde_json::from_value::<EditInput>(v)
                .unwrap()
                .normalized()
                .is_err()
        );
    }
    for (field, value) in [
        ("subject", json!(" ")),
        ("summary", json!("x".repeat(20001))),
        ("entity_kind", json!("made_up")),
        ("experience", json!("verified")),
    ] {
        let mut v = fixture();
        v[field] = value;
        assert!(
            serde_json::from_value::<EditInput>(v)
                .unwrap()
                .normalized()
                .is_err()
        );
    }
    let mut v = fixture();
    v["types"] = json!(["service.doctor"]);
    assert!(
        serde_json::from_value::<EditInput>(v)
            .unwrap()
            .normalized()
            .unwrap()
            .build()
            .is_err()
    );
    let mut v = fixture();
    v["origin"] = json!("verified");
    assert!(serde_json::from_value::<EditInput>(v).is_err());
}
