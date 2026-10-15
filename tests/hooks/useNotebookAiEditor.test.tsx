import { useCallback, useState, type ReactNode } from "react";
import { act, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { editor } from "monaco-editor";
import { AiEditorContext } from "../../src/contexts/AiEditorContext";
import { useAiEditor } from "../../src/hooks/useAiEditor";
import { useNotebookAiEditor } from "../../src/hooks/useNotebookAiEditor";
import { createAiEditorStore } from "../../src/utils/aiEditor";
import type { NotebookCell } from "../../src/types/notebook";

function setup(initialCells: NotebookCell[] = [
  { id: "a", type: "sql", content: "SELECT 1" },
  { id: "b", type: "sql", content: "SELECT broken", schema: "override", error: "missing column" },
]) {
  const store = createAiEditorStore();
  const openConsole = vi.fn();
  const wrapper = ({ children }: { children: ReactNode }) => <AiEditorContext.Provider value={store}>{children}</AiEditorContext.Provider>;
  const rendered = renderHook(({ active = true }: { active?: boolean }) => {
    const [cells, setCells] = useState(initialCells);
    const updateCell = useCallback((id: string, partial: Partial<NotebookCell>) =>
      setCells((items) => items.map((cell) => cell.id === id ? { ...cell, ...partial } : cell)), []);
    const notebook = useNotebookAiEditor({ scopeId: "root", notebookId: "book", connectionId: "connection", database: "db", schema: "public",
      cells, isActive: active, openConsole,
      updateCell,
    });
    return { notebook, cells, setCells, target: useAiEditor() };
  }, { wrapper, initialProps: { active: true } });
  return { ...rendered, store, openConsole };
}

describe("notebook AI editor target", () => {
  it("tracks the selected cell's scope and error and writes to its stable ID after reordering", () => {
    const fixture = setup();
    act(() => { fixture.result.current.notebook.selectCell("b"); });
    expect(fixture.result.current.target).toMatchObject({ id: "notebook:book:b", database: "db", schema: "override", hasError: true });
    expect(fixture.result.current.target?.readContext?.()).toMatchObject({ query: "SELECT broken", last_error: "missing column" });
    act(() => { fixture.result.current.setCells((cells) => [...cells].reverse()); });
    act(() => { fixture.result.current.target?.actions.replace("SELECT fixed"); });
    expect(fixture.result.current.cells.find((cell) => cell.id === "b")?.content).toBe("SELECT fixed");
    expect(fixture.result.current.cells.find((cell) => cell.id === "a")?.content).toBe("SELECT 1");
    expect(fixture.openConsole).not.toHaveBeenCalled();
  });

  it("preserves hidden cell content on Insert and runs only through the supplied console pipeline", () => {
    const fixture = setup();
    act(() => { fixture.result.current.notebook.selectCell("b"); });
    act(() => { fixture.result.current.target?.actions.insert("SELECT 2"); });
    expect(fixture.result.current.cells[1].content).toBe("SELECT broken\nSELECT 2");
    act(() => { fixture.result.current.target?.actions.run("SELECT explicit"); });
    expect(fixture.openConsole).toHaveBeenCalledWith("SELECT explicit", "override");
    expect(fixture.result.current.cells[1].content).toBe("SELECT broken\nSELECT 2");
  });

  it("uses live Monaco selections and undo stops, and falls back to cell state after disposal", () => {
    const fixture = setup();
    let dispose: () => void = () => {};
    const range = { startLineNumber: 1, startColumn: 1, endLineNumber: 1, endColumn: 5, isEmpty: () => false };
    const codeEditor = { getModel: () => ({ getFullModelRange: () => range, getValueInRange: () => "live selection" }),
      getSelection: () => range, getPosition: () => null, getValue: () => "live query", pushUndoStop: vi.fn(), executeEdits: vi.fn(), focus: vi.fn(),
      onDidDispose: (callback: () => void) => { dispose = callback; },
    };
    act(() => { fixture.result.current.notebook.registerEditor("b", codeEditor as unknown as editor.IStandaloneCodeEditor);
      fixture.result.current.notebook.selectCell("b"); });
    expect(fixture.result.current.target?.readContext?.()).toMatchObject({ query: "live query", selection: "live selection" });
    act(() => { fixture.result.current.target?.actions.insert("SELECT inserted"); });
    expect(codeEditor.executeEdits).toHaveBeenCalledWith("ai-assistant", [{ range, text: "SELECT inserted", forceMoveMarkers: true }]);
    expect(codeEditor.pushUndoStop).toHaveBeenCalledTimes(2);
    act(dispose);
    act(() => { fixture.result.current.target?.actions.replace("fallback"); });
    expect(fixture.result.current.cells[1].content).toBe("fallback");
  });

  it("releases inactive and deleted targets and never selects a Markdown cell", () => {
    const fixture = setup();
    act(() => { fixture.result.current.notebook.selectCell("b"); });
    act(() => { fixture.result.current.setCells((cells) => cells.filter((cell) => cell.id !== "b")); });
    expect(fixture.result.current.target?.id).toBe("notebook:book:a");
    fixture.rerender({ active: false });
    expect(fixture.store.get("root")).toBeUndefined();
    act(() => { fixture.result.current.setCells([{ id: "markdown", type: "markdown", content: "Notes" }]); });
    fixture.rerender({ active: true });
    expect(fixture.result.current.target).toBeUndefined();
    expect(fixture.result.current.notebook.selectCell("markdown")).toBeNull();
  });
});
