# M1. Provider layer: messages, streaming, tool calling

**Depends on:** none · **Size:** L

## Goal

Turn the single-shot, blocking `(system, user) -> String` AI layer into a multi-turn, streaming, tool-capable layer, without changing the behaviour of today's AI features.

## Current state

- `src-tauri/src/ai.rs` (~1150 lines): `dispatch_provider()` routes by provider string to per-provider `generate_*` functions. Each sends one system prompt and one user message, no streaming (Ollama sets `"stream": false`), no tools. `clean_response()` strips code fences.
- Providers: `openai`, `anthropic`, `openrouter`, `ollama`, `custom-openai`, `minimax` (region failover).
- Keys: `config::get_ai_api_key()` (credential cache, keychain via `keychain_utils::get_ai_key`, env var fallback).
- HTTP clients per provider via `ai_client_for`, honouring `ai_provider_proxies`.
- Models: `ai_models.yaml` + live fetch, cached 24h (`AiModelsCache`).

## Design

### Module layout

```
src-tauri/src/ai/
  mod.rs            # public API, existing commands re-exported
  message.rs        # Message, Block, Role, Usage
  provider.rs       # ChatProvider trait, ChatRequest, ChatEvent, Capabilities
  sse.rs            # SSE / NDJSON line parser
  providers/
    openai_compat.rs   # openai, openrouter, custom-openai, minimax
    anthropic.rs
    ollama.rs
  legacy.rs         # generate_ai_query, explain_ai_query, ... built on top of the new layer
  tests.rs
```

### Types

```rust
pub enum Role { User, Assistant, Tool }

pub struct Message {
    pub id: String,
    pub role: Role,
    pub blocks: Vec<Block>,
    pub created_at: i64,
    pub usage: Option<Usage>,
    pub provider: Option<String>,
    pub model: Option<String>,
}

#[serde(tag = "type", rename_all = "snake_case")]
pub enum Block {
    Text { text: String },
    Reasoning { text: String, signature: Option<String> },   // signature: opaque, echoed back (Anthropic)
    ToolCall { id: String, name: String, args: serde_json::Value, status: ToolStatus },
    ToolResult { call_id: String, content: String, truncated: bool, is_error: bool },
    Image { blob_id: String, mime: String, width: u32, height: u32 },
    Attachment { kind: AttachmentKind, name: String, blob_id: Option<String>, size: u64 },
    Context { items: Vec<ContextItemRef> },  // what the context builder sent (M6)
    Error { message: String },
}

pub struct ChatRequest {
    pub provider: String,
    pub model: String,
    pub system: String,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolDef>,           // empty = no tools
    pub max_output_tokens: Option<u32>,
    pub reasoning: Option<ReasoningLevel>,
}

pub enum ChatEvent {
    TextDelta(String),
    ReasoningDelta(String),
    ToolCallStart { id: String, name: String },
    ToolCallArgsDelta { id: String, delta: String },
    ToolCallEnd { id: String, args: serde_json::Value },
    Usage(Usage),
    Done { finish: FinishReason },        // stop | length | tool_use | cancelled
    Error(ProviderError),                 // typed: auth | rate_limit | context_length | network | other
}

pub struct Capabilities { tools: bool, images: bool, reasoning: bool, context_window: Option<u32> }

#[async_trait]
pub trait ChatProvider {
    async fn stream(&self, req: ChatRequest, cancel: CancellationToken)
        -> Result<BoxStream<'static, ChatEvent>, ProviderError>;
    fn capabilities(&self, model: &str) -> Capabilities;
}
```

Version the serialized shape (`schema_version` on the conversation, M2) so stored conversations survive later changes. Keep `Block` deserialization lenient: unknown block types are skipped and logged, not fatal.

### Wire mapping notes

- **OpenAI-compatible:** `messages[]` with `tool_calls` / `role: "tool"`; images as `image_url` data URLs; streaming `choices[].delta` with tool-call argument fragments keyed by index.
- **Anthropic Messages:** `system` separate; content blocks `text`, `image`, `tool_use`, `tool_result`, `thinking` (signature must be echoed back on later turns); SSE events `content_block_start/delta/stop`, `message_delta` (usage, stop_reason).
- **Ollama:** `/api/chat` with `stream: true` (NDJSON), `tools` for models that support it. Detect support from `/api/show` capabilities and cache it; without tools, the caller falls back to prompt-only context.
- **MiniMax:** keep the region failover from today's code.

### Runs and Tauri events

```
src-tauri/src/ai_chat/run.rs
```

- `RunManager` in app state: `HashMap<RunId, CancellationToken>`.
- Command `ai_chat_send(req) -> RunId` spawns a task; in M1 it only does provider streaming (the tool loop comes in M9).
- Emit on `ai-chat://<runId>`. Coalesce `TextDelta` / `ReasoningDelta` and flush every ~50 ms, or immediately on any non-delta event.
- Command `ai_chat_cancel(runId)`: trigger the token, emit `Done { finish: cancelled }`.
- The frontend never receives API keys; everything happens in Rust.

### Switching provider mid-conversation

The chat lets the user change provider/model at any turn (M4), so a history produced by one provider must be sendable to another:

- The stored `Message`/`Block` model is provider-neutral; adapters only translate at send time.
- Tool call ids are normalized on storage and re-mapped per provider (some providers have id format constraints).
- `Reasoning.signature` is only echoed back to the provider that produced it (check `Message.provider`); otherwise reasoning blocks are dropped from the request.
- `Image` blocks are dropped from the request when the target model has no image support; tool blocks are flattened to text (`[tool list_tables -> ...]`) when the target model has no tool support.
- The adapter reports what it dropped, so the UI can show a notice.

### Capabilities and models

Extend `ai_models.yaml` with optional `capabilities` per model. Live-fetched models without data use provider defaults (for example, Anthropic: tools + images; Ollama: probe).

## Steps

- [x] Run `impact` on `dispatch_provider`, `generate_ai_query`, `explain_ai_query`, `generate_cell_name`, `generate_tab_rename`, `suggest_table_name`, `analyze_ai_explain_plan`.
- [x] Split `ai.rs` into `ai/` without behaviour changes (pure move, green tests).
- [ ] Add `message.rs`, `provider.rs`, the SSE/NDJSON parser.
- [ ] Implement `stream()` for openai_compat, anthropic, ollama (text only).
- [ ] Re-implement the legacy commands on top of `stream()` (collect into a string), keep `clean_response()`.
- [ ] Add `RunManager`, `ai_chat_send`, `ai_chat_cancel`, events.
- [ ] Add tool definitions and tool-call parsing for each adapter.
- [ ] Add image blocks to each adapter (used by M10).
- [ ] Capabilities: yaml + Ollama probe + cache.
- [ ] Typed provider errors with readable messages (auth, rate limit, context length).

## Tests

- SSE parser: chunk boundaries in the middle of lines, `[DONE]`, comments, malformed JSON.
- Per-adapter request serialization snapshots (text, multi-turn, tools, images).
- Tool-call argument assembly from fragments (OpenAI index-based, Anthropic `input_json_delta`).
- Cross-provider history: a conversation with tool calls, reasoning and an image, recorded with Anthropic, serializes correctly for OpenAI-compatible and Ollama (and vice versa), with the expected drops reported.
- Cancellation stops the stream and emits `Done { cancelled }`.
- Legacy commands return the same output as before for a mocked response.

## Done when

- All current AI features work unchanged.
- A dev-only call to `ai_chat_send` streams text from OpenAI-compatible, Anthropic and Ollama, and can be cancelled.
- A tool-capable model returns a parsed tool call through the event stream.
