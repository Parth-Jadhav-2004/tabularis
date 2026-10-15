use super::history::{call_id, History};
use crate::ai::message::{Block, Role};
use crate::ai::provider::{Capabilities, ChatEvent, ChatRequest, DropReason, ReasoningLevel};
use serde_json::{json, Value};

pub(super) fn request(req: &ChatRequest, caps: Capabilities) -> (Value, Vec<ChatEvent>) {
    let mut history = History::new(req, caps);
    let mut messages: Vec<Value> = Vec::new();
    for message in &req.messages {
        let mut content = Vec::new();
        for block in &message.blocks {
            match block {
                Block::Text { text } => content.push(json!({"type": "text", "text": text})),
                Block::Reasoning { text, signature: Some(signature) } if caps.reasoning && message.provider.as_deref() == Some("anthropic") && message.role == Role::Assistant => {
                    content.push(json!({"type": "thinking", "thinking": text, "signature": signature}));
                }
                Block::Reasoning { .. } => history.notice(message, DropReason::ReasoningOmitted),
                Block::Image { blob_id, .. } => {
                    if let Some(image) = history.image(message, blob_id) {
                        content.push(json!({"type": "image", "source": {"type": "base64", "media_type": image.mime, "data": image.base64}}));
                    }
                }
                Block::ToolCall { id, name, args, .. } if caps.tools => content.push(json!({"type": "tool_use", "id": call_id(id), "name": name, "input": args})),
                Block::ToolResult { call_id: id, content: result, is_error, .. } if caps.tools => content.push(json!({"type": "tool_result", "tool_use_id": call_id(id), "content": result, "is_error": is_error})),
                Block::ToolCall { .. } | Block::ToolResult { .. } => content.push(json!({"type": "text", "text": history.flattened_tool(message, block)})),
                _ => {}
            }
        }
        if content.is_empty() {
            continue;
        }
        let role = if message.role == Role::Assistant {
            "assistant"
        } else {
            "user"
        };
        if let Some(previous) = messages.last_mut().filter(|m| m["role"] == role) {
            previous["content"].as_array_mut().unwrap().extend(content);
        } else {
            messages.push(json!({"role": role, "content": content}));
        }
    }
    let mut body = json!({"model": req.model, "system": req.system, "messages": messages, "stream": true, "max_tokens": req.max_output_tokens.unwrap_or(4096)});
    if caps.reasoning {
        if let Some(level) = req.reasoning {
            let budget = match level {
                ReasoningLevel::Low => 1024,
                ReasoningLevel::Medium => 4096,
                ReasoningLevel::High => 8192,
            };
            body["thinking"] = json!({"type": "enabled", "budget_tokens": budget});
            body["max_tokens"] = json!(req
                .max_output_tokens
                .unwrap_or(budget + 1024)
                .max(budget + 1));
        }
    }
    if caps.tools && !req.tools.is_empty() {
        body["tools"] = json!(req.tools.iter().map(|tool| json!({"name": tool.name, "description": tool.description, "input_schema": tool.parameters})).collect::<Vec<_>>());
    }
    (body, history.notices)
}
