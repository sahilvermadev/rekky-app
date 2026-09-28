//! Local, licensed geography. Source phrases and relationship roles stay intact.
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{FromRow, PgConnection, PgPool, Row};
use std::{collections::BTreeSet, sync::OnceLock};
use unicode_normalization::{UnicodeNormalization, char::canonical_combining_class};

pub fn normalized(text: &str) -> String {
    static WORDS: OnceLock<Regex> = OnceLock::new();
    let folded: String = text
        .to_lowercase()
        .nfkd()
        .filter(|c| canonical_combining_class(*c) == 0)
        .collect();
    WORDS
        .get_or_init(|| Regex::new(r"[\p{L}\p{N}]+").unwrap())
        .find_iter(&folded)
        .map(|m| m.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Strip conversational prefixes only, never service qualifiers or named areas.
/// A near/around claim must not turn into exact containment.
pub fn clean_name(text: &str) -> String {
    static PREFIX: OnceLock<Regex> = OnceLock::new();
    let re = PREFIX.get_or_init(|| Regex::new(r"(?i)^(?:(?:is |are )?(?:based|located|situated|practices?|works|operates?|available|serves?|covers?)\s+(?:(?:right )?here\s+)?(?:in |at |from )?|(?:right )?here in |in |at )").unwrap());
    re.replace(text.trim(), "")
        .trim()
        .trim_end_matches(['.', ';'])
        .trim()
        .to_owned()
}
fn parts(text: &str) -> Vec<String> {
    static SPLIT: OnceLock<Regex> = OnceLock::new();
    SPLIT
        .get_or_init(|| Regex::new(r"(?i)\s*,\s*(?:in\s+)?|\s+in\s+").unwrap())
        .split(text)
        .map(normalized)
        .filter(|s| !s.is_empty())
        .collect()
}

#[derive(Clone, Debug, FromRow, Serialize, Deserialize)]
pub struct Area {
    pub id: String,
    pub name: String,
    pub label: String,
    pub country: String,
    pub feature: String,
    pub population: i64,
    pub aliases: Vec<String>,
    pub ancestors: Vec<String>,
    pub hierarchy: Value,
}
fn compatible(a: &Area, b: &Area) -> bool {
    if a.id == b.id || a.ancestors.contains(&b.id) || b.ancestors.contains(&a.id) {
        return true;
    }
    // Explicit city + area context can disambiguate within the same district.
    // This does not manufacture a global city-parent edge in the gazetteer.
    a.country == b.country
        && a.hierarchy.as_array().into_iter().flatten().any(|h| {
            (h["kind"] == "ADM2"
                || (h["kind"] == "ADM1"
                    && (a.feature.starts_with("PPLA") || b.feature.starts_with("PPLA"))))
                && b.ancestors.iter().any(|id| h["id"] == *id)
        })
}
fn choose<'a>(key: &str, areas: &'a [Area], anchors: &[&Area]) -> Option<&'a Area> {
    let mut candidates: Vec<_> = areas
        .iter()
        .filter(|a| a.aliases.iter().any(|v| v == key) && anchors.iter().all(|b| compatible(a, b)))
        .collect();
    // Prefer the named settlement over an administrative alias with the same name.
    if candidates
        .iter()
        .any(|a| a.feature.starts_with('P') && a.feature != "PCLI")
    {
        candidates.retain(|a| !a.feature.starts_with("ADM"));
    }
    if candidates.iter().any(|a| normalized(&a.name) == key) {
        candidates.retain(|a| normalized(&a.name) == key);
    }
    candidates.sort_by_key(|a| std::cmp::Reverse(a.population));
    if candidates.len() == 1 {
        return candidates.first().copied();
    }
    // A globally dominant city name can beat tiny namesakes; this is an explicit
    // heuristic, not a calibrated probability. Ordinary same-name towns abstain.
    if candidates.len() > 1
        && candidates[0].population >= 1_000_000
        && candidates[0].population > candidates[1].population.saturating_mul(100)
    {
        return Some(candidates[0]);
    }
    None
}

pub fn resolve(name: &str, areas: &[Area]) -> Value {
    let keys = parts(name);
    if keys.is_empty() || keys.len() > 4 {
        return json!({"status":"unresolved"});
    }
    let mut matched: Vec<Option<&Area>> = keys.iter().map(|k| choose(k, areas, &[])).collect();
    let anchors: Vec<_> = matched.iter().flatten().copied().collect();
    for (i, k) in keys.iter().enumerate() {
        matched[i] = choose(k, areas, &anchors);
    }
    if matched.iter().any(Option::is_none) {
        // A saved phrase can name a neighbourhood absent from our licensed
        // gazetteer while explicitly naming an unambiguous city. Keep only
        // that supported city scope for browsing. Do not apply this fallback
        // when a named area exists but conflicts with the city (for example
        // Landour, Delhi), or when the trailing city is itself ambiguous.
        let unknown_only = keys.iter().zip(&matched).all(|(key, chosen)| {
            chosen.is_some() || !areas.iter().any(|area| area.aliases.contains(key))
        });
        if keys.len() > 1 && unknown_only && matched.last().is_some_and(Option::is_some) {
            let anchor = matched.last().and_then(|area| *area).unwrap();
            if anchor.feature.starts_with("PPL") || anchor.feature.starts_with("PPLA") {
                let mut coarse = resolve(keys.last().unwrap(), areas);
                if coarse["status"] == "resolved" {
                    coarse["match_method"] = json!("explicit_coarse_context");
                    coarse["unresolved_parts"] = json!(
                        keys.iter()
                            .zip(&matched)
                            .filter_map(|(key, chosen)| chosen.is_none().then_some(key))
                            .collect::<Vec<_>>()
                    );
                    return coarse;
                }
            }
        }
        return json!({"status":"unresolved"});
    }
    let matched: Vec<_> = matched.into_iter().flatten().collect();
    if matched
        .iter()
        .any(|a| matched.iter().any(|b| !compatible(a, b)))
    {
        return json!({"status":"unresolved"});
    }
    // Deepest named geography first. Explicit city context remains a supported
    // scope even where the open dataset lacks a neighbourhood→city parent edge.
    let primary = matched
        .iter()
        .max_by_key(|a| {
            (
                a.ancestors.len(),
                a.feature == "PPL" || a.feature == "PPLX" || a.feature == "FT",
            )
        })
        .unwrap();
    let mut ids = BTreeSet::new();
    for a in &matched {
        ids.insert(a.id.clone());
        ids.extend(a.ancestors.iter().cloned());
    }
    let mut names = vec![primary.name.clone()];
    for a in &matched {
        if a.id != primary.id && !names.contains(&a.name) {
            names.push(a.name.clone());
        }
    }
    let label = if names.len() > 1 {
        names.join(", ")
    } else {
        primary.label.clone()
    };
    let mut label = label
        .replace(", State of ", ", ")
        .replace(", Province of ", ", ");
    if let Some(short) = primary.name.strip_suffix(" Cantonment")
        && keys.contains(&normalized(short))
        && primary.aliases.contains(&normalized(short))
    {
        label = label.replacen(&primary.name, short, 1);
    }
    json!({"status":"resolved","area_id":primary.id,"name":primary.name,"label":label,
        "country_code":primary.country,"hierarchy":primary.hierarchy,
        "filter_ids":ids,"source":"geonames","source_url":"https://www.geonames.org/",
        "match_method":"name_and_context"})
}

/// Public display names are presentation, not a replacement for geographic IDs.
pub fn display_name(name: &str) -> String {
    for prefix in [
        "National Capital Territory of ",
        "State of ",
        "Province of ",
    ] {
        if let Some(short) = name.strip_prefix(prefix) {
            return short.to_owned();
        }
    }
    name.to_owned()
}
fn browse_area(area: &Area, kind: &str) -> Value {
    static COUNTRIES: OnceLock<Value> = OnceLock::new();
    let countries = COUNTRIES.get_or_init(|| {
        serde_json::from_str(include_str!("../data/country_names.json"))
            .expect("bundled GeoNames country names")
    });
    let label = if area.feature == "PCLI" {
        countries[&area.country]
            .as_str()
            .unwrap_or(&area.name)
            .to_owned()
    } else {
        display_name(&area.name)
    };
    json!({"id":area.id,"label":label,"kind":kind,
        "aliases":area.aliases,"country_code":area.country})
}

/// Only supported filter identities can establish browsing membership. Never
/// map an entire district to its namesake city or infer membership by proximity.
pub fn browse_projection(geo: &Value, areas: &[Area]) -> Value {
    if geo["status"] != "resolved" {
        return Value::Null;
    }
    let supported: Vec<&Area> = areas
        .iter()
        .filter(|a| {
            geo["filter_ids"]
                .as_array()
                .is_some_and(|ids| ids.iter().any(|id| id == &a.id))
        })
        .collect();
    let Some(primary) = supported.iter().find(|a| geo["area_id"] == a.id).copied() else {
        return Value::Null;
    };
    let mut cities: Vec<&Area> = supported
        .iter()
        .copied()
        .filter(|a| a.feature.starts_with("PPLA") || a.feature == "PPLC" || a.feature == "PPL")
        .collect();
    // Gazetteer neighbourhoods can be typed PPL. An explicitly supported
    // administrative-seat city remains the browse parent, not its suburb.
    if cities
        .iter()
        .any(|a| a.feature.starts_with("PPLA") || a.feature == "PPLC")
    {
        cities.retain(|a| a.feature.starts_with("PPLA") || a.feature == "PPLC");
    }
    // A settlement nested in another named settlement browses under its parent.
    let candidates = cities.clone();
    cities.retain(|a| {
        !candidates
            .iter()
            .any(|b| a.id != b.id && a.ancestors.contains(&b.id))
    });
    let destination = if cities.len() == 1 {
        cities[0]
    } else {
        primary
    };
    let kind = if destination.feature.starts_with("ADM") || destination.feature == "PCLI" {
        "region"
    } else if destination.feature == "PPLX" || cities.len() > 1 {
        "locality"
    } else {
        "city"
    };
    let neighbourhood = (kind == "city"
        && primary.id != destination.id
        && !primary.feature.starts_with("ADM")
        && primary.feature != "PCLI")
        .then(|| browse_area(primary, "neighbourhood"));
    let regions: Vec<Value> = supported
        .iter()
        .filter(|a| a.feature == "ADM1" || a.feature == "PCLI")
        .map(|a| {
            browse_area(
                a,
                if a.feature == "PCLI" {
                    "country"
                } else {
                    "region"
                },
            )
        })
        .collect();
    let mut destination_json = browse_area(destination, kind);
    if primary.id == destination.id && primary.name.ends_with(" Cantonment") {
        destination_json["label"] = json!(
            geo["label"]
                .as_str()
                .unwrap_or(&primary.name)
                .split(',')
                .next()
                .unwrap_or(&primary.name)
        );
    }
    json!({"version":1,"destination":destination_json,"neighbourhood":neighbourhood,"regions":regions,
        "membership":"supported_geographic_scope"})
}

pub async fn enrich(
    connection: &mut PgConnection,
    locations: &mut [Value],
) -> Result<(), sqlx::Error> {
    for l in locations.iter_mut() {
        let name = clean_name(l["text"].as_str().unwrap_or_default());
        let keys = parts(&name);
        let areas:Vec<Area>=sqlx::query_as("SELECT id,name,label,country,feature,population,aliases,ancestors,hierarchy FROM geographic_areas WHERE aliases && $1 ORDER BY population DESC,id LIMIT 201")
            .bind(&keys).fetch_all(&mut *connection).await?;
        l["name"] = json!(name);
        // Never choose from a truncated set of possible identities.
        l["geography"] = if areas.len() > 200 {
            json!({"status":"unresolved"})
        } else {
            resolve(&name, &areas)
        };
        if l["geography"]["status"] == "resolved" {
            let ids: Vec<String> =
                serde_json::from_value(l["geography"]["filter_ids"].clone()).unwrap_or_default();
            let scoped: Vec<Area> = sqlx::query_as("SELECT id,name,label,country,feature,population,aliases,ancestors,hierarchy FROM geographic_areas WHERE id=ANY($1)")
                .bind(ids).fetch_all(&mut *connection).await?;
            l["geography"]["browse"] = browse_projection(&l["geography"], &scoped);
        }
    }
    Ok(())
}

/// Local-only work: no transcript reads, provider calls, device location or
/// processing permission changes. Locking fences owner edits and deletion.
pub async fn process_one(pool: &PgPool, owner: Option<uuid::Uuid>) -> Result<bool, sqlx::Error> {
    const RULES_VERSION: i64 = 2;
    let mut tx = pool.begin().await?;
    let catalog: i64 =
        sqlx::query_scalar("SELECT revision FROM geographic_catalog WHERE singleton")
            .fetch_one(&mut *tx)
            .await?;
    if catalog == 0 {
        return Ok(false);
    }
    let row=sqlx::query("SELECT id,recommendation FROM knowledge_items WHERE deleted_at IS NULL AND jsonb_typeof(recommendation)='object' AND jsonb_typeof(recommendation->'locations')='array' AND ($1::uuid IS NULL OR owner_id=$1) AND (recommendation->'geography_revision' IS DISTINCT FROM to_jsonb($2::bigint) OR recommendation->'geography_rules_version' IS DISTINCT FROM to_jsonb($3::bigint) OR EXISTS(SELECT 1 FROM jsonb_array_elements(recommendation->'locations') l WHERE l->'geography'->>'status'='resolved' AND l->'geography'->'browse'->'version' IS DISTINCT FROM '1'::jsonb)) ORDER BY created_at LIMIT 1 FOR UPDATE SKIP LOCKED")
        .bind(owner).bind(catalog).bind(RULES_VERSION).fetch_optional(&mut *tx).await?;
    let Some(row) = row else { return Ok(false) };
    let id: uuid::Uuid = row.get("id");
    let mut r: Value = row.get("recommendation");
    let locations = r["locations"].as_array_mut().unwrap();
    enrich(&mut tx, locations).await?;
    let snapshot = json!(locations);
    sqlx::query("DELETE FROM item_location_index WHERE item_id=$1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    for (i, l) in locations.iter().enumerate() {
        if l["geography"]["status"] != "resolved" {
            continue;
        }
        let ids: Vec<String> =
            serde_json::from_value(l["geography"]["filter_ids"].clone()).unwrap_or_default();
        sqlx::query("INSERT INTO item_location_index(item_id,ordinal,role,area_ids,locations_snapshot) VALUES($1,$2,$3,$4,$5)")
            .bind(id).bind(i as i32).bind(l["role"].as_str().unwrap_or("context")).bind(ids).bind(&snapshot).execute(&mut *tx).await?;
    }
    r["geography_revision"] = json!(catalog);
    r["geography_rules_version"] = json!(RULES_VERSION);
    sqlx::query("UPDATE knowledge_items SET recommendation=$2,revision=revision+1 WHERE id=$1")
        .bind(id)
        .bind(r)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(true)
}
