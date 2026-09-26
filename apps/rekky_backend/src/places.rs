//! Ephemeral public venue details. Never sends recommendation prose or sources.
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::BTreeSet, time::Duration};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlaceMatch {
    pub place_id: String,
    pub address: String,
}
#[derive(Clone, Debug)]
pub struct PlaceQuery {
    pub subject: String,
    pub locality: String,
}

pub fn query(subject: &str, recommendation: &Value) -> Option<PlaceQuery> {
    if recommendation["entity_kind"] != "place"
        || recommendation["destination"]["mode"]
            .as_str()
            .is_some_and(|v| v != "auto")
    {
        return None;
    }
    // Start with explicit public hospitality categories; never look up private
    // people, homes or a vague place inferred solely from a spoken name.
    let public_venue = recommendation["classification"]["types"]
        .as_array()
        .is_some_and(|types| {
            types.iter().any(|t| {
                matches!(
                    t["id"].as_str(),
                    Some("place.restaurant" | "place.cafe" | "place.bar" | "place.hotel")
                )
            })
        });
    if !public_venue {
        return None;
    }
    let venues = recommendation["locations"]
        .as_array()?
        .iter()
        .filter(|l| l["role"] == "venue")
        .filter_map(|l| l["text"].as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect::<BTreeSet<_>>();
    if venues.len() != 1 || subject.trim().is_empty() || subject.len() > 200 {
        return None;
    }
    let locality = *venues.first()?;
    if locality.len() > 200 {
        return None;
    }
    Some(PlaceQuery {
        subject: subject.to_owned(),
        locality: locality.to_owned(),
    })
}
fn normalized(text: &str, name: bool) -> Vec<String> {
    crate::taxonomy::words(text)
        .into_iter()
        .filter(|w| {
            !["the", "in", "at", "near", "of"].contains(&w.as_str())
                && !(name && ["cafe", "café", "restaurant", "bar", "s"].contains(&w.as_str()))
        })
        .map(|w| match w.as_str() {
            "bangalore" if !name => "bengaluru".into(),
            "bombay" if !name => "mumbai".into(),
            _ => w,
        })
        .collect()
}
/// Reject multiple plausible branches, truncated result sets, and weak names.
/// This is a conservative pilot matcher, not a general identity guarantee.
pub fn match_response(query: &PlaceQuery, response: &Value) -> Option<PlaceMatch> {
    if response["nextPageToken"]
        .as_str()
        .is_some_and(|s| !s.is_empty())
    {
        return None;
    }
    let names = normalized(&query.subject, true);
    let locality = normalized(&query.locality, false);
    if names.is_empty() || locality.is_empty() {
        return None;
    }
    let mut matches = Vec::new();
    for place in response["places"].as_array()? {
        let id = place["id"].as_str()?;
        let address = place["formattedAddress"].as_str()?;
        let name = place["displayName"]["text"].as_str()?;
        if !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
            || id.is_empty()
            || id.len() > 256
            || address.is_empty()
            || address.len() > 1000
            || place["businessStatus"] != "OPERATIONAL"
            || place["attributions"]
                .as_array()
                .is_some_and(|a| !a.is_empty())
            || !place["types"]
                .as_array()
                .is_some_and(|t| t.iter().any(|v| v == "establishment"))
        {
            continue;
        }
        if names == normalized(name, true)
            && locality
                .iter()
                .all(|word| normalized(address, false).contains(word))
        {
            matches.push(PlaceMatch {
                place_id: id.to_owned(),
                address: address.to_owned(),
            });
        }
    }
    matches.dedup_by(|a, b| a.place_id == b.place_id);
    (matches.len() == 1).then(|| matches.remove(0))
}
#[async_trait]
pub trait PlaceResolver: Send + Sync {
    fn available(&self) -> bool;
    async fn resolve(&self, query: &PlaceQuery) -> Option<PlaceMatch>;
}
pub struct GooglePlaces {
    key: Option<String>,
    client: reqwest::Client,
}
impl GooglePlaces {
    pub fn from_env() -> Self {
        Self {
            key: (std::env::var("GOOGLE_PLACES_ENABLED").as_deref() == Ok("true"))
                .then(|| {
                    std::env::var("GOOGLE_MAPS_API_KEY")
                        .ok()
                        .filter(|s| !s.is_empty())
                })
                .flatten(),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(6))
                .build()
                .expect("HTTP client"),
        }
    }
}
#[async_trait]
impl PlaceResolver for GooglePlaces {
    fn available(&self) -> bool {
        self.key.is_some()
    }
    async fn resolve(&self, query: &PlaceQuery) -> Option<PlaceMatch> {
        let response = self.client.post("https://places.googleapis.com/v1/places:searchText")
            .header("X-Goog-Api-Key", self.key.as_ref()?)
            .header("X-Goog-FieldMask", "places.id,places.displayName,places.formattedAddress,places.businessStatus,places.types,places.attributions,nextPageToken")
            .json(&json!({"textQuery":format!("{}, {}",query.subject,query.locality),"pageSize":5,"languageCode":"en","includePureServiceAreaBusinesses":false}))
            .send().await.ok()?;
        if !response.status().is_success() {
            return None;
        }
        let value: Value = response.json().await.ok()?;
        match_response(query, &value)
    }
}
