import { useCallback, useMemo, useState } from "react";
import type { AiEditorTarget } from "../types/aiEditor";
import type { NotebookCell } from "../types/notebook";
import type { AiNotebookContextValue } from "../contexts/AiNotebookContext";
import { createAiNotebookEditors, createAiNotebookTarget } from "../utils/aiNotebook";
import { useRegisterAiEditor } from "./useAiEditor";

export function useNotebookAiEditor({ scopeId, notebookId, connectionId, database, schema, dialect, cells, isActive, updateCell, openConsole }: {
  scopeId: string; notebookId: string; connectionId: string; database: string | null; schema: string | null;
  dialect?: string; cells: NotebookCell[]; isActive: boolean;
  updateCell: (cellId: string, partial: Partial<NotebookCell>) => void;
  openConsole?: (sql: string, schema: string | null) => void;
}): AiNotebookContextValue {
  const [editors] = useState(createAiNotebookEditors);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const selected = cells.find((cell) => cell.id === selectedId && cell.type === "sql") ?? cells.find((cell) => cell.type === "sql");
  const buildTarget = useCallback((cellId: string): AiEditorTarget | null => {
    const cell = cells.find((item) => item.id === cellId && item.type === "sql");
    if (!cell || !connectionId || !isActive) return null;
    return createAiNotebookTarget({ cell, notebookId, connectionId, database, schema, dialect,
      getEditor: () => editors.get(cellId), updateCell, openConsole });
  }, [cells, connectionId, database, dialect, editors, isActive, notebookId, openConsole, schema, updateCell]);
  const target = useMemo(() => selected && connectionId && isActive
    ? createAiNotebookTarget({ cell: selected, notebookId, connectionId, database, schema, dialect,
      getEditor: () => editors.get(selected.id), updateCell, openConsole }) : null,
  [selected, connectionId, isActive, notebookId, database, schema, dialect, editors, updateCell, openConsole]);
  useRegisterAiEditor(scopeId, target);
  const selectCell = useCallback((cellId: string) => {
    const next = buildTarget(cellId);
    if (next) setSelectedId(cellId);
    return next;
  }, [buildTarget]);
  const registerEditor = editors.register;
  return useMemo(() => ({ scopeId, selectCell, registerEditor }), [scopeId, selectCell, registerEditor]);
}
