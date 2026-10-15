import { describe, expect, it, vi } from "vitest";
import { aiEditorTargetKey, createAiEditorStore } from "../../src/utils/aiEditor";
import { aiChatReducer, createAiChatState, createAiConversation, createAiMessage } from "../../src/utils/aiChat";
import type { AiEditorTarget } from "../../src/types/aiEditor";

describe("AI chat entry requests", () => {
  const target: AiEditorTarget = { id: "cell", connectionId: "connection", database: "db", schema: "public",
    actions: { insert: vi.fn(), replace: vi.fn(), run: vi.fn(), focus: vi.fn() } };

  it("captures the editor at invocation and claims only the latest request, once", () => {
    const store = createAiEditorStore();
    let selection = "first selection";
    const source = { ...target, readContext: () => ({ selection }) };
    store.requestEntry("pane", source, "explain");
    const first = store.getEntry()!;
    selection = "second selection";
    expect(first.snapshot.selection).toBe("first selection");
    store.requestEntry("pane", source, "generate");
    const latest = store.getEntry()!;
    expect(store.consumeEntry(first)).toBe(false);
    expect(store.getEntry()).toBe(latest);
    expect(store.consumeEntry(latest)).toBe(true);
    expect(store.consumeEntry(latest)).toBe(false);
    expect(store.getEntry()).toBeNull();
  });

  it("distinguishes equal cell IDs across panes, databases, schemas and notebooks", () => {
    const key = aiEditorTargetKey(target, "left");
    expect(aiEditorTargetKey(target, "right")).not.toBe(key);
    expect(aiEditorTargetKey({ ...target, database: "other" }, "left")).not.toBe(key);
    expect(aiEditorTargetKey({ ...target, schema: "other" }, "left")).not.toBe(key);
    expect(aiEditorTargetKey({ ...target, id: "other-book:cell" }, "left")).not.toBe(key);
    expect(aiEditorTargetKey({ ...target, id: undefined }, "left")).toBeUndefined();
    expect(aiEditorTargetKey(undefined, "left")).toBeUndefined();
  });

  it("binds individual replies in memory and resets bindings on conversation selection", () => {
    const conversation = createAiConversation({ connection_id: "connection", provider: "openai", model: "model" });
    const firstAssistant = createAiMessage("assistant");
    const secondAssistant = createAiMessage("assistant");
    let state = createAiChatState(conversation);
    state = aiChatReducer(state, { type: "start", runId: "first", user: createAiMessage("user"), assistant: firstAssistant, editorTarget: "first-cell" });
    state = aiChatReducer(state, { type: "event", runId: "first", event: { type: "done", finish: "stop" } });
    state = aiChatReducer(state, { type: "start", runId: "second", user: createAiMessage("user"), assistant: secondAssistant, editorTarget: "second-cell" });
    expect(state.replyTargets).toEqual({ [firstAssistant.id]: "first-cell", [secondAssistant.id]: "second-cell" });
    expect(JSON.stringify(state.conversation)).not.toContain("first-cell");
    state = aiChatReducer(state, { type: "restore", state: { ...state, draft: "preserved" } });
    expect(state.replyTargets[firstAssistant.id]).toBe("first-cell");
    state = aiChatReducer(state, { type: "select", conversation: null });
    expect(state.replyTargets).toEqual({});
  });
});
