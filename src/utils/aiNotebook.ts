import type { editor } from "monaco-editor";
import type { AiEditorTarget } from "../types/aiEditor";
import type { NotebookCell } from "../types/notebook";
import type { AiNotebookTarget } from "../types/aiNotebook";
import { createAiSqlActions, readAiEditorContext } from "./aiEditor";

export function createAiNotebookEditors() {
  const editors = new Map<string, editor.IStandaloneCodeEditor>();
  return {
    get: (cellId: string) => editors.get(cellId) ?? null,
    register(cellId: string, codeEditor: editor.IStandaloneCodeEditor) {
      editors.set(cellId, codeEditor);
      codeEditor.onDidDispose(() => {
        if (editors.get(cellId) === codeEditor) editors.delete(cellId);
      });
    },
  };
}

export function createAiNotebookTarget({ cell, notebookId, notebook, connectionId, database, schema, dialect, getEditor, updateCell, openConsole }: {
  cell: NotebookCell; notebookId: string; connectionId: string; database: string | null; schema: string | null; dialect?: string;
  notebook?: AiNotebookTarget;
  getEditor: () => editor.IStandaloneCodeEditor | null;
  updateCell: (cellId: string, partial: Partial<NotebookCell>) => void;
  openConsole?: (sql: string, schema: string | null) => void;
}): AiEditorTarget {
  const effectiveSchema = cell.schema || schema;
  const actions = createAiSqlActions({ getEditor,
    replaceState: (content) => updateCell(cell.id, { content }),
    openConsole: (sql, run) => { if (run) openConsole?.(sql, effectiveSchema); },
  });
  const edit = (sql: string, replace: boolean) => {
    if (getEditor()?.getModel()) {
      if (replace) actions.replace(sql);
      else actions.insert(sql);
    } else {
      // Hidden/collapsed cells still receive edits through notebook history.
      updateCell(cell.id, { content: replace ? sql : [cell.content, sql].filter(Boolean).join("\n") });
    }
  };
  return { id: `notebook:${notebookId}:${cell.id}`, connectionId, database, schema: effectiveSchema, dialect,
    notebook: notebook ?? { id: notebookId },
    hasError: !!cell.error,
    readContext: () => readAiEditorContext(getEditor(), cell.content, dialect, cell.error),
    actions: { ...actions, insert: (sql) => edit(sql, false), replace: (sql) => edit(sql, true) },
  };
}

export function createAiNotebookReadOnlyTarget({ notebook, cellId, connectionId, database, schema, dialect }: {
  notebook: AiNotebookTarget; cellId?: string; connectionId: string; database: string | null; schema: string | null; dialect?: string;
}): AiEditorTarget {
  return { id: `notebook:${notebook.id}:${cellId ?? ""}`, notebook, connectionId, database, schema, dialect,
    sqlActionsAvailable: false, readContext: () => ({}),
    actions: { insert: () => {}, replace: () => {}, run: () => {}, focus: () => {} } };
}
