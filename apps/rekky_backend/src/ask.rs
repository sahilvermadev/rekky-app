//! Bounded owner-only Ask agent. Tools expose saved evidence, never raw transcripts.
use crate::app::{ApiError, ApiResult, AppState, ok, owner, parse};
use async_trait::async_trait;
use axum::{
    body::{Body, Bytes, to_bytes},
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::Row;
use std::{
    collections::{HashMap, HashSet},
    convert::Infallible,
    time::Duration,
};
use tokio::sync::mpsc;
use tokio_stream::{StreamExt, wrappers::UnboundedReceiverStream};
use uuid::Uuid;

const MODEL: &str = "gpt-6-luna";
const MAX_DECISIONS: usize = 3;
const PAGE: usize = 8;

#[derive(Debug)]
pub struct AgentError {
    pub code: &'static str,
}
impl AgentError {
    const INVALID: Self = Self {
        code: "invalid_answer",
    };
}
#[derive(Clone, Debug)]
pub struct Decision {
    pub calls: Vec<ToolCall>,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub output_items: Vec<Value>,
}
#[derive(Clone, Debug)]
pub struct ToolCall {
    pub name: String,
    pub call_id: String,
    pub arguments: Value,
}
#[async_trait]
pub trait AskModel: Send + Sync {
    fn available(&self) -> bool;
    async fn decide(&self, context: Value, final_turn: bool) -> Result<Decision, AgentError>;
    async fn decide_stream(
        &self,
        context: Value,
        final_turn: bool,
        _events: Option<mpsc::UnboundedSender<Value>>,
    ) -> Result<Decision, AgentError> {
        self.decide(context, final_turn).await
    }
}
pub struct OpenAiAsk {
    key: Option<String>,
    enabled: bool,
    client: reqwest::Client,
}
impl OpenAiAsk {
    pub fn from_env() -> Self {
        Self {
            key: std::env::var("OPENAI_API_KEY")
                .ok()
                .filter(|s| !s.is_empty()),
            enabled: std::env::var("OPENAI_ASK_ENABLED").as_deref() == Ok("true"),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(18))
                .build()
                .expect("HTTP client"),
        }
    }
}
fn object(fields: Value) -> Value {
    let required: Vec<_> = fields
        .as_object()
        .expect("schema")
        .keys()
        .cloned()
        .collect();
    json!({"type":"object","properties":fields,"required":required,"additionalProperties":false})
}
fn strings() -> Value {
    json!({"type":"array","items":{"type":"string"}})
}
fn tool(name: &str, description: &str, parameters: Value) -> Value {
    json!({"type":"function","name":name,"description":description,"strict":true,"parameters":parameters})
}
fn tools() -> Vec<Value> {
    vec![
        tool(
            "open_collection",
            "Open a complete browsable set only when exploring several matching recommendations would help the person, including option seeking without the word 'all'. For a narrow question, first seek a direct cited answer. A zero- or one-item match returns no view ID: answer directly from any returned item instead of showing a collection. Use canonical category IDs from knowledge_overview. Empty strings/arrays mean unrestricted; query is literal subject/category text, never a subjective quality such as quiet or date-worthy. source must be mine. location_role relevant matches venue/service coverage, any includes trip context, unknown exposes unlocated recommendations. sort saved_newest or name.",
            object(json!({
                "title":{"type":"string"},"location":{"type":"string"},"category_ids":strings(),"kind":{"type":"string"},"query":{"type":"string"},"source":{"type":"string","enum":["mine"]},"location_role":{"type":"string","enum":["relevant","any","unknown"]},"sort":{"type":"string","enum":["saved_newest","name"]}
            })),
        ),
        tool(
            "search_knowledge",
            "Search saved evidence using tokenized OR alternatives. For local discovery set location to the explicit requested city, otherwise the device city if supplied. Leave location empty for personal recall or worldwide search. An empty terms list browses. Up to 8 terms, page 0..20.",
            object(
                json!({"terms":strings(),"location":{"type":"string"},"page":{"type":"integer"}}),
            ),
        ),
        tool(
            "inspect_evidence",
            "Read current saved evidence for up to 8 previously returned item IDs.",
            object(json!({"item_ids":strings()})),
        ),
        tool(
            "present_answer",
            "Present all useful seen items with cited evidence, or a clarification. No made-up actions.",
            object(json!({
                "intent":{"type":"string","enum":["recall","discovery","comparison"]},"title":{"type":"string"},"location":{"type":"string"},
                "reply":{"type":"string","description":"A short natural reply; facts about specific items belong in cited result reasons. Explain gaps and next steps without inventing facts."},"view_ids":strings(),"clarification":{"type":"string"},"choices":strings(),"new_topic":{"type":"boolean","description":"True only for a clearly unrelated new request; false for refinements and clarification replies."},
                "comparison":{"anyOf":[{"type":"null"},object(json!({
                    "item_ids":strings(),
                    "dimensions":{"type":"array","items":object(json!({"label":{"type":"string"},"cells":{"type":"array","items":object(json!({"item_id":{"type":"string"},"text":{"type":"string"},"evidence_ids":strings()}))}}))},
                    "conclusion":{"type":"string"},
                    "citations":{"type":"array","items":object(json!({"item_id":{"type":"string"},"evidence_ids":strings()}))}
                }))]},
                "results":{"type":"array","items":object(json!({"item_id":{"type":"string"},"section":{"type":"string","enum":["supported","worth_checking"],"description":"Suitability for the user request, NOT whether an exclusion statement has evidence. Include only viable options; omit irrelevant or contradicted candidates."},"reason":{"type":"string"},"caveat":{"type":"string"},"evidence_ids":strings()}))}
            })),
        ),
    ]
}
#[async_trait]
impl AskModel for OpenAiAsk {
    fn available(&self) -> bool {
        self.enabled && self.key.is_some()
    }
    async fn decide(&self, context: Value, final_turn: bool) -> Result<Decision, AgentError> {
        self.decide_stream(context, final_turn, None).await
    }
    async fn decide_stream(
        &self,
        context: Value,
        final_turn: bool,
        events: Option<mpsc::UnboundedSender<Value>>,
    ) -> Result<Decision, AgentError> {
        if !self.available() {
            return Err(AgentError::INVALID);
        }
        let input = context.get("provider_input").cloned().unwrap_or_else(|| {
            json!([
            {"role":"system","content":include_str!("../prompts/ask_v1.txt")},
            {"role":"user","content":context.to_string()}])
        });
        let body = json!({"model":MODEL,"store":false,"stream":events.is_some(),"reasoning":{"effort":"none"},"max_output_tokens":2000,
            "input":input,
            "tools":tools(),"tool_choice":if final_turn {json!({"type":"function","name":"present_answer"})}else{json!("required")}});
        // Conservative token bound at <= one token per UTF-8 byte plus headroom.
        // 3 x (48k input at $0.10/M + 2k output at $0.50/M) <= $0.0174.
        if body.to_string().len() > 47_000 {
            return Err(AgentError {
                code: "payload_limit",
            });
        }
        let mut response = self
            .client
            .post("https://api.openai.com/v1/responses")
            .bearer_auth(self.key.as_ref().ok_or(AgentError::INVALID)?)
            .json(&body)
            .send()
            .await
            .map_err(|e| AgentError {
                code: if e.is_timeout() {
                    "provider_timeout"
                } else {
                    "provider_transport"
                },
            })?;
        if !response.status().is_success() {
            return Err(AgentError {
                code: "provider_http",
            });
        }
        let data: Value = if let Some(events) = events {
            let mut pending = Vec::<u8>::new();
            let mut complete = None;
            let mut answer_indexes = HashSet::new();
            let mut arguments: HashMap<i64, String> = HashMap::new();
            let mut emitted = HashMap::<i64, usize>::new();
            while let Some(chunk) = response.chunk().await.map_err(|_| AgentError::INVALID)? {
                pending.extend_from_slice(&chunk);
                if pending.len() > 300_000 {
                    return Err(AgentError::INVALID);
                }
                while let Some(frame) = take_sse_frame(&mut pending) {
                    let frame = String::from_utf8(frame).map_err(|_| AgentError::INVALID)?;
                    let payload = frame.lines().find_map(|line| line.strip_prefix("data: "));
                    let Some(payload) = payload else { continue };
                    if payload == "[DONE]" {
                        continue;
                    }
                    let event: Value =
                        serde_json::from_str(payload).map_err(|_| AgentError::INVALID)?;
                    match event["type"].as_str().unwrap_or_default() {
                        "response.output_item.added" => {
                            if event["item"]["name"] == "present_answer" {
                                if let Some(index) = event["output_index"].as_i64() {
                                    answer_indexes.insert(index);
                                }
                            }
                        }
                        "response.function_call_arguments.delta" => {
                            let Some(index) = event["output_index"].as_i64() else {
                                continue;
                            };
                            if !answer_indexes.contains(&index) {
                                continue;
                            }
                            let Some(delta) = event["delta"].as_str() else {
                                continue;
                            };
                            let buffer = arguments.entry(index).or_default();
                            buffer.push_str(delta);
                            if buffer.len() > 20_000 {
                                return Err(AgentError::INVALID);
                            }
                            if let Some(prefix) = partial_reply(buffer) {
                                let old = emitted.entry(index).or_default();
                                // Only completed words are provisional; a rejected answer resets them.
                                let safe_end = prefix.rfind(char::is_whitespace).unwrap_or(0);
                                if safe_end > *old && safe_end - *old >= 8 {
                                    let _ = events.send(
                                        json!({"type":"text","text":&prefix[*old..safe_end]}),
                                    );
                                    *old = safe_end;
                                }
                            }
                        }
                        "response.completed" => complete = event.get("response").cloned(),
                        "response.failed" | "error" => return Err(AgentError::INVALID),
                        _ => {}
                    }
                }
            }
            complete.ok_or(AgentError::INVALID)?
        } else {
            response.json().await.map_err(|_| AgentError::INVALID)?
        };
        if data["status"] != "completed" {
            return Err(AgentError {
                code: "provider_incomplete",
            });
        }
        let calls = data["output"]
            .as_array()
            .ok_or(AgentError::INVALID)?
            .iter()
            .filter(|v| v["type"] == "function_call")
            .map(|v| {
                Ok(ToolCall {
                    name: v["name"].as_str().ok_or(AgentError::INVALID)?.into(),
                    call_id: v["call_id"].as_str().ok_or(AgentError::INVALID)?.into(),
                    arguments: serde_json::from_str(
                        v["arguments"].as_str().ok_or(AgentError::INVALID)?,
                    )
                    .map_err(|_| AgentError::INVALID)?,
                })
            })
            .collect::<Result<Vec<_>, AgentError>>()?;
        if calls.is_empty() || calls.len() > 4 {
            return Err(AgentError::INVALID);
        }
        Ok(Decision {
            output_items: data["output"].as_array().cloned().unwrap_or_default(),
            calls,
            input_tokens: data["usage"]["input_tokens"].as_i64().unwrap_or(0),
            output_tokens: data["usage"]["output_tokens"].as_i64().unwrap_or(0),
        })
    }
}

// Read only the reply string from an in-progress function argument. Never send
// JSON syntax, result IDs, evidence or an unfinished escape to the client.
fn partial_reply(arguments: &str) -> Option<String> {
    let key = arguments.find("\"reply\"")?;
    let tail = arguments.get(key + 7..)?.trim_start();
    let tail = tail.strip_prefix(':')?.trim_start().strip_prefix('"')?;
    let bytes = tail.as_bytes();
    let mut end = 0;
    while end < bytes.len() {
        match bytes[end] {
            b'"' => break,
            b'\\' => {
                let Some(&next) = bytes.get(end + 1) else {
                    break;
                };
                if next == b'u' {
                    if end + 6 > bytes.len() {
                        break;
                    }
                    end += 6;
                } else {
                    end += 2;
                }
            }
            _ => end += 1,
        }
    }
    let encoded = tail.get(..end)?;
    serde_json::from_str::<String>(&format!("\"{encoded}\"")).ok()
}

fn take_sse_frame(pending: &mut Vec<u8>) -> Option<Vec<u8>> {
    let lf = pending
        .windows(2)
        .position(|w| w == b"\n\n")
        .map(|n| (n, 2));
    let crlf = pending
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|n| (n, 4));
    let (start, width) = match (lf, crlf) {
        (Some(a), Some(b)) => {
            if a.0 < b.0 {
                a
            } else {
                b
            }
        }
        (Some(a), None) => a,
        (None, Some(b)) => b,
        (None, None) => return None,
    };
    Some(pending.drain(..start + width).collect())
}

#[cfg(test)]
mod stream_tests {
    use super::{partial_reply, take_sse_frame};

    #[test]
    fn reads_only_complete_reply_characters() {
        assert_eq!(
            partial_reply(r#"{"reply":"A quiet "#),
            Some("A quiet ".into())
        );
        assert_eq!(partial_reply(r#"{"reply":"A \"#), Some("A ".into()));
        assert_eq!(partial_reply(r#"{"reply":"A \u00"#), Some("A ".into()));
        assert_eq!(partial_reply(r#"{"reply":"A \u00e9"#), Some("A é".into()));
        assert_eq!(
            partial_reply(r#"{"reply":"Done.","results":[{"item_id":"x""#),
            Some("Done.".into())
        );
        assert_eq!(partial_reply(r#"{"results":[]}"#), None);
    }

    #[test]
    fn reads_sse_frame_boundaries() {
        let mut pending = b"data: one\r\n\r\ndata: two\n\nrest".to_vec();
        assert_eq!(
            take_sse_frame(&mut pending),
            Some(b"data: one\r\n\r\n".to_vec())
        );
        assert_eq!(
            take_sse_frame(&mut pending),
            Some(b"data: two\n\n".to_vec())
        );
        assert_eq!(pending, b"rest");
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Evidence {
    pub id: String,
    pub kind: String,
    pub text: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Candidate {
    pub item_id: Uuid,
    pub revision: i32,
    pub subject: String,
    pub entity_kind: String,
    pub evidence: Vec<Evidence>,
    pub locations: Value,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    request_id: Uuid,
    question: String,
    #[serde(default)]
    active_view_id: Option<Uuid>,
    #[serde(default)]
    scope_city: Option<String>,
    #[serde(default)]
    previous_request_id: Option<Uuid>,
    #[serde(default)]
    selected_item_ids: Vec<Uuid>,
    #[serde(default)]
    excluded_item_ids: Vec<Uuid>,
}
#[derive(Deserialize, Default)]
pub struct PageQuery {
    offset: Option<usize>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProposedResult {
    pub item_id: Uuid,
    pub section: String,
    pub reason: String,
    pub caveat: String,
    pub evidence_ids: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Answer {
    #[serde(default)]
    pub reply: String,
    #[serde(default)]
    pub view_ids: Vec<Uuid>,
    pub intent: String,
    #[serde(default)]
    pub new_topic: bool,
    pub title: String,
    pub location: String,
    pub clarification: String,
    pub choices: Vec<String>,
    pub results: Vec<ProposedResult>,
    #[serde(default)]
    pub comparison: Option<Comparison>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Comparison {
    pub item_ids: Vec<Uuid>,
    pub dimensions: Vec<Dimension>,
    pub conclusion: String,
    pub citations: Vec<Citation>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dimension {
    pub label: String,
    pub cells: Vec<ComparisonCell>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComparisonCell {
    pub item_id: Uuid,
    pub text: String,
    pub evidence_ids: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Citation {
    pub item_id: Uuid,
    pub evidence_ids: Vec<String>,
}
impl Answer {
    fn item_ids(&self) -> Vec<Uuid> {
        self.results
            .iter()
            .map(|r| r.item_id)
            .chain(
                self.comparison
                    .iter()
                    .flat_map(|c| c.item_ids.iter().copied()),
            )
            .collect()
    }
}
#[derive(Serialize, Deserialize)]
struct Snapshot {
    #[serde(default)]
    conversation: Conversation,
    answer: Answer,
    candidates: Vec<Candidate>,
    mode: String,
    #[serde(default)]
    location_geo: Value,
    #[serde(default)]
    search_incomplete: bool,
}
#[derive(Clone, Default, Serialize, Deserialize)]
struct Conversation {
    #[serde(default)]
    replies: Vec<String>,
    turns: Vec<String>,
    selected_item_ids: Vec<Uuid>,
    excluded_item_ids: Vec<Uuid>,
}
struct TurnContext {
    conversation: Conversation,
    previous_items: Vec<Candidate>,
    clarification: String,
    active_view: Option<Value>,
}
async fn turn_context(
    state: &AppState,
    owner_id: Uuid,
    generation: i64,
    input: &Input,
) -> Result<TurnContext, ApiError> {
    let active_view = if let Some(id) = input.active_view_id {
        Some(crate::ask_views::view_page(state, owner_id, id, 0).await?)
    } else {
        None
    };
    let view_ids = if let Some(id) = input.active_view_id {
        crate::ask_views::view_ids(state, owner_id, id).await?
    } else {
        vec![]
    };
    let mut conversation = Conversation::default();
    let mut previous_items = vec![];
    let mut clarification = String::new();
    if let Some(id) = input.previous_request_id {
        let value:Option<Value>=sqlx::query_scalar("SELECT result FROM ask_runs WHERE id=$1 AND owner_id=$2 AND permission_generation=$3 AND status='completed' AND expires_at>now() AND result IS NOT NULL")
            .bind(id).bind(owner_id).bind(generation).fetch_optional(&state.pool).await?;
        let snapshot: Snapshot = value
            .and_then(|v| serde_json::from_value(v).ok())
            .ok_or_else(|| {
                ApiError::new(
                    StatusCode::CONFLICT,
                    "ask_context_expired",
                    "This conversation expired. Start a new question.",
                )
            })?;
        conversation = snapshot.conversation;
        conversation.replies.push(format!(
            "{} {}",
            snapshot.answer.reply, snapshot.answer.clarification
        ));
        if conversation.turns.len() >= 8 {
            return Err(ApiError::new(
                StatusCode::CONFLICT,
                "ask_context_full",
                "Start a new question to continue exploring.",
            ));
        }
        let allowed: HashSet<_> = snapshot
            .answer
            .item_ids()
            .into_iter()
            .chain(view_ids.iter().copied())
            .collect();
        if input
            .selected_item_ids
            .iter()
            .chain(input.excluded_item_ids.iter())
            .any(|id| !allowed.contains(id))
        {
            return Err(ApiError::bad(
                "Choose a recommendation from the previous answer",
            ));
        }
        // Selection applies to the next turn only. Exclusions remain session-scoped.
        conversation.selected_item_ids = input.selected_item_ids.clone();
        conversation
            .excluded_item_ids
            .extend(&input.excluded_item_ids);
        conversation.excluded_item_ids.sort();
        conversation.excluded_item_ids.dedup();
        if conversation.excluded_item_ids.len() > 32 {
            return Err(ApiError::bad(
                "Start a new question to reset excluded options",
            ));
        }
        let mut ids = input.selected_item_ids.clone();
        ids.extend(snapshot.answer.item_ids());
        let mut used = HashSet::new();
        let mut changed = false;
        for id in ids {
            if !used.insert(id) || conversation.excluded_item_ids.contains(&id) {
                continue;
            }
            if let Some(item) = crate::app::owner_item(state, owner_id, id).await? {
                let revision = item["revision"].as_i64().unwrap_or(0) as i32;
                changed |= !snapshot
                    .candidates
                    .iter()
                    .any(|c| c.item_id == id && c.revision == revision);
                let rec = &item["recommendation"];
                previous_items.push(Candidate {
                    item_id: id,
                    revision,
                    subject: item["subject"].as_str().unwrap_or_default().into(),
                    entity_kind: rec["entity_kind"].as_str().unwrap_or("note").into(),
                    evidence: evidence(rec, item["body"].as_str().unwrap_or_default()),
                    locations: rec["locations"].clone(),
                });
            } else {
                changed = true;
            }
            if previous_items.len() >= 8 {
                break;
            }
        }
        if conversation
            .selected_item_ids
            .iter()
            .any(|id| !previous_items.iter().any(|c| c.item_id == *id))
        {
            return Err(ApiError::new(
                StatusCode::CONFLICT,
                "ask_reference_changed",
                "The selected recommendation is no longer available. Choose another or start a new question.",
            ));
        }
        if !changed {
            clarification = snapshot.answer.clarification;
        }
    } else if active_view.is_none()
        && (!input.selected_item_ids.is_empty() || !input.excluded_item_ids.is_empty())
    {
        return Err(ApiError::bad(
            "A previous answer is required for a selection",
        ));
    }
    if active_view.is_some() {
        if input
            .selected_item_ids
            .iter()
            .any(|id| !view_ids.contains(id))
        {
            return Err(ApiError::bad("Select recommendations from this collection"));
        }
        conversation.selected_item_ids = input.selected_item_ids.clone();
        for id in input
            .selected_item_ids
            .iter()
            .chain(view_ids.iter().take(8))
        {
            if previous_items.iter().any(|c| c.item_id == *id) {
                continue;
            }
            if let Some(item) = crate::app::owner_item(state, owner_id, *id).await? {
                previous_items.push(candidate_from_item(&item));
            }
            if previous_items.len() >= 8 {
                break;
            }
        }
    }
    conversation.turns.push(input.question.clone());
    Ok(TurnContext {
        conversation,
        previous_items,
        clarification,
        active_view,
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Search {
    terms: Vec<String>,
    #[serde(default)]
    location: String,
    page: usize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Inspect {
    item_ids: Vec<Uuid>,
}

fn bounded(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}
fn evidence(rec: &Value, body: &str) -> Vec<Evidence> {
    let mut result = vec![];
    for key in ["summary", "experience"] {
        if let Some(s) = rec[key].as_str().filter(|s| !s.is_empty()) {
            result.push(Evidence {
                id: key.into(),
                kind: key.into(),
                text: bounded(s, 800),
            });
        }
    }
    for key in ["observations", "use_cases", "locations"] {
        if let Some(values) = rec[key].as_array() {
            for (i, v) in values.iter().take(24).enumerate() {
                if let Some(s) = v["text"].as_str().or_else(|| v.as_str()) {
                    result.push(Evidence {
                        id: format!("{key}.{i}"),
                        kind: v["kind"]
                            .as_str()
                            .or_else(|| v["role"].as_str())
                            .unwrap_or(key)
                            .into(),
                        text: bounded(s, 600),
                    });
                }
            }
        }
    }
    if let Some(label) = rec["classification"]["display_label"].as_str() {
        result.push(Evidence {
            id: "category".into(),
            kind: "category".into(),
            text: bounded(label, 120),
        });
    }
    if result.is_empty() {
        result.push(Evidence {
            id: "body".into(),
            kind: "saved_note".into(),
            text: bounded(body, 2400),
        });
    }
    result
}
pub(crate) fn candidate_from_item(item: &Value) -> Candidate {
    let rec = &item["recommendation"];
    Candidate {
        item_id: Uuid::parse_str(item["id"].as_str().unwrap_or_default())
            .expect("database item id"),
        revision: item["revision"].as_i64().unwrap_or(0) as i32,
        subject: item["subject"].as_str().unwrap_or_default().into(),
        entity_kind: rec["entity_kind"].as_str().unwrap_or("note").into(),
        evidence: evidence(rec, item["body"].as_str().unwrap_or_default()),
        locations: rec["locations"].clone(),
    }
}
async fn search(
    state: &AppState,
    owner_id: Uuid,
    terms: &[String],
    location: &str,
    page: usize,
) -> Result<(Vec<Candidate>, bool, Value), ApiError> {
    if terms.len() > 8
        || terms.iter().any(|s| s.chars().count() > 80)
        || location.chars().count() > 100
        || page > 20
    {
        return Err(ApiError::bad("Invalid search tool arguments"));
    }
    let geo = crate::ask_views::resolve_scope(state, location).await?;
    if !location.trim().is_empty() && geo["status"] != "resolved" {
        return Err(ApiError::bad("Area unresolved; specify its city or region"));
    }
    let area_ids: Vec<String> =
        serde_json::from_value(geo["match_ids"].clone()).unwrap_or_default();
    // Model-proposed phrases are recall clues, not mandatory exact substrings.
    // Retrieve broad token alternatives; the agent checks the complete evidence.
    let mut terms: Vec<_> = terms
        .iter()
        .flat_map(|s| s.split_whitespace())
        .map(|s| {
            s.trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase()
        })
        .filter(|s| s.chars().count() >= 2)
        .take(64)
        .collect();
    terms.sort();
    terms.dedup();
    let rows = sqlx::query(
        r#"
        SELECT id,revision,subject,body,recommendation FROM knowledge_items
        WHERE owner_id=$1 AND deleted_at IS NULL
          AND (cardinality($2::text[])=0 OR EXISTS(
            SELECT 1 FROM unnest($2::text[]) t
            WHERE strpos(lower(subject||' '||body||' '||category_search),t)>0))
          AND (cardinality($4::text[])=0 OR NOT EXISTS(
            SELECT 1 FROM item_location_index l
            WHERE l.item_id=knowledge_items.id
              AND l.locations_snapshot=knowledge_items.recommendation->'locations'
              AND ((recommendation->>'entity_kind'='place' AND l.role='venue')
                OR (coalesce(recommendation->>'entity_kind','note')<>'place'
                  AND l.role IN ('service_area','practice'))))
            OR EXISTS(
            SELECT 1 FROM item_location_index l
            WHERE l.item_id=knowledge_items.id
              AND l.locations_snapshot=knowledge_items.recommendation->'locations'
              AND l.area_ids && $4::text[]
              AND ((recommendation->>'entity_kind'='place' AND l.role='venue')
                OR (coalesce(recommendation->>'entity_kind','note')<>'place'
                  AND l.role IN ('service_area','practice')))))
        ORDER BY CASE WHEN cardinality($4::text[])>0 AND EXISTS(
            SELECT 1 FROM item_location_index l
            WHERE l.item_id=knowledge_items.id
              AND l.locations_snapshot=knowledge_items.recommendation->'locations'
              AND l.area_ids && $4::text[]
              AND ((recommendation->>'entity_kind'='place' AND l.role='venue')
                OR (coalesce(recommendation->>'entity_kind','note')<>'place'
                  AND l.role IN ('service_area','practice')))) THEN 1 ELSE 0 END DESC,
          (SELECT coalesce(sum(
            CASE WHEN strpos(lower(subject),t)>0 THEN 4 ELSE 0 END +
            CASE WHEN strpos(lower(body||' '||category_search),t)>0 THEN 1 ELSE 0 END),0)
            FROM unnest($2::text[]) t) DESC,
          created_at DESC,id DESC LIMIT 9 OFFSET $3
    "#,
    )
    .bind(owner_id)
    .bind(&terms)
    .bind((page * PAGE) as i64)
    .bind(area_ids)
    .fetch_all(&state.pool)
    .await?;
    let more = rows.len() > PAGE;
    let candidates = rows
        .iter()
        .take(PAGE)
        .map(|r| {
            let rec = r
                .try_get::<Value, _>("recommendation")
                .unwrap_or(Value::Null);
            Candidate {
                item_id: r.get("id"),
                revision: r.get("revision"),
                subject: r.get("subject"),
                entity_kind: rec["entity_kind"].as_str().unwrap_or("note").into(),
                evidence: evidence(&rec, r.get::<String, _>("body").as_str()),
                locations: rec["locations"].clone(),
            }
        })
        .collect();
    Ok((candidates, more, geo))
}
async fn permitted(state: &AppState, owner_id: Uuid) -> Result<Option<i64>, ApiError> {
    Ok(sqlx::query_scalar("SELECT generation FROM transcript_extraction_permissions p WHERE account_id=$1 AND enabled AND NOT EXISTS(SELECT 1 FROM processing_permissions g WHERE g.account_id=p.account_id AND NOT g.enabled)")
        .bind(owner_id).fetch_optional(&state.pool).await?)
}
// The complete location object contains browse aliases and hierarchy for the
// Library. Ask needs only the canonical identity and the relation to the item.
fn model_candidate(candidate: &Candidate) -> Value {
    let locations: Vec<_> = candidate
        .locations
        .as_array()
        .into_iter()
        .flatten()
        .take(8)
        .map(|location| {
            json!({
                "role":location["role"],
                "name":location["name"],
                "status":location["geography"]["status"],
                "label":location["geography"]["label"],
                "area_id":location["geography"]["area_id"]
            })
        })
        .collect();
    json!({"item_id":candidate.item_id,"revision":candidate.revision,
        "subject":candidate.subject,"entity_kind":candidate.entity_kind,
        "evidence":candidate.evidence,"locations":locations})
}

pub fn validate_answer(
    mut a: Answer,
    seen: &HashMap<Uuid, Candidate>,
) -> Result<Answer, AgentError> {
    if !["recall", "discovery", "comparison"].contains(&a.intent.as_str())
        || a.reply.chars().count() > 700
        || a.view_ids.len() > 4
        || a.title.chars().count() > 100
        || a.clarification.chars().count() > 240
        || a.choices.len() > 3
        || a.choices.iter().any(|v| v.chars().count() > 80)
        || a.results.len() > 32
        || a.location.chars().count() > 100
    {
        return Err(AgentError::INVALID);
    }
    if let Some(c) = &mut a.comparison {
        let participants: HashSet<_> = c.item_ids.iter().copied().collect();
        if a.intent != "comparison"
            || !(2..=4).contains(&c.item_ids.len())
            || participants.len() != c.item_ids.len()
            || participants.iter().any(|id| !seen.contains_key(id))
            || !(1..=4).contains(&c.dimensions.len())
            || c.conclusion.chars().count() > 400
            || c.citations.len() > 4
        {
            return Err(AgentError::INVALID);
        }
        let supported = |item_id: &Uuid, ids: &[String]| {
            participants.contains(item_id)
                && (1..=3).contains(&ids.len())
                && seen.get(item_id).is_some_and(|item| {
                    ids.iter()
                        .all(|id| item.evidence.iter().any(|e| &e.id == id))
                })
        };
        let mut labels = HashSet::new();
        for dimension in &mut c.dimensions {
            if dimension.label.trim().is_empty()
                || dimension.label.chars().count() > 60
                || !labels.insert(dimension.label.trim().to_lowercase())
                || dimension.cells.len() != participants.len()
            {
                return Err(AgentError::INVALID);
            }
            let mut ids = HashSet::new();
            for cell in &mut dimension.cells {
                if !participants.contains(&cell.item_id)
                    || !ids.insert(cell.item_id)
                    || cell.text.chars().count() > 240
                {
                    return Err(AgentError::INVALID);
                }
                if cell.evidence_ids.is_empty() {
                    // Unknown values cannot carry unsupported generated claims.
                    cell.text = "Not saved".into();
                } else if cell.text.trim().is_empty()
                    || !supported(&cell.item_id, &cell.evidence_ids)
                {
                    return Err(AgentError::INVALID);
                }
            }
            dimension
                .cells
                .sort_by_key(|cell| c.item_ids.iter().position(|id| *id == cell.item_id));
        }
        if c.citations
            .iter()
            .any(|r| !supported(&r.item_id, &r.evidence_ids))
        {
            return Err(AgentError::INVALID);
        }
        // A comparative conclusion needs evidence from every participant.
        if !c.conclusion.is_empty()
            && participants
                .iter()
                .any(|id| !c.citations.iter().any(|r| &r.item_id == id))
        {
            return Err(AgentError::INVALID);
        }
        a.results.clear();
        a.location.clear();
    } else if a.intent == "comparison" && a.clarification.trim().is_empty() {
        return Err(AgentError::INVALID);
    }
    let mut ids = HashSet::new();
    a.results.retain(|r| {
        seen.get(&r.item_id).is_some_and(|c| {
            ["supported", "worth_checking"].contains(&r.section.as_str())
                && r.reason.chars().count() <= 400
                && r.caveat.chars().count() <= 300
                && (1..=3).contains(&r.evidence_ids.len())
                && r.evidence_ids
                    .iter()
                    .all(|id| c.evidence.iter().any(|e| &e.id == id))
                && ids.insert(r.item_id)
        })
    });
    Ok(a)
}

pub async fn run(State(state): State<AppState>, headers: HeaderMap, body: Bytes) -> ApiResult {
    run_inner(state, headers, body, None).await
}

pub async fn run_stream(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let (sender, receiver) = mpsc::unbounded_channel::<Value>();
    let stream = UnboundedReceiverStream::new(receiver)
        .map(|event| Ok::<Bytes, Infallible>(Bytes::from(format!("{event}\n"))));
    tokio::spawn(async move {
        let result = run_inner(state, headers, body, Some(sender.clone())).await;
        match result {
            Ok(response) => {
                if let Ok(bytes) = to_bytes(response.into_body(), 1_000_000).await {
                    if let Ok(answer) = serde_json::from_slice::<Value>(&bytes) {
                        let _ = sender.send(json!({"type":"answer","answer":answer}));
                    }
                }
            }
            Err(error) => {
                let status = error.status.as_u16();
                let code = error.code;
                let message = error.message;
                let _ = sender
                    .send(json!({"type":"error","status":status,"code":code,"message":message}));
            }
        }
    });
    (
        [
            (axum::http::header::CONTENT_TYPE, "application/x-ndjson"),
            (axum::http::header::CACHE_CONTROL, "no-store"),
        ],
        Body::from_stream(stream),
    )
        .into_response()
}

async fn run_inner(
    state: AppState,
    headers: HeaderMap,
    body: Bytes,
    events: Option<mpsc::UnboundedSender<Value>>,
) -> ApiResult {
    let owner_id = owner(&state, &headers, true).await?;
    let mut input: Input = parse(&body)?;
    input.question = input.question.trim().into();
    if !(1..=500).contains(&input.question.chars().count()) {
        return Err(ApiError::bad("Write a question of up to 500 characters"));
    }
    let generation = permitted(&state, owner_id).await?.ok_or_else(|| {
        ApiError::new(
            StatusCode::FORBIDDEN,
            "processing_disabled",
            "Enable understanding in Voice processing to use intelligent Ask.",
        )
    })?;
    if input.selected_item_ids.len() > 4 || input.excluded_item_ids.len() > 8 {
        return Err(ApiError::bad("Too many selected recommendations"));
    }
    if input
        .scope_city
        .as_ref()
        .is_some_and(|v| v.chars().count() > 100 || v.trim().is_empty())
    {
        return Err(ApiError::bad("Invalid city"));
    }
    input.selected_item_ids.sort();
    input.selected_item_ids.dedup();
    input.excluded_item_ids.sort();
    input.excluded_item_ids.dedup();
    let request_hash = crate::auth::hash_token(&serde_json::to_string(&input).expect("input"));
    // Serialize admission across accounts: cap aggregate reserved spend, including unknown outcomes.
    let mut tx = state.pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(827713)")
        .execute(&mut *tx)
        .await?;
    if let Some(row) = sqlx::query("SELECT owner_id,request_hash,status FROM ask_runs WHERE id=$1")
        .bind(input.request_id)
        .fetch_optional(&mut *tx)
        .await?
    {
        if row.get::<Uuid, _>("owner_id") != owner_id
            || row.get::<String, _>("request_hash") != request_hash
        {
            return Err(ApiError::conflict(
                "Ask request changed. Start a new question.",
            ));
        }
        let status: String = row.get("status");
        tx.commit().await?;
        if status == "running" {
            return Err(ApiError::new(
                StatusCode::CONFLICT,
                "ask_running",
                "This question is still being answered. Try again shortly.",
            ));
        }
        return page_response(&state, owner_id, input.request_id, 0).await;
    }
    let context = turn_context(&state, owner_id, generation, &input).await?;
    if !state.ask_model.available() {
        return Err(ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "ask_unavailable",
            "Intelligent Ask is temporarily unavailable. Your Library is still available.",
        ));
    }
    let counts=sqlx::query("SELECT count(*) total,count(*) FILTER(WHERE owner_id=$1) personal,count(*) FILTER(WHERE owner_id=$1 AND status='running' AND created_at>now()-interval '1 minute') active FROM ask_runs WHERE created_at>now()-interval '24 hours' AND reserved_microusd>0").bind(owner_id).fetch_one(&mut *tx).await?;
    if counts.get::<i64, _>("active") >= 1 {
        return Err(ApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "ask_running",
            "Ask is still answering another question. Try again in a moment.",
        ));
    }
    if counts.get::<i64, _>("personal") >= state.ask_daily_account_limit {
        return Err(ApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "ask_daily_limit",
            "You’ve reached the 24-hour Ask limit. Try again later.",
        ));
    }
    if counts.get::<i64, _>("total") >= 500 {
        return Err(ApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "ask_busy",
            "Ask is busy right now. Try again later.",
        ));
    }
    sqlx::query("UPDATE ask_runs SET result=NULL,status=CASE WHEN status='running' THEN 'failed' ELSE status END WHERE expires_at<now() AND (result IS NOT NULL OR status='running')").execute(&mut *tx).await?;
    sqlx::query("INSERT INTO ask_runs(id,owner_id,request_hash,status,permission_generation) VALUES($1,$2,$3,'running',$4)").bind(input.request_id).bind(owner_id).bind(request_hash).bind(generation).execute(&mut *tx).await?;
    tx.commit().await?;
    let result = tokio::time::timeout(
        Duration::from_secs(50),
        execute(
            &state,
            &headers,
            owner_id,
            generation,
            input.request_id,
            &input.question,
            input.scope_city.as_deref(),
            context,
            events.clone(),
        ),
    )
    .await;
    let (snapshot, status) = match result {
        Ok(Ok(s)) => (Some(s), "completed"),
        _ => (None, "failed"),
    };
    // Do not persist private output after withdrawal or session revocation.
    if permitted(&state, owner_id).await? != Some(generation)
        || owner(&state, &headers, true).await.ok() != Some(owner_id)
    {
        sqlx::query("UPDATE ask_runs SET status='failed',result=NULL WHERE id=$1")
            .bind(input.request_id)
            .execute(&state.pool)
            .await?;
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "processing_changed",
            "Your processing permission or session changed. Please retry.",
        ));
    }
    sqlx::query("UPDATE ask_runs SET status=$2,result=$3 WHERE id=$1 AND status='running'")
        .bind(input.request_id)
        .bind(status)
        .bind(snapshot.map(|s| serde_json::to_value(s).expect("snapshot")))
        .execute(&state.pool)
        .await?;
    page_response(&state, owner_id, input.request_id, 0).await
}
async fn execute(
    state: &AppState,
    headers: &HeaderMap,
    owner_id: Uuid,
    generation: i64,
    run: Uuid,
    question: &str,
    scope_city: Option<&str>,
    turn: TurnContext,
    events: Option<mpsc::UnboundedSender<Value>>,
) -> Result<Snapshot, ApiError> {
    let mut conversation = turn.conversation;
    let mut seen: HashMap<Uuid, Candidate> = turn
        .previous_items
        .iter()
        .map(|c| (c.item_id, c.clone()))
        .collect();
    let mut observations = vec![];
    let overview = crate::ask_views::overview(state, owner_id).await?;
    let mut provider_input: Vec<Value> = vec![];
    let mut opened_views: HashSet<Uuid> = HashSet::new();
    if let Some(view) = &turn.active_view {
        if let Some(id) = view["view_id"]
            .as_str()
            .and_then(|v| Uuid::parse_str(v).ok())
        {
            opened_views.insert(id);
        }
    }

    let mut calls = HashSet::new();
    let mut reads = 0;
    let mut search_incomplete = false;
    let mut rejection = "decision_limit";
    let mut presentation_rejected = false;
    // Ordinary turns stay within three decisions. A rejected presentation gets
    // one bounded repair call instead of immediately failing the conversation.
    for step in 0..=MAX_DECISIONS {
        if step == MAX_DECISIONS && !presentation_rejected {
            break;
        }
        if owner(state, headers, true).await? != owner_id
            || permitted(state, owner_id).await? != Some(generation)
        {
            return Err(ApiError::bad("Processing permission changed"));
        }
        if !sqlx::query_scalar::<_, bool>("SELECT status='running' FROM ask_runs WHERE id=$1")
            .bind(run)
            .fetch_one(&state.pool)
            .await?
        {
            return Err(ApiError::conflict("Question cancelled"));
        }
        sqlx::query("UPDATE ask_runs SET decisions=decisions+1 WHERE id=$1")
            .bind(run)
            .execute(&state.pool)
            .await?;
        for candidate in seen.values() {
            let Some(current) = crate::app::owner_item(state, owner_id, candidate.item_id).await?
            else {
                return Err(ApiError::conflict("A recommendation changed. Ask again."));
            };
            if current["revision"].as_i64() != Some(candidate.revision as i64) {
                return Err(ApiError::conflict("A recommendation changed. Ask again."));
            }
        }
        let previous_results: Vec<_> = turn.previous_items.iter().map(model_candidate).collect();
        let mut context = json!({"knowledge_overview":overview,"active_view":turn.active_view.as_ref().map(|v|json!({"view_id":v["view_id"],"spec":v["spec"],"total":v["total"]})),"question":question,"conversation":conversation,"scope_city":scope_city,"previous_results":previous_results,"previous_clarification":turn.clarification,"tool_observations":observations,"decisions_left":(MAX_DECISIONS-step).max(1),"read_tools_left":4-reads});
        if provider_input.is_empty() {
            provider_input = vec![
                json!({"role":"system","content":include_str!("../prompts/ask_v1.txt")}),
                json!({"role":"user","content":context.to_string()}),
            ];
        }
        context["provider_input"] = json!(provider_input);
        let decision = match state
            .ask_model
            .decide_stream(context, step >= MAX_DECISIONS - 1, events.clone())
            .await
        {
            Ok(v) => v,
            Err(e) => {
                if let Some(events) = &events {
                    let _ = events.send(json!({"type":"reset"}));
                }
                sqlx::query("UPDATE ask_runs SET failure_code=$2 WHERE id=$1")
                    .bind(run)
                    .bind(e.code)
                    .execute(&state.pool)
                    .await?;
                break;
            }
        };
        sqlx::query("UPDATE ask_runs SET input_tokens=input_tokens+$2,output_tokens=output_tokens+$3 WHERE id=$1").bind(run).bind(decision.input_tokens.max(0)).bind(decision.output_tokens.max(0)).execute(&state.pool).await?;
        provider_input.extend(decision.output_items);
        for call in decision.calls {
            // Every call gets a matching result, including malformed calls, so
            // continuation remains a valid Responses conversation.
            let output_index = provider_input.len();
            if !call.call_id.is_empty() {
                provider_input.push(json!({"type":"function_call_output","call_id":call.call_id,"output":"Tool rejected or budget exhausted. Correct arguments or present a supported answer."}));
            }

            if call.name == "present_answer" {
                if let Some(mut a) = serde_json::from_value::<Answer>(call.arguments)
                    .ok()
                    .and_then(|a| validate_answer(a, &seen).ok())
                {
                    // A bad optional view reference is omitted; it must not
                    // discard an otherwise usable answer or expose another view.
                    a.view_ids.retain(|id| opened_views.contains(id));
                    if let Some(comparison) = &a.comparison {
                        let selected = &conversation.selected_item_ids;
                        if comparison
                            .item_ids
                            .iter()
                            .any(|id| conversation.excluded_item_ids.contains(id))
                            || (!selected.is_empty()
                                && (selected.len() != comparison.item_ids.len()
                                    || selected.iter().any(|id| !comparison.item_ids.contains(id))))
                        {
                            rejection = "comparison_scope";
                            presentation_rejected = true;
                            if let Some(events) = &events {
                                let _ = events.send(json!({"type":"reset"}));
                            }
                            observations.push(json!({"error":"Compare exactly the explicitly selected IDs, never excluded items. Ask for clarification if fewer than two are identified."}));
                            continue;
                        }
                    }
                    if a.new_topic && conversation.selected_item_ids.is_empty() {
                        conversation.turns = vec![question.to_owned()];
                        conversation.excluded_item_ids.clear();
                    }
                    a.results
                        .retain(|r| !conversation.excluded_item_ids.contains(&r.item_id));
                    if a.intent == "recall" {
                        a.location.clear();
                    }
                    if a.intent == "discovery" && a.location.is_empty() {
                        a.location = scope_city.unwrap_or_default().to_owned();
                    }
                    let location_geo = crate::ask_views::resolve_scope(state, &a.location).await?;
                    return Ok(Snapshot {
                        conversation,
                        answer: a,
                        candidates: seen.into_values().collect(),
                        mode: "agent".into(),
                        location_geo,
                        search_incomplete,
                    });
                }
                rejection = "invalid_answer";
                presentation_rejected = true;
                if let Some(events) = &events {
                    let _ = events.send(json!({"type":"reset"}));
                }
                observations.push(
                    json!({"error":"Invalid answer. Use only seen item IDs and evidence IDs."}),
                );
                continue;
            }
            if reads >= 4 {
                continue;
            }
            reads += 1;
            let signature = format!("{}:{}", call.name, call.arguments);
            if !calls.insert(signature) {
                observations.push(json!({"error":"Repeated unchanged tool call; inspect existing evidence or finish."}));
                continue;
            }
            if owner(state, headers, true).await? != owner_id
                || permitted(state, owner_id).await? != Some(generation)
            {
                return Err(ApiError::bad("Processing permission changed"));
            }
            let output = match call.name.as_str() {
                "open_collection" => {
                    match serde_json::from_value::<crate::ask_views::BrowseSpec>(
                        call.arguments.clone(),
                    ) {
                        Ok(spec) => {
                            match crate::ask_views::create_view(state, owner_id, spec).await {
                                Ok(view) => {
                                    let view_id = view["view_id"]
                                        .as_str()
                                        .and_then(|s| Uuid::parse_str(s).ok());
                                    let found: Vec<_> = view["items"]
                                        .as_array()
                                        .into_iter()
                                        .flatten()
                                        .take(8)
                                        .map(candidate_from_item)
                                        .collect();
                                    for c in &found {
                                        seen.insert(c.item_id, c.clone());
                                    }
                                    let total = view["total"].as_u64().unwrap_or(0);
                                    if total < 2 {
                                        // Sparse agent-created views are just answer candidates,
                                        // not useful navigation. Native Explore may still open them.
                                        if let Some(id) = view_id {
                                            sqlx::query(
                                                "DELETE FROM ask_views WHERE id=$1 AND owner_id=$2",
                                            )
                                            .bind(id)
                                            .bind(owner_id)
                                            .execute(&state.pool)
                                            .await?;
                                        }
                                        json!({"view_id":null,"total":total,"items":found.iter().map(model_candidate).collect::<Vec<_>>(),"presentation":"direct_answer","message":"No browsable collection was created. Give a direct cited result if one item fits; otherwise explain the gap without an empty collection."})
                                    } else {
                                        if let Some(id) = view_id {
                                            opened_views.insert(id);
                                        }
                                        json!({"view_id":view["view_id"],"title":view["title"],"total":view["total"],"spec":view["spec"],"facets":view["facets"],"items":found.iter().map(model_candidate).collect::<Vec<_>>(),"complete_collection":true})
                                    }
                                }
                                Err(_) => {
                                    json!({"error":"Could not resolve/create that collection. An unindexed neighbourhood is not the whole city: search the exact locality phrase within a verified city, or offer a clearly broader city collection. Check available category IDs."})
                                }
                            }
                        }
                        Err(_) => json!({"error":"Invalid collection specification"}),
                    }
                }
                "search_knowledge" => {
                    if let Ok(s) = serde_json::from_value::<Search>(call.arguments.clone()) {
                        match search(state, owner_id, &s.terms, &s.location, s.page).await {
                            Ok((found, more, geo)) => {
                                search_incomplete |= more;
                                for c in &found {
                                    seen.insert(c.item_id, c.clone());
                                }
                                json!({"items":found.iter().map(model_candidate).collect::<Vec<_>>(),"next_page":if more {Some(s.page+1)}else{None},
                                    "resolved_scope":if geo["status"] == "resolved" {json!({"area_id":geo["area_id"],"label":geo["label"]})}else{Value::Null}})
                            }
                            Err(error) if error.is_client_correction() => {
                                json!({"error":"That area is not indexed to the requested precision. Use an explicit city with exact locality text as a search clue, and do not call city-wide results neighbourhood matches."})
                            }
                            Err(error) => return Err(error),
                        }
                    } else {
                        json!({"error":"Use terms and page"})
                    }
                }
                "inspect_evidence" => {
                    if let Ok(i) = serde_json::from_value::<Inspect>(call.arguments.clone()) {
                        json!({"items":i.item_ids.iter().take(8).filter_map(|id|seen.get(id)).map(model_candidate).collect::<Vec<_>>()})
                    } else {
                        json!({"error":"Use seen item_ids"})
                    }
                }
                _ => json!({"error":"Tool unavailable"}),
            };
            if !call.call_id.is_empty() {
                provider_input[output_index]["output"] = json!(output.to_string());
            }
            observations.push(json!({"tool":call.name,"arguments":call.arguments,"result":output}));
        }
    }
    sqlx::query("UPDATE ask_runs SET failure_code=$2 WHERE id=$1 AND failure_code IS NULL")
        .bind(run)
        .bind(rejection)
        .execute(&state.pool)
        .await?;
    // A failed agent decision is an interrupted request, not evidence that the
    // owner's Library has no suitable recommendations. The client retains the
    // previous answer and can retry with a fresh request ID.
    Err(ApiError::new(
        StatusCode::SERVICE_UNAVAILABLE,
        "ask_failed",
        "Couldn’t finish checking your saved recommendations. Try again.",
    ))
}

pub async fn page(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Query(q): Query<PageQuery>,
) -> ApiResult {
    let owner_id = owner(&state, &headers, true).await?;
    page_response(&state, owner_id, id, q.offset.unwrap_or(0)).await
}
async fn page_response(state: &AppState, owner_id: Uuid, id: Uuid, offset: usize) -> ApiResult {
    if offset > 32 || !offset.is_multiple_of(PAGE) {
        return Err(ApiError::bad("Invalid answer page"));
    }
    let row=sqlx::query("SELECT result,permission_generation FROM ask_runs WHERE id=$1 AND owner_id=$2 AND expires_at>now()")
        .bind(id).bind(owner_id).fetch_optional(&state.pool).await?.ok_or_else(||ApiError::not_found("This answer expired. Ask again."))?;
    if permitted(state, owner_id).await? != Some(row.get("permission_generation")) {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "processing_changed",
            "Your processing settings changed. Ask again.",
        ));
    }
    let snapshot: Snapshot = serde_json::from_value(row.try_get("result").unwrap_or(Value::Null))
        .map_err(|_| {
        ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "ask_failed",
            "Couldn’t finish this answer. Try again.",
        )
    })?;
    let mut valid = vec![];
    let mut changed = false;
    for result in &snapshot.answer.results {
        let Some(candidate) = snapshot
            .candidates
            .iter()
            .find(|c| c.item_id == result.item_id)
        else {
            continue;
        };
        if let Some(item) = crate::app::owner_item(state, owner_id, result.item_id).await? {
            if item["revision"].as_i64() != Some(candidate.revision as i64) {
                changed = true;
                continue;
            }
            let mut section = result.section.clone();
            let mut caveat = result.caveat.clone();
            if snapshot.answer.intent == "discovery" && !snapshot.answer.location.is_empty() {
                let mut current_candidate = candidate.clone();
                current_candidate.locations = item["recommendation"]["locations"].clone();
                match location_match(&current_candidate, &snapshot.location_geo) {
                    LocationMatch::No => continue,
                    LocationMatch::Unknown => {
                        section = "worth_checking".into();
                        caveat = "Location or service area needs checking.".into();
                    }
                    LocationMatch::Yes => {}
                }
            }
            let support: Vec<_> = candidate
                .evidence
                .iter()
                .filter(|e| result.evidence_ids.contains(&e.id))
                .collect();
            valid.push(json!({"item":item,"section":section,"reason":result.reason,"caveat":caveat,"evidence":support}));
        } else {
            changed = true;
        }
    }
    let mut comparison = Value::Null;
    if let Some(c) = &snapshot.answer.comparison {
        let mut items = vec![];
        for item_id in &c.item_ids {
            let candidate = snapshot.candidates.iter().find(|v| &v.item_id == item_id);
            let item = crate::app::owner_item(state, owner_id, *item_id).await?;
            match (candidate, item) {
                (Some(saved), Some(item))
                    if item["revision"].as_i64() == Some(saved.revision as i64) =>
                {
                    items.push(item)
                }
                _ => changed = true,
            }
        }
        // Invalidate the whole comparison, including its conclusion, if any source changes.
        if !changed {
            let support = |id: Uuid, ids: &[String]| -> Vec<&Evidence> {
                snapshot
                    .candidates
                    .iter()
                    .find(|v| v.item_id == id)
                    .into_iter()
                    .flat_map(|v| v.evidence.iter())
                    .filter(|e| ids.contains(&e.id))
                    .collect()
            };
            comparison = json!({"items":items,"dimensions":c.dimensions.iter().map(|d| json!({
                "label":d.label,"cells":d.cells.iter().map(|cell| json!({"item_id":cell.item_id,
                    "text":cell.text,"evidence":support(cell.item_id, &cell.evidence_ids)})).collect::<Vec<_>>()
            })).collect::<Vec<_>>(),"conclusion":c.conclusion,
                "citations":c.citations.iter().map(|r| json!({"item_id":r.item_id,"evidence":support(r.item_id,&r.evidence_ids)})).collect::<Vec<_>>()});
        }
    }
    let mut views = vec![];
    for view_id in &snapshot.answer.view_ids {
        match crate::ask_views::view_page(state, owner_id, *view_id, 0).await {
            // Also hide sparse views from answers generated before the agent
            // stopped creating them. Direct Explore has its own view route.
            Ok(v) if v["total"].as_u64().unwrap_or(0) >= 2 => views.push(v),
            Ok(_) => {}
            Err(_) => {
                changed = true;
            }
        }
    }
    let next = if valid.len() > offset + PAGE {
        Some(offset + PAGE)
    } else {
        None
    };
    Ok(ok(
        json!({"version":1,"request_id":id,"question":snapshot.conversation.turns.last(),"turn_count":snapshot.conversation.turns.len(),"selected_item_ids":snapshot.conversation.selected_item_ids,"excluded_item_ids":snapshot.conversation.excluded_item_ids,"mode":snapshot.mode,"search_incomplete":snapshot.search_incomplete,"intent":snapshot.answer.intent,
        "title":if changed {"Saved recommendations"}else{&snapshot.answer.title},"clarification":if changed {""}else{&snapshot.answer.clarification},
        "choices":if changed {vec![]}else{snapshot.answer.choices},"location":snapshot.answer.location,"changed":changed,
        "reply":if changed {""}else{&snapshot.answer.reply},"views":views,"comparison":comparison,"results":valid.into_iter().skip(offset).take(PAGE).collect::<Vec<_>>(),"next_offset":next}),
    ))
}
#[derive(PartialEq, Debug)]
enum LocationMatch {
    Yes,
    No,
    Unknown,
}
fn location_match(c: &Candidate, query: &Value) -> LocationMatch {
    let Some(area_id) = query["area_id"]
        .as_str()
        .filter(|_| query["status"] == "resolved")
    else {
        return LocationMatch::Unknown;
    };
    let roles: &[&str] = if c.entity_kind == "place" {
        &["venue"]
    } else {
        &["service_area", "practice"]
    };
    let Some(locations) = c.locations.as_array() else {
        return LocationMatch::Unknown;
    };
    let relevant: Vec<_> = locations
        .iter()
        .filter(|l| roles.contains(&l["role"].as_str().unwrap_or("")))
        .collect();
    let mut resolved = false;
    let mut unknown = relevant.is_empty();
    for location in relevant {
        let geography = &location["geography"];
        if geography["status"] != "resolved" {
            unknown = true;
            continue;
        }
        resolved = true;
        if geography["filter_ids"].as_array().is_some_and(|ids| {
            ids.iter().any(|v| {
                v.as_str() == Some(area_id)
                    || query["match_ids"].as_array().is_some_and(|q| q.contains(v))
            })
        }) {
            return LocationMatch::Yes;
        }
    }
    if resolved && !unknown {
        LocationMatch::No
    } else {
        LocationMatch::Unknown
    }
}

/// Expire answer content independently of client activity; retain only bounded usage receipts.
pub async fn sweep(pool: &sqlx::PgPool) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE ask_runs SET result=NULL,status=CASE WHEN status='running' THEN 'failed' ELSE status END WHERE expires_at<now() AND (result IS NOT NULL OR status='running')").execute(pool).await?;
    sqlx::query("UPDATE ask_runs SET status='failed',result=NULL WHERE status='running' AND created_at<now()-interval '1 minute'").execute(pool).await?;
    sqlx::query("DELETE FROM ask_views WHERE expires_at<now()")
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM ask_runs WHERE created_at<now()-interval '30 days'")
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn cancel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> ApiResult {
    let owner_id = owner(&state, &headers, true).await?;
    let mut tx = state.pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(827713)")
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE ask_runs SET status='cancelled',result=NULL WHERE id=$1 AND owner_id=$2")
        .bind(id)
        .bind(owner_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO ask_runs(id,owner_id,request_hash,status,permission_generation,reserved_microusd) SELECT $1,$2,'cancelled','cancelled',0,0 WHERE (SELECT count(*) FROM ask_runs WHERE owner_id=$2 AND created_at>now()-interval '24 hours')<100 ON CONFLICT(id) DO NOTHING")
        .bind(id).bind(owner_id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(ok(json!({"cancelled":true})))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn candidate() -> Candidate {
        Candidate {
            item_id: Uuid::nil(),
            revision: 1,
            subject: "Lantern".into(),
            entity_kind: "place".into(),
            evidence: vec![Evidence {
                id: "summary".into(),
                kind: "summary".into(),
                text: "Quiet meal, but small tables.".into(),
            }],
            locations: json!([{"role":"venue","geography":{"status":"resolved","filter_ids":["pune","india"]}}]),
        }
    }
    #[test]
    fn geography_enforces_roles_and_unknowns() {
        let mut c = candidate();
        let pune = json!({"status":"resolved","area_id":"pune"});
        assert_eq!(location_match(&c, &pune), LocationMatch::Yes);
        assert_eq!(
            location_match(&c, &json!({"status":"resolved","area_id":"delhi"})),
            LocationMatch::No
        );
        c.locations[0]["role"] = json!("past_experience");
        assert_eq!(location_match(&c, &pune), LocationMatch::Unknown);
        c.entity_kind = "person_service".into();
        assert_eq!(location_match(&c, &pune), LocationMatch::Unknown);
        c.locations[0]["role"] = json!("service_area");
        assert_eq!(location_match(&c, &pune), LocationMatch::Yes);
        assert_eq!(location_match(&c, &Value::Null), LocationMatch::Unknown);
    }
    #[test]
    fn forged_ids_and_evidence_cannot_enter_an_answer() {
        let c = candidate();
        let seen = HashMap::from([(c.item_id, c.clone())]);
        let good = ProposedResult {
            item_id: c.item_id,
            section: "supported".into(),
            reason: "A quiet meal.".into(),
            caveat: "Small tables.".into(),
            evidence_ids: vec!["summary".into()],
        };
        let mut bad = good.clone();
        bad.item_id = Uuid::new_v4();
        let mut fake_evidence = good.clone();
        fake_evidence.evidence_ids = vec!["invented".into()];
        let a = Answer {
            reply: String::new(),
            view_ids: vec![],
            intent: "discovery".into(),
            comparison: None,
            new_topic: false,
            title: "For dinner".into(),
            location: String::new(),
            clarification: String::new(),
            choices: vec![],
            results: vec![bad, fake_evidence, good],
        };
        // Invalid entries must not poison an otherwise valid entry of the same ID.
        assert_eq!(validate_answer(a, &seen).unwrap().results.len(), 1);
    }
    #[test]
    fn comparison_requires_same_participants_and_grounded_cells_and_conclusion() {
        let first = candidate();
        let mut second = candidate();
        second.item_id = Uuid::new_v4();
        let seen = HashMap::from([
            (first.item_id, first.clone()),
            (second.item_id, second.clone()),
        ]);
        let value = json!({"intent":"comparison","title":"A useful comparison","location":"", "clarification":"","choices":[],"results":[],
            "comparison":{"item_ids":[first.item_id,second.item_id],"dimensions":[{"label":"Price","cells":[
                {"item_id":first.item_id,"text":"Definitely cheap","evidence_ids":[]},
                {"item_id":second.item_id,"text":"Not saved","evidence_ids":[]}
            ]}],"conclusion":"","citations":[]}});
        let check = |v| validate_answer(serde_json::from_value(v).unwrap(), &seen);
        let valid = check(value.clone()).unwrap();
        assert_eq!(
            valid.comparison.unwrap().dimensions[0].cells[0].text,
            "Not saved"
        );
        let mut bad = value.clone();
        bad["comparison"]["dimensions"][0]["cells"][0]["evidence_ids"] = json!(["invented"]);
        assert!(check(bad).is_err());
        let mut bad = value.clone();
        bad["comparison"]["dimensions"][0]["cells"][1]["item_id"] = json!(first.item_id);
        assert!(check(bad).is_err());
        let mut bad = value.clone();
        bad["comparison"]["item_ids"][1] = json!(Uuid::new_v4());
        assert!(check(bad).is_err());
        let mut bad = value.clone();
        bad["comparison"]["conclusion"] = json!("A is better");
        assert!(check(bad).is_err());
        let mut bad = value;
        bad["comparison"]["dimensions"][0]["cells"]
            .as_array_mut()
            .unwrap()
            .pop();
        assert!(check(bad).is_err());
    }
    #[test]
    fn projections_exclude_raw_contact_fields_and_source_support() {
        let r = json!({"summary":"Reliable driver.","contact":{"phone":"+919876543210"},"source_support":{"raw":"private transcript"}});
        let text = serde_json::to_string(&evidence(&r, "unused")).unwrap();
        assert!(!text.contains("9876543210"));
        assert!(!text.contains("private transcript"));
    }
    #[test]
    fn ask_projection_omits_browse_aliases_and_hierarchy() {
        let mut c = candidate();
        c.locations = json!([{"role":"venue","name":"Bengaluru",
            "geography":{"status":"resolved","area_id":"city-1","label":"Bengaluru",
                "browse":{"aliases":["Bangalore","Bangalore East"]},
                "hierarchy":{"private":"bulky catalog"}}}]);
        let projection = model_candidate(&c);
        let encoded = projection.to_string();
        assert!(encoded.contains("Bengaluru"));
        assert!(encoded.contains("city-1"));
        assert!(!encoded.contains("Bangalore East"));
        assert!(!encoded.contains("bulky catalog"));
    }
}
