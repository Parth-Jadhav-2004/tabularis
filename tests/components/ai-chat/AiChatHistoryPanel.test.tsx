import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { AiChatProvider } from "../../../src/contexts/AiChatProvider";
import { AiEditorContext } from "../../../src/contexts/AiEditorContext";
import { AiChatPanel } from "../../../src/components/ai-chat/AiChatPanel";
import { createAiEditorStore } from "../../../src/utils/aiEditor";
import { createAiConversation, createAiMessage } from "../../../src/utils/aiChat";
import type { AiChatTransport, AiConversation } from "../../../src/types/aiChat";

vi.mock("lucide-react", () => vi.importActual("lucide-react"));
vi.mock("../../../src/hooks/useSettings", () => ({ useSettings: () => ({ settings: { aiProvider: "openai", aiModel: "model" } }) }));
vi.mock("../../../src/hooks/useAiChatModels", () => {
  const choices = [{ provider: "openai", model: "model" }];
  return { useAiChatModels: () => ({ choices, loading: false, error: false, refresh: () => {} }) };
});
vi.mock("../../../src/hooks/useAiAvailability", () => ({ useAiAvailability: () => ({ available: true, loading: false, providers: [] }) }));
vi.mock("../../../src/hooks/useRightSidebar", () => ({ useRightSidebar: () => ({ close: () => {} }) }));

function conversation(connectionId: string, model = "model"): AiConversation {
  return { ...createAiConversation({ connection_id: connectionId, provider: "openai", model }), title: `Saved ${connectionId}`,
    messages: [createAiMessage("user", [{ type: "text", text: `Question ${connectionId}` }])] };
}
function setup(chats: AiConversation[], strict = false) {
  const history = {
    list: vi.fn<NonNullable<AiChatTransport["history"]>["list"]>(async (id) => chats.filter((chat) => !id || chat.connection_id === id).map((chat) => ({ ...chat, message_count: 1 }))),
    load: vi.fn<NonNullable<AiChatTransport["history"]>["load"]>(async (connectionId, id) => chats.find((chat) => chat.connection_id === connectionId && chat.id === id)!),
    rename: async () => {}, delete: async () => {},
  };
  const transport: AiChatTransport = { history, listen: async () => () => {}, listenStorageCleared: async () => () => {},
    send: async (_req, id) => id, cancel: async () => {}, save: async () => ({ bytes: 100, large: false }) };
  const store = createAiEditorStore();
  const actions = { insert: vi.fn(), replace: vi.fn(), run: vi.fn(), focus: vi.fn() };
  store.register("root", { connectionId: "one", database: null, schema: null, actions });
  const tree = (visible = true) => <MemoryRouter><AiEditorContext.Provider value={store}>
    <AiChatProvider transport={transport}>{visible && <AiChatPanel />}</AiChatProvider>
  </AiEditorContext.Provider></MemoryRouter>;
  const rendered = render(tree(), { reactStrictMode: strict });
  return { ...rendered, history, tree, store, actions };
}

describe("chat history panel integration", () => {
  beforeEach(() => {
    localStorage.clear();
    vi.mocked(invoke).mockResolvedValue({ tools: false, images: false, reasoning: false, context_window: null });
  });
  it("restores after mounting in StrictMode and preserves the session while browsing history", async () => {
    const fixture = setup([conversation("one")], true);
    await screen.findByText("Question one");
    fireEvent.change(screen.getByLabelText("aiChat.message"), { target: { value: "draft" } });
    fireEvent.click(screen.getByRole("button", { name: "aiChat.history.title" }));
    await screen.findByRole("button", { name: /Saved one/ });
    expect(screen.queryByRole("region", { name: "aiChat.transcript" })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "aiChat.history.back" }));
    expect(screen.getByLabelText("aiChat.message")).toHaveValue("draft");
    fixture.rerender(fixture.tree(false));
    fixture.rerender(fixture.tree());
    expect(screen.getByText("Question one")).toBeInTheDocument();
    expect(screen.getByLabelText("aiChat.message")).toHaveValue("draft");
  });

  it("switches to each connection's last conversation and restores its draft", async () => {
    const fixture = setup([conversation("one"), conversation("two")]);
    await screen.findByText("Question one");
    fireEvent.change(screen.getByLabelText("aiChat.message"), { target: { value: "one draft" } });
    act(() => { fixture.store.register("root", { connectionId: "two", database: null, schema: null, actions: fixture.actions }); });
    await screen.findByText("Question two");
    expect(screen.queryByText("Question one")).not.toBeInTheDocument();
    act(() => { fixture.store.register("root", { connectionId: "one", database: null, schema: null, actions: fixture.actions }); });
    await screen.findByText("Question one");
    expect(screen.getByLabelText("aiChat.message")).toHaveValue("one draft");
    expect(fixture.history.load).toHaveBeenCalledTimes(2);
  });

  it("announces a missing model fallback and leaves an empty transcript for a new connection", async () => {
    const fixture = setup([conversation("one", "removed-model")]);
    await screen.findByText("aiChat.history.modelFallback");
    expect(screen.getByLabelText("aiChat.model")).toHaveValue(JSON.stringify(["openai", "model"]));
    act(() => { fixture.store.register("root", { connectionId: "empty", database: null, schema: null, actions: fixture.actions }); });
    await screen.findByText("aiChat.empty");
    await waitFor(() => expect(screen.queryByText("aiChat.history.modelFallback")).not.toBeInTheDocument());
    expect(screen.queryByText("Question one")).not.toBeInTheDocument();
  });
});
