import { describe, expect, it, vi } from "vitest";
import { createAiChatSession } from "../../src/utils/aiChatSession";
import { createAiContextOptions, defaultAiContextSelection } from "../../src/utils/aiContext";
import type { AiChatEvent, AiChatTransport } from "../../src/types/aiChat";

async function fixture() {
  const listeners = new Map<string, (event: AiChatEvent) => void>();
  const transport: AiChatTransport = {
    listenStorageCleared: async () => () => {},
    listen: async (id, callback) => { listeners.set(id, callback); return () => {}; },
    send: vi.fn<AiChatTransport["send"]>(async (_request, id) => id),
    cancel: vi.fn(async () => {}),
    save: vi.fn<AiChatTransport["save"]>(async () => ({ bytes: 123, large: false })),
  };
  const session = createAiChatSession(transport);
  const unmount = session.mount();
  await Promise.resolve();
  session.newConversation({ connection_id: "connection", database: "app", schema: "public", provider: "openai", model: "model" });
  return { session, transport, unmount, listeners };
}

describe("AI resolved context lifecycle", () => {
  it("derives the action from the message instead of trusting an option and resets it next turn", async () => {
    const { session, transport, listeners, unmount } = await fixture();
    const context = { ...createAiContextOptions({ query: "SELECT 1" }, defaultAiContextSelection(), "it"), command: "fix" as const };
    await session.send("/optimize", { context });
    expect(vi.mocked(transport.send).mock.calls[0][2]?.command).toBe("optimize");
    expect(vi.mocked(transport.send).mock.calls[0][0].messages[0].blocks).toEqual([{ type: "text", text: "/optimize" }]);
    listeners.get(vi.mocked(transport.send).mock.calls[0][1])!({ type: "done", finish: "stop" });
    await Promise.resolve();
    await session.send("follow-up", { context });
    expect(vi.mocked(transport.send).mock.calls[1][2]?.command).toBeUndefined();
    unmount();
  });
  it("rejects local, unknown and unresolved commands without starting a run", async () => {
    const { session, transport, unmount } = await fixture();
    for (const text of ["/help", "/unknown", "/fix"]) expect(await session.send(text)).toBe(false);
    expect(session.getSnapshot().activeRun).toBeNull();
    expect(transport.send).not.toHaveBeenCalled();
    expect(session.getSnapshot().conversation?.messages).toEqual([]);
    unmount();
  });
  it("binds context to the conversation scope and resets next-message choices after send", async () => {
    const { session, transport, unmount } = await fixture();
    const include = { ...defaultAiContextSelection(), query: false };
    session.setContextInclude(include);
    session.setDraft("question");
    expect(session.getSnapshot().contextInclude.query).toBe(false);
    const options = { ...createAiContextOptions({ query: "not included" }, include, "it"), connection_id: "wrong", database: "wrong", schema: "wrong" };
    expect(await session.send("question", { context: options })).toBe(true);
    const sent = vi.mocked(transport.send).mock.calls[0][2];
    expect(sent).toMatchObject({ connection_id: "connection", database: "app", schema: "public", include: { query: false }, editor: { query: null } });
    expect(session.getSnapshot().contextInclude.query).toBe(true);
    unmount();
  });
  it("stores backend summaries exactly once on the producing user message before encrypted save", async () => {
    const { session, transport, unmount, listeners } = await fixture();
    await session.send("question", { context: createAiContextOptions({}, defaultAiContextSelection(), "en") });
    const [request, id] = vi.mocked(transport.send).mock.calls[0];
    const userId = request.messages.at(-1)!.id;
    const items = [{ kind: "table", id: "users", label: "users", chars: 123, truncated: false }];
    const receive = listeners.get(id)!;
    receive({ type: "context", message_id: "another-message", items });
    expect(session.getSnapshot().conversation?.messages[0].blocks).toHaveLength(1);
    receive({ type: "context", message_id: userId, items });
    receive({ type: "context", message_id: userId, items });
    expect(session.getSnapshot().conversation?.messages[0].blocks).toEqual([{ type: "text", text: "question" }, { type: "context", items }]);
    receive({ type: "done", finish: "stop" });
    await Promise.resolve();
    expect(transport.save).toHaveBeenCalledTimes(1);
    expect(vi.mocked(transport.save).mock.calls[0][0].messages[0].blocks.at(-1)).toEqual({ type: "context", items });
    receive({ type: "context", message_id: userId, items: [] });
    expect(session.getSnapshot().conversation?.messages[0].blocks.at(-1)).toEqual({ type: "context", items });
    unmount();
  });
  it("ignores context from a reset run", async () => {
    const { session, transport, unmount, listeners } = await fixture();
    await session.send("first");
    const [request, id] = vi.mocked(transport.send).mock.calls[0];
    session.newConversation({ connection_id: "other", provider: "openai", model: "model" });
    listeners.get(id)!({ type: "context", message_id: request.messages[0].id, items: [] });
    expect(session.getSnapshot().conversation?.messages).toEqual([]);
    unmount();
  });
});

describe("AI table context lifecycle", () => {
  it("retains cell choices per notebook across target switches and clears them after send", async () => {
    const { session, unmount } = await fixture();
    const options = { choices: [{ provider: "openai", model: "model" }], fallback: { provider: "openai", model: "model" },
      target: { type: "notebook" as const, notebook_id: "one" } };
    await session.activateConnection("connection", options);
    session.newConversation({ connection_id: "connection", provider: "openai", model: "model", target: options.target });
    const choices = { connection_id: "connection", database: null, schema: null, tables: [], excluded_tables: [],
      notebook: { notebook_id: "one", cell_ids: ["cell-a"] } };
    session.setContextTables(choices);
    await session.activateConnection("connection", { ...options, target: { type: "notebook", notebook_id: "two" } });
    expect(session.getSnapshot().contextTables).toBeNull();
    await session.activateConnection("connection", options);
    expect(session.getSnapshot().contextTables).toEqual(choices);
    await session.send("Compare cells", { context: createAiContextOptions({}, defaultAiContextSelection(), "en") });
    expect(session.getSnapshot().contextTables).toBeNull();
    unmount();
  });
  it("retains scoped choices through connection switches and resets them for the next message", async () => {
    const { session, transport, unmount } = await fixture();
    const options = { choices: [{ provider: "openai", model: "model" }], fallback: { provider: "openai", model: "model" } };
    await session.activateConnection("connection", options);
    const tables = { connection_id: "connection", database: "app", schema: "public", tables: ["orders"], excluded_tables: ["users"] };
    session.setContextTables(tables);
    await session.activateConnection("other", options);
    expect(session.getSnapshot().contextTables).toBeNull();
    await session.activateConnection("connection", options);
    expect(session.getSnapshot().contextTables).toEqual(tables);
    const context = createAiContextOptions({}, defaultAiContextSelection(), "en", tables);
    await session.send("question", { context });
    expect(vi.mocked(transport.send).mock.calls[0][2]).toMatchObject(tables);
    expect(session.getSnapshot().contextTables).toBeNull();
    session.setContextTables(tables);
    expect(session.getSnapshot().contextTables).toBeNull();
    unmount();
  });
  it("new chats discard all table choices", async () => {
    const { session, unmount } = await fixture();
    session.setContextTables({ connection_id: "connection", database: "app", schema: "public", tables: ["users"], excluded_tables: [] });
    session.newConversation({ connection_id: "connection", provider: "openai", model: "model" });
    expect(session.getSnapshot().contextTables).toBeNull();
    unmount();
  });
});
