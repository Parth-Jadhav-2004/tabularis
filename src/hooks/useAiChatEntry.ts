import { useCallback, useContext, useSyncExternalStore } from "react";
import { AiChatContext } from "../contexts/AiChatContext";
import { AiEditorContext } from "../contexts/AiEditorContext";
import { CommandPaletteScopeContext } from "../contexts/CommandPaletteScopeContext";
import { RightSidebarContext } from "../contexts/RightSidebarContext";
import type { AiChatEntry, AiChatEntryOptions, AiEditorTarget } from "../types/aiEditor";
import { ROOT_COMMAND_SCOPE_ID } from "../utils/commandScopeStore";
import { useAiAvailability } from "./useAiAvailability";

const subscribeEmpty = () => () => {};
const emptyEntry = () => null;

export function useAiChatEntry() {
  const store = useContext(AiEditorContext);
  const palette = useContext(CommandPaletteScopeContext);
  const entry = useSyncExternalStore(store?.subscribe ?? subscribeEmpty, store?.getEntry ?? emptyEntry, store?.getEntry ?? emptyEntry);
  const consumeEntry = useCallback((expected: AiChatEntry) => store?.consumeEntry(expected) ?? false, [store]);
  return { entry, consumeEntry, activeScopeId: palette?.activeScopeId ?? ROOT_COMMAND_SCOPE_ID };
}

export function useOpenAiChat(requestedScopeId?: string) {
  const palette = useContext(CommandPaletteScopeContext);
  const scopeId = requestedScopeId ?? palette?.activeScopeId ?? ROOT_COMMAND_SCOPE_ID;
  const store = useContext(AiEditorContext);
  const chat = useContext(AiChatContext);
  const sidebar = useContext(RightSidebarContext);
  const { available } = useAiAvailability();
  const disabled = !available || !store || !sidebar || !chat?.ready || !!chat.activeRun
    || chat.saveState === "saving" || chat.saveState === "error" || chat.history.busy || chat.history.restoring;
  const open = useCallback((command: AiChatEntry["command"], target?: AiEditorTarget, options?: AiChatEntryOptions) => {
    if (disabled || !store || !sidebar) return false;
    const resolved = target ?? store.get(scopeId);
    if (!resolved) return false;
    store.requestEntry(scopeId, resolved, command, options);
    sidebar.setActivePanel("ai-chat");
    if (!sidebar.isOpen) sidebar.toggle();
    return true;
  }, [disabled, scopeId, sidebar, store]);
  return { open, disabled };
}
