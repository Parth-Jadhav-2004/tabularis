import { useCallback, useContext, useLayoutEffect, useSyncExternalStore } from "react";
import { AiEditorContext } from "../contexts/AiEditorContext";
import { CommandPaletteScopeContext } from "../contexts/CommandPaletteScopeContext";
import type { AiEditorTarget } from "../types/aiEditor";
import { ROOT_COMMAND_SCOPE_ID } from "../utils/commandScopeStore";

const subscribeEmpty = () => () => {};

export function useRegisterAiEditor(scopeId: string, target: AiEditorTarget | null) {
  const store = useContext(AiEditorContext);
  useLayoutEffect(() => {
    if (store && target) return store.register(scopeId, target);
  }, [scopeId, store, target]);
}

export function useAiEditor() {
  const store = useContext(AiEditorContext);
  const scope = useContext(CommandPaletteScopeContext);
  const activeScopeId = scope?.activeScopeId ?? ROOT_COMMAND_SCOPE_ID;
  const getSnapshot = useCallback(() => store?.get(activeScopeId), [activeScopeId, store]);
  return useSyncExternalStore(store?.subscribe ?? subscribeEmpty, getSnapshot, getSnapshot);
}
