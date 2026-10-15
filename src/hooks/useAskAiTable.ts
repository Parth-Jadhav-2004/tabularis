import { useCallback, useContext } from "react";
import { AiEditorContext } from "../contexts/AiEditorContext";
import { CommandPaletteScopeContext } from "../contexts/CommandPaletteScopeContext";
import type { TableTarget } from "../types/databaseObjects";
import { aiTableEntry, createAiTableTarget } from "../utils/aiEntryPoints";
import { useOpenAiChat } from "./useAiChatEntry";
import { useAiEditor } from "./useAiEditor";

export function useAskAiTable() {
  const store = useContext(AiEditorContext);
  const palette = useContext(CommandPaletteScopeContext);
  const launcher = useOpenAiChat();
  const activeTarget = useAiEditor();
  const ask = useCallback((table: TableTarget) => {
    if (launcher.disabled || !store || !palette) return false;
    const scope = palette.store.getScope(palette.activeScopeId);
    const base = store.get(palette.activeScopeId);
    if (!scope || scope.connectionId !== table.connectionId || base?.connectionId !== table.connectionId) return false;
    const target = createAiTableTarget(table, base, scope.runtime.openEditor);
    store.setContextTarget(palette.activeScopeId, target);
    return launcher.open(null, target, aiTableEntry(table.tableName));
  }, [launcher, palette, store]);
  return { ask, disabled: launcher.disabled || !palette || !activeTarget };
}
