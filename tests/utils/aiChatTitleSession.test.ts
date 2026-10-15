import { describe, expect, it, vi } from "vitest";
import type { AiChatEvent, AiChatTransport, AiConversationSummary } from "../../src/types/aiChat";
import { createAiChatSession } from "../../src/utils/aiChatSession";

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => { resolve = done; });
  return { promise, resolve };
}

async function setup() {
  let receive: (event: AiChatEvent) => void = () => {};
  let clear = () => {};
  const generated = deferred<AiConversationSummary | null>();
  const transport: AiChatTransport = {
    generateTitle: vi.fn(async () => generated.promise),
    listen: async (_id, callback) => { receive = callback; return () => {}; },
    listenStorageCleared: async (callback) => { clear = callback; return () => {}; },
    send: vi.fn(async (_req, id) => id), cancel: vi.fn(async () => {}),
    save: vi.fn(async () => ({ bytes: 100, large: false })),
    history: { list: async () => [], load: vi.fn(), rename: vi.fn(async () => {}), delete: vi.fn(async () => {}) },
  };
  const session = createAiChatSession(transport);
  const unmount = session.mount();
  await Promise.resolve();
  session.newConversation({ connection_id: "connection", provider: "openai", model: "model" });
  const summary = (title = "Generated title"): AiConversationSummary => ({
    ...session.getSnapshot().conversation!, title, title_source: "generated", message_count: 2,
  });
  const reply = (finish: "stop" | "cancelled" = "stop") => {
    receive({ type: "text_delta", text: "First answer" });
    receive({ type: "done", finish });
  };
  return { session, transport, generated, summary, reply, unmount, clear: () => clear() };
}

describe("AI conversation generated titles", () => {
  it("generates only after a successful encrypted save and keeps a manual rename over a delayed title", async () => {
    const f = await setup();
    await f.session.send("First question");
    const saved = deferred<{ bytes: number; large: boolean }>();
    vi.mocked(f.transport.save).mockReturnValueOnce(saved.promise);
    f.reply();
    expect(f.transport.generateTitle).not.toHaveBeenCalled();
    saved.resolve({ bytes: 100, large: false });
    await vi.waitFor(() => expect(f.transport.generateTitle).toHaveBeenCalledOnce());
    const generated = f.summary();
    expect(f.transport.generateTitle).toHaveBeenCalledWith("connection", generated.id);
    await f.session.renameConversation(generated, "Manual title");
    f.generated.resolve(generated);
    await Promise.resolve();
    await Promise.resolve();
    expect(f.session.getSnapshot().conversation).toMatchObject({ title: "Manual title", title_source: "user" });
    await f.session.send("Next question");
    f.reply();
    await Promise.resolve();
    expect(f.transport.generateTitle).toHaveBeenCalledOnce();
    f.unmount();
  });

  it("applies a generated title while another turn streams without replacing its messages", async () => {
    const f = await setup();
    await f.session.send("First question");
    f.reply();
    await vi.waitFor(() => expect(f.transport.generateTitle).toHaveBeenCalledOnce());
    const title = f.summary();
    await f.session.send("Next question");
    f.generated.resolve(title);
    await vi.waitFor(() => expect(f.session.getSnapshot().conversation?.title_source).toBe("generated"));
    expect(f.session.getSnapshot().conversation?.messages).toHaveLength(4);
    expect(f.session.getSnapshot().activeRun).not.toBeNull();
    f.unmount();
  });

  it.each(["reset", "unmount", "delete"] as const)("ignores title completion after %s", async (action) => {
    const f = await setup();
    await f.session.send("First question");
    f.reply();
    await vi.waitFor(() => expect(f.transport.generateTitle).toHaveBeenCalledOnce());
    const title = f.summary();
    if (action === "reset") f.clear();
    else if (action === "unmount") f.unmount();
    else await f.session.deleteConversation(title);
    f.generated.resolve(title);
    await Promise.resolve();
    await Promise.resolve();
    expect(f.session.getSnapshot().conversation?.title_source).not.toBe("generated");
    if (action !== "unmount") expect(f.session.getSnapshot().conversation).toBeNull();
    f.unmount();
  });

  it("updates an in-memory connection session after switching away", async () => {
    const f = await setup();
    await f.session.send("First question");
    f.reply();
    await vi.waitFor(() => expect(f.transport.generateTitle).toHaveBeenCalledOnce());
    const title = f.summary();
    const options = { choices: [{ provider: "openai", model: "model" }], fallback: { provider: "openai", model: "model" } };
    await f.session.activateConnection("other", options);
    f.generated.resolve(title);
    await Promise.resolve();
    await Promise.resolve();
    await f.session.activateConnection("connection", options);
    expect(f.session.getSnapshot().conversation).toMatchObject({ title: "Generated title", title_source: "generated" });
    f.unmount();
  });

  it("silently keeps the initial title on failure and skips cancelled replies", async () => {
    const f = await setup();
    vi.mocked(f.transport.generateTitle!).mockRejectedValue(new Error("private provider error"));
    await f.session.send("First question");
    f.reply();
    await vi.waitFor(() => expect(f.transport.generateTitle).toHaveBeenCalledOnce());
    expect(f.session.getSnapshot()).toMatchObject({ error: null, conversation: { title: "First question", title_source: "first_message" } });
    f.session.newConversation({ connection_id: "connection", provider: "openai", model: "model" });
    await f.session.send("Cancelled question");
    f.reply("cancelled");
    await vi.waitFor(() => expect(f.session.getSnapshot().saveState).toBe("saved"));
    expect(f.transport.generateTitle).toHaveBeenCalledOnce();
    f.unmount();
  });

  it("waits for a failed save to be retried before requesting a title", async () => {
    const f = await setup();
    vi.mocked(f.transport.save).mockRejectedValueOnce(new Error("locked"));
    await f.session.send("First question");
    f.reply();
    await vi.waitFor(() => expect(f.session.getSnapshot().saveState).toBe("error"));
    expect(f.transport.generateTitle).not.toHaveBeenCalled();
    await f.session.retrySave();
    expect(f.transport.generateTitle).toHaveBeenCalledOnce();
    f.unmount();
  });
});
