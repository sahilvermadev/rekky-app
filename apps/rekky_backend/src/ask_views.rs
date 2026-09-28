//! Complete owner-scoped browse queries, independent of model candidate limits.
use crate::app::{ApiError, ApiResult, AppState, ok, owner, parse};
use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::Row;
use std::collections::{BTreeMap, HashSet};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BrowseSpec {
    pub title: String,
    pub location: String,
    pub category_ids: Vec<String>,
    pub kind: String,
    pub query: String,
    pub location_role: String,
    pub sort: String,
    pub source: String,
}
impl BrowseSpec {
    fn validate(&mut self) -> Result<(), ApiError> {
        self.title = self.title.trim().to_owned();
        self.location = self.location.trim().to_owned();
        if self.source.is_empty() {
            self.source = "mine".into();
        }
        if self.sort.is_empty() {
            self.sort = "saved_newest".into();
        }
        if self.location_role.is_empty() {
            self.location_role = "relevant".into();
        }
        if self.source != "mine" {
            return Err(ApiError::bad("Friend exploration is not available yet"));
        }
        if self.title.chars().count() > 100
            || self.location.chars().count() > 100
            || self.query.chars().count() > 100
            || self.category_ids.len() > 8
            || self.category_ids.iter().any(|id| id.len() > 100)
            || ![
                "",
                "place",
                "person_service",
                "thing",
                "activity_event",
                "idea_tip",
                "unspecified",
            ]
            .contains(&self.kind.as_str())
            || !["relevant", "any", "unknown"].contains(&self.location_role.as_str())
            || !["saved_newest", "name"].contains(&self.sort.as_str())
        {
            return Err(ApiError::bad("Invalid collection filters"));
        }
        Ok(())
    }
}
#[derive(Clone)]
struct Entry {
    id: Uuid,
    revision: i32,
    subject: String,
    categories: Vec<String>,
    rec: Value,
}
async fn entries(state: &AppState, owner_id: Uuid) -> Result<(Vec<Entry>, String), ApiError> {
    let rows = sqlx::query("SELECT id,revision,subject,category_ids,jsonb_build_object('entity_kind',recommendation->'entity_kind','classification',recommendation->'classification','locations',recommendation->'locations','geography_revision',recommendation->'geography_revision') rec FROM knowledge_items WHERE owner_id=$1 AND deleted_at IS NULL ORDER BY created_at DESC,id DESC LIMIT 10001")
        .bind(owner_id).fetch_all(&state.pool).await?;
    if rows.len() > 10000 {
        return Err(ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "browse_capacity",
            "This collection is too large to load right now. Please try later.",
        ));
    }
    let catalog: i64 =
        sqlx::query_scalar("SELECT revision FROM geographic_catalog WHERE singleton")
            .fetch_one(&state.pool)
            .await?;
    let entries: Vec<_> = rows
        .into_iter()
        .map(|r| Entry {
            id: r.get("id"),
            revision: r.get("revision"),
            subject: r.get("subject"),
            categories: r.get("category_ids"),
            rec: r.get("rec"),
        })
        .collect();
    let fingerprint = crate::auth::hash_token(&format!(
        "{}:{:?}",
        catalog,
        entries
            .iter()
            .map(|e| (e.id, e.revision, e.rec.to_string(), &e.categories))
            .collect::<Vec<_>>()
    ));
    Ok((entries, fingerprint))
}
/// Resolve city aliases in the gazetteer, then apply curated urban memberships.
/// No model calls and no district-wide fuzzy matching.
pub async fn resolve_scope(state: &AppState, location: &str) -> Result<Value, ApiError> {
    if location.trim().is_empty() {
        return Ok(Value::Null);
    }
    let mut conn = state.pool.acquire().await?;
    // Native facet taps send a stable area ID. Resolve those by identity;
    // running an ID through the name resolver would make every area chip fail.
    let mut geo = if location.starts_with("geonames:") {
        match sqlx::query("SELECT name,label FROM geographic_areas WHERE id=$1")
            .bind(location)
            .fetch_optional(&mut *conn)
            .await?
        {
            Some(row) => json!({"status":"resolved","area_id":location,
                "name":row.get::<String,_>("name"),"label":row.get::<String,_>("label")}),
            None => json!({"status":"unresolved"}),
        }
    } else {
        let mut locations = vec![json!({"text":location,"role":"context"})];
        crate::geography::enrich(&mut conn, &mut locations).await?;
        locations[0]["geography"].clone()
    };
    if geo["match_method"] == "explicit_coarse_context" {
        // The saved item can be browsed under its explicit city, but a user's
        // request for an unindexed neighbourhood must not silently expand to
        // every recommendation in that city.
        geo["status"] = json!("coarse");
        return Ok(geo);
    }
    if let Some(id) = geo["area_id"].as_str().map(str::to_owned) {
        let mapped: Option<String> =
            sqlx::query_scalar("SELECT scope_id FROM ask_area_scopes WHERE area_id=$1")
                .bind(&id)
                .fetch_optional(&mut *conn)
                .await?;
        if let Some(scope) = mapped {
            geo["area_id"] = json!(scope);
            geo["label"] = json!("Delhi");
        }
        let primary = geo["area_id"].as_str().unwrap_or(&id);
        let mut ids = vec![primary.to_owned()];
        let children: Vec<String> =
            sqlx::query_scalar("SELECT area_id FROM ask_area_scopes WHERE scope_id=$1")
                .bind(primary)
                .fetch_all(&mut *conn)
                .await?;
        ids.extend(children);
        geo["match_ids"] = json!(ids);
    }
    Ok(geo)
}
fn kind(e: &Entry) -> &str {
    e.rec["entity_kind"].as_str().unwrap_or("unspecified")
}
fn located(e: &Entry, ids: &[String], role: &str) -> bool {
    let mut known = false;
    let mut matches = false;
    for l in e.rec["locations"].as_array().into_iter().flatten() {
        let relevant = role == "any"
            || if kind(e) == "place" {
                l["role"] == "venue"
            } else {
                l["role"] == "practice" || l["role"] == "service_area"
            };
        if !relevant || l["geography"]["status"] != "resolved" {
            continue;
        }
        known = true;
        matches |= ids.is_empty()
            || l["geography"]["filter_ids"].as_array().is_some_and(|a| {
                a.iter()
                    .any(|v| ids.iter().any(|id| v.as_str() == Some(id)))
            });
    }
    if role == "unknown" {
        !known
    } else {
        ids.is_empty() || matches
    }
}
fn fits(e: &Entry, s: &BrowseSpec, ids: &[String], ignore: &str) -> bool {
    let query = s.query.trim().to_lowercase();
    let query_matches = query.is_empty()
        || e.subject.to_lowercase().contains(&query)
        || e.categories
            .iter()
            .any(|id| id.to_lowercase().contains(&query))
        || ["types", "facets"].iter().any(|field| {
            e.rec["classification"][field]
                .as_array()
                .into_iter()
                .flatten()
                .any(|category| {
                    category["label"]
                        .as_str()
                        .is_some_and(|label| label.to_lowercase().contains(&query))
                })
        });
    (ignore == "kind" || s.kind.is_empty() || kind(e) == s.kind)
        && (ignore == "category" || s.category_ids.iter().all(|id| e.categories.contains(id)))
        && (ignore == "location" || located(e, ids, &s.location_role))
        && query_matches
}
fn facets(entries: &[Entry], s: &BrowseSpec, ids: &[String]) -> Value {
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    let mut categories: BTreeMap<String, (String, usize)> = BTreeMap::new();
    let mut areas: BTreeMap<String, (String, usize)> = BTreeMap::new();
    for e in entries {
        if fits(e, s, ids, "kind") {
            *kinds.entry(kind(e).into()).or_default() += 1;
        }
        if fits(e, s, ids, "category") {
            let mut counted = HashSet::new();
            for field in ["types", "facets"] {
                for c in e.rec["classification"][field]
                    .as_array()
                    .into_iter()
                    .flatten()
                {
                    if let (Some(id), Some(label)) = (c["id"].as_str(), c["label"].as_str()) {
                        if counted.insert(id) {
                            categories.entry(id.into()).or_insert((label.into(), 0)).1 += 1;
                        }
                    }
                }
            }
        }
        if fits(e, s, ids, "location") {
            let mut counted = HashSet::new();
            for l in e.rec["locations"].as_array().into_iter().flatten() {
                if s.location_role != "any"
                    && !["venue", "practice", "service_area"]
                        .contains(&l["role"].as_str().unwrap_or(""))
                {
                    continue;
                }
                for a in [
                    &l["geography"]["browse"]["destination"],
                    &l["geography"]["browse"]["neighbourhood"],
                ] {
                    if let (Some(id), Some(label)) = (a["id"].as_str(), a["label"].as_str()) {
                        if counted.insert(id.to_owned()) {
                            areas.entry(id.into()).or_insert((label.into(), 0)).1 += 1;
                        }
                    }
                }
            }
        }
    }
    json!({"kinds":kinds.into_iter().map(|(id,count)|json!({"id":id,"label":id,"count":count})).collect::<Vec<_>>(),
        "categories":categories.into_iter().map(|(id,(label,count))|json!({"id":id,"label":label,"count":count})).collect::<Vec<_>>(),
        "areas":areas.into_iter().map(|(id,(label,count))|json!({"id":id,"label":label,"count":count})).collect::<Vec<_>>()})
}
async fn create_view_once(
    state: &AppState,
    owner_id: Uuid,
    mut spec: BrowseSpec,
) -> Result<Value, ApiError> {
    spec.validate()?;
    let geo = resolve_scope(state, &spec.location).await?;
    if !spec.location.is_empty() && geo["status"] != "resolved" {
        return Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "area_unresolved",
            "That area is ambiguous or not indexed yet. Try adding its city.",
        ));
    }
    let ids: Vec<String> = serde_json::from_value(geo["match_ids"].clone()).unwrap_or_default();
    let (entries, fingerprint) = entries(state, owner_id).await?;
    let mut matching: Vec<_> = entries
        .iter()
        .filter(|e| fits(e, &spec, &ids, ""))
        .collect();
    if spec.sort == "name" {
        matching.sort_by_key(|e| (e.subject.to_lowercase(), e.id));
    }
    let item_ids: Vec<_> = matching.iter().map(|e| e.id).collect();
    let title = if spec.title.is_empty() {
        if spec.location.is_empty() {
            "Your saved recommendations".to_owned()
        } else {
            geo["label"].as_str().unwrap_or(&spec.location).to_owned()
        }
    } else {
        spec.title.clone()
    };
    let metadata = json!({"schema_version":"ask_ui.v1","title":title,"total":item_ids.len(),"count_kind":"exact","source":"mine","facets":facets(&entries,&spec,&ids),"area":geo});
    let id = Uuid::new_v4();
    sqlx::query("DELETE FROM ask_views WHERE expires_at<now()")
        .execute(&state.pool)
        .await?;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM ask_views WHERE owner_id=$1")
        .bind(owner_id)
        .fetch_one(&state.pool)
        .await?;
    if count >= 200 {
        return Err(ApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "browse_limit",
            "Too many recent collections. Try again shortly.",
        ));
    }
    sqlx::query("INSERT INTO ask_views(id,owner_id,spec,fingerprint,item_ids,metadata) VALUES($1,$2,$3,$4,$5,$6)")
        .bind(id).bind(owner_id).bind(json!(spec)).bind(fingerprint).bind(&item_ids).bind(metadata).execute(&state.pool).await?;
    let result = view_page(state, owner_id, id, 0).await;
    if result.as_ref().is_err_and(ApiError::is_conflict) {
        sqlx::query("DELETE FROM ask_views WHERE id=$1 AND owner_id=$2")
            .bind(id)
            .bind(owner_id)
            .execute(&state.pool)
            .await?;
    }
    result
}

pub async fn create_view(
    state: &AppState,
    owner_id: Uuid,
    spec: BrowseSpec,
) -> Result<Value, ApiError> {
    // Background geographic projection can revise a library between the
    // snapshot and its first hydrated page. Rebuild a bounded number of times
    // so the user receives a fresh view instead of a transient stale error.
    for _ in 0..2 {
        match create_view_once(state, owner_id, spec.clone()).await {
            Err(error) if error.is_conflict() => continue,
            outcome => return outcome,
        }
    }
    create_view_once(state, owner_id, spec).await
}
pub async fn view_page(
    state: &AppState,
    owner_id: Uuid,
    id: Uuid,
    offset: usize,
) -> Result<Value, ApiError> {
    let row=sqlx::query("SELECT spec,fingerprint,item_ids,metadata FROM ask_views WHERE id=$1 AND owner_id=$2 AND expires_at>now()")
        .bind(id).bind(owner_id).fetch_optional(&state.pool).await?.ok_or_else(||ApiError::new(StatusCode::NOT_FOUND,"view_expired","This collection expired. Refresh it to continue."))?;
    let (_, fingerprint) = entries(state, owner_id).await?;
    if row.get::<String, _>("fingerprint") != fingerprint {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "view_stale",
            "Your recommendations changed. Refresh this collection.",
        ));
    }
    let ids: Vec<Uuid> = row.get("item_ids");
    if offset > ids.len() {
        return Err(ApiError::bad("Invalid collection page"));
    }
    let mut items = vec![];
    for item_id in ids.iter().skip(offset).take(20) {
        if let Some(item) = crate::app::owner_item(state, owner_id, *item_id).await? {
            items.push(item);
        } else {
            return Err(ApiError::conflict(
                "A recommendation changed. Refresh this collection.",
            ));
        }
    }
    // Catch concurrent edits/deletion before committing hydrated content.
    if entries(state, owner_id).await?.1 != fingerprint {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "view_stale",
            "Your recommendations changed. Refresh this collection.",
        ));
    }
    let mut result: Value = row.get("metadata");
    result["view_id"] = json!(id);
    result["spec"] = row.get("spec");
    result["items"] = json!(items);
    result["next_offset"] = if offset + 20 < ids.len() {
        json!(offset + 20)
    } else {
        Value::Null
    };
    Ok(result)
}
pub async fn view_ids(state: &AppState, owner_id: Uuid, id: Uuid) -> Result<Vec<Uuid>, ApiError> {
    view_page(state, owner_id, id, 0).await?;
    Ok(
        sqlx::query_scalar("SELECT item_ids FROM ask_views WHERE id=$1 AND owner_id=$2")
            .bind(id)
            .bind(owner_id)
            .fetch_one(&state.pool)
            .await?,
    )
}
pub async fn overview(state: &AppState, owner_id: Uuid) -> Result<Value, ApiError> {
    let (entries, _) = entries(state, owner_id).await?;
    Ok(
        json!({"source":"mine","total":entries.len(),"facets":facets(&entries,&BrowseSpec::default(),&[]),"network_available":false}),
    )
}
pub async fn create(State(state): State<AppState>, headers: HeaderMap, body: Bytes) -> ApiResult {
    let owner_id = owner(&state, &headers, true).await?;
    let spec: BrowseSpec = parse(&body)?;
    Ok(ok(create_view(&state, owner_id, spec).await?))
}
#[derive(Deserialize)]
pub struct Page {
    #[serde(default)]
    offset: usize,
}
pub async fn page(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Query(q): Query<Page>,
) -> ApiResult {
    let owner_id = owner(&state, &headers, true).await?;
    Ok(ok(view_page(&state, owner_id, id, q.offset).await?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_native_view_fixture_has_a_valid_query() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../contracts/rekky/v1/fixtures/ask_ui.json"
        ))
        .unwrap();
        let mut spec: BrowseSpec = serde_json::from_value(fixture["view"]["spec"].clone()).unwrap();
        spec.validate().unwrap();
        assert_eq!(fixture["view"]["schema_version"], "ask_ui.v1");
        assert_eq!(
            fixture["view"]["total"],
            fixture["view"]["items"].as_array().unwrap().len()
        );
    }

    #[test]
    fn service_history_does_not_prove_current_coverage() {
        let entry = Entry {
            id: Uuid::nil(),
            revision: 1,
            subject: "A carpenter".into(),
            categories: vec![],
            rec: json!({"entity_kind":"person_service","locations":[{
                "role":"past_experience","geography":{"status":"resolved","filter_ids":["geonames:1273294"]}
            }]}),
        };
        let area = vec!["geonames:1273294".to_owned()];
        assert!(!located(&entry, &area, "relevant"));
        assert!(located(&entry, &area, "any"));
        assert!(located(&entry, &area, "unknown"));
    }

    #[test]
    fn unfamiliar_type_search_uses_classification_not_only_subject() {
        let entry = Entry {
            id: Uuid::nil(),
            revision: 1,
            subject: "Mr Heetlal".into(),
            categories: vec!["service.carpenter".into()],
            rec: json!({"entity_kind":"person_service","classification":{
                "types":[{"id":"service.carpenter","label":"Carpenter"}]}}),
        };
        assert!(fits(
            &entry,
            &BrowseSpec {
                query: "carpenter".into(),
                ..Default::default()
            },
            &[],
            ""
        ));
    }
}
