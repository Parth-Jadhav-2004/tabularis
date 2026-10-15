import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";
import { AiChatProvider } from "../../../src/contexts/AiChatProvider";
import { useAiChat } from "../../../src/hooks/useAiChat";
import { ConversationList } from "../../../src/components/ai-chat/ConversationList";
import type { AiChatTransport, AiConversation, AiResumeOptions } from "../../../src/types/aiChat";
import { createAiConversation, createAiMessage } from "../../../src/utils/aiChat";

vi.mock("lucide-react", () => vi.importActual("lucide-react"));

const options: AiResumeOptions = { choices: [{ provider: "openai", model: "model" }], fallback: { provider: "openai", model: "model" } };
function conversation(connectionId: string, title: string): AiConversation {
  return { ...createAiConversation({ connection_id: connectionId, provider: "openai", model: "model" }), title,
    messages: [createAiMessage("user", [{ type: "text", text: "body must not be searched" }])] };
}
function setup(chats: AiConversation[] = []) {
  let clear = () => {};
  const onClose = vi.fn();
  const history = {
    list: vi.fn<NonNullable<AiChatTransport["history"]>["list"]>(async (id) => chats.filter((chat) => id === null || chat.connection_id === id)
      .map((chat) => ({ ...chat, message_count: chat.messages.length }))),
    load: vi.fn<NonNullable<AiChatTransport["history"]>["load"]>(async (connectionId, id) => structuredClone(chats.find((chat) => chat.connection_id === connectionId && chat.id === id)!)),
    rename: vi.fn<NonNullable<AiChatTransport["history"]>["rename"]>(async (connectionId, id, title) => {
      chats = chats.map((chat) => chat.connection_id === connectionId && chat.id === id ? { ...chat, title, title_source: "user" } : chat);
    }),
    delete: vi.fn<NonNullable<AiChatTransport["history"]>["delete"]>(async (connectionId, id) => {
      chats = chats.filter((chat) => chat.connection_id !== connectionId || chat.id !== id);
    }),
  };
  const transport: AiChatTransport = { history,
    listen: async () => () => {}, listenStorageCleared: async (callback) => { clear = callback; return () => {}; },
    send: async (_req, id) => id, cancel: async () => {}, save: async () => ({ bytes: 100, large: false }),
  };
  function Harness() {
    const chat = useAiChat();
    return <>
      <output data-testid="selected">{chat.conversation?.title ?? "none"}</output>
      <ConversationList chat={chat} connectionId="one" connectionNames={{ one: "First connection", two: "Second connection" }} resumeOptions={options} onClose={onClose} />
    </>;
  }
  const rendered = render(<MemoryRouter><AiChatProvider transport={transport}><Harness /></AiChatProvider></MemoryRouter>);
  return { ...rendered, history, onClose, clear: () => act(() => { chats = []; clear(); }) };
}

describe("conversation history list", () => {
  it("filters titles, groups dates and scopes All connections without searching message bodies", async () => {
    const first = conversation("one", "First title");
    const fixture = setup([first, conversation("two", "Second title")]);
    await screen.findByRole("button", { name: /First title/ });
    expect(screen.getByRole("heading", { name: "aiChat.history.buckets.today" })).toBeInTheDocument();
    expect(screen.queryByText("Second title")).not.toBeInTheDocument();
    fireEvent.change(screen.getByRole("searchbox"), { target: { value: "body must" } });
    expect(screen.getByText("aiChat.history.noMatches")).toBeInTheDocument();
    fireEvent.change(screen.getByRole("searchbox"), { target: { value: "" } });
    fireEvent.change(screen.getByLabelText("aiChat.history.filter"), { target: { value: "all" } });
    await screen.findByRole("button", { name: /Second title/ });
    expect(screen.getByRole("heading", { name: "First connection" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Second connection" })).toBeInTheDocument();
    expect(fixture.history.list).toHaveBeenLastCalledWith(null);
  });

  it("resumes a saved conversation through the shared provider", async () => {
    const chat = conversation("one", "Resume me");
    const fixture = setup([chat]);
    fireEvent.click(await screen.findByRole("button", { name: /Resume me/ }));
    await waitFor(() => expect(screen.getByTestId("selected")).toHaveTextContent("Resume me"));
    expect(fixture.history.load).toHaveBeenCalledWith("one", chat.id);
    expect(fixture.onClose).toHaveBeenCalledTimes(1);
  });

  it("renames inline and deletes only after confirmation", async () => {
    const chat = conversation("one", "Original title");
    const fixture = setup([chat]);
    await screen.findByRole("button", { name: /Original title/ });
    fireEvent.click(screen.getByRole("button", { name: "aiChat.history.rename" }));
    fireEvent.change(screen.getByRole("textbox", { name: "aiChat.history.rename" }), { target: { value: "Renamed title" } });
    fireEvent.click(screen.getByRole("button", { name: "common.save" }));
    await screen.findByRole("button", { name: /Renamed title/ });
    expect(fixture.history.rename).toHaveBeenCalledWith("one", chat.id, "Renamed title");
    fireEvent.click(screen.getByRole("button", { name: "aiChat.history.delete" }));
    expect(fixture.history.delete).not.toHaveBeenCalled();
    fireEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "common.cancel" }));
    expect(fixture.history.delete).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "aiChat.history.delete" }));
    fireEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "common.delete" }));
    await screen.findByText("aiChat.history.empty");
    expect(fixture.history.delete).toHaveBeenCalledWith("one", chat.id);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("shows an empty state after reset and a recovery link for keychain errors", async () => {
    const fixture = setup([conversation("one", "Stored title")]);
    await screen.findByRole("button", { name: /Stored title/ });
    fixture.clear();
    await screen.findByText("aiChat.history.empty");
    fixture.history.list.mockRejectedValueOnce({ kind: "key_unavailable", secret: "never display" });
    fireEvent.click(screen.getByRole("button", { name: "aiChat.history.refresh" }));
    await screen.findByText("aiChat.history.errors.key_unavailable");
    expect(screen.getByRole("link", { name: "settings.ai.storage.reset" })).toHaveAttribute("href", "/settings?tab=ai");
    expect(screen.queryByText("never display")).not.toBeInTheDocument();
  });
});
