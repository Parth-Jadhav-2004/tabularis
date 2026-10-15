import { type ReactNode } from "react";
import { act, render, renderHook, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { AiChatProvider } from "../../src/contexts/AiChatProvider";
import { useAiChat } from "../../src/hooks/useAiChat";
import type { AiChatEvent, AiChatSaveResult, AiChatTransport } from "../../src/types/aiChat";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => { resolve = res; reject = rej; });
  return { promise, resolve, reject };
}

function mockTransport() {
  const callbacks = new Map<string, (event: AiChatEvent) => void>();
  const unlisteners = new Map<string, ReturnType<typeof vi.fn>>();
  let clear: () => void = () => {};
  const storageUnlisten = vi.fn();
  const transport = {
    listen: vi.fn<AiChatTransport["listen"]>(async (id, onEvent) => {
      callbacks.set(id, onEvent);
      const unlisten = vi.fn();
      unlisteners.set(id, unlisten);
      return unlisten;
    }),
    listenStorageCleared: vi.fn<AiChatTransport["listenStorageCleared"]>(async (onClear) => {
      clear = onClear;
      return storageUnlisten;
    }),
    send: vi.fn<AiChatTransport["send"]>(async (_request, id) => id),
    cancel: vi.fn<AiChatTransport["cancel"]>(async () => {}),
    save: vi.fn<AiChatTransport["save"]>(async () => ({ bytes: 123, large: false })),
  } satisfies AiChatTransport;
  return {
    transport, unlisteners, storageUnlisten,
    emit: (id: string, event: AiChatEvent) => { callbacks.get(id)?.(event); },
    clear: () => clear(),
  };
}

async function setup(mock = mockTransport(), strict = false) {
  const wrapper = ({ children }: { children: ReactNode }) =>
    <AiChatProvider transport={mock.transport}>{children}</AiChatProvider>;
  const hook = renderHook(() => useAiChat(), { wrapper, reactStrictMode: strict });
  await waitFor(() => expect(hook.result.current.ready).toBe(true));
  act(() => hook.result.current.newConversation({
    connection_id: "connection", database: "db", schema: "public", provider: "openai", model: "first-model",
  }));
  return { ...hook, ...mock };
}

describe("useAiChat lifecycle", () => {
  it("subscribes before invoking and saves an early completed response exactly once", async () => {
    const chat = await setup();
    const order: string[] = [];
    chat.transport.listen.mockImplementationOnce(async (id, onEvent) => {
      order.push("listen");
      return chat.transport.listen.getMockImplementation()!(id, onEvent);
    });
    chat.transport.send.mockImplementationOnce(async (_request, id) => {
      order.push("send");
      chat.emit(id, { type: "text_delta", text: "SELECT " });
      chat.emit(id, { type: "text_delta", text: "1" });
      chat.emit(id, { type: "done", finish: "stop" });
      chat.emit(id, { type: "done", finish: "stop" });
      return id;
    });
    act(() => chat.result.current.setDraft("question"));
    await act(async () => { expect(await chat.result.current.send("question", { system: "instruction" })).toBe(true); });
    expect(order).toEqual(["listen", "send"]);
    expect(chat.result.current).toMatchObject({ runState: "idle", draft: "", saveState: "saved" });
    const [request, id] = chat.transport.send.mock.calls[0];
    expect(request).toMatchObject({ provider: "openai", model: "first-model", system: "instruction", tools: [] });
    expect(request.messages).toHaveLength(1);
    expect(request.messages[0]).toMatchObject({ role: "user", blocks: [{ type: "text", text: "question" }] });
    expect(chat.transport.save).toHaveBeenCalledTimes(1);
    expect(chat.transport.save.mock.calls[0][0]).toMatchObject({
      connection_id: "connection", database: "db", schema: "public", title: "question",
      messages: [expect.objectContaining({ role: "user" }), expect.objectContaining({
        role: "assistant", blocks: [{ type: "text", text: "SELECT 1" }], provider: "openai", model: "first-model",
      })],
    });
    expect(chat.unlisteners.get(id)).toHaveBeenCalledTimes(1);
  });

  it("rejects blank sends and concurrent sends synchronously", async () => {
    const chat = await setup();
    await act(async () => { expect(await chat.result.current.send("  \n ")).toBe(false); });
    let first!: Promise<boolean>;
    await act(async () => {
      first = chat.result.current.send("first");
      expect(await chat.result.current.send("duplicate")).toBe(false);
      await first;
    });
    expect(chat.transport.send).toHaveBeenCalledTimes(1);
    expect(chat.result.current.conversation?.messages).toHaveLength(2);
  });

  it("keeps sanitized provider errors and persists partial failed output", async () => {
    const chat = await setup();
    await act(async () => { await chat.result.current.send("question"); });
    const id = chat.transport.send.mock.calls[0][1];
    await act(async () => {
      chat.emit(id, { type: "text_delta", text: "partial" });
      chat.emit(id, { type: "error", error: { kind: "network", message: "Safe backend message" } });
      chat.emit(id, { type: "done", finish: "error" });
    });
    expect(chat.result.current).toMatchObject({ runState: "error", saveState: "saved", error: { kind: "provider" } });
    expect(chat.transport.save.mock.calls[0][0].messages[1].blocks).toHaveLength(2);
  });

  it("does not invoke a provider when event subscription fails", async () => {
    const chat = await setup();
    chat.transport.listen.mockRejectedValueOnce(new Error("raw sensitive exception"));
    await act(async () => { expect(await chat.result.current.send("question")).toBe(false); });
    expect(chat.result.current.error).toEqual({ kind: "subscription" });
    expect(chat.transport.send).not.toHaveBeenCalled();
    expect(chat.transport.save).not.toHaveBeenCalled();
  });

  it("cleans up and sanitizes invoke failures without accepting late deltas", async () => {
    const chat = await setup();
    chat.transport.send.mockRejectedValueOnce({ message: "raw request or credentials" });
    await act(async () => { expect(await chat.result.current.send("question")).toBe(false); });
    const id = chat.transport.send.mock.calls[0][1];
    act(() => chat.emit(id, { type: "text_delta", text: "late" }));
    expect(chat.result.current).toMatchObject({ activeRun: null, runState: "error", error: { kind: "send" } });
    expect(chat.result.current.conversation?.messages[1].blocks).toEqual([]);
    expect(chat.unlisteners.get(id)).toHaveBeenCalledTimes(1);
    expect(chat.transport.cancel).toHaveBeenCalledWith(id);
  });

  it("cancels an active run and waits for Done before saving the partial response", async () => {
    const chat = await setup();
    await act(async () => { await chat.result.current.send("question"); });
    const id = chat.transport.send.mock.calls[0][1];
    act(() => chat.emit(id, { type: "text_delta", text: "partial" }));
    await act(async () => { await chat.result.current.stop(); });
    expect(chat.transport.cancel).toHaveBeenCalledWith(id);
    expect(chat.transport.save).not.toHaveBeenCalled();
    expect(chat.result.current.runState).toBe("streaming");
    await act(async () => { chat.emit(id, { type: "done", finish: "cancelled" }); });
    expect(chat.result.current).toMatchObject({ runState: "idle", finish: "cancelled", saveState: "saved" });
    expect(chat.transport.save.mock.calls[0][0].messages[1].blocks).toEqual([{ type: "text", text: "partial" }]);
  });

  it("stops before subscription completes without starting the backend run", async () => {
    const chat = await setup();
    const subscription = deferred<() => void>();
    const unlisten = vi.fn();
    chat.transport.listen.mockReturnValueOnce(subscription.promise);
    let pending!: Promise<boolean>;
    await act(async () => {
      pending = chat.result.current.send("question");
      await chat.result.current.stop();
    });
    await act(async () => { subscription.resolve(unlisten); expect(await pending).toBe(false); });
    expect(unlisten).toHaveBeenCalledTimes(1);
    expect(chat.transport.send).not.toHaveBeenCalled();
    expect(chat.result.current.finish).toBe("cancelled");
    expect(chat.transport.save).toHaveBeenCalledTimes(1);
  });

  it("repeats cancellation after a pending send acknowledges backend registration", async () => {
    const chat = await setup();
    const acknowledgement = deferred<string>();
    chat.transport.send.mockReturnValueOnce(acknowledgement.promise);
    let pending!: Promise<boolean>;
    act(() => { pending = chat.result.current.send("question"); });
    await waitFor(() => expect(chat.transport.send).toHaveBeenCalledTimes(1));
    const id = chat.transport.send.mock.calls[0][1];
    await act(async () => { await chat.result.current.stop(); });
    expect(chat.transport.cancel).toHaveBeenCalledTimes(1);
    await act(async () => { acknowledgement.resolve(id); await pending; });
    expect(chat.transport.cancel).toHaveBeenCalledTimes(2);
  });

  it("keeps the run owned when cancellation fails, allowing Stop to be retried", async () => {
    const chat = await setup();
    await act(async () => { await chat.result.current.send("question"); });
    chat.transport.cancel.mockRejectedValueOnce(new Error("sensitive exception"));
    await act(async () => { await chat.result.current.stop(); });
    expect(chat.result.current).toMatchObject({ runState: "streaming", error: { kind: "cancel" } });
    expect(chat.result.current.activeRun).not.toBeNull();
    await act(async () => {
      expect(await chat.result.current.send("duplicate")).toBe(false);
      await chat.result.current.stop();
    });
    expect(chat.transport.cancel).toHaveBeenCalledTimes(2);
  });

  it("isolates switched conversations from late deltas, errors and terminal events", async () => {
    const chat = await setup();
    await act(async () => { await chat.result.current.send("question"); });
    const id = chat.transport.send.mock.calls[0][1];
    act(() => chat.result.current.newConversation({ connection_id: "other", provider: "ollama", model: "local" }));
    await act(async () => {
      chat.emit(id, { type: "text_delta", text: "other connection's data" });
      chat.emit(id, { type: "error", error: { kind: "network", message: "late" } });
      chat.emit(id, { type: "done", finish: "stop" });
    });
    expect(chat.result.current.conversation).toMatchObject({ connection_id: "other", messages: [] });
    expect(chat.result.current).toMatchObject({ error: null, runState: "idle" });
    expect(chat.unlisteners.get(id)).toHaveBeenCalledTimes(1);
    expect(chat.transport.cancel).toHaveBeenCalledWith(id);
    expect(chat.transport.save).not.toHaveBeenCalled();
  });

  it("cleans up subscriptions and cancels a run on provider unmount", async () => {
    const chat = await setup();
    await act(async () => { await chat.result.current.send("question"); });
    const id = chat.transport.send.mock.calls[0][1];
    chat.unmount();
    chat.emit(id, { type: "done", finish: "stop" });
    expect(chat.unlisteners.get(id)).toHaveBeenCalledTimes(1);
    expect(chat.storageUnlisten).toHaveBeenCalledTimes(1);
    expect(chat.transport.cancel).toHaveBeenCalledWith(id);
    expect(chat.transport.save).not.toHaveBeenCalled();
  });

  it("releases a late listener when the provider unmounts during subscription", async () => {
    const chat = await setup();
    const subscription = deferred<() => void>();
    const unlisten = vi.fn();
    chat.transport.listen.mockReturnValueOnce(subscription.promise);
    let pending!: Promise<boolean>;
    act(() => { pending = chat.result.current.send("question"); });
    chat.unmount();
    subscription.resolve(unlisten);
    expect(await pending).toBe(false);
    expect(unlisten).toHaveBeenCalledTimes(1);
    expect(chat.transport.send).not.toHaveBeenCalled();
  });

  it("handles StrictMode effect replay without leaking the first storage listener", async () => {
    const chat = await setup(mockTransport(), true);
    expect(chat.transport.listenStorageCleared).toHaveBeenCalledTimes(2);
    expect(chat.storageUnlisten).toHaveBeenCalledTimes(1);
    await act(async () => { await chat.result.current.send("question"); });
    expect(chat.transport.send).toHaveBeenCalledTimes(1);
    chat.unmount();
    expect(chat.storageUnlisten).toHaveBeenCalledTimes(2);
  });

  it("clears the transcript and blocks late persistence after a storage reset/deletion", async () => {
    const chat = await setup();
    await act(async () => { await chat.result.current.send("question"); });
    const id = chat.transport.send.mock.calls[0][1];
    act(() => chat.result.current.setDraft("private draft"));
    await act(async () => {
      chat.clear();
      chat.emit(id, { type: "done", finish: "stop" });
    });
    expect(chat.result.current).toMatchObject({ conversation: null, draft: "", runState: "idle", ready: true });
    expect(chat.transport.cancel).toHaveBeenCalledWith(id);
    expect(chat.transport.save).not.toHaveBeenCalled();
  });

  it("does not send if storage-clear subscription cannot be established", async () => {
    const mock = mockTransport();
    mock.transport.listenStorageCleared.mockRejectedValue(new Error("raw error"));
    const wrapper = ({ children }: { children: ReactNode }) => <AiChatProvider transport={mock.transport}>{children}</AiChatProvider>;
    const { result } = renderHook(() => useAiChat(), { wrapper });
    await waitFor(() => expect(result.current.error).toEqual({ kind: "storage_subscription" }));
    act(() => result.current.newConversation({ connection_id: "connection", provider: "openai", model: "model" }));
    await act(async () => { expect(await result.current.send("question")).toBe(false); });
    expect(mock.transport.send).not.toHaveBeenCalled();
    expect(result.current.ready).toBe(false);
    expect(result.current.error).toEqual({ kind: "storage_subscription" });
  });

  it("surfaces save failure separately and retries without sending another provider request", async () => {
    const chat = await setup();
    chat.transport.save.mockRejectedValueOnce(new Error("keychain unavailable"));
    await act(async () => { await chat.result.current.send("question"); });
    const id = chat.transport.send.mock.calls[0][1];
    await act(async () => { chat.emit(id, { type: "done", finish: "stop" }); });
    expect(chat.result.current).toMatchObject({ runState: "idle", saveState: "error" });
    expect(chat.result.current.conversation?.messages).toHaveLength(2);
    chat.transport.save.mockResolvedValueOnce({ bytes: 3_000_000, large: true });
    await act(async () => { await chat.result.current.retrySave(); });
    expect(chat.result.current).toMatchObject({ saveState: "saved", saveResult: { bytes: 3_000_000, large: true } });
    expect(chat.transport.send).toHaveBeenCalledTimes(1);
  });

  it("serializes saves with subsequent turns and ignores stale save completion after a switch", async () => {
    const chat = await setup();
    const save = deferred<AiChatSaveResult>();
    chat.transport.save.mockReturnValueOnce(save.promise);
    await act(async () => { await chat.result.current.send("question"); });
    const id = chat.transport.send.mock.calls[0][1];
    act(() => chat.emit(id, { type: "done", finish: "stop" }));
    await act(async () => {
      expect(await chat.result.current.send("next")).toBe(false);
      await chat.result.current.retrySave();
    });
    expect(chat.transport.save).toHaveBeenCalledTimes(1);
    act(() => chat.result.current.newConversation({ connection_id: "other", provider: "ollama", model: "local" }));
    await act(async () => { save.resolve({ bytes: 123, large: false }); });
    expect(chat.result.current).toMatchObject({ saveState: "idle", conversation: { connection_id: "other" } });
  });

  it("switches models for the next turn with full history and stable assistant attribution", async () => {
    const chat = await setup();
    await act(async () => { await chat.result.current.send("first"); });
    const id = chat.transport.send.mock.calls[0][1];
    act(() => chat.result.current.selectModel({ provider: "anthropic", model: "second-model" }));
    await act(async () => {
      chat.emit(id, { type: "text_delta", text: "first response" });
      chat.emit(id, { type: "done", finish: "stop" });
    });
    await act(async () => { await chat.result.current.send("second", { reasoning: "high", max_output_tokens: 1234 }); });
    const request = chat.transport.send.mock.calls[1][0];
    expect(request).toMatchObject({ provider: "anthropic", model: "second-model", reasoning: "high", max_output_tokens: 1234, tools: [] });
    expect(request.messages).toHaveLength(3);
    expect(request.messages[1]).toMatchObject({ provider: "openai", model: "first-model", blocks: [{ type: "text", text: "first response" }] });
    expect(chat.result.current.conversation?.messages[3]).toMatchObject({ provider: "anthropic", model: "second-model" });
    expect(chat.transport.save.mock.calls[0][0]).toMatchObject({ provider: "anthropic", model: "second-model" });
  });

  it("restores a selected conversation and persists an idle model change", async () => {
    const chat = await setup();
    const conversation = { ...chat.result.current.conversation!, title: "Saved", messages: [{
      id: "message", role: "user" as const, blocks: [{ type: "text" as const, text: "old" }],
      created_at: 123, usage: null, provider: null, model: null,
    }] };
    act(() => chat.result.current.selectConversation(conversation));
    await act(async () => { chat.result.current.selectModel({ provider: "ollama", model: "local" }); });
    expect(chat.transport.save.mock.calls[0][0]).toMatchObject({ title: "Saved", provider: "ollama", model: "local", messages: conversation.messages });
    expect(chat.transport.send).not.toHaveBeenCalled();
  });

  it("retains state when the chat panel consumer is removed and mounted again", async () => {
    const mock = mockTransport();
    const Consumer = () => {
      const chat = useAiChat();
      return <>
        <span>{chat.ready ? "ready" : "waiting"}</span>
        <button onClick={() => {
          chat.newConversation({ connection_id: "connection", provider: "openai", model: "model" });
          chat.setDraft("retained draft");
        }}>Start</button>
        <span>{chat.draft}</span>
        <span>{chat.conversation?.connection_id}</span>
      </>;
    };
    const view = (show: boolean) => <AiChatProvider transport={mock.transport}>{show && <Consumer />}</AiChatProvider>;
    const { rerender } = render(view(true));
    await screen.findByText("ready");
    act(() => screen.getByRole("button", { name: "Start" }).click());
    rerender(view(false));
    expect(screen.queryByText("retained draft")).not.toBeInTheDocument();
    rerender(view(true));
    expect(screen.getByText("retained draft")).toBeInTheDocument();
    expect(screen.getByText("connection")).toBeInTheDocument();
    expect(mock.storageUnlisten).not.toHaveBeenCalled();
  });

  it("cancels again on a late acknowledgement after the conversation has switched", async () => {
    const chat = await setup();
    const acknowledgement = deferred<string>();
    chat.transport.send.mockReturnValueOnce(acknowledgement.promise);
    let pending!: Promise<boolean>;
    act(() => { pending = chat.result.current.send("question"); });
    await waitFor(() => expect(chat.transport.send).toHaveBeenCalledTimes(1));
    const id = chat.transport.send.mock.calls[0][1];
    act(() => chat.result.current.newConversation({ connection_id: "other", provider: "ollama", model: "local" }));
    expect(chat.transport.cancel).toHaveBeenCalledTimes(1);
    await act(async () => { acknowledgement.resolve(id); expect(await pending).toBe(false); });
    expect(chat.transport.cancel).toHaveBeenCalledTimes(2);
    expect(chat.result.current.conversation?.connection_id).toBe("other");
    expect(chat.transport.save).not.toHaveBeenCalled();
  });
});
