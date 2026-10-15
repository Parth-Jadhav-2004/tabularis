use super::history::call_id;
use crate::ai::message::Usage;
use crate::ai::provider::{ChatEvent, FinishReason, ProviderError};
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Protocol {
    OpenAi,
    Anthropic,
    Ollama,
}

struct Tool {
    id: String,
    name: String,
    args: String,
    initial: Value,
}

pub(super) struct Decoder {
    pub protocol: Protocol,
    tools: BTreeMap<usize, Tool>,
    usage: Usage,
    finish: Option<FinishReason>,
    pub done: bool,
    ollama_calls: usize,
    ollama_id: String,
}

impl Decoder {
    pub fn new(protocol: Protocol) -> Self {
        Self {
            protocol,
            tools: BTreeMap::new(),
            usage: Usage::default(),
            finish: None,
            done: false,
            ollama_calls: 0,
            ollama_id: uuid::Uuid::new_v4().to_string(),
        }
    }

    pub fn frame(&mut self, frame: &str) -> Result<Vec<ChatEvent>, ProviderError> {
        if self.done {
            return Ok(Vec::new());
        }
        if frame.trim() == "[DONE]" && self.protocol == Protocol::OpenAi {
            return self.complete();
        }
        let value: Value = serde_json::from_str(frame).map_err(|_| ProviderError::protocol())?;
        if !value["error"].is_null() {
            return Err(ProviderError::from_status(0, &value["error"].to_string()));
        }
        match self.protocol {
            Protocol::OpenAi => self.openai(&value),
            Protocol::Anthropic => self.anthropic(&value),
            Protocol::Ollama => self.ollama(&value),
        }
    }

    pub fn eof(&mut self) -> Result<Vec<ChatEvent>, ProviderError> {
        if self.done {
            Ok(Vec::new())
        } else if self.protocol == Protocol::OpenAi && self.finish.is_some() {
            self.complete()
        } else {
            Err(ProviderError::protocol())
        }
    }

    fn complete(&mut self) -> Result<Vec<ChatEvent>, ProviderError> {
        let finish = self.finish.ok_or_else(ProviderError::protocol)?;
        if !self.tools.is_empty() {
            return Err(ProviderError::protocol());
        }
        self.done = true;
        Ok(vec![ChatEvent::Done { finish }])
    }

    fn end_tool(&mut self, index: usize) -> Result<ChatEvent, ProviderError> {
        let tool = self
            .tools
            .remove(&index)
            .ok_or_else(ProviderError::protocol)?;
        let args = if tool.args.is_empty() {
            tool.initial
        } else {
            serde_json::from_str(&tool.args).map_err(|_| ProviderError::protocol())?
        };
        if !args.is_object() {
            return Err(ProviderError::protocol());
        }
        Ok(ChatEvent::ToolCallEnd { id: tool.id, args })
    }

    fn openai(&mut self, value: &Value) -> Result<Vec<ChatEvent>, ProviderError> {
        let mut events = Vec::new();
        if let Some(usage) = value.get("usage").filter(|v| v.is_object()) {
            self.usage.input_tokens = usage["prompt_tokens"].as_u64().unwrap_or(0);
            self.usage.output_tokens = usage["completion_tokens"].as_u64().unwrap_or(0);
            self.usage.cache_read_tokens = usage["prompt_tokens_details"]["cached_tokens"]
                .as_u64()
                .unwrap_or(0);
            events.push(ChatEvent::Usage {
                usage: self.usage.clone(),
            });
        }
        if let Some(choices) = value["choices"].as_array() {
            for choice in choices
                .iter()
                .filter(|c| c["index"].as_u64().unwrap_or(0) == 0)
            {
                let delta = &choice["delta"];
                text_event(&mut events, &delta["content"], false);
                text_event(&mut events, &delta["reasoning_content"], true);
                if let Some(calls) = delta["tool_calls"].as_array() {
                    for call in calls {
                        let index =
                            call["index"].as_u64().ok_or_else(ProviderError::protocol)? as usize;
                        if !self.tools.contains_key(&index) {
                            let id = call["id"].as_str().ok_or_else(ProviderError::protocol)?;
                            let name = call["function"]["name"]
                                .as_str()
                                .ok_or_else(ProviderError::protocol)?;
                            let tool = Tool {
                                id: call_id(id),
                                name: name.to_string(),
                                args: String::new(),
                                initial: json!({}),
                            };
                            events.push(ChatEvent::ToolCallStart {
                                id: tool.id.clone(),
                                name: tool.name.clone(),
                            });
                            self.tools.insert(index, tool);
                        }
                        if let Some(fragment) = call["function"]["arguments"].as_str() {
                            let tool = self.tools.get_mut(&index).unwrap();
                            tool.args.push_str(fragment);
                            if !fragment.is_empty() {
                                events.push(ChatEvent::ToolCallArgsDelta {
                                    id: tool.id.clone(),
                                    delta: fragment.to_string(),
                                });
                            }
                        }
                    }
                }
                if let Some(reason) = choice["finish_reason"].as_str() {
                    self.finish = Some(finish_reason(reason));
                    let indices: Vec<_> = self.tools.keys().copied().collect();
                    for index in indices {
                        events.push(self.end_tool(index)?);
                    }
                }
            }
        }
        Ok(events)
    }

    fn anthropic(&mut self, value: &Value) -> Result<Vec<ChatEvent>, ProviderError> {
        let mut events = Vec::new();
        let index = value["index"].as_u64().unwrap_or(0) as usize;
        match value["type"].as_str() {
            Some("message_start") => {
                let usage = &value["message"]["usage"];
                self.usage.input_tokens = usage["input_tokens"].as_u64().unwrap_or(0);
                self.usage.output_tokens = usage["output_tokens"].as_u64().unwrap_or(0);
                self.usage.cache_read_tokens =
                    usage["cache_read_input_tokens"].as_u64().unwrap_or(0);
                self.usage.cache_write_tokens =
                    usage["cache_creation_input_tokens"].as_u64().unwrap_or(0);
                events.push(ChatEvent::Usage {
                    usage: self.usage.clone(),
                });
            }
            Some("content_block_start") => {
                let block = &value["content_block"];
                match block["type"].as_str() {
                    Some("tool_use") => {
                        let id = block["id"].as_str().ok_or_else(ProviderError::protocol)?;
                        let name = block["name"].as_str().ok_or_else(ProviderError::protocol)?;
                        let tool = Tool {
                            id: call_id(id),
                            name: name.to_string(),
                            args: String::new(),
                            initial: block["input"].clone(),
                        };
                        events.push(ChatEvent::ToolCallStart {
                            id: tool.id.clone(),
                            name: tool.name.clone(),
                        });
                        self.tools.insert(index, tool);
                    }
                    Some("text") => text_event(&mut events, &block["text"], false),
                    Some("thinking") => text_event(&mut events, &block["thinking"], true),
                    _ => {}
                }
            }
            Some("content_block_delta") => {
                let delta = &value["delta"];
                match delta["type"].as_str() {
                    Some("text_delta") => text_event(&mut events, &delta["text"], false),
                    Some("thinking_delta") => text_event(&mut events, &delta["thinking"], true),
                    Some("signature_delta") => {
                        let signature = delta["signature"]
                            .as_str()
                            .ok_or_else(ProviderError::protocol)?;
                        events.push(ChatEvent::ReasoningSignature {
                            signature: signature.to_string(),
                        });
                    }
                    Some("input_json_delta") => {
                        let tool = self
                            .tools
                            .get_mut(&index)
                            .ok_or_else(ProviderError::protocol)?;
                        let fragment = delta["partial_json"]
                            .as_str()
                            .ok_or_else(ProviderError::protocol)?;
                        tool.args.push_str(fragment);
                        events.push(ChatEvent::ToolCallArgsDelta {
                            id: tool.id.clone(),
                            delta: fragment.to_string(),
                        });
                    }
                    _ => {}
                }
            }
            Some("content_block_stop") if self.tools.contains_key(&index) => {
                events.push(self.end_tool(index)?)
            }
            Some("message_delta") => {
                if let Some(reason) = value["delta"]["stop_reason"].as_str() {
                    self.finish = Some(finish_reason(reason));
                }
                if let Some(output) = value["usage"]["output_tokens"].as_u64() {
                    self.usage.output_tokens = output;
                }
                events.push(ChatEvent::Usage {
                    usage: self.usage.clone(),
                });
            }
            Some("message_stop") => events.extend(self.complete()?),
            _ => {}
        }
        Ok(events)
    }

    fn ollama(&mut self, value: &Value) -> Result<Vec<ChatEvent>, ProviderError> {
        let mut events = Vec::new();
        let message = &value["message"];
        text_event(&mut events, &message["content"], false);
        text_event(&mut events, &message["thinking"], true);
        if let Some(calls) = message["tool_calls"].as_array() {
            for call in calls {
                let name = call["function"]["name"]
                    .as_str()
                    .ok_or_else(ProviderError::protocol)?;
                let args = &call["function"]["arguments"];
                let args: Value = if let Some(text) = args.as_str() {
                    serde_json::from_str(text).map_err(|_| ProviderError::protocol())?
                } else {
                    args.clone()
                };
                if !args.is_object() {
                    return Err(ProviderError::protocol());
                }
                let id = call_id(&format!("{}:{}", self.ollama_id, self.ollama_calls));
                self.ollama_calls += 1;
                events.push(ChatEvent::ToolCallStart {
                    id: id.clone(),
                    name: name.to_string(),
                });
                events.push(ChatEvent::ToolCallEnd { id, args });
            }
        }
        if value["done"] == true {
            self.usage.input_tokens = value["prompt_eval_count"].as_u64().unwrap_or(0);
            self.usage.output_tokens = value["eval_count"].as_u64().unwrap_or(0);
            events.push(ChatEvent::Usage {
                usage: self.usage.clone(),
            });
            self.finish = Some(if self.ollama_calls > 0 {
                FinishReason::ToolUse
            } else {
                finish_reason(value["done_reason"].as_str().unwrap_or("stop"))
            });
            events.extend(self.complete()?);
        }
        Ok(events)
    }
}

fn text_event(events: &mut Vec<ChatEvent>, value: &Value, reasoning: bool) {
    if let Some(text) = value.as_str().filter(|text| !text.is_empty()) {
        events.push(if reasoning {
            ChatEvent::ReasoningDelta {
                text: text.to_string(),
            }
        } else {
            ChatEvent::TextDelta {
                text: text.to_string(),
            }
        });
    }
}

fn finish_reason(reason: &str) -> FinishReason {
    match reason {
        "length" | "max_tokens" => FinishReason::Length,
        "tool_calls" | "tool_use" | "function_call" => FinishReason::ToolUse,
        "content_filter" | "refusal" => FinishReason::ContentFilter,
        _ => FinishReason::Stop,
    }
}
