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
import type { AiChatEvent, AiChatTransport, AiConversation } from "../../../src/types/aiChat";
import type { AiEditorTarget } from "../../../src/types/aiEditor";
import type { AiNotebookSource } from "../../../src/types/aiNotebook";
import { createAiEditorStore } from "../../../src/utils/aiEditor";
import { createAiConversation, createAiMessage } from "../../../src/utils/aiChat";
import { NOTEBOOKS_CHANGED_EVENT } from "../../../src/utils/notebookStore";

vi.mock("lucide-react", () => vi.importActual("lucide-react"));
vi.mock("../../../src/hooks/useSettings", () => ({
  useSettings: () => ({ settings: { ...DEFAULT_SETTINGS, aiEnabled: true, aiProvider: "openai", aiModel: "example" } }),
}));

function conversation(title: string, notebookId?: string): AiConversation {
  return { ...createAiConversation({ connection_id: "connection", database: "db", schema: "public", provider: "openai", model: "example",
    target: notebookId ? { type: "notebook", notebook_id: notebookId } : null }), title,
    messages: [createAiMessage("user", [{ type: "text", text: title }]), createAiMessage("assistant", [{ type: "text", text: "```sql\nSELECT 1;\n```" }])] };
}
function setup(chats: AiConversation[] = [], notebookId?: string) {
  let receive: (event: AiChatEvent) => void = () => {};
  const history = {
    list: vi.fn<NonNullable<AiChatTransport["history"]>["list"]>(async (id) => chats.filter((chat) => id === null || chat.connection_id === id)
      .map((chat) => ({ ...chat, message_count: chat.messages.length }))),
    load: vi.fn<NonNullable<AiChatTransport["history"]>["load"]>(async (connectionId, id) => structuredClone(chats.find((chat) => chat.connection_id === connectionId && chat.id === id)!)),
    rename: vi.fn(async () => {}), delete: vi.fn(async () => {}),
  };
  const transport = { history,
    listen: async (_id, callback) => { receive = callback; return () => {}; }, listenStorageCleared: async () => () => {},
    send: vi.fn<AiChatTransport["send"]>(async (_request, id) => id), cancel: vi.fn(async () => {}),
    save: vi.fn<AiChatTransport["save"]>(async () => ({ bytes: 100, large: false })),
  } satisfies AiChatTransport;
  const actions = { insert: vi.fn(), replace: vi.fn(), run: vi.fn(), focus: vi.fn() };
  const target = (id?: string): AiEditorTarget => ({ id: id ? `notebook:${id}:sql` : "editor", connectionId: "connection", database: "db", schema: "public", actions,
    notebook: id ? { id, readState: vi.fn<() => AiNotebookSource>(() => ({ id, title: id, cells: [{ id: "sql", type: "sql", content: "SELECT 1", result: { columns: [], rows: [["private-result-marker"]], affected_rows: 0 } }], params: [], stopOnError: false, focusedCellId: "sql", runAllResult: null })) } : undefined,
    readContext: () => ({ query: "SELECT 1" }),
  });
  const store = createAiEditorStore();
  const initialTarget = target(notebookId);
  store.register("root", initialTarget);
  const rendered = render(<MemoryRouter><AiAvailabilityContext.Provider value={{ available: true, loading: false, refresh: vi.fn(), providers: [
    { provider: "openai", label: "OpenAI", source: "env", models_cached: true, usable: true, reason: null },
  ] }}>
    <AiEditorContext.Provider value={store}><RightSidebarProvider><AiChatProvider transport={transport}><AiChatPanel /></AiChatProvider></RightSidebarProvider></AiEditorContext.Provider>
  </AiAvailabilityContext.Provider></MemoryRouter>);
  return { ...rendered, history, transport, store, actions, initialTarget,
    switchTo: (id?: string) => act(() => { store.register("root", target(id)); }),
    emit: (event: AiChatEvent) => act(() => receive(event)),
  };
}

describe("notebook-bound chat UI", () => {
  beforeEach(() => {
    localStorage.clear();
    vi.mocked(invoke).mockReset().mockImplementation(async (command) => command === "get_ai_models" ? { openai: ["example"] }
      : command === "list_notebooks" ? [{ id: "one", title: "Revenue" }, { id: "two", title: "Costs" }]
      : command === "ai_chat_preview_context" ? { items: [], truncated: false, total_chars: 100 }
      : { tools: false, images: false, reasoning: false, context_window: null });
  });

  it("reopens a notebook chat and defaults history to This notebook", async () => {
    const f = setup([conversation("Editor chat"), conversation("Revenue chat", "one"), conversation("Costs chat", "two")], "one");
    await screen.findByText("Revenue chat");
    expect(screen.queryByText("Costs chat")).not.toBeInTheDocument();
    await waitFor(() => expect(f.history.load).toHaveBeenCalledOnce());
    fireEvent.click(screen.getByRole("button", { name: "aiChat.history.title" }));
    expect(screen.getByLabelText("aiChat.history.filter")).toHaveValue("notebook");
    await screen.findByRole("button", { name: /Revenue chat/ });
    expect(screen.queryByRole("button", { name: /Editor chat|Costs chat/ })).not.toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("aiChat.history.filter"), { target: { value: "current" } });
    await screen.findByRole("button", { name: /Editor chat/ });
    expect(screen.getByRole("button", { name: /Costs chat/ })).toBeInTheDocument();
    f.switchTo("two");
    await waitFor(() => expect(screen.getByLabelText("aiChat.history.filter")).toHaveValue("notebook"));
    await screen.findByRole("button", { name: /Costs chat/ });
    expect(screen.queryByRole("button", { name: /Revenue chat/ })).not.toBeInTheDocument();
  });

  it("persists notebook binding, isolates transcripts and does not serialize local notebook results", async () => {
    const f = setup([], "one");
    fireEvent.change(screen.getByLabelText("aiChat.message"), { target: { value: "First notebook question" } });
    await waitFor(() => expect(screen.getByRole("button", { name: "aiChat.send" })).toBeEnabled());
    fireEvent.click(screen.getByRole("button", { name: "aiChat.send" }));
    await waitFor(() => expect(f.transport.send).toHaveBeenCalledOnce());
    f.emit({ type: "done", finish: "stop" });
    await waitFor(() => expect(f.transport.save).toHaveBeenCalledOnce());
    expect(f.transport.save.mock.calls[0][0].target).toEqual({ type: "notebook", notebook_id: "one" });
    expect(JSON.stringify(f.transport.send.mock.calls)).not.toContain("private-result-marker");
    expect(JSON.stringify(f.transport.save.mock.calls)).not.toContain("private-result-marker");
    fireEvent.change(screen.getByLabelText("aiChat.message"), { target: { value: "First notebook draft" } });
    f.switchTo("two");
    await waitFor(() => expect(screen.queryByText("First notebook question")).not.toBeInTheDocument());
    expect(screen.getByLabelText("aiChat.message")).toHaveValue("");
    f.switchTo("one");
    await screen.findByText("First notebook question");
    expect(screen.getByLabelText("aiChat.message")).toHaveValue("First notebook draft");
  });

  it("keeps a missing notebook conversation readable and disables actions in a different notebook", async () => {
    const missing = conversation("Archived notebook chat", "deleted");
    const f = setup([missing], "one");
    await waitFor(() => expect(screen.getByRole("button", { name: "aiChat.newChat" })).toBeEnabled());
    fireEvent.click(screen.getByRole("button", { name: "aiChat.history.title" }));
    fireEvent.change(screen.getByLabelText("aiChat.history.filter"), { target: { value: "current" } });
    const archived = await screen.findByRole("button", { name: /Archived notebook chat/ });
    await screen.findByText("aiChat.history.notebookUnavailable");
    fireEvent.click(archived);
    await screen.findByText("Archived notebook chat");
    expect(screen.getByText("aiChat.history.notebookUnavailable")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "aiChat.run" })).toBeDisabled();
    fireEvent.change(screen.getByLabelText("aiChat.message"), { target: { value: "Do not send" } });
    expect(screen.getByRole("button", { name: "aiChat.send" })).toBeDisabled();
    expect(f.transport.send).not.toHaveBeenCalled();
    expect(vi.mocked(invoke).mock.calls.some(([command]) => command === "load_notebook")).toBe(false);
    fireEvent.click(screen.getByRole("button", { name: "aiChat.newChat" }));
    expect(screen.queryByText("aiChat.history.notebookUnavailable")).not.toBeInTheDocument();
    expect(screen.queryByText("Archived notebook chat")).not.toBeInTheDocument();
  });

  it("updates notebook availability after deletion while preserving encrypted chat history", async () => {
    const f = setup([conversation("Keep this chat", "one")], "one");
    await screen.findByText("Keep this chat");
    const original = vi.mocked(invoke).getMockImplementation()!;
    vi.mocked(invoke).mockImplementation(async (command, args) => command === "list_notebooks" ? [] : original(command, args));
    act(() => { window.dispatchEvent(new Event(NOTEBOOKS_CHANGED_EVENT)); });
    await screen.findByText("aiChat.history.notebookUnavailable");
    expect(screen.getByText("Keep this chat")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "aiChat.run" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "aiChat.newChat" })).toBeDisabled();
    expect(f.history.delete).not.toHaveBeenCalled();
  });

  it("disables SQL actions while a Markdown cell is focused", async () => {
    const f = setup([conversation("Notebook chat", "one")], "one");
    await screen.findByText("Notebook chat");
    act(() => { f.store.register("root", { ...f.initialTarget, id: "notebook:one:notes", sqlActionsAvailable: false }); });
    expect(screen.getByRole("button", { name: "aiChat.run" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "aiChat.insert" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "aiChat.replace" })).toBeDisabled();
    expect(f.actions.replace).not.toHaveBeenCalled();
  });

  it("claims a notebook entry once after restoration and preserves the notebook binding", async () => {
    const f = setup([conversation("Restored notebook chat", "one")], "one");
    act(() => { f.store.requestEntry("root", f.initialTarget, "explain", { snapshot: { query: "SELECT fresh_entry" } }); });
    await waitFor(() => expect(f.transport.send).toHaveBeenCalledOnce());
    expect(f.transport.send.mock.calls[0][0].messages[0].blocks).toEqual([{ type: "text", text: "Restored notebook chat" }]);
    expect(f.transport.send.mock.calls[0][2]).toMatchObject({ command: "explain", editor: { query: null },
      notebook: { focused: { content: "SELECT fresh_entry" } } });
    f.emit({ type: "done", finish: "stop" });
    await waitFor(() => expect(f.transport.save).toHaveBeenCalledOnce());
    expect(f.transport.save.mock.calls[0][0].target).toEqual({ type: "notebook", notebook_id: "one" });
  });

  it("previews notebook context and reads the current cells again when sending", async () => {
    const f = setup([], "one");
    fireEvent.click(screen.getByRole("button", { name: "aiChat.context.options" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "aiChat.context.kinds.notebook" })).toBeEnabled());
    await waitFor(() => expect(vi.mocked(invoke).mock.calls.some(([command, args]) => command === "ai_chat_preview_context"
      && (args as { context?: { notebook?: { id: string } } })?.context?.notebook?.id === "one")).toBe(true));
    expect(screen.queryByRole("button", { name: "aiChat.context.kinds.query" })).not.toBeInTheDocument();
    const readState = vi.mocked(f.initialTarget.notebook!.readState!);
    const original = readState();
    readState.mockReturnValue({ ...original, title: "Fresh title", cells: [{ id: "sql", type: "sql", content: "SELECT fresh_snapshot" }] });
    fireEvent.change(screen.getByLabelText("aiChat.message"), { target: { value: "Explain this notebook" } });
    fireEvent.click(screen.getByRole("button", { name: "aiChat.send" }));
    await waitFor(() => expect(f.transport.send).toHaveBeenCalledOnce());
    expect(f.transport.send.mock.calls[0][2]).toMatchObject({ notebook: { title: "Fresh title", focused: { content: "SELECT fresh_snapshot" }, results: [] } });
  });

  it("sends results only after explicit inclusion and resets that choice for the next message", async () => {
    const f = setup([], "one");
    fireEvent.click(screen.getByRole("button", { name: "aiChat.context.options" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "aiChat.context.kinds.notebook_results" })).toBeEnabled());
    const readState = vi.mocked(f.initialTarget.notebook!.readState!);
    const original = readState();
    readState.mockReturnValue({ ...original, cells: [{ ...original.cells[0], result: { columns: ["value"], rows: [["private-result-marker"]], affected_rows: 0 } }] });
    const chip = screen.getByRole("button", { name: "aiChat.context.kinds.notebook_results" });
    expect(chip).toHaveAttribute("aria-pressed", "false");
    fireEvent.click(chip);
    expect(chip).toHaveAttribute("aria-pressed", "true");
    fireEvent.change(screen.getByLabelText("aiChat.message"), { target: { value: "Summarize these results" } });
    fireEvent.click(screen.getByRole("button", { name: "aiChat.send" }));
    await waitFor(() => expect(f.transport.send).toHaveBeenCalledOnce());
    expect(f.transport.send.mock.calls[0][2]?.notebook?.results[0].rows).toEqual([["private-result-marker"]]);
    expect(chip).toHaveAttribute("aria-pressed", "false");
    f.emit({ type: "done", finish: "stop" });
    await waitFor(() => expect(f.transport.save).toHaveBeenCalledOnce());
    expect(JSON.stringify(f.transport.save.mock.calls)).not.toContain("private-result-marker");
    fireEvent.change(screen.getByLabelText("aiChat.message"), { target: { value: "Another question" } });
    fireEvent.click(screen.getByRole("button", { name: "aiChat.send" }));
    await waitFor(() => expect(f.transport.send).toHaveBeenCalledTimes(2));
    expect(f.transport.send.mock.calls[1][2]?.notebook?.results).toEqual([]);
  });

  it("removes the entire notebook layer and disables result inclusion with it", async () => {
    const f = setup([], "one");
    fireEvent.click(screen.getByRole("button", { name: "aiChat.context.options" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "aiChat.context.kinds.notebook" })).toBeEnabled());
    fireEvent.click(screen.getByRole("button", { name: "aiChat.context.kinds.notebook" }));
    expect(screen.getByRole("button", { name: "aiChat.context.kinds.notebook_results" })).toBeDisabled();
    fireEvent.change(screen.getByLabelText("aiChat.message"), { target: { value: "General SQL question" } });
    fireEvent.click(screen.getByRole("button", { name: "aiChat.send" }));
    await waitFor(() => expect(f.transport.send).toHaveBeenCalledOnce());
    expect(f.transport.send.mock.calls[0][2]?.notebook).toBeUndefined();
    expect(f.transport.send.mock.calls[0][2]?.editor.query).toBeNull();
  });
});
