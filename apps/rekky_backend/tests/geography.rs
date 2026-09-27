use rekky_backend::geography::{Area, clean_name, normalized, resolve};
use serde_json::json;
fn area(id: &str, name: &str, aliases: &[&str], parents: &[(&str, &str)]) -> Area {
    Area {
        id: id.into(),
        name: name.into(),
        label: name.into(),
        country: "IN".into(),
        feature: "PPL".into(),
        population: 0,
        aliases: aliases.iter().map(|s| normalized(s)).collect(),
        ancestors: parents.iter().map(|p| p.0.to_string()).collect(),
        hierarchy: json!(
            parents
                .iter()
                .map(|p| json!({"id":p.0,"kind":p.1,"name":p.0}))
                .collect::<Vec<_>>()
        ),
    }
}
#[test]
fn clean_display_never_turns_nearby_or_a_trip_into_coverage() {
    assert_eq!(clean_name("based right here in Delhi"), "Delhi");
    assert_eq!(clean_name("serves Landour"), "Landour");
    assert_eq!(
        clean_name("located in Connaught Place in Delhi"),
        "Connaught Place in Delhi"
    );
    assert_eq!(clean_name("near Delhi"), "near Delhi");
    assert_eq!(clean_name("serves around Delhi"), "around Delhi");
    assert_eq!(clean_name("went to Delhi"), "went to Delhi");
}
#[test]
fn geographic_identity_aliases_context_and_ambiguity() {
    let mut delhi = area("geonames:1", "Delhi", &["Delhi"], &[("geonames:7", "ADM1")]);
    delhi.feature = "PPLA".into();
    delhi.population = 10_000_000;
    let dwarka = area(
        "geonames:2",
        "Dwarka",
        &["Dwarka"],
        &[("geonames:7", "ADM1")],
    );
    let other_dwarka = area(
        "geonames:3",
        "Dwarka",
        &["Dwarka"],
        &[("geonames:9", "ADM1")],
    );
    let landour = area(
        "geonames:4",
        "Landour",
        &["Landour"],
        &[("geonames:8", "ADM1")],
    );
    let landor = area(
        "geonames:5",
        "Landor",
        &["Landor"],
        &[("geonames:9", "ADM1")],
    );
    let bengaluru = area("geonames:6", "Bengaluru", &["Bangalore", "Bengaluru"], &[]);
    let areas = vec![delhi, dwarka, other_dwarka, landour, landor, bengaluru];
    assert_eq!(resolve("Dwarka", &areas)["status"], "unresolved");
    assert_eq!(resolve("Dwarka, Delhi", &areas)["area_id"], "geonames:2");
    assert_eq!(resolve("Delhi in Dwarka", &areas)["status"], "resolved");
    assert_eq!(resolve("Bangalore", &areas)["name"], "Bengaluru");
    assert_eq!(resolve("Landor", &areas)["area_id"], "geonames:5");
    assert_eq!(resolve("Landour", &areas)["area_id"], "geonames:4");
    assert_eq!(resolve("Landour, Delhi", &areas)["status"], "unresolved");
    assert_eq!(resolve("near Delhi", &areas)["status"], "unresolved");
    assert_eq!(resolve("Unknown village", &areas)["status"], "unresolved");
    assert_eq!(normalized("Dwārka"), normalized("Dwarka"));
    // Same Unicode tokenization as the importer; no Latin-only lookup path.
    assert_eq!(normalized("दिल्ली"), "द लल");
    assert_eq!(normalized("北京"), "北京");
}

#[test]
fn geographic_wire_separates_source_clean_name_and_resolved_identity() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../contracts/rekky/v1/fixtures/geographic_locations.json"
    ))
    .unwrap();
    let l = &fixture["locations"][0];
    assert_eq!(
        clean_name(l["text"].as_str().unwrap()),
        l["name"].as_str().unwrap()
    );
    assert_eq!(l["role"], "practice");
    assert_eq!(l["geography"]["area_id"], "geonames:1273294");
    assert_eq!(fixture["locations"][1]["geography"]["status"], "unresolved");
}

#[test]
fn browse_scopes_use_supported_cities_not_administrative_namesakes() {
    use rekky_backend::geography::{browse_projection, display_name};
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../contracts/rekky/v1/fixtures/location_browsing.json"
    ))
    .unwrap();
    let mut city = area(
        "geonames:1",
        "Bengaluru",
        &["Bengaluru", "Bangalore"],
        &[("geonames:4", "ADM2")],
    );
    city.feature = "PPLA".into();
    let neighbourhood = area(
        "geonames:2",
        "Indiranagar",
        &["Indiranagar"],
        &[("geonames:4", "ADM2")],
    );
    let mut state = area("geonames:3", "State of Karnataka", &["Karnataka"], &[]);
    state.feature = "ADM1".into();
    let mut district = area("geonames:4", "Bangalore Urban", &["Bangalore Urban"], &[]);
    district.feature = "ADM2".into();
    let areas = vec![city, neighbourhood, state, district];
    assert_eq!(
        browse_projection(&fixture["geography"], &areas),
        fixture["geography"]["browse"]
    );
    // Without a supported city ID, a shared district is not a city boundary.
    let local = json!({"status":"resolved","area_id":"geonames:2","filter_ids":["geonames:2","geonames:3","geonames:4"]});
    assert_eq!(
        browse_projection(&local, &areas)["destination"]["id"],
        "geonames:2"
    );
    let district_only = json!({"status":"resolved","area_id":"geonames:4","filter_ids":["geonames:3","geonames:4"]});
    let b = browse_projection(&district_only, &areas);
    assert_eq!(b["destination"]["kind"], "region");
    assert_eq!(b["destination"]["id"], "geonames:4");
    assert!(b["neighbourhood"].is_null());
    let mut country = area("geonames:10", "Republic of India", &["India"], &[]);
    country.feature = "PCLI".into();
    let country_scope =
        json!({"status":"resolved","area_id":"geonames:10","filter_ids":["geonames:10"]});
    assert_eq!(
        browse_projection(&country_scope, &[country])["destination"]["label"],
        "India"
    );
    assert_eq!(display_name("State of Uttarakhand"), "Uttarakhand");
    assert_eq!(display_name("National Capital Territory of Delhi"), "Delhi");
    assert!(browse_projection(&json!({"status":"unresolved"}), &areas).is_null());
}
