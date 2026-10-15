import type { AiChatEntry, AiEditorStore, AiEditorTarget, AiSqlActions } from "../types/aiEditor";
import type { editor } from "monaco-editor";
import type { AiEditorSnapshot } from "../types/aiContext";
import { findStatementAtOffset, splitBatches } from "./sql";

export function aiEditorTargetKey(target: AiEditorTarget | undefined, scopeId: string): string | undefined {
  return target?.id ? JSON.stringify([scopeId, target.id, target.connectionId, target.database, target.schema]) : undefined;
}

export function readAiEditorContext(codeEditor: editor.IStandaloneCodeEditor | null, fallbackQuery: string | null, dialect?: string, lastError?: string | null): AiEditorSnapshot {
  const model = codeEditor?.getModel();
  const selection = codeEditor?.getSelection();
  const position = codeEditor?.getPosition();
  const query = codeEditor?.getValue() ?? fallbackQuery;
  const selected = model && selection && !selection.isEmpty() ? model.getValueInRange(selection) : null;
  const statement = model && position && query
    ? findStatementAtOffset(splitBatches(query, dialect), model.getOffsetAt(position))?.text ?? null : null;
  return { query, selection: selected, cursor_statement: statement, last_error: lastError || null };
}

export function createAiEditorStore(): AiEditorStore {
  let entry: AiChatEntry | null = null;
  const targets = new Map<string, AiEditorTarget>();
  const contextTargets = new Map<string, AiEditorTarget>();
  const listeners = new Set<() => void>();
  const notify = () => listeners.forEach((listener) => listener());
  return {
    register(scopeId, target) {
      contextTargets.delete(scopeId);
      targets.set(scopeId, target);
      notify();
      return () => {
        if (targets.get(scopeId) !== target) return;
        contextTargets.delete(scopeId);
        targets.delete(scopeId);
        notify();
      };
    },
    get: (scopeId) => contextTargets.get(scopeId) ?? targets.get(scopeId),
    setContextTarget(scopeId, target) {
      contextTargets.set(scopeId, target);
      notify();
    },
    requestEntry(scopeId, target, command, options = {}) {
      entry = { ...options, scopeId, target, command, snapshot: options.snapshot ?? target.readContext?.() ?? {} };
      notify();
    },
    getEntry: () => entry,
    consumeEntry(expected) {
      if (entry !== expected) return false;
      entry = null;
      notify();
      return true;
    },
    subscribe(listener) {
      listeners.add(listener);
      return () => { listeners.delete(listener); };
    },
  };
}

interface AiEditorRuntime {
  getEditor: () => editor.IStandaloneCodeEditor | null;
  replaceState: (sql: string) => void;
  openConsole: (sql: string, run: boolean) => void;
}

// Execution opens a console through the editor's existing navigation pipeline.
// That pipeline handles batches, query parameters and every execution guard.
export function createAiSqlActions(runtime: AiEditorRuntime): AiSqlActions {
  const edit = (sql: string, replace: boolean) => {
    const editor = runtime.getEditor();
    const model = editor?.getModel();
    const range = replace ? model?.getFullModelRange() : editor?.getSelection();
    if (!editor || !model || !range) {
      runtime.openConsole(sql, false);
      return;
    }
    editor.pushUndoStop();
    editor.executeEdits("ai-assistant", [{ range, text: sql, forceMoveMarkers: true }]);
    editor.pushUndoStop();
    // Sync immediately; the Monaco onChange handler is debounced.
    runtime.replaceState(editor.getValue());
    editor.focus();
  };
  return {
    insert: (sql) => edit(sql, false),
    replace: (sql) => edit(sql, true),
    run: (sql) => runtime.openConsole(sql, true),
    focus: () => { runtime.getEditor()?.focus(); },
  };
}
