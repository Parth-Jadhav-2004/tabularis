import { describe, expect, it } from "vitest";
import type { AiChatEvent, AiChatState } from "../../src/types/aiChat";
import { aiChatReducer, createAiChatState, createAiConversation, createAiMessage } from "../../src/utils/aiChat";

function streaming(): AiChatState {
  const conversation = createAiConversation({ connection_id: "connection", provider: "anthropic", model: "model" });
  return aiChatReducer(createAiChatState(conversation), {
    type: "start", runId: "run", user: createAiMessage("user", [{ type: "text", text: "hello" }]),
    assistant: { ...createAiMessage("assistant"), provider: "anthropic", model: "model" },
  });
}

function event(state: AiChatState, value: AiChatEvent) {
  return aiChatReducer(state, { type: "event", runId: "run", event: value });
}

describe("AI chat wire state", () => {
  it("creates connection-scoped Ask conversations with canonical IDs and snake_case fields", () => {
    const conversation = createAiConversation({
      connection_id: "connection", database: "db", schema: "public", provider: "openai", model: "model",
      target: { type: "notebook", notebook_id: "notebook" },
    });
    expect(conversation).toMatchObject({
      schema_version: 1, connection_id: "connection", database: "db", schema: "public", mode: "ask",
      title_source: "first_message", target: { type: "notebook", notebook_id: "notebook" }, messages: [],
    });
    expect(conversation.id).toMatch(/^[\da-f]{8}-[\da-f]{4}-4[\da-f]{3}-[89ab][\da-f]{3}-[\da-f]{12}$/);
    expect(conversation.created_at).toBe(conversation.updated_at);
    expect(JSON.parse(JSON.stringify(conversation))).toEqual(conversation);
    expect(createAiConversation({ connection_id: "other", provider: "ollama", model: "local" }))
      .toMatchObject({ database: null, schema: null, target: { type: "editor" } });
  });

  it("starts with a clean idle state and resets all transient state on selection", () => {
    expect(createAiChatState()).toMatchObject({ conversation: null, ready: false, runState: "idle", draft: "" });
    const state = { ...streaming(), ready: true, draft: "old draft" };
    const selected = createAiConversation({ connection_id: "other", provider: "ollama", model: "local" });
    expect(aiChatReducer(state, { type: "select", conversation: selected }))
      .toEqual({ ...createAiChatState(selected), ready: true });
  });

  it("does not start without a conversation or start a second run", () => {
    const action = { type: "start" as const, runId: "other", user: createAiMessage("user"), assistant: createAiMessage("assistant") };
    const empty = createAiChatState();
    expect(aiChatReducer(empty, action)).toBe(empty);
    const busy = streaming();
    expect(aiChatReducer(busy, action)).toBe(busy);
  });

  it("derives a bounded Unicode title and preserves an existing user title", () => {
    const conversation = createAiConversation({ connection_id: "connection", provider: "openai", model: "model" });
    const start = (title: string, text: string) => aiChatReducer(createAiChatState({ ...conversation, title }), {
      type: "start", runId: "run", user: createAiMessage("user", [{ type: "text", text }]),
      assistant: createAiMessage("assistant"),
    });
    expect(start("", "  hello\n world  ").conversation?.title).toBe("hello world");
    expect(start("", "🦊".repeat(200)).conversation?.title).toBe("🦊".repeat(50));
    expect(start("Custom title", "question").conversation?.title).toBe("Custom title");
  });

  it("coalesces adjacent deltas and retains text/reasoning order", () => {
    let state = streaming();
    for (const value of [
      { type: "reasoning_delta", text: "Think " }, { type: "reasoning_delta", text: "first" },
      { type: "text_delta", text: "SELECT " }, { type: "text_delta", text: "1" },
      { type: "reasoning_delta", text: "Again" }, { type: "text_delta", text: "" },
    ] satisfies AiChatEvent[]) state = event(state, value);
    expect(state.conversation?.messages[1].blocks).toEqual([
      { type: "reasoning", text: "Think first", signature: null }, { type: "text", text: "SELECT 1" },
      { type: "reasoning", text: "Again", signature: null },
    ]);
  });

  it("accumulates signature fragments and starts a new reasoning block after a signed block", () => {
    let state = event(streaming(), { type: "reasoning_signature", signature: "orphan" });
    expect(state.conversation?.messages[1].blocks).toEqual([]);
    for (const value of [
      { type: "reasoning_delta", text: "Think" }, { type: "reasoning_signature", signature: "sig-" },
      { type: "reasoning_signature", signature: "end" }, { type: "reasoning_delta", text: "Next" },
    ] satisfies AiChatEvent[]) state = event(state, value);
    expect(state.conversation?.messages[1].blocks).toEqual([
      { type: "reasoning", text: "Think", signature: "sig-end" },
      { type: "reasoning", text: "Next", signature: null },
    ]);
  });

  it("retains usage and scoped conversion notices without inserting them into the provider history", () => {
    const usage = { input_tokens: 10, output_tokens: 20, cache_read_tokens: 3, cache_write_tokens: 4 };
    let state = event(streaming(), { type: "usage", usage });
    state = event(state, { type: "notice", message_id: "old-image", reason: "images_unsupported" });
    expect(state.conversation?.messages[1].usage).toEqual(usage);
    expect(state.notices).toEqual([{
      assistantId: state.activeRun?.assistantId, message_id: "old-image", reason: "images_unsupported",
    }]);
    expect(state.conversation?.messages[1].blocks).toEqual([]);
  });

  it.each(["stop", "length", "cancelled", "tool_use", "content_filter"] as const)("finishes %s and ignores late events", (finish) => {
    const state = event(streaming(), { type: "done", finish });
    expect(state).toMatchObject({ runState: "idle", activeRun: null, finish });
    expect(event(state, { type: "text_delta", text: "late" })).toBe(state);
    expect(event(state, { type: "done", finish: "error" })).toBe(state);
  });

  it("keeps provider errors and partial output when the terminal error arrives", () => {
    const error = { kind: "network" as const, message: "Sanitized backend error" };
    let state = event(streaming(), { type: "text_delta", text: "partial" });
    state = event(state, { type: "error", error });
    expect(state.activeRun).not.toBeNull();
    state = event(state, { type: "done", finish: "error" });
    expect(state).toMatchObject({ runState: "error", error: { kind: "provider", error }, finish: "error" });
    expect(state.conversation?.messages[1].blocks).toEqual([
      { type: "text", text: "partial" }, { type: "error", message: error.message },
    ]);
    expect(event(streaming(), { type: "done", finish: "error" }).error).toEqual({ kind: "stream" });
  });

  it("ignores tool events in M4 and rejects events/failures for another run", () => {
    const state = streaming();
    for (const value of [
      { type: "tool_call_start", id: "call", name: "run_statement" },
      { type: "tool_call_args_delta", id: "call", delta: "{}" },
      { type: "tool_call_end", id: "call", args: { query: "DELETE FROM t" } },
    ] satisfies AiChatEvent[]) expect(event(state, value)).toBe(state);
    expect(aiChatReducer(state, { type: "event", runId: "other", event: { type: "text_delta", text: "late" } })).toBe(state);
    expect(aiChatReducer(state, { type: "failure", runId: "other", error: { kind: "send" } })).toBe(state);
  });

  it("retains the producing model on messages when the next model changes", () => {
    const state = aiChatReducer(streaming(), { type: "model", choice: { provider: "ollama", model: "local" } });
    expect(state.conversation).toMatchObject({ provider: "ollama", model: "local" });
    expect(state.conversation?.messages[1]).toMatchObject({ provider: "anthropic", model: "model" });
  });

  it("tracks local failures, drafts and save retries independently", () => {
    let state = aiChatReducer(streaming(), { type: "draft", draft: "next" });
    state = aiChatReducer(state, { type: "failure", runId: "run", error: { kind: "cancel" }, keepRun: true });
    expect(state).toMatchObject({ runState: "streaming", draft: "next", error: { kind: "cancel" } });
    expect(state.activeRun).not.toBeNull();
    state = aiChatReducer(state, { type: "failure", runId: "run", error: { kind: "send" } });
    expect(state).toMatchObject({ runState: "error", activeRun: null });
    state = aiChatReducer(state, { type: "saving" });
    expect(state.saveState).toBe("saving");
    state = aiChatReducer(state, { type: "save_failed" });
    expect(state.saveState).toBe("error");
    state = aiChatReducer(state, { type: "saved", result: { bytes: 123, large: true } });
    expect(state).toMatchObject({ saveState: "saved", saveResult: { bytes: 123, large: true } });
  });
});
