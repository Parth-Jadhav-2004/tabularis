import { useCallback, useMemo, useState } from "react";
import type { AiEditorTarget } from "../types/aiEditor";
import type { NotebookCell, NotebookParam, RunAllResult } from "../types/notebook";
import type { AiNotebookContextValue } from "../contexts/AiNotebookContext";
import { createAiNotebookEditors, createAiNotebookReadOnlyTarget, createAiNotebookTarget } from "../utils/aiNotebook";
import { getNotebookTitle } from "../utils/notebookStore";
import { useRegisterAiEditor } from "./useAiEditor";

export function useNotebookAiEditor({ scopeId, notebookId, connectionId, database, schema, dialect, cells, title, params, stopOnError, runAllResult, isActive, updateCell, openConsole }: {
  scopeId: string; notebookId: string; connectionId: string; database: string | null; schema: string | null;
  dialect?: string; cells: NotebookCell[]; isActive: boolean;
  title?: string; params?: NotebookParam[]; stopOnError?: boolean; runAllResult?: RunAllResult | null;
  updateCell: (cellId: string, partial: Partial<NotebookCell>) => void;
  openConsole?: (sql: string, schema: string | null) => void;
}): AiNotebookContextValue {
  const [editors] = useState(createAiNotebookEditors);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const selected = cells.find((cell) => cell.id === selectedId) ?? cells.find((cell) => cell.type === "sql") ?? cells[0];
  const notebook = useMemo(() => ({ id: notebookId, readState: () => ({
    id: notebookId, title: getNotebookTitle(notebookId) ?? title ?? "Notebook", cells,
    params: params ?? [], stopOnError: stopOnError ?? false, focusedCellId: selected?.id ?? null,
    runAllResult: runAllResult ?? null,
  }) }), [notebookId, title, cells, params, stopOnError, selected?.id, runAllResult]);
  const buildTarget = useCallback((cellId: string): AiEditorTarget | null => {
    const cell = cells.find((item) => item.id === cellId);
    if (!cell || !connectionId || !isActive) return null;
    const focusedNotebook = { ...notebook, readState: () => ({ ...notebook.readState(), focusedCellId: cellId }) };
    if (cell.type !== "sql") return createAiNotebookReadOnlyTarget({ notebook: focusedNotebook, cellId, connectionId, database, schema, dialect });
    return createAiNotebookTarget({ cell, notebookId, notebook: focusedNotebook, connectionId, database, schema, dialect,
      getEditor: () => editors.get(cellId), updateCell, openConsole });
  }, [cells, connectionId, database, dialect, editors, isActive, notebookId, notebook, openConsole, schema, updateCell]);
  const target = useMemo(() => !connectionId || !isActive ? null : selected ? buildTarget(selected.id)
    : createAiNotebookReadOnlyTarget({ notebook, connectionId, database, schema, dialect }),
  [connectionId, isActive, selected, buildTarget, notebook, database, schema, dialect]);
  useRegisterAiEditor(scopeId, target);
  const selectCell = useCallback((cellId: string) => {
    const next = buildTarget(cellId);
    if (next) setSelectedId(cellId);
    return next;
  }, [buildTarget]);
  const registerEditor = editors.register;
  return useMemo(() => ({ scopeId, selectCell, registerEditor }), [scopeId, selectCell, registerEditor]);
}
