import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { OnMount } from "@monaco-editor/react";
import { SqlCellEditor } from "../../../src/components/notebook/SqlCellEditor";
import { AiNotebookContext } from "../../../src/contexts/AiNotebookContext";
import type { AiEditorMenuActions, AiEditorTarget } from "../../../src/types/aiEditor";

const mounted = vi.hoisted(() => ({
  onFocus: () => {}, onDispose: () => {}, disposeFocus: vi.fn(),
  available: false, open: vi.fn(), aiActions: undefined as AiEditorMenuActions | undefined,
}));
vi.mock("../../../src/hooks/useAiAvailability", () => ({ useAiAvailability: () => ({ available: mounted.available }) }));
vi.mock("../../../src/hooks/useAiChatEntry", () => ({ useOpenAiChat: () => ({ disabled: false, open: mounted.open }) }));
vi.mock("../../../src/components/ui/SqlEditorWrapper", () => ({
  SqlEditorWrapper: ({ onMount, aiActions }: { onMount: OnMount; aiActions?: AiEditorMenuActions }) => {
    mounted.aiActions = aiActions;
    return <button type="button" onClick={() => onMount({
    getContentHeight: () => 80,
    onDidContentSizeChange: vi.fn(),
    onDidFocusEditorText: (callback: () => void) => { mounted.onFocus = callback; return { dispose: mounted.disposeFocus }; },
    onDidDispose: (callback: () => void) => { mounted.onDispose = callback; },
  } as unknown as Parameters<OnMount>[0], {} as Parameters<OnMount>[1])}>Mount editor</button>;
  },
}));

describe("notebook SQL editor AI focus bridge", () => {
  beforeEach(() => { mounted.disposeFocus.mockClear(); mounted.available = false; mounted.open.mockClear(); });

  it.each(["explain", "optimize", "ask"] as const)("routes Monaco %s to its own notebook cell", (action) => {
    mounted.available = true;
    const target: AiEditorTarget = { id: "notebook:book:b", connectionId: "connection", database: "db", schema: "sales",
      actions: { insert: vi.fn(), replace: vi.fn(), run: vi.fn(), focus: vi.fn() } };
    const selectCell = vi.fn(() => target);
    render(<AiNotebookContext.Provider value={{ scopeId: "pane", selectCell, registerEditor: vi.fn() }}>
      <SqlCellEditor cellId="b" content="SELECT 1" onContentChange={vi.fn()} onRun={vi.fn()} connectionId="connection" onToggleCollapse={vi.fn()} />
    </AiNotebookContext.Provider>);
    mounted.aiActions?.run(action, { query: "private whole query", selection: "SELECT selected" });
    expect(selectCell).toHaveBeenCalledWith("b");
    expect(mounted.open).toHaveBeenCalledWith(action === "ask" ? null : action, target,
      expect.objectContaining({ include: expect.objectContaining({ query: false, selection: true, cursor_statement: false }) }));
  });

  it("registers Monaco and selects the focused cell using the latest notebook context", () => {
    const registerEditor = vi.fn();
    const selectCell = vi.fn(() => null);
    const replacement = vi.fn(() => null);
    const tree = (select = selectCell) => <AiNotebookContext.Provider value={{ scopeId: "pane", registerEditor, selectCell: select }}>
      <SqlCellEditor cellId="cell-b" content="SELECT 1" onContentChange={vi.fn()} onRun={vi.fn()} connectionId="connection" onToggleCollapse={vi.fn()} />
    </AiNotebookContext.Provider>;
    const fixture = render(tree());
    fireEvent.click(screen.getByRole("button", { name: "Mount editor" }));
    expect(registerEditor).toHaveBeenCalledWith("cell-b", expect.objectContaining({ getContentHeight: expect.any(Function) }));
    mounted.onFocus();
    expect(selectCell).toHaveBeenCalledWith("cell-b");
    fixture.rerender(tree(replacement));
    mounted.onFocus();
    expect(replacement).toHaveBeenCalledWith("cell-b");
    mounted.onDispose();
    expect(mounted.disposeFocus).toHaveBeenCalledTimes(1);
  });

  it("does not require an AI notebook provider when AI is disabled", () => {
    render(<SqlCellEditor cellId="ordinary" content="SELECT 1" onContentChange={vi.fn()} onRun={vi.fn()} connectionId="connection" onToggleCollapse={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "Mount editor" }));
    expect(() => mounted.onFocus()).not.toThrow();
    mounted.onDispose();
    expect(mounted.disposeFocus).toHaveBeenCalledTimes(1);
  });
});
