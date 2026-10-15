use super::history::History;
use crate::ai::message::{Block, Role};
use crate::ai::provider::{Capabilities, ChatEvent, ChatRequest, DropReason};
use serde_json::{json, Value};

pub(super) fn request(req: &ChatRequest, caps: Capabilities) -> (Value, Vec<ChatEvent>) {
    let mut history = History::new(req, caps);
    let mut messages = vec![json!({"role": "system", "content": req.system})];
    for message in &req.messages {
        let mut text = Vec::new();
        let mut images = Vec::new();
        let mut calls = Vec::new();
        let mut results = Vec::new();
        for block in &message.blocks {
            match block {
                Block::Text { text: value } => text.push(value.clone()),
                Block::Reasoning { .. } => history.notice(message, DropReason::ReasoningOmitted),
                Block::Image { blob_id, .. } => {
                    if let Some(image) = history.image(message, blob_id) { images.push(image.base64.clone()); }
                }
                Block::ToolCall { name, args, .. } if caps.tools => calls.push(json!({"function": {"name": name, "arguments": args}})),
                Block::ToolResult { call_id, content, .. } if caps.tools => results.push(json!({"role": "tool", "tool_name": history.tool_name(call_id), "content": content})),
                Block::ToolCall { .. } | Block::ToolResult { .. } => text.push(history.flattened_tool(message, block)),
                _ => {}
            }
        }
        if !text.is_empty() || !images.is_empty() || !calls.is_empty() {
            let mut item = json!({"role": if message.role == Role::Assistant { "assistant" } else { "user" }, "content": text.join("\n")});
            if !images.is_empty() {
                item["images"] = json!(images);
            }
            if !calls.is_empty() {
                item["tool_calls"] = json!(calls);
            }
            messages.push(item);
        }
        messages.extend(results);
    }
    let mut body = json!({"model": req.model, "messages": messages, "stream": true, "options": {"temperature": 0.0}});
    if let Some(max) = req.max_output_tokens {
        body["options"]["num_predict"] = json!(max);
    }
    if caps.reasoning && req.reasoning.is_some() {
        body["think"] = json!(true);
    }
    if caps.tools && !req.tools.is_empty() {
        body["tools"] = json!(req.tools.iter().map(|tool| json!({"type": "function", "function": {"name": tool.name, "description": tool.description, "parameters": tool.parameters}})).collect::<Vec<_>>());
    }
    (body, history.notices)
}
