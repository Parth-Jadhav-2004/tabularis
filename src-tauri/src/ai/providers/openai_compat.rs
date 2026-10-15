use super::history::{call_id, History};
use crate::ai::message::{Block, Role};
use crate::ai::provider::{Capabilities, ChatEvent, ChatRequest, DropReason};
use serde_json::{json, Value};

pub(super) fn request(req: &ChatRequest, caps: Capabilities) -> (Value, Vec<ChatEvent>) {
    let mut history = History::new(req, caps);
    let mut messages = vec![json!({"role": "system", "content": req.system})];
    for message in &req.messages {
        let mut content = Vec::new();
        let mut calls = Vec::new();
        let mut results = Vec::new();
        for block in &message.blocks {
            match block {
                Block::Text { text } => content.push(json!({"type": "text", "text": text})),
                Block::Image { blob_id, .. } => {
                    if let Some(image) = history.image(message, blob_id) {
                        content.push(json!({"type": "image_url", "image_url": {"url": format!("data:{};base64,{}", image.mime, image.base64)}}));
                    }
                }
                Block::ToolCall { id, name, args, .. } if caps.tools => calls.push(json!({"id": call_id(id), "type": "function", "function": {"name": name, "arguments": args.to_string()}})),
                Block::ToolResult { call_id: id, content, .. } if caps.tools => results.push(json!({"role": "tool", "tool_call_id": call_id(id), "content": content})),
                Block::ToolCall { .. } | Block::ToolResult { .. } => {
                    content.push(json!({"type": "text", "text": history.flattened_tool(message, block)}));
                }
                Block::Reasoning { .. } => history.notice(message, DropReason::ReasoningOmitted),
                _ => {}
            }
        }
        if !content.is_empty() || !calls.is_empty() {
            let role = if message.role == Role::Assistant {
                "assistant"
            } else {
                "user"
            };
            // Plain text preserves compatibility with older compatible servers.
            let text_only = content.iter().all(|part| part["type"] == "text");
            let content = if text_only {
                Value::String(
                    content
                        .iter()
                        .filter_map(|p| p["text"].as_str())
                        .collect::<Vec<_>>()
                        .join("\n"),
                )
            } else {
                json!(content)
            };
            let mut item = json!({"role": role, "content": content});
            if !calls.is_empty() {
                item["tool_calls"] = json!(calls);
            }
            messages.push(item);
        }
        messages.extend(results);
    }
    let mut body = json!({"model": req.model, "messages": messages, "stream": true, "stream_options": {"include_usage": true}});
    if let Some(max) = req.max_output_tokens {
        let field = if req.provider == "openai" {
            "max_completion_tokens"
        } else {
            "max_tokens"
        };
        body[field] = json!(max);
    }
    if caps.reasoning {
        if let Some(level) = req.reasoning {
            body["reasoning_effort"] = json!(level);
        }
    }
    // Reasoning models may reject temperature; legacy non-reasoning calls retain it.
    if !caps.reasoning {
        body["temperature"] = json!(if req.provider == "minimax" { 0.1 } else { 0.0 });
    }
    if caps.tools && !req.tools.is_empty() {
        body["tools"] = json!(req.tools.iter().map(|tool| json!({"type": "function", "function": {"name": tool.name, "description": tool.description, "parameters": tool.parameters}})).collect::<Vec<_>>());
    }
    (body, history.notices)
}
