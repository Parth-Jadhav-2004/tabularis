import { describe, expect, it, vi } from "vitest";
import type { AiChatEvent, AiChatTransport, AiConversation, AiConversationSummary, AiResumeOptions } from "../../src/types/aiChat";
import { createAiConversation, createAiMessage } from "../../src/utils/aiChat";
import { createAiChatSession } from "../../src/utils/aiChatSession";

const options: AiResumeOptions = { choices: [{ provider: "openai", model: "model" }], fallback: { provider: "openai", model: "model" } };
function conversation(connectionId: string, id: string = crypto.randomUUID()): AiConversation {
  return { ...createAiConversation({ connection_id: connectionId, provider: "openai", model: "model" }),
    id, title: `Title ${connectionId}`, messages: [createAiMessage("user", [{ type: "text", text: "question" }])] };
}
function summary(chat: AiConversation): AiConversationSummary {
  return { ...chat, message_count: chat.messages.length };
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => { resolve = done; });
  return { promise, resolve };
}
async function setup(chats: AiConversation[] = []) {
  let clear = () => {};
  let receive: (event: AiChatEvent) => void = () => {};
  const history = {
    list: vi.fn(async (connectionId: string | null) => chats.filter((chat) => !connectionId || chat.connection_id === connectionId).map(summary)),
    load: vi.fn(async (connectionId: string, id: string) => {
      const chat = chats.find((item) => item.connection_id === connectionId && item.id === id);
      if (!chat) throw { kind: "io" };
      return structuredClone(chat);
    }),
    rename: vi.fn(async () => {}),
    delete: vi.fn(async () => {}),
  };
  const transport = {
    history,
    listen: async (_id, callback) => { receive = callback; return () => {}; },
    listenStorageCleared: async (callback) => { clear = callback; return () => {}; },
    send: vi.fn<AiChatTransport["send"]>(async (_request, id) => id),
    cancel: vi.fn<AiChatTransport["cancel"]>(async () => {}),
    save: vi.fn<AiChatTransport["save"]>(async () => ({ bytes: 100, large: false })),
  } satisfies AiChatTransport;
  const session = createAiChatSession(transport);
  const unmount = session.mount();
  await Promise.resolve();
  return { session, transport, history, unmount, clear: () => clear(), emit: (event: AiChatEvent) => receive(event) };
}

describe("conversation history session", () => {
  it("loads the most recent conversation with scope and mode, falling back from an unavailable model", async () => {
    const first = { ...conversation("one"), updated_at: 1 };
    const latest = { ...conversation("one"), updated_at: 2, mode: "agent" as const, model: "removed", database: "db", schema: "public" };
    const fixture = await setup([first, latest]);
    await fixture.session.activateConnection("one", options);
    expect(fixture.history.load).toHaveBeenCalledWith("one", latest.id);
    expect(fixture.session.getSnapshot()).toMatchObject({ conversation: { id: latest.id, database: "db", schema: "public", mode: "agent", model: "model" }, history: { modelFallback: true, restoring: false } });
    fixture.unmount();
  });

  it("remembers selected conversations and drafts per connection, including unsaved transcripts", async () => {
    const fixture = await setup([conversation("one"), conversation("two")]);
    await fixture.session.activateConnection("one", options);
    await fixture.session.send("another question");
    fixture.transport.save.mockRejectedValueOnce(new Error("locked"));
    fixture.emit({ type: "done", finish: "stop" });
    await vi.waitFor(() => expect(fixture.session.getSnapshot().saveState).toBe("error"));
    fixture.session.setDraft("keep this draft");
    await fixture.session.activateConnection("two", options);
    await fixture.session.activateConnection("one", options);
    expect(fixture.session.getSnapshot()).toMatchObject({ draft: "keep this draft", saveState: "error", conversation: { connection_id: "one" } });
    expect(fixture.session.getSnapshot().conversation?.messages).toHaveLength(3);
    expect(fixture.history.load).toHaveBeenCalledTimes(2);
    await fixture.session.retrySave();
    expect(fixture.session.getSnapshot().saveState).toBe("saved");
    fixture.unmount();
  });

  it("defers connection switching during a run and its pending save", async () => {
    const fixture = await setup([conversation("one"), conversation("two")]);
    await fixture.session.activateConnection("one", options);
    await fixture.session.send("question");
    await fixture.session.activateConnection("two", options);
    expect(fixture.session.getSnapshot().conversation?.connection_id).toBe("one");
    const saving = deferred<{ bytes: number; large: boolean }>();
    fixture.transport.save.mockReturnValueOnce(saving.promise);
    fixture.emit({ type: "done", finish: "stop" });
    await fixture.session.activateConnection("two", options);
    expect(fixture.session.getSnapshot().conversation?.connection_id).toBe("one");
    saving.resolve({ bytes: 100, large: false });
    await Promise.resolve();
    await fixture.session.activateConnection("two", options);
    expect(fixture.session.getSnapshot().conversation?.connection_id).toBe("two");
    fixture.unmount();
  });

  it("ignores late connection restoration and late filtered-list results", async () => {
    const one = conversation("one");
    const two = conversation("two");
    const fixture = await setup([one, two]);
    const listing = deferred<AiConversationSummary[]>();
    fixture.history.list.mockReturnValueOnce(listing.promise);
    const first = fixture.session.activateConnection("one", options);
    await fixture.session.activateConnection("two", options);
    listing.resolve([summary(one)]);
    await first;
    expect(fixture.session.getSnapshot().conversation?.connection_id).toBe("two");
    expect(fixture.history.load).toHaveBeenCalledTimes(1);
    await fixture.session.activateConnection("one", options);
    expect(fixture.session.getSnapshot().conversation?.id).toBe(one.id);
    const filtered = deferred<AiConversationSummary[]>();
    fixture.history.list.mockReturnValueOnce(filtered.promise);
    const old = fixture.session.refreshHistory("one");
    await fixture.session.refreshHistory(null);
    filtered.resolve([summary(one)]);
    await old;
    expect(fixture.session.getSnapshot().history).toMatchObject({ scope: null, items: [summary(one), summary(two)], loading: false });
    fixture.unmount();
  });

  it("does not accept a late resume after storage reset or unmount", async () => {
    const chat = conversation("one");
    const fixture = await setup([chat]);
    const loading = deferred<AiConversation>();
    fixture.history.load.mockReturnValueOnce(loading.promise);
    const resumed = fixture.session.resumeConversation(summary(chat), options);
    fixture.clear();
    loading.resolve(chat);
    expect(await resumed).toBe(false);
    expect(fixture.session.getSnapshot()).toMatchObject({ conversation: null, history: { items: [], busy: false, restoring: false } });
    const next = deferred<AiConversation>();
    fixture.history.load.mockReturnValueOnce(next.promise);
    const late = fixture.session.resumeConversation(summary(chat), options);
    fixture.unmount();
    next.resolve(chat);
    expect(await late).toBe(false);
  });

  it("renames the active conversation and retains a manual title on subsequent sends", async () => {
    const chat = conversation("one");
    const fixture = await setup([chat]);
    await fixture.session.activateConnection("one", options);
    expect(await fixture.session.renameConversation(summary(chat), "  Manual title  ")).toBe(true);
    expect(fixture.history.rename).toHaveBeenCalledWith("one", chat.id, "Manual title");
    await fixture.session.send("next");
    fixture.emit({ type: "done", finish: "stop" });
    expect(fixture.transport.save.mock.calls[0][0]).toMatchObject({ title: "Manual title", title_source: "user" });
    fixture.unmount();
  });

  it("deletes only the addressed connection and clears its selected transcript", async () => {
    const one = conversation("one", "same");
    const two = conversation("two", "same");
    const fixture = await setup([one, two]);
    await fixture.session.activateConnection("one", options);
    expect(await fixture.session.deleteConversation(summary(two))).toBe(true);
    expect(fixture.history.delete).toHaveBeenCalledWith("two", "same");
    expect(fixture.session.getSnapshot().conversation?.connection_id).toBe("one");
    expect(await fixture.session.deleteConversation(summary(one))).toBe(true);
    expect(fixture.session.getSnapshot().conversation).toBeNull();
    fixture.unmount();
  });

  it("rejects mutations while streaming, saving or retaining an unsaved transcript", async () => {
    const chat = conversation("one");
    const fixture = await setup([chat]);
    await fixture.session.activateConnection("one", options);
    await fixture.session.send("next");
    expect(await fixture.session.renameConversation(summary(chat), "changed")).toBe(false);
    expect(await fixture.session.deleteConversation(summary(chat))).toBe(false);
    expect(await fixture.session.resumeConversation(summary(chat), options)).toBe(false);
    fixture.transport.save.mockRejectedValueOnce(new Error("locked"));
    fixture.emit({ type: "done", finish: "stop" });
    await vi.waitFor(() => expect(fixture.session.getSnapshot().saveState).toBe("error"));
    expect(await fixture.session.deleteConversation(summary(chat))).toBe(false);
    expect(await fixture.session.renameConversation(summary(chat), "changed")).toBe(false);
    expect(fixture.history.delete).not.toHaveBeenCalled();
    fixture.unmount();
  });

  it("sanitizes list, load and mutation errors while preserving the transcript", async () => {
    const chat = conversation("one");
    const fixture = await setup([chat]);
    fixture.history.list.mockRejectedValueOnce({ kind: "key_unavailable", data: "secret" });
    await fixture.session.activateConnection("one", options);
    expect(fixture.session.getSnapshot().history.error).toBe("key_unavailable");
    fixture.history.load.mockRejectedValueOnce({ kind: "corrupted" });
    expect(await fixture.session.resumeConversation(summary(chat), options)).toBe(false);
    expect(fixture.session.getSnapshot().history.error).toBe("corrupted");
    await fixture.session.resumeConversation(summary(chat), options);
    fixture.history.rename.mockRejectedValueOnce(new Error("secret"));
    expect(await fixture.session.renameConversation(summary(chat), "Manual")).toBe(false);
    expect(fixture.session.getSnapshot()).toMatchObject({ conversation: { id: chat.id, title: chat.title }, history: { error: "io", busy: false } });
    fixture.unmount();
  });

  it("preserves typing during initial history restoration", async () => {
    const chat = conversation("one");
    const fixture = await setup([chat]);
    const loading = deferred<AiConversation>();
    fixture.history.load.mockReturnValueOnce(loading.promise);
    const restoring = fixture.session.activateConnection("one", options);
    await Promise.resolve();
    fixture.session.setDraft("typed while loading");
    loading.resolve(chat);
    await restoring;
    expect(fixture.session.getSnapshot().draft).toBe("typed while loading");
    fixture.unmount();
  });
});
