import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { AiChatPanel } from "../../../src/components/ai-chat/AiChatPanel";
import { AiChatProvider } from "../../../src/contexts/AiChatProvider";
import { SavedQueriesContext, type SavedQueriesContextType } from "../../../src/contexts/SavedQueriesContext";
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

function setup(saved?: SavedQueriesContextType) {
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
      <SavedQueriesContext.Provider value={saved}><RightSidebarProvider><AiChatProvider transport={transport}>{visible && <AiChatPanel />}</AiChatProvider></RightSidebarProvider></SavedQueriesContext.Provider>
    </AiEditorContext.Provider></AiAvailabilityContext.Provider>
  </MemoryRouter>;
  const rendered = render(tree());
  const emit = (event: AiChatEvent) => {
    const id = transport.send.mock.calls.at(-1)?.[1];
    if (id) act(() => listeners.get(id)?.(event));
  };
  return { ...rendered, transport, target, store, actions, availability, tree, emit, clear: () => act(clear) };
}

function openContext() { fireEvent.click(screen.getByRole("button", { name: "aiChat.context.options" })); }

async function send(text = "Write a query") {
  await waitFor(() => expect(screen.getByRole("button", { name: "aiChat.send" })).toBeDisabled());
  fireEvent.change(screen.getByLabelText("aiChat.message"), { target: { value: text } });
  await waitFor(() => expect(screen.getByRole("button", { name: "aiChat.send" })).toBeEnabled());
  fireEvent.click(screen.getByRole("button", { name: "aiChat.send" }));
}

describe("AI chat panel", () => {
  it("sends a slash command with fresh selected SQL and keeps the command in history", async () => {
    const fixture = setup();
    act(() => { fixture.store.register("root", { ...fixture.target, readContext: () => ({ query: "private-whole-query", selection: "SELECT * FROM users" }) }); });
    await send("/explain why this join");
    await waitFor(() => expect(fixture.transport.send).toHaveBeenCalledTimes(1));
    const [request, , context] = fixture.transport.send.mock.calls[0];
    expect(context).toMatchObject({ command: "explain", include: { query: false, selection: true, error: false }, editor: { query: null, selection: "SELECT * FROM users" } });
    expect(JSON.stringify(context)).not.toContain("private-whole-query");
    expect(request.messages[0].blocks).toEqual([{ type: "text", text: "/explain why this join" }]);
    expect(screen.getByLabelText("aiChat.commands.explain")).toHaveTextContent("/explain");
    fixture.emit({ type: "done", finish: "stop" });
    await waitFor(() => expect(fixture.transport.save).toHaveBeenCalledTimes(1));
    expect(fixture.transport.save.mock.calls[0][0].messages[0].blocks).toEqual(request.messages[0].blocks);
  });
  it("defaults /fix to the last error and lets the user remove it before sending", async () => {
    const fixture = setup();
    act(() => { fixture.store.register("root", { ...fixture.target, hasError: true, readContext: () => ({ query: "SELECT broken", last_error: "private-error" }) }); });
    openContext();
    await waitFor(() => expect(screen.getByRole("button", { name: "aiChat.context.kinds.error" })).toBeEnabled());
    const input = screen.getByLabelText("aiChat.message");
    fireEvent.change(input, { target: { value: "/fix " } });
    const error = await screen.findByRole("button", { name: "aiChat.context.kinds.error" });
    await waitFor(() => expect(error).toBeEnabled());
    expect(error).toHaveAttribute("aria-pressed", "true");
    fireEvent.click(error);
    fireEvent.click(screen.getByRole("button", { name: "aiChat.send" }));
    await waitFor(() => expect(fixture.transport.send).toHaveBeenCalledTimes(1));
    expect(fixture.transport.send.mock.calls[0][2]).toMatchObject({ command: "fix", last_error: null, include: { error: false } });
  });
  it("handles /help locally without creating history or a model request", async () => {
    const fixture = setup();
    await send("/help");
    expect(screen.getByRole("region", { name: "aiChat.commands.helpTitle" })).toBeInTheDocument();
    expect(screen.getByLabelText("aiChat.message")).toHaveValue("");
    expect(fixture.transport.send).not.toHaveBeenCalled();
    expect(fixture.transport.save).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "common.close" }));
    expect(screen.queryByRole("region", { name: "aiChat.commands.helpTitle" })).not.toBeInTheDocument();
  });
  it("preserves an unknown command for correction without contacting the model", async () => {
    const fixture = setup();
    await send("/unknown args");
    expect(screen.getByRole("alert")).toHaveTextContent("aiChat.commands.unknown");
    expect(screen.getByLabelText("aiChat.message")).toHaveValue("/unknown args");
    expect(fixture.transport.send).not.toHaveBeenCalled();
  });
  it("restores ordinary context defaults when a command is removed", async () => {
    const fixture = setup();
    act(() => { fixture.store.register("root", { ...fixture.target, hasError: true, readContext: () => ({ query: "SELECT 1", last_error: "private-error" }) }); });
    openContext();
    await waitFor(() => expect(screen.getByRole("button", { name: "aiChat.context.kinds.error" })).toBeEnabled());
    fireEvent.change(screen.getByLabelText("aiChat.message"), { target: { value: "/fix " } });
    expect(screen.getByRole("button", { name: "aiChat.context.kinds.error" })).toHaveAttribute("aria-pressed", "true");
    fireEvent.change(screen.getByLabelText("aiChat.message"), { target: { value: "ordinary question" } });
    expect(screen.getByRole("button", { name: "aiChat.context.kinds.error" })).toHaveAttribute("aria-pressed", "false");
  });
  it("mentions a saved query by ID, preserves it through model changes/remounts and omits cached SQL from sends", async () => {
    const id = "ab1f316f-e0c8-4681-9bf3-e56cba339ce0";
    const fixture = setup({ queries: [{ id, name: "Revenue", sql: "private-cached-sql-marker", connection_id: "one", database: "db", created_at: null, updated_at: null }], isLoading: false,
      saveQuery: vi.fn(), updateQuery: vi.fn(), deleteQuery: vi.fn(), refreshQueries: vi.fn() });
    openContext();
    await waitFor(() => expect(screen.getByRole("button", { name: "aiChat.context.kinds.query" })).toBeEnabled());
    const input = screen.getByRole("combobox", { name: "aiChat.message" });
    fireEvent.focus(input);
    fireEvent.change(input, { target: { value: "@Rev" } });
    fireEvent.click(screen.getByRole("option", { name: "Revenue · aiChat.context.kinds.saved_query" }));
    expect(screen.getByRole("button", { name: "aiChat.context.removeReference" })).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("aiChat.model"), { target: { value: JSON.stringify(["anthropic", "other"]) } });
    fixture.rerender(fixture.tree(false));
    fixture.rerender(fixture.tree());
    expect(screen.getByRole("button", { name: "aiChat.context.removeReference" })).toBeInTheDocument();
    await send("Explain the saved query");
    await waitFor(() => expect(fixture.transport.send).toHaveBeenCalledTimes(1));
    const [request, , context] = fixture.transport.send.mock.calls[0];
    expect(request.provider).toBe("anthropic");
    expect(context).toMatchObject({ saved_queries: [id], views: [], connection_id: "one", database: "db" });
    expect(JSON.stringify(context)).not.toContain("private-cached-sql-marker");
  });
  it("takes a fresh editor snapshot at send and preserves removed context across panel remounts", async () => {
    const fixture = setup();
    const readContext = vi.fn(() => ({ query: "SELECT * FROM users", selection: "name", cursor_statement: "SELECT * FROM users", last_error: "private-error" }));
    act(() => { fixture.store.register("root", { ...fixture.target, readContext }); });
    openContext();
    await waitFor(() => expect(screen.getByRole("button", { name: "aiChat.context.kinds.query" })).toBeEnabled());
    fireEvent.click(screen.getByRole("button", { name: "aiChat.context.kinds.query" }));
    fixture.rerender(fixture.tree(false));
    fixture.rerender(fixture.tree());
    openContext();
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
      : command === "ai_chat_preview_context" ? { items: [], truncated: false, total_chars: 1000 }
      : { tools: false, images: false, reasoning: true, context_window: null });
    Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText: vi.fn().mockResolvedValue(undefined) } });
  });

  it("previews table details, keeps exclusions across remounts and sends them on the first turn", async () => {
    const original = vi.mocked(invoke).getMockImplementation()!;
    vi.mocked(invoke).mockImplementation(async (command, args) => command === "ai_chat_preview_context" ? {
      items: [{ kind: "table", id: "users", label: "users", chars: 200 }, { kind: "schema", id: "schema", label: "1 tables", table_count: 1, chars: 100 }],
      truncated: false, total_chars: 1400,
    } : original(command, args));
    const fixture = setup();
    openContext();
    const detail = await screen.findByRole("button", { name: "aiChat.context.tableDetail" });
    expect(detail).toHaveAttribute("aria-pressed", "true");
    fireEvent.click(detail);
    fixture.rerender(fixture.tree(false));
    fixture.rerender(fixture.tree());
    openContext();
    expect(screen.getByRole("button", { name: "aiChat.context.tableDetail" })).toHaveAttribute("aria-pressed", "false");
    await send("Explain users");
    await waitFor(() => expect(fixture.transport.send).toHaveBeenCalledTimes(1));
    expect(fixture.transport.send.mock.calls[0][2]).toMatchObject({ tables: [], excluded_tables: ["users"], connection_id: "one", database: "db", schema: "public" });
    expect(screen.queryByRole("button", { name: "aiChat.context.tableDetail" })).not.toBeInTheDocument();
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
