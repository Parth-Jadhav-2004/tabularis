import { describe, expect, it, vi } from "vitest";
import type { AiChatEvent, AiChatTransport, AiConversation, AiConversationSummary, AiResumeOptions } from "../../src/types/aiChat";
import { createAiConversation, createAiMessage } from "../../src/utils/aiChat";
import { createAiChatSession } from "../../src/utils/aiChatSession";

const editorOptions: AiResumeOptions = { choices: [{ provider: "openai", model: "model" }], fallback: { provider: "openai", model: "model" } };
const notebookOptions = (id: string): AiResumeOptions => ({ ...editorOptions, target: { type: "notebook", notebook_id: id } });
function conversation(notebookId?: string, updated = 1, connectionId = "connection"): AiConversation {
  return { ...createAiConversation({ connection_id: connectionId, provider: "openai", model: "model",
    target: notebookId ? { type: "notebook", notebook_id: notebookId } : null }), updated_at: updated,
    title: notebookId ?? "Editor", messages: [createAiMessage("user", [{ type: "text", text: "question" }])] };
}
const summary = (chat: AiConversation): AiConversationSummary => ({ ...chat, message_count: chat.messages.length });
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => { resolve = done; });
  return { promise, resolve };
}
async function setup(chats: AiConversation[] = []) {
  let receive: (event: AiChatEvent) => void = () => {};
  let clear = () => {};
  const history = {
    list: vi.fn<NonNullable<AiChatTransport["history"]>["list"]>(async (id) => chats.filter((chat) => id === null || chat.connection_id === id).map(summary)),
    load: vi.fn<NonNullable<AiChatTransport["history"]>["load"]>(async (connectionId, id) => structuredClone(chats.find((chat) => chat.connection_id === connectionId && chat.id === id)!)),
    rename: vi.fn(async () => {}), delete: vi.fn(async () => {}),
  };
  const transport = { history,
    listen: async (_id, callback) => { receive = callback; return () => {}; },
    listenStorageCleared: async (callback) => { clear = callback; return () => {}; },
    send: vi.fn<AiChatTransport["send"]>(async (_request, id) => id), cancel: vi.fn(async () => {}),
    save: vi.fn<AiChatTransport["save"]>(async () => ({ bytes: 100, large: false })),
  } satisfies AiChatTransport;
  const session = createAiChatSession(transport);
  const unmount = session.mount();
  await Promise.resolve();
  return { session, transport, history, unmount, emit: (event: AiChatEvent) => receive(event), clear: () => clear() };
}

describe("notebook conversation sessions", () => {
  it("reopens the latest conversation of each notebook and keeps legacy editor history separate", async () => {
    const editor = conversation(undefined, 1);
    const first = conversation("one", 2);
    const latest = conversation("one", 3);
    const other = conversation("two", 4);
    const f = await setup([editor, first, latest, other]);
    await f.session.activateConnection("connection", notebookOptions("one"));
    expect(f.session.getSnapshot().conversation?.id).toBe(latest.id);
    await f.session.activateConnection("connection", editorOptions);
    expect(f.session.getSnapshot().conversation?.id).toBe(editor.id);
    await f.session.activateConnection("connection", notebookOptions("two"));
    expect(f.session.getSnapshot().conversation?.id).toBe(other.id);
    f.unmount();
  });

  it("preserves separate drafts even when neither notebook has a conversation", async () => {
    const f = await setup();
    await f.session.activateConnection("connection", notebookOptions("one"));
    f.session.setDraft("first draft");
    await f.session.activateConnection("connection", notebookOptions("two"));
    expect(f.session.getSnapshot().draft).toBe("");
    f.session.setDraft("second draft");
    await f.session.activateConnection("connection", notebookOptions("one"));
    expect(f.session.getSnapshot().draft).toBe("first draft");
    await f.session.activateConnection("connection", notebookOptions("two"));
    expect(f.session.getSnapshot().draft).toBe("second draft");
    f.unmount();
  });

  it("retains a failed notebook save and its transcript across target switches", async () => {
    const first = conversation("one");
    const f = await setup([first, conversation("two")]);
    await f.session.activateConnection("connection", notebookOptions("one"));
    await f.session.send("new question");
    f.transport.save.mockRejectedValueOnce(new Error("locked"));
    f.emit({ type: "done", finish: "stop" });
    await vi.waitFor(() => expect(f.session.getSnapshot().saveState).toBe("error"));
    f.session.setDraft("unsaved draft");
    await f.session.activateConnection("connection", notebookOptions("two"));
    await f.session.activateConnection("connection", notebookOptions("one"));
    expect(f.session.getSnapshot()).toMatchObject({ saveState: "error", draft: "unsaved draft", conversation: { id: first.id } });
    expect(f.session.getSnapshot().conversation?.messages).toHaveLength(3);
    expect(f.history.load).toHaveBeenCalledTimes(2);
    await f.session.retrySave();
    expect(f.transport.save.mock.calls.at(-1)?.[0].target).toEqual({ type: "notebook", notebook_id: "one" });
    f.unmount();
  });

  it("defers switching notebooks during streaming and pending persistence", async () => {
    const first = conversation("one");
    const second = conversation("two");
    const f = await setup([first, second]);
    await f.session.activateConnection("connection", notebookOptions("one"));
    await f.session.send("question");
    await f.session.activateConnection("connection", notebookOptions("two"));
    expect(f.session.getSnapshot().conversation?.id).toBe(first.id);
    const saving = deferred<{ bytes: number; large: boolean }>();
    f.transport.save.mockReturnValueOnce(saving.promise);
    f.emit({ type: "done", finish: "stop" });
    await f.session.activateConnection("connection", notebookOptions("two"));
    expect(f.session.getSnapshot().conversation?.id).toBe(first.id);
    saving.resolve({ bytes: 100, large: false });
    await Promise.resolve();
    await f.session.activateConnection("connection", notebookOptions("two"));
    expect(f.session.getSnapshot().conversation?.id).toBe(second.id);
    f.unmount();
  });

  it.each(["list", "load"] as const)("ignores a late notebook %s after switching within the same connection", async (stage) => {
    const first = conversation("one");
    const second = conversation("two");
    const f = await setup([first, second]);
    const listing = deferred<AiConversationSummary[]>();
    const loading = deferred<AiConversation>();
    if (stage === "list") f.history.list.mockReturnValueOnce(listing.promise);
    else f.history.load.mockReturnValueOnce(loading.promise);
    const pending = f.session.activateConnection("connection", notebookOptions("one"));
    await Promise.resolve();
    await f.session.activateConnection("connection", notebookOptions("two"));
    listing.resolve([summary(first)]);
    loading.resolve(first);
    await pending;
    expect(f.session.getSnapshot().conversation?.id).toBe(second.id);
    await f.session.activateConnection("connection", notebookOptions("one"));
    expect(f.session.getSnapshot().conversation?.id).toBe(first.id);
    f.unmount();
  });

  it("rejects a loaded conversation that no longer matches the notebook index entry", async () => {
    const f = await setup([conversation("one")]);
    f.history.load.mockResolvedValueOnce(conversation("two"));
    await f.session.activateConnection("connection", notebookOptions("one"));
    expect(f.session.getSnapshot()).toMatchObject({ conversation: null, history: { restoring: false, error: "wrong_context" } });
    f.unmount();
  });

  it("keeps manually resumed conversations bound to their original notebook", async () => {
    const first = conversation("one");
    const second = conversation("two");
    const f = await setup([first, second]);
    await f.session.activateConnection("connection", notebookOptions("one"));
    await f.session.resumeConversation(summary(second), notebookOptions("one"));
    await f.session.activateConnection("connection", notebookOptions("one"));
    expect(f.session.getSnapshot().conversation).toMatchObject({ id: second.id, target: second.target });
    await f.session.activateConnection("connection", editorOptions);
    await f.session.activateConnection("connection", notebookOptions("one"));
    expect(f.session.getSnapshot().conversation?.id).toBe(first.id);
    f.unmount();
  });

  it("renames and deletes cached notebook chats without changing the active notebook", async () => {
    const first = conversation("one");
    const second = conversation("two");
    const f = await setup([first, second]);
    await f.session.activateConnection("connection", notebookOptions("one"));
    await f.session.activateConnection("connection", notebookOptions("two"));
    await f.session.renameConversation(summary(first), "Renamed notebook chat");
    expect(f.session.getSnapshot().conversation?.id).toBe(second.id);
    await f.session.activateConnection("connection", notebookOptions("one"));
    expect(f.session.getSnapshot().conversation?.title).toBe("Renamed notebook chat");
    await f.session.deleteConversation(summary(second));
    await f.session.activateConnection("connection", notebookOptions("two"));
    expect(f.history.load).toHaveBeenCalledTimes(3);
    f.unmount();
  });

  it("clears every notebook draft on storage reset", async () => {
    const f = await setup();
    await f.session.activateConnection("connection", notebookOptions("one"));
    f.session.setDraft("private first draft");
    await f.session.activateConnection("connection", notebookOptions("two"));
    f.session.setDraft("private second draft");
    f.clear();
    await f.session.activateConnection("connection", notebookOptions("one"));
    expect(f.session.getSnapshot().draft).toBe("");
    f.unmount();
  });
});
