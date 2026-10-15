use crate::ai::message::{Block, Message};
use crate::ai::provider::{Capabilities, ChatEvent, ChatRequest, DropReason, ImageData};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// Stable provider-safe IDs also link tool results after a provider switch.
pub(super) fn call_id(raw: &str) -> String {
    if raw.len() == 37
        && raw.starts_with("call_")
        && raw[5..].bytes().all(|b| b.is_ascii_hexdigit())
    {
        return raw.to_string();
    }
    let digest = Sha256::digest(raw.as_bytes());
    format!("call_{:x}", digest)[..37].to_string()
}

pub(super) struct History<'a> {
    pub req: &'a ChatRequest,
    pub caps: Capabilities,
    pub notices: Vec<ChatEvent>,
    names: HashMap<String, String>,
}

impl<'a> History<'a> {
    pub fn new(req: &'a ChatRequest, caps: Capabilities) -> Self {
        let names = req
            .messages
            .iter()
            .flat_map(|m| &m.blocks)
            .filter_map(|b| match b {
                Block::ToolCall { id, name, .. } => Some((id.clone(), name.clone())),
                _ => None,
            })
            .collect();
        let mut history = Self {
            req,
            caps,
            notices: Vec::new(),
            names,
        };
        if req.reasoning.is_some() && !caps.reasoning {
            history.notices.push(ChatEvent::Notice {
                message_id: None,
                reason: DropReason::ReasoningUnsupported,
            });
        }
        if !req.tools.is_empty() && !caps.tools {
            history.notices.push(ChatEvent::Notice {
                message_id: None,
                reason: DropReason::ToolsUnsupported,
            });
        }
        history
    }

    pub fn notice(&mut self, message: &Message, reason: DropReason) {
        let notice = ChatEvent::Notice {
            message_id: Some(message.id.clone()),
            reason,
        };
        if !self.notices.contains(&notice) {
            self.notices.push(notice);
        }
    }

    pub fn image(&mut self, message: &Message, blob_id: &str) -> Option<&'a ImageData> {
        if !self.caps.images {
            self.notice(message, DropReason::ImagesUnsupported);
            return None;
        }
        match self.req.resolved_images.get(blob_id) {
            Some(image)
                if matches!(
                    image.mime.as_str(),
                    "image/png" | "image/jpeg" | "image/gif" | "image/webp"
                ) =>
            {
                Some(image)
            }
            _ => {
                self.notice(message, DropReason::ImageUnavailable);
                None
            }
        }
    }

    pub fn tool_name(&self, id: &str) -> String {
        self.names
            .get(id)
            .cloned()
            .unwrap_or_else(|| "tool".to_string())
    }

    pub fn flattened_tool(&mut self, message: &Message, block: &Block) -> String {
        self.notice(message, DropReason::ToolsFlattened);
        match block {
            Block::ToolCall { name, args, .. } => {
                format!("<untrusted_tool_call name={name:?}>\n{args}\n</untrusted_tool_call>")
            }
            Block::ToolResult {
                call_id, content, ..
            } => format!(
                "<untrusted_tool_result name={:?}>\n{content}\n</untrusted_tool_result>",
                self.tool_name(call_id)
            ),
            _ => String::new(),
        }
    }
}
