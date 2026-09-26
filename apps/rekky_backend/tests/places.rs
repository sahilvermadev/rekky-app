use rekky_backend::places::{PlaceQuery, match_response, query};
use serde_json::{Value, json};
fn candidate(id: &str, name: &str, address: &str) -> Value {
    json!({"id":id,"displayName":{"text":name},"formattedAddress":address,
        "businessStatus":"OPERATIONAL","types":["establishment","restaurant"]})
}
#[test]
fn matches_name_and_every_locality_token_but_never_just_the_top_result() {
    let q = PlaceQuery {
        subject: "The Cedar Cafe".into(),
        locality: "Old Town in Pune".into(),
    };
    let good = candidate("place-a", "CEDAR", "12 Example Street, Old Town, Pune");
    assert_eq!(
        match_response(&q, &json!({"places":[good.clone()]}))
            .unwrap()
            .place_id,
        "place-a"
    );
    assert!(
        match_response(
            &q,
            &json!({"places":[candidate("other","Cedar","Delhi"),good.clone()]})
        )
        .is_some()
    );
    assert!(match_response(&q,&json!({"places":[good.clone(),candidate("branch","Cedar Cafe","14 Example Street, Old Town, Pune")]})).is_none());
    assert!(match_response(&q, &json!({"places":[good],"nextPageToken":"more"})).is_none());
    assert!(
        match_response(
            &q,
            &json!({"places":[candidate("wrong","Cedar Garden","Old Town, Pune")]})
        )
        .is_none()
    );
}
#[test]
fn refuses_closed_unattributed_or_non_business_results_and_maps_city_aliases() {
    let q = PlaceQuery {
        subject: "Cedar Bar".into(),
        locality: "Indiranagar in Bangalore".into(),
    };
    let good = candidate("place-b", "Cedar Bar", "Indiranagar, Bengaluru");
    assert!(match_response(&q, &json!({"places":[good.clone()]})).is_some());
    for (key, value) in [
        ("businessStatus", json!("CLOSED_PERMANENTLY")),
        ("types", json!(["street_address"])),
        ("attributions", json!([{"provider":"another source"}])),
        ("id", json!("../../evil")),
    ] {
        let mut bad = good.clone();
        bad[key] = value;
        assert!(match_response(&q, &json!({"places":[bad]})).is_none());
    }
}
#[test]
fn only_queries_eligible_public_venues_with_one_explicit_location_and_auto_link() {
    let r = json!({"entity_kind":"place","locations":[{"role":"venue","text":"Pune"}],"classification":{"types":[{"id":"place.restaurant"}]}});
    assert!(query("Cedar", &r).is_some());
    for (key, value) in [
        ("entity_kind", json!("person_service")),
        ("destination", json!({"mode":"none"})),
        ("destination", json!({"mode":"custom"})),
        ("classification", json!({"types":[]})),
        (
            "locations",
            json!([{"role":"past_experience","text":"Pune"}]),
        ),
        (
            "locations",
            json!([{"role":"venue","text":"Pune"},{"role":"venue","text":"Delhi"}]),
        ),
    ] {
        let mut bad = r.clone();
        bad[key] = value;
        assert!(query("Cedar", &bad).is_none());
    }
}

#[test]
fn name_word_order_and_city_words_in_brands_are_not_interchangeable() {
    for (subject, other) in [
        ("Red Rose Cafe", "Rose Red Cafe"),
        ("Bombay Cafe", "Mumbai Cafe"),
    ] {
        let q = PlaceQuery {
            subject: subject.into(),
            locality: "Pune".into(),
        };
        assert!(match_response(&q, &json!({"places":[candidate("wrong",other,"Pune")]})).is_none());
    }
}
