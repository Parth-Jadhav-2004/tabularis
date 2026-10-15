import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { listen, type EventCallback } from "@tauri-apps/api/event";
import { aiChatTransport } from "../../src/utils/aiChatTransport";
import { createAiConversation, createAiMessage } from "../../src/utils/aiChat";
import type { AiChatEvent, AiChatRequest } from "../../src/types/aiChat";

vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

describe("AI chat Tauri transport", () => {
  it("addresses history by both connection and conversation ID and uses null for All connections", async () => {
    const history = aiChatTransport.history!;
    await history.list("connection");
    expect(invoke).toHaveBeenCalledWith("ai_chat_list", { connectionId: "connection" });
    await history.list(null);
    expect(invoke).toHaveBeenCalledWith("ai_chat_list", { connectionId: null });
    await history.load("connection", "chat");
    expect(invoke).toHaveBeenCalledWith("ai_chat_load", { connectionId: "connection", id: "chat" });
    await history.rename("connection", "chat", "Title");
    expect(invoke).toHaveBeenCalledWith("ai_chat_rename", { connectionId: "connection", id: "chat", title: "Title" });
    await history.delete("connection", "chat");
    expect(invoke).toHaveBeenCalledWith("ai_chat_delete", { connectionId: "connection", id: "chat" });
  });
  beforeEach(() => { vi.clearAllMocks(); });

  it("uses Rust's event channel and forwards only the payload", async () => {
    const unlisten = vi.fn();
    vi.mocked(listen).mockResolvedValueOnce(unlisten);
    const receive = vi.fn();
    expect(await aiChatTransport.listen("run", receive)).toBe(unlisten);
    expect(listen).toHaveBeenCalledWith("ai-chat://run", expect.any(Function));
    const callback = vi.mocked(listen).mock.calls[0][1] as EventCallback<AiChatEvent>;
    callback({ event: "ai-chat://run", id: 1, payload: { type: "text_delta", text: "SELECT 1" } });
    expect(receive).toHaveBeenCalledWith({ type: "text_delta", text: "SELECT 1" });
  });

  it("subscribes to the backend reset/delete-all notification", async () => {
    const clear = vi.fn();
    await aiChatTransport.listenStorageCleared(clear);
    expect(listen).toHaveBeenCalledWith("ai-chat://storage-cleared", clear);
  });

  it("retains snake_case payloads and camelCase Tauri command arguments", async () => {
    const req: AiChatRequest = {
      provider: "openai", model: "model", system: "", tools: [],
      messages: [createAiMessage("user", [{ type: "text", text: "question" }])],
      max_output_tokens: 123, reasoning: "low",
    };
    vi.mocked(invoke).mockResolvedValueOnce("run");
    expect(await aiChatTransport.send(req, "run")).toBe("run");
    expect(invoke).toHaveBeenCalledWith("ai_chat_send", { req, runId: "run" });
    await aiChatTransport.cancel("run");
    expect(invoke).toHaveBeenCalledWith("ai_chat_cancel", { runId: "run" });
    const conversation = createAiConversation({ connection_id: "connection", provider: "openai", model: "model" });
    vi.mocked(invoke).mockResolvedValueOnce({ bytes: 123, large: false });
    expect(await aiChatTransport.save(conversation)).toEqual({ bytes: 123, large: false });
    expect(invoke).toHaveBeenCalledWith("ai_chat_save", { conversation });
  });
});
