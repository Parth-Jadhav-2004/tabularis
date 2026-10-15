import { StrictMode } from "react";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { AiChatPanel } from "../../../src/components/ai-chat/AiChatPanel";
import { QueryErrorDisplay } from "../../../src/components/ai-chat/QueryErrorDisplay";
import { AiDropdownButton } from "../../../src/components/ui/AiDropdownButton";
import { NotebookAiButtons } from "../../../src/components/notebook/NotebookAiButtons";
import { AiChatProvider } from "../../../src/contexts/AiChatProvider";
import { AiAvailabilityContext } from "../../../src/contexts/AiAvailabilityContext";
import { AiEditorContext } from "../../../src/contexts/AiEditorContext";
import { AiNotebookContext } from "../../../src/contexts/AiNotebookContext";
import { CommandPaletteScopeContext } from "../../../src/contexts/CommandPaletteScopeContext";
import { RightSidebarProvider } from "../../../src/contexts/RightSidebarProvider";
import { DEFAULT_SETTINGS } from "../../../src/contexts/SettingsContext";
import { useOpenAiChat } from "../../../src/hooks/useAiChatEntry";
import { useAiEditor } from "../../../src/hooks/useAiEditor";
import { useAskAiTable } from "../../../src/hooks/useAskAiTable";
import { useRightSidebar } from "../../../src/hooks/useRightSidebar";
import { createAiEditorStore } from "../../../src/utils/aiEditor";
import { createCommandScopeStore } from "../../../src/utils/commandScopeStore";
import { aiSelectionEntry } from "../../../src/utils/aiEntryPoints";
import type { AiChatEvent, AiChatTransport, AiConversationSummary } from "../../../src/types/aiChat";
import type { AiEditorTarget } from "../../../src/types/aiEditor";
import type { AiAvailability } from "../../../src/types/aiAvailability";

vi.mock("lucide-react", () => vi.importActual("lucide-react"));
vi.mock("../../../src/hooks/useSettings", () => ({
  useSettings: () => ({ settings: { ...DEFAULT_SETTINGS, aiEnabled: true, aiProvider: "openai", aiModel: "example" } }),
}));

function EditorButtons() {
  const launcher = useOpenAiChat("root");
  const target = useAiEditor();
  const table = useAskAiTable();
  const { t } = useTranslation();
  return <><AiDropdownButton onGenerate={() => launcher.open("generate")}
    onExplain={() => launcher.open("explain")} disableAll={launcher.disabled} />
    <QueryErrorDisplay error={"missing column\n\nserver detail"} connectionId="one" t={t} />
    <button type="button" disabled={launcher.disabled} onClick={() => launcher.open(null, target, aiSelectionEntry(target?.readContext?.() ?? {}))}>Ask selection</button>
    <button type="button" disabled={table.disabled} onClick={() => table.ask({ connectionId: "one", tableName: "orders", database: "reporting", schema: "sales" })}>Ask table</button>
    <button type="button" disabled={table.disabled} onClick={() => table.ask({ connectionId: "other", tableName: "orders", database: "reporting", schema: "sales" })}>Ask foreign table</button>
  </>;
}

function Panel() {
  const sidebar = useRightSidebar();
  return sidebar.isOpen && sidebar.activePanel === "ai-chat" ? <AiChatPanel /> : null;
}

function setup(history?: AiChatTransport["history"], initialCommand?: "explain") {
  const listeners = new Map<string, (event: AiChatEvent) => void>();
  const transport = {
    history,
    listen: vi.fn<AiChatTransport["listen"]>(async (id, callback) => { listeners.set(id, callback); return () => { listeners.delete(id); }; }),
    listenStorageCleared: vi.fn<AiChatTransport["listenStorageCleared"]>(async () => () => {}),
    send: vi.fn<AiChatTransport["send"]>(async (_request, id) => id),
    save: vi.fn<AiChatTransport["save"]>(async () => ({ bytes: 123, large: false })),
    cancel: vi.fn<AiChatTransport["cancel"]>(async () => {}),
  } satisfies AiChatTransport;
  const actions = { insert: vi.fn(), replace: vi.fn(), run: vi.fn(), focus: vi.fn() };
  const readContext = vi.fn(() => ({ query: "private-full-query", selection: "SELECT * FROM selected_table" }));
  const target: AiEditorTarget = { id: "first", connectionId: "one", database: "db", schema: "public", readContext, actions };
  const store = createAiEditorStore();
  const scopes = createCommandScopeStore();
  const openEditor = vi.fn();
  scopes.registerScope("root", { connectionId: "one", driver: "postgres", table: null, runtime: { navigate: vi.fn(), openEditor } });
  store.register("root", target);
  if (initialCommand) store.requestEntry("root", target, initialCommand);
  const availability: AiAvailability = { available: true, loading: false, refresh: vi.fn(), providers: [
    { provider: "openai", label: "OpenAI", source: "env", models_cached: true, usable: true, reason: null },
  ] };
  const cellTarget = { ...target, id: "notebook:book:cell", schema: "cell_db" };
  const selectCell = vi.fn(() => {
    store.register("root", cellTarget);
    return cellTarget;
  });
  const tree = (scopeId = "root", available = true) => <StrictMode><MemoryRouter>
    <AiAvailabilityContext.Provider value={{ ...availability, available }}><AiEditorContext.Provider value={store}>
      <CommandPaletteScopeContext.Provider value={{ activeScopeId: scopeId, store: scopes }}>
        <RightSidebarProvider><AiChatProvider transport={transport}>
          <EditorButtons />
          <AiNotebookContext.Provider value={{ scopeId: "root", selectCell, registerEditor: vi.fn() }}>
            <div data-testid="notebook"><NotebookAiButtons cellId="cell" content="SELECT 1" connectionId="one" /></div>
          </AiNotebookContext.Provider>
          {initialCommand ? <AiChatPanel /> : <Panel />}
        </AiChatProvider></RightSidebarProvider>
      </CommandPaletteScopeContext.Provider>
    </AiEditorContext.Provider></AiAvailabilityContext.Provider>
  </MemoryRouter></StrictMode>;
  const rendered = render(tree());
  const emit = (event: AiChatEvent) => {
    const id = transport.send.mock.calls.at(-1)?.[1];
    if (id) act(() => listeners.get(id)?.(event));
  };
  return { ...rendered, tree, target, cellTarget, actions, readContext, selectCell, store, transport, emit, openEditor };
}

async function choose(name: string, notebook = false) {
  const buttons = await screen.findAllByRole("button", { name: "aiChat.commands.title" });
  await waitFor(() => expect(buttons[notebook ? 1 : 0]).toBeEnabled());
  fireEvent.click(buttons[notebook ? 1 : 0]);
  fireEvent.click(screen.getByRole("button", { name }));
}

describe("AI sidebar entry points", () => {
  beforeEach(() => {
    localStorage.clear();
    vi.mocked(invoke).mockReset().mockImplementation(async (command) => command === "get_ai_models"
      ? { openai: ["example"] }
      : command === "ai_chat_preview_context" ? { items: [], truncated: false, total_chars: 0 }
      : { tools: false, images: false, reasoning: false, context_window: null });
  });

  it("Fix with AI uses the failed SQL and full error instead of the current draft or selection", async () => {
    const fixture = setup();
    act(() => { fixture.store.register("root", { ...fixture.target, readErrorContext: () => ({ query: "SELECT original_failure" }) }); });
    const fix = screen.getByRole("button", { name: "aiChat.entryPoints.fix" });
    await waitFor(() => expect(fix).toBeEnabled());
    fireEvent.click(fix);
    await waitFor(() => expect(fixture.transport.send).toHaveBeenCalledTimes(1));
    const context = fixture.transport.send.mock.calls[0][2];
    expect(context).toMatchObject({ command: "fix", editor: { query: "SELECT original_failure", selection: null, cursor_statement: null },
      last_error: "missing column\n\nserver detail", include: { query: true, error: true, selection: false } });
    expect(fixture.actions.run).not.toHaveBeenCalled();
  });

  it("Ask about selection opens a focused empty composer without a provider call", async () => {
    const fixture = setup();
    const ask = screen.getByRole("button", { name: "Ask selection" });
    await waitFor(() => expect(ask).toBeEnabled());
    fireEvent.click(ask);
    await waitFor(() => expect(screen.getByLabelText("aiChat.message")).toHaveFocus());
    expect(screen.getByLabelText("aiChat.message")).toHaveValue("");
    expect(screen.getByRole("button", { name: "aiChat.context.kinds.query" })).toHaveAttribute("aria-pressed", "false");
    expect(screen.getByRole("button", { name: "aiChat.context.kinds.selection" })).toHaveAttribute("aria-pressed", "true");
    expect(fixture.transport.send).not.toHaveBeenCalled();
    fireEvent.change(screen.getByLabelText("aiChat.message"), { target: { value: "What does this do?" } });
    fireEvent.click(screen.getByRole("button", { name: "aiChat.send" }));
    await waitFor(() => expect(fixture.transport.send).toHaveBeenCalledTimes(1));
    expect(fixture.transport.send.mock.calls[0][2]?.editor).toMatchObject({ query: null, selection: "SELECT * FROM selected_table" });
  });

  it("Ask about a table keeps its chip and explicit database/schema until sending", async () => {
    const fixture = setup();
    const ask = screen.getByRole("button", { name: "Ask table" });
    await waitFor(() => expect(ask).toBeEnabled());
    fireEvent.click(ask);
    await waitFor(() => expect(screen.getByLabelText("aiChat.message")).toHaveFocus());
    expect(screen.getByRole("button", { name: "aiChat.context.tableDetail" })).toBeInTheDocument();
    expect(fixture.transport.send).not.toHaveBeenCalled();
    fireEvent.change(screen.getByLabelText("aiChat.message"), { target: { value: "Explain this table" } });
    fireEvent.click(screen.getByRole("button", { name: "aiChat.send" }));
    await waitFor(() => expect(fixture.transport.send).toHaveBeenCalledTimes(1));
    expect(fixture.transport.send.mock.calls[0][2]).toMatchObject({ connection_id: "one", database: "reporting", schema: "sales",
      tables: ["orders"], editor: { query: null, selection: null } });
    fixture.emit({ type: "text_delta", text: "```sql\nSELECT 42;\n```" });
    fixture.emit({ type: "done", finish: "stop" });
    await waitFor(() => expect(screen.getByRole("button", { name: "aiChat.insert" })).toBeEnabled());
    fireEvent.click(screen.getByRole("button", { name: "aiChat.insert" }));
    expect(fixture.openEditor).toHaveBeenCalledWith(expect.objectContaining({ database: "reporting", schema: "sales", preventAutoRun: true }));
    act(() => { fixture.store.register("root", fixture.target); });
    expect(screen.getByRole("button", { name: "aiChat.insert" })).toBeDisabled();
  });

  it("rejects table requests for a different connection before changing the current target", async () => {
    const fixture = setup();
    const ask = screen.getByRole("button", { name: "Ask foreign table" });
    await waitFor(() => expect(ask).toBeEnabled());
    fireEvent.click(ask);
    expect(fixture.store.get("root")).toBe(fixture.target);
    expect(fixture.store.getEntry()).toBeNull();
    expect(screen.queryByLabelText("aiChat.message")).not.toBeInTheDocument();
  });

  it("opens Generate with a focused draft and no model call, SQL replacement or execution", async () => {
    const fixture = setup();
    expect(screen.queryByLabelText("aiChat.message")).not.toBeInTheDocument();
    await choose("ai.generateSql");
    await waitFor(() => expect(screen.getByLabelText("aiChat.message")).toHaveValue("/generate "));
    expect(screen.getByLabelText("aiChat.message")).toHaveFocus();
    expect(fixture.transport.send).not.toHaveBeenCalled();
    expect(fixture.actions.replace).not.toHaveBeenCalled();
    expect(fixture.actions.run).not.toHaveBeenCalled();
    fireEvent.change(screen.getByLabelText("aiChat.message"), { target: { value: "/generate list users" } });
    fireEvent.click(screen.getByRole("button", { name: "aiChat.send" }));
    await waitFor(() => expect(fixture.transport.send).toHaveBeenCalledTimes(1));
    expect(fixture.transport.send.mock.calls[0][2]).toMatchObject({ command: "generate", include: { query: true, schema: true } });
    expect(fixture.actions.run).not.toHaveBeenCalled();
  });

  it("sends Explain once after panel mounting, using the selection captured at the click", async () => {
    const fixture = setup();
    await choose("ai.explain");
    fixture.readContext.mockReturnValue({ query: "later-query", selection: "later-selection" });
    await waitFor(() => expect(fixture.transport.send).toHaveBeenCalledTimes(1));
    const [request, , context] = fixture.transport.send.mock.calls[0];
    expect(request.messages[0].blocks).toEqual([{ type: "text", text: "/explain" }]);
    expect(context).toMatchObject({ connection_id: "one", database: "db", schema: "public", command: "explain",
      include: { query: false, selection: true }, editor: { query: null, selection: "SELECT * FROM selected_table" } });
    expect(JSON.stringify(request)).not.toContain("editorTarget");
    expect(JSON.stringify(context)).not.toContain("private-full-query");
    expect(fixture.store.getEntry()).toBeNull();
  });

  it("waits for history restoration and cannot replay a claimed request in StrictMode", async () => {
    let release: (items: AiConversationSummary[]) => void = () => {};
    const pending = new Promise<AiConversationSummary[]>((resolve) => { release = resolve; });
    const history = { list: vi.fn(() => pending), load: vi.fn(), rename: vi.fn(), delete: vi.fn() };
    const fixture = setup(history, "explain");
    await waitFor(() => expect(history.list).toHaveBeenCalled());
    expect(fixture.transport.send).not.toHaveBeenCalled();
    await act(async () => { release([]); });
    await waitFor(() => expect(fixture.transport.send).toHaveBeenCalledTimes(1));
    fixture.emit({ type: "done", finish: "stop" });
    await waitFor(() => expect(fixture.transport.save).toHaveBeenCalledTimes(1));
    fixture.rerender(fixture.tree());
    expect(fixture.transport.send).toHaveBeenCalledTimes(1);
  });

  it("discards pending entry points after a pane switch", async () => {
    let release: (items: AiConversationSummary[]) => void = () => {};
    const pending = new Promise<AiConversationSummary[]>((resolve) => { release = resolve; });
    const fixture = setup({ list: vi.fn(() => pending), load: vi.fn(), rename: vi.fn(), delete: vi.fn() }, "explain");
    await waitFor(() => expect(screen.getByText("common.loading")).toBeInTheDocument());
    act(() => fixture.store.register("other-pane", { ...fixture.target, id: "other" }));
    fixture.rerender(fixture.tree("other-pane"));
    await act(async () => { release([]); });
    await waitFor(() => expect(fixture.store.getEntry()).toBeNull());
    expect(fixture.transport.send).not.toHaveBeenCalled();
  });

  it("uses the notebook button's cell scope and prevents applying its reply to a different cell", async () => {
    const fixture = setup();
    await choose("ai.explain", true);
    expect(fixture.selectCell).toHaveBeenCalledWith("cell");
    await waitFor(() => expect(fixture.transport.send).toHaveBeenCalledTimes(1));
    expect(fixture.transport.send.mock.calls[0][2]).toMatchObject({ schema: "cell_db", command: "explain" });
    fixture.emit({ type: "text_delta", text: "```sql\nSELECT 42;\n```" });
    fixture.emit({ type: "done", finish: "stop" });
    await waitFor(() => expect(screen.getByRole("button", { name: "aiChat.insert" })).toBeEnabled());
    act(() => fixture.store.register("root", { ...fixture.cellTarget, id: "notebook:book:other-cell" }));
    expect(screen.getByRole("button", { name: "aiChat.insert" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "aiChat.replace" })).toBeDisabled();
    act(() => fixture.store.register("root", fixture.cellTarget));
    fireEvent.click(screen.getByRole("button", { name: "aiChat.insert" }));
    expect(fixture.actions.insert).toHaveBeenCalledWith("SELECT 42;");
  });

  it("disables entry points while streaming and when AI becomes unavailable", async () => {
    const fixture = setup();
    await choose("ai.explain");
    await waitFor(() => expect(fixture.transport.send).toHaveBeenCalledTimes(1));
    expect(screen.getAllByRole("button", { name: "aiChat.commands.title" }).every((button) => button.hasAttribute("disabled"))).toBe(true);
    fixture.emit({ type: "done", finish: "stop" });
    fixture.rerender(fixture.tree("root", false));
    expect(screen.getAllByRole("button", { name: "aiChat.commands.title" }).every((button) => button.hasAttribute("disabled"))).toBe(true);
  });
});
