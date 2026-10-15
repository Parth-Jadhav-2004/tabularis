import { describe, expect, it, vi } from "vitest";
import type { editor } from "monaco-editor";
import { createAiEditorStore, createAiSqlActions } from "../../src/utils/aiEditor";
import type { AiEditorTarget } from "../../src/types/aiEditor";

describe("AI editor commands", () => {
  function fixture() {
    const selection = { startLineNumber: 2, startColumn: 1, endLineNumber: 2, endColumn: 1 };
    const fullRange = { startLineNumber: 1, startColumn: 1, endLineNumber: 3, endColumn: 1 };
    const codeEditor = {
      getModel: () => ({ getFullModelRange: () => fullRange }),
      getSelection: () => selection,
      getValue: () => "SELECT 42;",
      executeEdits: vi.fn(), pushUndoStop: vi.fn(), focus: vi.fn(),
    };
    const runtime = {
      getEditor: () => codeEditor as unknown as editor.IStandaloneCodeEditor,
      replaceState: vi.fn(), openConsole: vi.fn(),
    };
    return { codeEditor, selection, fullRange, runtime, actions: createAiSqlActions(runtime) };
  }

  it("inserts at the Monaco selection between undo stops and syncs state immediately", () => {
    const { codeEditor, selection, runtime, actions } = fixture();
    actions.insert("SELECT 42;");
    expect(codeEditor.executeEdits).toHaveBeenCalledWith("ai-assistant", [{ range: selection, text: "SELECT 42;", forceMoveMarkers: true }]);
    expect(codeEditor.pushUndoStop).toHaveBeenCalledTimes(2);
    expect(runtime.replaceState).toHaveBeenCalledWith("SELECT 42;");
    expect(codeEditor.focus).toHaveBeenCalled();
  });
  it("replaces the complete query with an undoable edit", () => {
    const { codeEditor, fullRange, actions } = fixture();
    actions.replace("SELECT 42;");
    expect(codeEditor.executeEdits.mock.calls[0][1][0].range).toEqual(fullRange);
  });
  it("opens a console without running when no editable Monaco instance exists", () => {
    const runtime = { getEditor: () => null, replaceState: vi.fn(), openConsole: vi.fn() };
    const actions = createAiSqlActions(runtime);
    actions.insert("SELECT 1;");
    actions.replace("SELECT 2;");
    expect(runtime.openConsole.mock.calls).toEqual([["SELECT 1;", false], ["SELECT 2;", false]]);
    expect(runtime.replaceState).not.toHaveBeenCalled();
  });
  it("runs only through the normal console navigation pipeline", () => {
    const { runtime, codeEditor, actions } = fixture();
    actions.run("DELETE FROM users;");
    expect(runtime.openConsole).toHaveBeenCalledWith("DELETE FROM users;", true);
    expect(codeEditor.executeEdits).not.toHaveBeenCalled();
  });
  it("keeps split-pane targets independent and ignores obsolete registrations", () => {
    const store = createAiEditorStore();
    const listener = vi.fn();
    const unlisten = store.subscribe(listener);
    const { actions } = fixture();
    const target: AiEditorTarget = { connectionId: "one", database: null, schema: null, actions };
    const old = store.register("one", target);
    const replacement = { ...target, database: "new" };
    const remove = store.register("one", replacement);
    store.register("two", { ...target, connectionId: "two" });
    old();
    expect(store.get("one")).toBe(replacement);
    expect(store.get("two")?.connectionId).toBe("two");
    remove();
    expect(store.get("one")).toBeUndefined();
    expect(listener).toHaveBeenCalledTimes(4);
    unlisten();
  });
});
