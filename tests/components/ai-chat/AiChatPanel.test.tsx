import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { AiChatPanel } from "../../../src/components/ai-chat/AiChatPanel";
import { AiChatProvider } from "../../../src/contexts/AiChatProvider";
import { AiAvailabilityContext } from "../../../src/contexts/AiAvailabilityContext";
import { AiEditorContext } from "../../../src/contexts/AiEditorContext";
import { RightSidebarProvider } from "../../../src/contexts/RightSidebarProvider";
import { DEFAULT_SETTINGS } from "../../../src/contexts/SettingsContext";
import type { AiChatEvent, AiChatTransport } from "../../../src/types/aiChat";
import type { AiEditorTarget } from "../../../src/types/aiEditor";
import type { AiAvailability } from "../../../src/types/aiAvailability";
import { createAiEditorStore } from "../../../src/utils/aiEditor";

vi.mock("lucide-react", () => vi.importActual("lucide-react"));

vi.mock("../../../src/hooks/useSettings", () => ({
  useSettings: () => ({ settings: { ...DEFAULT_SETTINGS, aiEnabled: true, aiProvider: "openai", aiModel: "example" } }),
}));

function setup() {
  const listeners = new Map<string, (event: AiChatEvent) => void>();
  let clear: () => void = () => {};
  const transport = {
    listen: vi.fn<AiChatTransport["listen"]>(async (id, callback) => { listeners.set(id, callback); return () => { listeners.delete(id); }; }),
    listenStorageCleared: async (callback) => { clear = callback; return () => {}; },
    send: vi.fn<AiChatTransport["send"]>(async (_request, id) => id),
    cancel: vi.fn<AiChatTransport["cancel"]>(async () => {}),
    save: vi.fn<AiChatTransport["save"]>(async () => ({ bytes: 123, large: false })),
  } satisfies AiChatTransport;
  const actions = { insert: vi.fn(), replace: vi.fn(), run: vi.fn(), focus: vi.fn() };
  const target: AiEditorTarget = { connectionId: "one", database: "db", schema: "public", actions };
  const store = createAiEditorStore();
  store.register("root", target);
  const availability: AiAvailability = { available: true, loading: false, refresh: vi.fn(), providers: [
    { provider: "openai", label: "OpenAI", source: "env", models_cached: true, usable: true, reason: null },
    { provider: "anthropic", label: "Anthropic", source: "keychain", models_cached: true, usable: true, reason: null },
  ] };
  const tree = (visible = true, available = availability) => <MemoryRouter>
    <AiAvailabilityContext.Provider value={available}><AiEditorContext.Provider value={store}>
      <RightSidebarProvider><AiChatProvider transport={transport}>{visible && <AiChatPanel />}</AiChatProvider></RightSidebarProvider>
    </AiEditorContext.Provider></AiAvailabilityContext.Provider>
  </MemoryRouter>;
  const rendered = render(tree());
  const emit = (event: AiChatEvent) => {
    const id = transport.send.mock.calls.at(-1)?.[1];
    if (id) act(() => listeners.get(id)?.(event));
  };
  return { ...rendered, transport, target, store, actions, availability, tree, emit, clear: () => act(clear) };
}

async function send(text = "Write a query") {
  await waitFor(() => expect(screen.getByRole("button", { name: "aiChat.send" })).toBeDisabled());
  fireEvent.change(screen.getByLabelText("aiChat.message"), { target: { value: text } });
  await waitFor(() => expect(screen.getByRole("button", { name: "aiChat.send" })).toBeEnabled());
  fireEvent.click(screen.getByRole("button", { name: "aiChat.send" }));
}

describe("AI chat panel", () => {
  it("takes a fresh editor snapshot at send and preserves removed context across panel remounts", async () => {
    const fixture = setup();
    const readContext = vi.fn(() => ({ query: "SELECT * FROM users", selection: "name", cursor_statement: "SELECT * FROM users", last_error: "private-error" }));
    act(() => { fixture.store.register("root", { ...fixture.target, readContext }); });
    await waitFor(() => expect(screen.getByRole("button", { name: "aiChat.context.kinds.query" })).toBeEnabled());
    fireEvent.click(screen.getByRole("button", { name: "aiChat.context.kinds.query" }));
    fixture.rerender(fixture.tree(false));
    fixture.rerender(fixture.tree());
    expect(screen.getByRole("button", { name: "aiChat.context.kinds.query" })).toHaveAttribute("aria-pressed", "false");
    readContext.mockReturnValue({ query: "new current query", selection: "fresh selection", cursor_statement: "new current query", last_error: "new error" });
    await send();
    await waitFor(() => expect(fixture.transport.send).toHaveBeenCalledTimes(1));
    expect(fixture.transport.send.mock.calls[0][2]).toMatchObject({
      connection_id: "one", database: "db", schema: "public",
      editor: { query: null, selection: "fresh selection", cursor_statement: "new current query" }, last_error: null,
    });
    const userId = fixture.transport.send.mock.calls[0][0].messages[0].id;
    fixture.emit({ type: "context", message_id: userId, items: [{ kind: "table", id: "users", label: "users", chars: 200, truncated: false }] });
    expect(screen.getByText("aiChat.context.sent")).toBeInTheDocument();
    fixture.emit({ type: "done", finish: "stop" });
    await waitFor(() => expect(fixture.transport.save).toHaveBeenCalledTimes(1));
    fixture.rerender(fixture.tree(false));
    fixture.rerender(fixture.tree());
    expect(screen.getByText("aiChat.context.sent")).toBeInTheDocument();
  });

  beforeEach(() => {
    localStorage.clear();
    vi.mocked(invoke).mockReset().mockImplementation(async (command) => command === "get_ai_models"
      ? { openai: ["example"], anthropic: ["other"] }
      : { tools: false, images: false, reasoning: true, context_window: null });
    Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText: vi.fn().mockResolvedValue(undefined) } });
  });

  it("streams Markdown, suppresses remote images/HTML, and enables exact SQL actions only on completion", async () => {
    const fixture = setup();
    await send();
    await waitFor(() => expect(fixture.transport.send).toHaveBeenCalledTimes(1));
    fixture.emit({ type: "text_delta", text: "**Result**\n\n```sql\nSELECT 42;\n```\n\n![remote](https://example.test/track)\n<script>bad()</script>" });
    expect(screen.getByText("Result").tagName).toBe("STRONG");
    expect(screen.queryByRole("img")).not.toBeInTheDocument();
    expect(document.querySelector("script")).toBeNull();
    expect(screen.getByRole("button", { name: "aiChat.run" })).toBeDisabled();
    fixture.emit({ type: "done", finish: "stop" });
    await waitFor(() => expect(fixture.transport.save).toHaveBeenCalledTimes(1));
    for (const action of ["insert", "replace", "run"] as const) {
      fireEvent.click(screen.getByRole("button", { name: `aiChat.${action}` }));
      expect(fixture.actions[action]).toHaveBeenCalledWith("SELECT 42;");
    }
    fireEvent.click(screen.getByRole("button", { name: "aiChat.copy" }));
    await waitFor(() => expect(navigator.clipboard.writeText).toHaveBeenCalledWith("SELECT 42;"));
    expect(fixture.transport.save.mock.calls[0][0].connection_id).toBe("one");
  });

  it("preserves transcript and draft when the panel is unmounted and remounted", async () => {
    const fixture = setup();
    await send("hello");
    await waitFor(() => expect(fixture.transport.send).toHaveBeenCalledTimes(1));
    fixture.emit({ type: "reasoning_delta", text: "Private thought" });
    fixture.emit({ type: "done", finish: "stop" });
    fireEvent.change(screen.getByLabelText("aiChat.message"), { target: { value: "next draft" } });
    fixture.rerender(fixture.tree(false));
    fixture.rerender(fixture.tree());
    expect(screen.getByText("hello")).toBeInTheDocument();
    expect(screen.getByLabelText("aiChat.message")).toHaveValue("next draft");
    expect(screen.getByText("aiChat.reasoning").closest("details")).not.toHaveAttribute("open");
  });

  it("switches providers for the next turn with full history and keeps the original assistant model", async () => {
    const fixture = setup();
    await send("first");
    await waitFor(() => expect(fixture.transport.send).toHaveBeenCalledTimes(1));
    fixture.emit({ type: "text_delta", text: "answer" });
    fixture.emit({ type: "done", finish: "stop" });
    await waitFor(() => expect(screen.getByText("aiChat.saved")).toBeInTheDocument());
    fireEvent.change(screen.getByLabelText("aiChat.model"), { target: { value: JSON.stringify(["anthropic", "other"]) } });
    expect(screen.getByText("openai · example")).toBeInTheDocument();
    await send("second");
    await waitFor(() => expect(fixture.transport.send).toHaveBeenCalledTimes(2));
    expect(fixture.transport.send.mock.calls[1][0]).toMatchObject({ provider: "anthropic", model: "other", tools: [] });
    expect(fixture.transport.send.mock.calls[1][0].messages).toHaveLength(3);
  });

  it("blocks SQL and sends after a target switch until a new chat is created", async () => {
    const fixture = setup();
    await send();
    await waitFor(() => expect(fixture.transport.send).toHaveBeenCalledTimes(1));
    fixture.emit({ type: "text_delta", text: "```sql\nDELETE FROM users;\n```" });
    fixture.emit({ type: "done", finish: "stop" });
    await waitFor(() => expect(screen.getByText("aiChat.saved")).toBeInTheDocument());
    act(() => { fixture.store.register("root", { ...fixture.target, database: "other" }); });
    expect(screen.getByText("aiChat.targetChanged")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "aiChat.run" })).toBeDisabled();
    fireEvent.change(screen.getByLabelText("aiChat.message"), { target: { value: "another" } });
    expect(screen.getByRole("button", { name: "aiChat.send" })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "aiChat.newChat" }));
    expect(screen.queryByText("aiChat.targetChanged")).not.toBeInTheDocument();
  });

  it("keeps the transcript and exposes Settings when the final provider disappears", async () => {
    const fixture = setup();
    await send("keep me");
    await waitFor(() => expect(fixture.transport.send).toHaveBeenCalledTimes(1));
    fixture.emit({ type: "done", finish: "stop" });
    fixture.rerender(fixture.tree(true, { ...fixture.availability, available: false, providers: [] }));
    expect(screen.getByText("keep me")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "aiChat.openSettings" })).toHaveAttribute("href", "/settings?tab=ai");
    expect(screen.getByRole("button", { name: "aiChat.send" })).toBeDisabled();
  });

  it("stops, retries failed encrypted saves and clears the transcript on storage reset", async () => {
    const fixture = setup();
    fixture.transport.save.mockRejectedValueOnce(new Error("locked"));
    await send();
    await waitFor(() => expect(fixture.transport.send).toHaveBeenCalledTimes(1));
    fireEvent.click(screen.getByRole("button", { name: "aiChat.stop" }));
    await waitFor(() => expect(fixture.transport.cancel).toHaveBeenCalled());
    fixture.emit({ type: "done", finish: "cancelled" });
    await waitFor(() => expect(screen.getByRole("button", { name: "aiChat.retrySave" })).toBeInTheDocument());
    fireEvent.click(screen.getByRole("button", { name: "aiChat.retrySave" }));
    await waitFor(() => expect(screen.getByText("aiChat.saved")).toBeInTheDocument());
    fixture.clear();
    expect(screen.getByText("aiChat.empty")).toBeInTheDocument();
  });

  it("supports Enter, Shift+Enter, IME composition and Escape focus", async () => {
    const fixture = setup();
    const composer = screen.getByLabelText("aiChat.message");
    fireEvent.change(composer, { target: { value: "hello" } });
    await waitFor(() => expect(screen.getByRole("button", { name: "aiChat.send" })).toBeEnabled());
    fireEvent.keyDown(composer, { key: "Enter", shiftKey: true });
    fireEvent.keyDown(composer, { key: "Enter", isComposing: true });
    expect(fixture.transport.send).not.toHaveBeenCalled();
    fireEvent.keyDown(composer, { key: "Escape" });
    expect(fixture.actions.focus).toHaveBeenCalled();
    fireEvent.keyDown(composer, { key: "Enter" });
    await waitFor(() => expect(fixture.transport.send).toHaveBeenCalledTimes(1));
  });
});
