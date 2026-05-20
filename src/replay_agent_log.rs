use chrono::DateTime;
use serde::Serialize;
use serde_json::Value;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

#[derive(Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ReplayAgentBlock {
    Text {
        text: String,
    },
    Thinking {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        summary: String,
        input: Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<ReplayToolResult>,
    },
}

#[derive(Clone, Serialize)]
pub struct ReplayToolResult {
    pub content: String,
    pub is_error: bool,
}

#[derive(Clone, Serialize)]
pub struct ReplayAgentMessage {
    pub id: String,
    pub role: String,
    pub timestamp_ms: i64,
    pub tick: f64,
    pub approx: bool,
    pub blocks: Vec<ReplayAgentBlock>,
}

#[derive(Clone)]
pub struct ReplayAgentTimeline {
    pub messages: Vec<ReplayAgentMessage>,
    pub anchor_count: usize,
}

impl ReplayAgentTimeline {
    pub fn messages_up_to(&self, tick: f64) -> Vec<ReplayAgentMessage> {
        let cutoff = self.messages.partition_point(|msg| msg.tick <= tick);
        self.messages[..cutoff].to_vec()
    }
}

#[derive(Clone)]
struct TickAnchor {
    ts_ms: i64,
    tick: f64,
}

#[derive(Clone)]
pub struct ReplayTickAnchor {
    pub timestamp_ms: i64,
    pub tick: u64,
}

struct RawReplayAgentMessage {
    id: String,
    role: String,
    timestamp_ms: i64,
    blocks: Vec<RawBlock>,
}

pub fn load_agent_timeline(
    path: &Path,
    tick_rate: u32,
    first_tick: u64,
) -> Result<ReplayAgentTimeline, String> {
    // Legacy fallback: callers without recording timeline anchors still infer ticks
    // from transcript contents and align the first message to first_tick. Prefer
    // load_agent_timeline_with_external_anchors for all replay formats; remove this
    // fallback once old recordings without event anchors are no longer supported.
    load_agent_timeline_with_external_anchors(path, tick_rate, first_tick, &[])
}

pub fn load_agent_timeline_with_external_anchors(
    path: &Path,
    tick_rate: u32,
    first_tick: u64,
    external_anchors: &[ReplayTickAnchor],
) -> Result<ReplayAgentTimeline, String> {
    let file = File::open(path).map_err(|e| format!("open failed: {}", e))?;
    let reader = BufReader::new(file);

    let mut anchors: Vec<TickAnchor> = external_anchors
        .iter()
        .map(|anchor| TickAnchor {
            ts_ms: anchor.timestamp_ms,
            tick: anchor.tick as f64,
        })
        .collect();
    let mut raw_messages: Vec<RawReplayAgentMessage> = Vec::new();
    let first_external_anchor_ts = external_anchors.iter().map(|anchor| anchor.timestamp_ms).min();

    for line in reader.lines() {
        let line = line.map_err(|e| format!("read failed: {}", e))?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(trimmed) else {
            continue;
        };
        let Some(ts_ms) = parse_agent_timestamp_ms(&value) else {
            continue;
        };
        if first_external_anchor_ts.is_some_and(|first_ts| ts_ms < first_ts) {
            continue;
        }
        // Legacy fallback: old agent transcripts sometimes contain tool output with
        // a serialized {"tick": ...}. New replay integrations should use external
        // recording event anchors instead of depending on transcript text.
        if let Some(tick) = extract_tick_anchor(&value) {
            anchors.push(TickAnchor {
                ts_ms,
                tick: tick as f64,
            });
        }
        if let Some(raw_message) = parse_raw_message(&value, ts_ms) {
            raw_messages.push(raw_message);
        }
    }

    let raw_messages = fold_tool_results(raw_messages);

    anchors.sort_by_key(|a| a.ts_ms);
    anchors.dedup_by(|a, b| a.ts_ms == b.ts_ms && (a.tick - b.tick).abs() < f64::EPSILON);

    let tick_rate_per_ms = tick_rate as f64 / 1000.0;
    let mut messages = raw_messages
        .into_iter()
        .filter_map(|raw| {
            let blocks: Vec<ReplayAgentBlock> = raw
                .blocks
                .into_iter()
                .filter_map(|b| match b {
                    RawBlock::Block(block) => Some(block),
                    RawBlock::ToolResult(_) => None,
                })
                .collect();
            if blocks.is_empty() {
                return None;
            }
            let (tick, approx) =
                infer_tick_from_anchors(raw.timestamp_ms, &anchors, tick_rate_per_ms);
            Some(ReplayAgentMessage {
                id: raw.id,
                role: raw.role,
                timestamp_ms: raw.timestamp_ms,
                tick,
                approx,
                blocks,
            })
        })
        .collect::<Vec<_>>();

    messages.sort_by(|a, b| {
        a.tick
            .partial_cmp(&b.tick)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.timestamp_ms.cmp(&b.timestamp_ms))
    });

    if external_anchors.is_empty() {
        // Legacy fallback for recordings that predate event anchors. This can place
        // stale transcript messages at replay start, so anchored callers bypass it.
        align_timeline_to_first_tick(&mut messages, first_tick);
    } else {
        messages.retain(|msg| msg.tick >= first_tick as f64);
    }

    Ok(ReplayAgentTimeline {
        messages,
        anchor_count: anchors.len(),
    })
}

fn align_timeline_to_first_tick(messages: &mut [ReplayAgentMessage], first_tick: u64) {
    let Some(first_message_tick) = messages.first().map(|msg| msg.tick) else {
        return;
    };
    let shift = first_tick as f64 - first_message_tick;
    if shift.abs() < f64::EPSILON {
        return;
    }
    let min_tick = first_tick as f64;
    for message in messages.iter_mut() {
        message.tick = (message.tick + shift).max(min_tick);
    }
}

fn parse_agent_timestamp_ms(value: &Value) -> Option<i64> {
    if let Some(ms) = value
        .get("message")
        .and_then(|m| m.get("timestamp"))
        .and_then(|v| v.as_i64())
    {
        return Some(ms);
    }
    let iso = value.get("timestamp").and_then(|v| v.as_str())?;
    let parsed = DateTime::parse_from_rfc3339(iso).ok()?;
    Some(parsed.timestamp_millis())
}

fn parse_raw_message(value: &Value, timestamp_ms: i64) -> Option<RawReplayAgentMessage> {
    if let Some(message) = value.get("message") {
        let top_level_type = value
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let role = message
            .get("role")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let effective_role = if role.is_empty() && matches!(top_level_type, "assistant" | "user") {
            top_level_type.to_string()
        } else {
            role
        };
        if effective_role == "assistant" || effective_role == "user" {
            let blocks = extract_raw_blocks(message);
            if !blocks.is_empty() {
                return Some(RawReplayAgentMessage {
                    id: extract_message_id(value),
                    role: effective_role,
                    timestamp_ms,
                    blocks,
                });
            }
        }
    }

    let top_level_type = value
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let payload = value.get("payload")?;
    match top_level_type {
        "response_item" => parse_codex_response_item(payload, value, timestamp_ms),
        "event_msg" => parse_codex_event_msg(payload, value, timestamp_ms),
        _ => None,
    }
}

fn parse_codex_response_item(
    payload: &Value,
    value: &Value,
    timestamp_ms: i64,
) -> Option<RawReplayAgentMessage> {
    let payload_type = payload
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let role = match payload_type {
        "message" => payload
            .get("role")
            .and_then(|v| v.as_str())
            .unwrap_or_default(),
        "function_call" | "custom_tool_call" | "function_call_output" | "custom_tool_call_output" => {
            "assistant"
        }
        _ => "",
    };
    if role != "assistant" && role != "user" {
        return None;
    }

    let blocks = match payload_type {
        "message" => extract_raw_blocks(payload),
        "function_call" => vec![RawBlock::Block(build_tool_use_block(
            payload.get("call_id").and_then(|v| v.as_str()).unwrap_or_default(),
            payload.get("name").and_then(|v| v.as_str()).unwrap_or("tool"),
            parse_json_string_arg(payload.get("arguments")),
        ))],
        "custom_tool_call" => vec![RawBlock::Block(build_tool_use_block(
            payload.get("call_id").and_then(|v| v.as_str()).unwrap_or_default(),
            payload.get("name").and_then(|v| v.as_str()).unwrap_or("tool"),
            payload.get("input").cloned().unwrap_or(Value::Null),
        ))],
        "function_call_output" => vec![RawBlock::ToolResult(PendingToolResult {
            tool_use_id: payload
                .get("call_id")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            content: truncate(
                payload
                    .get("output")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .trim(),
                TOOL_RESULT_MAX,
            ),
            is_error: false,
        })],
        "custom_tool_call_output" => vec![RawBlock::ToolResult(PendingToolResult {
            tool_use_id: payload
                .get("call_id")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            content: truncate(
                payload
                    .get("output")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .trim(),
                TOOL_RESULT_MAX,
            ),
            is_error: false,
        })],
        _ => Vec::new(),
    };
    if blocks.is_empty() {
        return None;
    }
    Some(RawReplayAgentMessage {
        id: extract_message_id(value),
        role: role.to_string(),
        timestamp_ms,
        blocks,
    })
}

fn parse_codex_event_msg(
    payload: &Value,
    value: &Value,
    timestamp_ms: i64,
) -> Option<RawReplayAgentMessage> {
    if payload
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        != "agent_message"
    {
        return None;
    }
    let text = payload
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .trim();
    if text.is_empty() {
        return None;
    }
    Some(RawReplayAgentMessage {
        id: extract_message_id(value),
        role: "assistant".to_string(),
        timestamp_ms,
        blocks: vec![RawBlock::Block(ReplayAgentBlock::Text {
            text: truncate(text, TEXT_BLOCK_MAX),
        })],
    })
}

fn extract_message_id(value: &Value) -> String {
    value
        .get("id")
        .and_then(|v| v.as_str())
        .or_else(|| value.get("payload").and_then(|p| p.get("id")).and_then(|v| v.as_str()))
        .or_else(|| {
            value
                .get("payload")
                .and_then(|p| p.get("call_id"))
                .and_then(|v| v.as_str())
        })
        .or_else(|| value.get("timestamp").and_then(|v| v.as_str()))
        .unwrap_or_default()
        .to_string()
}

fn parse_json_string_arg(value: Option<&Value>) -> Value {
    let Some(raw) = value.and_then(|v| v.as_str()) else {
        return Value::Null;
    };
    serde_json::from_str(raw).unwrap_or_else(|_| Value::String(raw.to_string()))
}

fn build_tool_use_block(id: &str, name: &str, input: Value) -> ReplayAgentBlock {
    let summary = summarize_tool_input(&input);
    ReplayAgentBlock::ToolUse {
        id: id.to_string(),
        name: name.to_string(),
        summary,
        input,
        result: None,
    }
}

const TEXT_BLOCK_MAX: usize = 4000;
const TOOL_SUMMARY_MAX: usize = 160;
const TOOL_RESULT_MAX: usize = 4000;

struct PendingToolResult {
    tool_use_id: String,
    content: String,
    is_error: bool,
}

enum RawBlock {
    Block(ReplayAgentBlock),
    ToolResult(PendingToolResult),
}

fn extract_raw_blocks(message: &Value) -> Vec<RawBlock> {
    let mut blocks: Vec<RawBlock> = Vec::new();

    if let Some(text) = message.get("content").and_then(|v| v.as_str()) {
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            blocks.push(RawBlock::Block(ReplayAgentBlock::Text {
                text: truncate(trimmed, TEXT_BLOCK_MAX),
            }));
        }
        return blocks;
    }

    let Some(content) = message.get("content").and_then(|v| v.as_array()) else {
        return blocks;
    };

    for item in content {
        let item_type = item
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        match item_type {
            "text" => {
                let text = item
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .trim();
                if !text.is_empty() {
                    blocks.push(RawBlock::Block(ReplayAgentBlock::Text {
                        text: truncate(text, TEXT_BLOCK_MAX),
                    }));
                }
            }
            "input_text" | "output_text" => {
                let text = item
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .trim();
                if !text.is_empty() {
                    blocks.push(RawBlock::Block(ReplayAgentBlock::Text {
                        text: truncate(text, TEXT_BLOCK_MAX),
                    }));
                }
            }
            "thinking" => {
                let text = item
                    .get("thinking")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .trim();
                if !text.is_empty() {
                    blocks.push(RawBlock::Block(ReplayAgentBlock::Thinking {
                        text: truncate(text, TEXT_BLOCK_MAX),
                    }));
                }
            }
            "tool_use" => {
                let id = item
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let name = item
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("tool")
                    .to_string();
                let input = item.get("input").cloned().unwrap_or(Value::Null);
                let summary = summarize_tool_input(&input);
                blocks.push(RawBlock::Block(ReplayAgentBlock::ToolUse {
                    id,
                    name,
                    summary,
                    input,
                    result: None,
                }));
            }
            "toolCall" => {
                let id = item
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let name = item
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("tool")
                    .to_string();
                let input = item.get("arguments").cloned().unwrap_or(Value::Null);
                let summary = summarize_tool_input(&input);
                blocks.push(RawBlock::Block(ReplayAgentBlock::ToolUse {
                    id,
                    name,
                    summary,
                    input,
                    result: None,
                }));
            }
            "tool_result" => {
                let tool_use_id = item
                    .get("tool_use_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let is_error = item
                    .get("is_error")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let content_text = tool_result_text(item.get("content"));
                blocks.push(RawBlock::ToolResult(PendingToolResult {
                    tool_use_id,
                    content: truncate(&content_text, TOOL_RESULT_MAX),
                    is_error,
                }));
            }
            _ => {}
        }
    }

    blocks
}

fn fold_tool_results(messages: Vec<RawReplayAgentMessage>) -> Vec<RawReplayAgentMessage> {
    let mut pending: Vec<PendingToolResult> = Vec::new();
    let mut out: Vec<RawReplayAgentMessage> = Vec::new();

    for mut msg in messages {
        let mut new_blocks: Vec<RawBlock> = Vec::with_capacity(msg.blocks.len());
        for block in msg.blocks.drain(..) {
            match block {
                RawBlock::ToolResult(result) => pending.push(result),
                other => new_blocks.push(other),
            }
        }
        msg.blocks = new_blocks;
        out.push(msg);
    }

    if pending.is_empty() {
        return out;
    }

    for result in pending {
        for msg in out.iter_mut().rev() {
            let mut attached = false;
            for block in msg.blocks.iter_mut() {
                if let RawBlock::Block(ReplayAgentBlock::ToolUse {
                    id,
                    result: existing,
                    ..
                }) = block
                {
                    if !result.tool_use_id.is_empty() && *id == result.tool_use_id {
                        *existing = Some(ReplayToolResult {
                            content: result.content.clone(),
                            is_error: result.is_error,
                        });
                        attached = true;
                        break;
                    }
                }
            }
            if attached {
                break;
            }
        }
    }

    out
}

fn summarize_tool_input(input: &Value) -> String {
    if let Some(cmd) = input.get("command").and_then(|v| v.as_str()) {
        return first_line_clipped(cmd, TOOL_SUMMARY_MAX);
    }
    if let Some(path) = input.get("file_path").and_then(|v| v.as_str()) {
        return first_line_clipped(path, TOOL_SUMMARY_MAX);
    }
    if let Some(path) = input.get("path").and_then(|v| v.as_str()) {
        return first_line_clipped(path, TOOL_SUMMARY_MAX);
    }
    if let Some(pattern) = input.get("pattern").and_then(|v| v.as_str()) {
        return first_line_clipped(pattern, TOOL_SUMMARY_MAX);
    }
    if let Some(query) = input.get("query").and_then(|v| v.as_str()) {
        return first_line_clipped(query, TOOL_SUMMARY_MAX);
    }
    if let Some(url) = input.get("url").and_then(|v| v.as_str()) {
        return first_line_clipped(url, TOOL_SUMMARY_MAX);
    }
    if let Value::Object(map) = input {
        if let Some((_, v)) = map.iter().next() {
            if let Some(s) = v.as_str() {
                return first_line_clipped(s, TOOL_SUMMARY_MAX);
            }
        }
    }
    String::new()
}

fn tool_result_text(content: Option<&Value>) -> String {
    let Some(value) = content else {
        return String::new();
    };
    if let Some(s) = value.as_str() {
        return s.trim().to_string();
    }
    if let Some(arr) = value.as_array() {
        let mut parts = Vec::new();
        for item in arr {
            if let Some(s) = item.get("text").and_then(|v| v.as_str()) {
                parts.push(s.trim().to_string());
            }
        }
        return parts.join("\n").trim().to_string();
    }
    String::new()
}

fn first_line_clipped(text: &str, max: usize) -> String {
    let first = text.lines().next().unwrap_or("").trim();
    truncate(first, max)
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max).collect();
    out.push_str("...");
    out
}

fn extract_tick_anchor(value: &Value) -> Option<u64> {
    if let Some(stdout) = value
        .get("toolUseResult")
        .and_then(|v| v.get("stdout"))
        .and_then(|v| v.as_str())
        .filter(|v| !v.is_empty())
    {
        if let Some(tick) = extract_tick_from_text(stdout) {
            return Some(tick);
        }
    }

    if let Some(output) = value
        .get("payload")
        .and_then(|v| v.get("output"))
        .and_then(|v| v.as_str())
        .filter(|v| !v.is_empty())
    {
        if let Some(tick) = extract_tick_from_text(output) {
            return Some(tick);
        }
    }

    for node in [value.get("message"), value.get("payload")] {
        let Some(node) = node else {
            continue;
        };
        if let Some(message_text) = node.get("message").and_then(|v| v.as_str()) {
            if let Some(tick) = extract_tick_from_text(message_text) {
                return Some(tick);
            }
        }
        if let Some(content) = node.get("content").and_then(|v| v.as_array()) {
            for item in content {
                let text = item
                    .get("content")
                    .and_then(|v| v.as_str())
                    .or_else(|| item.get("text").and_then(|v| v.as_str()))
                    .unwrap_or_default();
                if let Some(tick) = extract_tick_from_text(text) {
                    return Some(tick);
                }
            }
        }
    }

    None
}

fn extract_tick_from_text(text: &str) -> Option<u64> {
    let key_idx = text.find("\"tick\"")?;
    let after_key = &text[key_idx + "\"tick\"".len()..];
    let colon_idx = after_key.find(':')?;
    let mut saw_digit = false;
    let mut digits = String::new();
    for ch in after_key[colon_idx + 1..].chars() {
        if ch.is_ascii_digit() {
            saw_digit = true;
            digits.push(ch);
            continue;
        }
        if saw_digit {
            break;
        }
    }
    if digits.is_empty() {
        None
    } else {
        digits.parse::<u64>().ok()
    }
}

fn infer_tick_from_anchors(
    ts_ms: i64,
    anchors: &[TickAnchor],
    fallback_ticks_per_ms: f64,
) -> (f64, bool) {
    if anchors.is_empty() {
        return (0.0, true);
    }
    match anchors.binary_search_by_key(&ts_ms, |a| a.ts_ms) {
        Ok(i) => (anchors[i].tick.max(0.0), false),
        Err(0) => {
            let slope = if anchors.len() >= 2 {
                local_slope(&anchors[0], &anchors[1], fallback_ticks_per_ms)
            } else {
                fallback_ticks_per_ms
            };
            let dt = (ts_ms - anchors[0].ts_ms) as f64;
            ((anchors[0].tick + dt * slope).max(0.0), true)
        }
        Err(i) if i >= anchors.len() => {
            let last = anchors.len() - 1;
            let slope = if anchors.len() >= 2 {
                local_slope(&anchors[last - 1], &anchors[last], fallback_ticks_per_ms)
            } else {
                fallback_ticks_per_ms
            };
            let dt = (ts_ms - anchors[last].ts_ms) as f64;
            ((anchors[last].tick + dt * slope).max(0.0), true)
        }
        Err(i) => {
            let a = &anchors[i - 1];
            let b = &anchors[i];
            let dt = (b.ts_ms - a.ts_ms) as f64;
            if dt.abs() < f64::EPSILON {
                return (a.tick.max(0.0), true);
            }
            let t = (ts_ms - a.ts_ms) as f64 / dt;
            let tick = a.tick + (b.tick - a.tick) * t;
            (tick.max(0.0), true)
        }
    }
}

fn local_slope(a: &TickAnchor, b: &TickAnchor, fallback_ticks_per_ms: f64) -> f64 {
    let dt = (b.ts_ms - a.ts_ms) as f64;
    if dt.abs() < f64::EPSILON {
        fallback_ticks_per_ms
    } else {
        (b.tick - a.tick) / dt
    }
}
