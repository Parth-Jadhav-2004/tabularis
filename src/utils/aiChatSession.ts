import type {
  AiChatActions, AiChatEvent, AiChatState, AiChatTransport, AiConversation, AiConversationSummary, AiResumeOptions, AiSendOptions,
} from "../types/aiChat";
import { aiChatReducer, createAiChatState, createAiConversation, createAiMessage, type AiChatAction } from "./aiChat";
import { aiHistoryError, restoreAiConversationModel, sameAiConversation } from "./aiConversationHistory";

interface PendingRun {
  id: string;
  sendStarted: boolean;
  cancelRequested: boolean;
  terminal: boolean;
  abandoned: boolean;
  unlisten: (() => void) | null;
}

export interface AiChatSession extends AiChatActions {
  getSnapshot: () => AiChatState;
  subscribe: (onChange: () => void) => () => void;
  mount: () => () => void;
}

// Owns the transcript outside the panel so changing sidebar panels preserves it.
// The injected transport also allows deterministic testing of IPC/event races.
export function createAiChatSession(transport: AiChatTransport): AiChatSession {
  let state = createAiChatState();
  let mounted = false;
  let mountVersion = 0;
  let selectionVersion = 0;
  let historyVersion = 0;
  let historyRequested = false;
  let connectionInitialized = false;
  let activeConnectionId: string | null = null;
  const connectionSessions = new Map<string, AiChatState>();
  const requestedTitles = new Set<string>();
  let titleVersion = 0;
  let active: PendingRun | null = null;
  let storageUnlisten: (() => void) | null = null;
  const subscribers = new Set<() => void>();

  function dispatch(action: AiChatAction) {
    state = aiChatReducer(state, action);
    subscribers.forEach((notify) => notify());
  }

  function releaseListener(run: PendingRun) {
    run.unlisten?.();
    run.unlisten = null;
  }

  function isCurrent(run: PendingRun) {
    return mounted && active === run && !run.terminal && !run.abandoned;
  }

  function abandonRun() {
    const run = active;
    active = null;
    if (!run) return;
    run.abandoned = true;
    run.cancelRequested = true;
    releaseListener(run);
    if (run.sendStarted) void transport.cancel(run.id).catch(() => {});
  }

  async function persist() {
    if (!mounted || active || state.saveState === "saving" || state.history.busy || state.history.restoring || !state.conversation?.messages.length) return;
    const version = selectionVersion;
    const conversation = state.conversation;
    const generateTitle = conversation.title_source === "first_message" && conversation.messages.length === 2
      && (state.finish === "stop" || state.finish === "length") && !state.error;
    dispatch({ type: "saving" });
    try {
      const result = await transport.save(conversation);
      if (mounted && version === selectionVersion) {
        dispatch({ type: "saved", result });
        if (generateTitle) void generateConversationTitle(conversation);
        if (historyRequested) void refreshHistory(state.history.scope);
      }
    } catch {
      if (mounted && version === selectionVersion) dispatch({ type: "save_failed" });
    }
  }

  async function generateConversationTitle(conversation: AiConversation) {
    const key = `${conversation.connection_id}:${conversation.id}`;
    if (!transport.generateTitle || requestedTitles.has(key)) return;
    requestedTitles.add(key);
    const version = titleVersion;
    const mount = mountVersion;
    try {
      const summary = await transport.generateTitle(conversation.connection_id, conversation.id);
      if (!summary || !mounted || version !== titleVersion || mount !== mountVersion) return;
      const action = { type: "generated_title", summary } as const;
      dispatch(action);
      const cached = connectionSessions.get(summary.connection_id);
      if (cached) connectionSessions.set(summary.connection_id, aiChatReducer(cached, action));
      if (historyRequested) void refreshHistory(state.history.scope);
    } catch {
      // Title generation is optional; keep the first-message title on failure.
    }
  }

  function receive(run: PendingRun, event: AiChatEvent) {
    if (!isCurrent(run)) return;
    dispatch({ type: "event", runId: run.id, event });
    if (event.type === "done") {
      run.terminal = true;
      active = null;
      releaseListener(run);
      void persist();
    }
  }

  function selectConversation(conversation: AiConversation | null) {
    if (!mounted) return;
    selectionVersion++;
    abandonRun();
    dispatch({ type: "select", conversation });
  }

  function canChangeSelection() {
    return mounted && state.ready && !active && state.saveState !== "saving" && !state.history.busy;
  }

  async function refreshHistory(connectionId: string | null) {
    if (!mounted || !transport.history) return;
    historyRequested = true;
    const version = ++historyVersion;
    dispatch({ type: "history", patch: { scope: connectionId, loading: true, items: [], error: null } });
    try {
      const items = await transport.history.list(connectionId);
      if (mounted && version === historyVersion) dispatch({ type: "history", patch: { items, loading: false } });
    } catch (error) {
      if (mounted && version === historyVersion) dispatch({ type: "history", patch: { loading: false, error: aiHistoryError(error) } });
    }
  }

  function restore(conversation: AiConversation, options: AiResumeOptions) {
    const restored = restoreAiConversationModel(conversation, options);
    dispatch({ type: "select", conversation: restored.conversation });
    dispatch({ type: "history", patch: { modelFallback: restored.modelFallback } });
  }

  async function activateConnection(connectionId: string | null, options: AiResumeOptions, force = false) {
    if (!canChangeSelection() || (force && state.saveState === "error")
      || (connectionInitialized && activeConnectionId === connectionId && !force)) return;
    const previousId = state.conversation?.connection_id ?? activeConnectionId;
    if (previousId && !state.history.restoring) connectionSessions.set(previousId, state);
    connectionInitialized = true;
    activeConnectionId = connectionId;
    const version = ++selectionVersion;
    // Invalidate list requests from the previous connection, including an All filter.
    historyVersion++;
    dispatch({ type: "history", patch: { scope: connectionId, items: [], loading: false, error: null } });
    if (historyRequested) void refreshHistory(connectionId);
    const cached = connectionId && !force ? connectionSessions.get(connectionId) : undefined;
    if (cached) {
      dispatch({ type: "restore", state: cached });
      // Keep unsaved transcripts/drafts in memory; never reload over a failed save.
      if (cached.conversation && cached.saveState !== "error") {
        const restored = restoreAiConversationModel(cached.conversation, options);
        if (restored.modelFallback && options.fallback) dispatch({ type: "model", choice: options.fallback });
        dispatch({ type: "history", patch: { modelFallback: restored.modelFallback } });
      }
      return;
    }
    const draft = !previousId ? state.draft : "";
    dispatch({ type: "select", conversation: null });
    if (draft) dispatch({ type: "draft", draft });
    if (!connectionId || !transport.history) return;
    dispatch({ type: "history", patch: { restoring: true } });
    try {
      const items = await transport.history.list(connectionId);
      if (!mounted || version !== selectionVersion) return;
      const latest = [...items].sort((a, b) => b.updated_at - a.updated_at || a.id.localeCompare(b.id))[0];
      if (latest) {
        const conversation = await transport.history.load(connectionId, latest.id);
        if (!mounted || version !== selectionVersion) return;
        const pendingDraft = state.draft;
        restore(conversation, options);
        if (pendingDraft) dispatch({ type: "draft", draft: pendingDraft });
      } else dispatch({ type: "history", patch: { restoring: false } });
    } catch (error) {
      if (mounted && version === selectionVersion) dispatch({ type: "history", patch: { restoring: false, error: aiHistoryError(error) } });
    }
  }

  async function resumeConversation(summary: AiConversationSummary, options: AiResumeOptions) {
    if (!canChangeSelection() || state.saveState === "error" || !transport.history) return false;
    const version = ++selectionVersion;
    dispatch({ type: "history", patch: { restoring: true, error: null } });
    try {
      const conversation = await transport.history.load(summary.connection_id, summary.id);
      if (!mounted || version !== selectionVersion) return false;
      restore(conversation, options);
      return true;
    } catch (error) {
      if (mounted && version === selectionVersion) dispatch({ type: "history", patch: { restoring: false, error: aiHistoryError(error) } });
      return false;
    }
  }

  async function mutateConversation(summary: AiConversationSummary, title?: string) {
    if (!canChangeSelection() || state.history.restoring || state.saveState === "error" || !transport.history) return false;
    if (title !== undefined && (!title.trim() || Array.from(title.trim()).length > 500)) return false;
    const version = selectionVersion;
    const mount = mountVersion;
    dispatch({ type: "history", patch: { busy: true, error: null } });
    try {
      if (title === undefined) await transport.history.delete(summary.connection_id, summary.id);
      else await transport.history.rename(summary.connection_id, summary.id, title.trim());
      if (!mounted || mount !== mountVersion || version !== selectionVersion) return false;
      const cached = connectionSessions.get(summary.connection_id);
      if (sameAiConversation(cached?.conversation ?? null, summary)) connectionSessions.delete(summary.connection_id);
      if (sameAiConversation(state.conversation, summary)) {
        if (title === undefined) selectConversation(null);
        else dispatch({ type: "rename", title: title.trim() });
      }
      dispatch({ type: "history", patch: { busy: false } });
      await refreshHistory(state.history.scope);
      return true;
    } catch (error) {
      if (mounted && mount === mountVersion && version === selectionVersion) {
        dispatch({ type: "history", patch: { busy: false, error: aiHistoryError(error) } });
      }
      return false;
    }
  }

  async function cancel(run: PendingRun) {
    try {
      await transport.cancel(run.id);
    } catch {
      if (isCurrent(run)) {
        run.cancelRequested = false;
        dispatch({ type: "failure", runId: run.id, error: { kind: "cancel" }, keepRun: true });
      }
    }
  }

  async function send(text: string, options: AiSendOptions = {}) {
    const conversation = state.conversation;
    if (!mounted || !state.ready || active || state.saveState === "saving" || state.history.restoring || state.history.busy || !text.trim()
      || !conversation?.provider || !conversation.model) return false;
    const version = selectionVersion;
    const run: PendingRun = {
      id: crypto.randomUUID(), sendStarted: false, cancelRequested: false,
      terminal: false, abandoned: false, unlisten: null,
    };
    const context = options.context ? { ...options.context,
      connection_id: conversation.connection_id, database: conversation.database, schema: conversation.schema,
    } : undefined;
    active = run;
    const user = createAiMessage("user", [{ type: "text", text }]);
    const assistant = {
      ...createAiMessage("assistant"), provider: conversation.provider, model: conversation.model,
    };
    dispatch({ type: "start", runId: run.id, user, assistant });
    const request = {
      provider: conversation.provider, model: conversation.model, system: options.system ?? "",
      messages: [...conversation.messages, user], tools: [],
      max_output_tokens: options.max_output_tokens, reasoning: options.reasoning,
    };
    try {
      // Subscribe first: ai_chat_send can emit before its invoke promise resolves.
      const unlisten = await transport.listen(run.id, (event) => receive(run, event));
      if (!isCurrent(run)) {
        unlisten();
        return false;
      }
      run.unlisten = unlisten;
    } catch {
      if (isCurrent(run)) {
        active = null;
        dispatch({ type: "failure", runId: run.id, error: { kind: "subscription" } });
      }
      return false;
    }
    try {
      run.sendStarted = true;
      const returnedId = context ? await transport.send(request, run.id, context) : await transport.send(request, run.id);
      // A cancel sent while invoke was pending may precede backend registration.
      if (run.cancelRequested) await cancel(run);
      if (returnedId !== run.id) throw new Error("Unexpected AI run ID");
      return mounted && version === selectionVersion && !run.abandoned;
    } catch {
      if (isCurrent(run)) {
        run.abandoned = true;
        active = null;
        releaseListener(run);
        void transport.cancel(run.id).catch(() => {});
        dispatch({ type: "failure", runId: run.id, error: { kind: "send" } });
      }
      return false;
    }
  }

  return {
    getSnapshot: () => state,
    subscribe: (onChange) => {
      subscribers.add(onChange);
      return () => { subscribers.delete(onChange); };
    },
    mount: () => {
      mounted = true;
      const version = ++mountVersion;
      void transport.listenStorageCleared(() => {
        if (mounted && version === mountVersion) {
          connectionSessions.clear();
          requestedTitles.clear();
          titleVersion++;
          connectionInitialized = false;
          historyVersion++;
          selectConversation(null);
          dispatch({ type: "history", patch: createAiChatState().history });
        }
      }).then((unlisten) => {
        if (!mounted || version !== mountVersion) {
          unlisten();
          return;
        }
        storageUnlisten = unlisten;
        dispatch({ type: "ready", ready: true });
      }).catch(() => {
        if (mounted && version === mountVersion) {
          dispatch({ type: "failure", error: { kind: "storage_subscription" } });
        }
      });
      return () => {
        mounted = false;
        mountVersion++;
        selectionVersion++;
        historyVersion++;
        connectionInitialized = false;
        abandonRun();
        storageUnlisten?.();
        storageUnlisten = null;
        dispatch({ type: "select", conversation: state.conversation });
        dispatch({ type: "history", patch: { busy: false, restoring: false, loading: false } });
        dispatch({ type: "ready", ready: false });
      };
    },
    setDraft: (draft) => { if (mounted) dispatch({ type: "draft", draft }); },
    setContextInclude: (include) => { if (mounted && !active) dispatch({ type: "context_include", include }); },
    setContextTables: (tables) => { if (mounted && !active) dispatch({ type: "context_tables", tables }); },
    selectConversation,
    newConversation: (options) => selectConversation(createAiConversation(options)),
    selectModel: (choice) => {
      if (!mounted || state.saveState === "saving" || state.history.restoring || state.history.busy || !choice.provider || !choice.model) return;
      dispatch({ type: "model", choice });
      if (!active) void persist();
    },
    send,
    stop: async () => {
      const run = active;
      if (!run || run.cancelRequested) return;
      run.cancelRequested = true;
      if (!run.sendStarted) receive(run, { type: "done", finish: "cancelled" });
      else await cancel(run);
    },
    retrySave: persist,
    activateConnection,
    refreshHistory,
    resumeConversation,
    renameConversation: (summary, title) => mutateConversation(summary, title),
    deleteConversation: (summary) => mutateConversation(summary),
  };
}
