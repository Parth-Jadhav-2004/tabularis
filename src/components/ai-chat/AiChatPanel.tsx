import { Link } from "react-router-dom";
import { useCallback, useContext, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { useAiChat } from "../../hooks/useAiChat";
import { useAiAvailability } from "../../hooks/useAiAvailability";
import { useAiChatModels } from "../../hooks/useAiChatModels";
import { useAiEditor } from "../../hooks/useAiEditor";
import { useSettings } from "../../hooks/useSettings";
import { useRightSidebar } from "../../hooks/useRightSidebar";
import type { AiModelChoice } from "../../types/aiChat";
import { DatabaseContext } from "../../contexts/DatabaseContext";
import { SavedQueriesContext } from "../../contexts/SavedQueriesContext";
import type { AiMentionItem } from "../../types/aiContext";
import { defaultAiChatModel, readAiChatModel, rememberAiChatModel } from "../../utils/aiChatModels";
import { ChatHeader } from "./ChatHeader";
import { ChatTranscript } from "./ChatTranscript";
import { ChatComposer } from "./ChatComposer";
import { ModelPicker } from "./ModelPicker";
import { ConversationList } from "./ConversationList";
import { ContextChips } from "./ContextChips";
import { changeAiContextMention, changeAiContextTable, createAiContextOptions, defaultAiContextSelection, sameAiContextScope } from "../../utils/aiContext";
import { aiContextCatalogTables, aiContextDroppedTable, aiContextMentionItems } from "../../utils/aiContextMentions";
import { useAiContextPreview } from "../../hooks/useAiContextPreview";
import { dragState } from "../../utils/dragState";
import { AI_SLASH_COMMANDS, aiCommandContext, parseAiCommand } from "../../utils/aiCommands";
import { useAiChatEntry } from "../../hooks/useAiChatEntry";
import { aiEditorTargetKey } from "../../utils/aiEditor";

export function AiChatPanel() {
  const { t, i18n } = useTranslation();
  const chat = useAiChat();
  const availability = useAiAvailability();
  const models = useAiChatModels();
  const target = useAiEditor();
  const { settings } = useSettings();
  const { close } = useRightSidebar();
  const database = useContext(DatabaseContext);
  const savedQueries = useContext(SavedQueriesContext);
  const [historyOpen, setHistoryOpen] = useState(false);
  const [helpOpen, setHelpOpen] = useState(false);
  const [commandError, setCommandError] = useState(false);
  const [focusRequest, setFocusRequest] = useState(0);
  const { entry, consumeEntry, activeScopeId } = useAiChatEntry();
  const parsed = parseAiCommand(chat.draft);
  const command = parsed.kind === "command" ? parsed.command : null;
  const localCommand = command === "help" || parsed.kind === "unknown";
  const connectionId = target?.connectionId ?? database?.activeConnectionId ?? null;
  const fallback = defaultAiChatModel(models.choices, settings, null);
  const resumeOptions = useMemo(() => ({ choices: models.choices, fallback }), [models.choices, fallback]);
  const { activateConnection } = chat;
  useEffect(() => {
    if (chat.ready && !models.loading) void activateConnection(connectionId, resumeOptions);
  }, [activateConnection, connectionId, resumeOptions, models.loading, chat.ready, chat.activeRun, chat.saveState, chat.history.busy, chat.conversation?.id]);
  const conversation = chat.conversation;
  const storedChoice = conversation?.provider && conversation.model
    ? { provider: conversation.provider, model: conversation.model } : null;
  const choice = storedChoice ?? defaultAiChatModel(models.choices, settings, readAiChatModel());
  const usableChoice = !!choice && models.choices.some((item) => item.provider === choice.provider && item.model === choice.model);
  const sameTarget = !!target && (!conversation || (conversation.connection_id === target.connectionId
    && conversation.database === target.database && conversation.schema === target.schema));
  const historyPending = chat.history.restoring || chat.history.busy;
  const contextEnabled = availability.available && usableChoice && chat.ready && !models.loading && !chat.activeRun
    && sameTarget && !historyPending && chat.saveState !== "saving";
  const targetConnectionId = target?.connectionId;
  const targetDatabase = target?.database ?? null;
  const targetSchema = target?.schema ?? null;
  const scope = useMemo(() => targetConnectionId ? { connection_id: targetConnectionId, database: targetDatabase, schema: targetSchema } : null,
    [targetConnectionId, targetDatabase, targetSchema]);
  const tables = useMemo(() => sameAiContextScope(chat.contextTables, scope) && chat.contextTables
    ? chat.contextTables : { tables: [], excluded_tables: [] }, [chat.contextTables, scope]);
  const getContext = useCallback(() => scope && !localCommand ? { ...scope,
    ...createAiContextOptions(target?.readContext?.() ?? {}, chat.contextInclude, i18n.resolvedLanguage ?? "en", tables),
    ...(command ? { command } : {}),
  } : null, [scope, target, chat.contextInclude, i18n.resolvedLanguage, tables, command, localCommand]);
  const preview = useAiContextPreview(getContext, contextEnabled && !historyOpen && !localCommand);
  const data = scope ? database?.connectionDataMap[scope.connection_id] : undefined;
  const mentionTables = useMemo(() => aiContextCatalogTables(data, scope), [data, scope]);
  const mentionItems = useMemo(() => aiContextMentionItems(data, scope, savedQueries?.queries ?? []), [data, scope, savedQueries?.queries]);
  const onMention = (item: AiMentionItem, selected: boolean) => {
    if (!contextEnabled || !scope) return false;
    const next = changeAiContextMention(tables, item, selected);
    if (next === tables) return false;
    chat.setContextTables({ ...scope, ...next });
    return true;
  };
  const onTable = (name: string, selected: boolean) => {
    if (!contextEnabled || !scope) return false;
    const next = changeAiContextTable(tables, name, selected);
    if (next === tables) return false;
    chat.setContextTables({ ...scope, ...next });
    return true;
  };
  const createConversation = (next = choice) => {
    if (!target || !next) return;
    chat.newConversation({ connection_id: target.connectionId, database: target.database,
      schema: target.schema, provider: next.provider, model: next.model });
    rememberAiChatModel(next);
    setHistoryOpen(false);
  };
  const selectModel = (next: AiModelChoice) => {
    if (chat.saveState === "saving" || historyPending) return;
    if (conversation) chat.selectModel(next);
    else {
      createConversation(next);
      chat.setDraft(chat.draft);
      chat.setContextInclude(chat.contextInclude);
      chat.setContextTables(chat.contextTables);
    }
    rememberAiChatModel(next);
  };
  useEffect(() => {
    if (!entry) return;
    let cancelled = false;
    // Wait for lazy mounting, connection restoration and model discovery. A
    // claimed entry is consumed once, including React StrictMode replay.
    queueMicrotask(() => {
      if (cancelled) return;
      const matches = entry.scopeId === activeScopeId && !!target
        && (entry.target.id ? entry.target.id === target.id : entry.target === target)
        && entry.target.connectionId === target.connectionId
        && entry.target.database === target.database && entry.target.schema === target.schema;
      if (!matches || !availability.available) { consumeEntry(entry); return; }
      if (!chat.ready || models.loading || !usableChoice || !choice || chat.activeRun
        || chat.saveState === "saving" || chat.saveState === "error" || historyPending
        || chat.history.scope !== connectionId) return;
      if (!consumeEntry(entry)) return;
      if (!conversation || !sameTarget) chat.newConversation({ connection_id: target.connectionId,
        database: target.database, schema: target.schema, provider: choice.provider, model: choice.model });
      const include = entry.include ?? (entry.command ? aiCommandContext(entry.command, entry.snapshot) : defaultAiContextSelection());
      chat.setContextInclude(include);
      chat.setContextTables(entry.tables ? { connection_id: target.connectionId, database: target.database,
        schema: target.schema, tables: entry.tables, excluded_tables: [] } : null);
      chat.setDraft(entry.command ? `/${entry.command} ` : "");
      setHistoryOpen(false);
      setHelpOpen(false);
      setCommandError(false);
      setFocusRequest((value) => value + 1);
      if (entry.command && entry.command !== "generate") void chat.send(`/${entry.command}`, { editorTarget: aiEditorTargetKey(target, activeScopeId), context: {
        ...createAiContextOptions(entry.snapshot, include, i18n.resolvedLanguage ?? "en", { tables: entry.tables ?? [], excluded_tables: [] }),
        command: entry.command,
      } });
    });
    return () => { cancelled = true; };
  }, [entry, consumeEntry, activeScopeId, target, availability.available, chat, models.loading, usableChoice, choice,
    historyPending, connectionId, conversation, sameTarget, i18n.resolvedLanguage]);
  const send = () => {
    if (!availability.available || !chat.ready || chat.activeRun || chat.saveState === "saving" || historyPending) return;
    if (parsed.kind === "unknown") { setCommandError(true); return; }
    if (command === "help") { setHelpOpen(true); chat.setDraft(""); chat.setContextInclude(defaultAiContextSelection()); return; }
    if (!usableChoice || !sameTarget) return;
    const draft = chat.draft;
    const context = getContext();
    if (!context) return;
    if (!conversation) createConversation();
    setHelpOpen(false);
    void chat.send(draft, { context, editorTarget: aiEditorTargetKey(target, activeScopeId) });
  };
  const onDraft = (draft: string) => {
    const next = parseAiCommand(draft);
    if (next.kind === "command" && next.command !== command) {
      chat.setContextInclude(aiCommandContext(next.command, target?.readContext?.() ?? {}));
    } else if (next.kind !== "command" && command) {
      chat.setContextInclude(defaultAiContextSelection());
    }
    setCommandError(false);
    chat.setDraft(draft);
  };
  return <section className="flex min-h-0 flex-1 flex-col" aria-label={t("aiChat.title")}>
    <ChatHeader onNew={() => createConversation()} onClose={close} onHistory={() => setHistoryOpen((open) => !open)} historyOpen={historyOpen}
      disabled={!target || !usableChoice || !!chat.activeRun || chat.saveState === "saving" || chat.saveState === "error" || historyPending} />
    {historyOpen ? <ConversationList key={connectionId} chat={chat} connectionId={connectionId} resumeOptions={resumeOptions}
      connectionNames={Object.fromEntries(database?.connections.map((connection) => [connection.id, connection.name]) ?? [])}
      onClose={() => setHistoryOpen(false)} /> : <>
    <ChatTranscript chat={chat} target={target} targetKey={aiEditorTargetKey(target, activeScopeId)} canAct={sameTarget && !historyPending} />
    {chat.history.restoring && <p role="status" className="px-4 py-2 text-xs text-secondary">{t("common.loading")}</p>}
    {chat.history.modelFallback && <p role="status" className="px-4 py-2 text-xs text-secondary">{t("aiChat.history.modelFallback")}</p>}
    {chat.history.error && <div role="alert" className="space-y-2 px-4 py-2 text-xs text-accent-error">
      <p>{t(`aiChat.history.errors.${chat.history.error}`)}</p>
      <button type="button" disabled={chat.saveState === "error" || historyPending} onClick={() => void activateConnection(connectionId, resumeOptions, true)} className="rounded underline disabled:opacity-40 focus-ring">{t("aiChat.history.retry")}</button>{" "}
      <Link to="/settings?tab=ai" className="rounded underline focus-ring">{t("settings.ai.storage.reset")}</Link>
    </div>}
    {!availability.available && <div className="space-y-2 p-4 text-sm text-secondary">
      <p>{t(availability.loading ? "aiChat.checkingProviders" : "aiChat.unavailable")}</p>
      <Link to="/settings?tab=ai" className="rounded text-accent focus-ring">{t("aiChat.openSettings")}</Link>
    </div>}
    {!target && <p className="px-4 py-2 text-xs text-secondary">{t("aiChat.noConnection")}</p>}
    {target && !sameTarget && <p className="px-4 py-2 text-xs text-secondary">{t("aiChat.targetChanged")}</p>}
    {chat.error && <p role="alert" className="px-4 py-2 text-xs text-accent-error">{t(`aiChat.errors.${chat.error.kind}`)}</p>}
    {commandError && <p role="alert" className="px-4 py-2 text-xs text-accent-warning">{t("aiChat.commands.unknown")}</p>}
    {helpOpen && <section aria-label={t("aiChat.commands.helpTitle")} className="max-h-48 space-y-2 overflow-y-auto border-t border-default p-3 text-sm text-secondary">
      <div className="flex items-center justify-between gap-2">
        <p className="font-medium text-primary">{t("aiChat.commands.helpTitle")}</p>
        <button type="button" onClick={() => setHelpOpen(false)} className="rounded px-2 py-1 text-xs focus-ring">{t("common.close")}</button>
      </div>
      <ul className="space-y-2">{AI_SLASH_COMMANDS.map((item) => <li key={item}>
        <code className="text-primary">/{item}</code>{" — "}{t(`aiChat.commands.${item}`)}
      </li>)}</ul>
    </section>}
    <div role="status" className="px-4 text-xs text-muted">
      {chat.saveState === "saving" && t("aiChat.saving")}
      {chat.saveState === "saved" && t(chat.saveResult?.large ? "aiChat.savedLarge" : "aiChat.saved")}
      {chat.saveState === "error" && <p className="py-2 text-accent-error">{t("aiChat.saveError")}{" "}
        <button type="button" onClick={() => void chat.retrySave()} className="rounded underline focus-ring">{t("aiChat.retrySave")}</button>
      </p>}
    </div>
    <ContextChips include={chat.contextInclude} onChange={chat.setContextInclude}
      hasInstructions={!!database?.connections.find((connection) => connection.id === connectionId)?.ai_instructions?.trim()}
      hasError={!!target?.hasError} disabled={!contextEnabled} tables={tables} onTable={onTable}
      onMention={onMention}
      preview={preview.preview} loading={preview.loading} error={preview.error} onRefresh={() => void preview.refresh()} />
    <ChatComposer draft={chat.draft} onDraft={onDraft} onSend={send} onStop={() => void chat.stop()}
      focusRequest={focusRequest}
      onEscape={() => target?.actions.focus()} streaming={!!chat.activeRun}
      mentionTables={mentionTables} onTable={onTable} onFocus={() => void preview.refresh()}
      mentionItems={mentionItems} onMention={onMention}
      onDropTable={() => {
        const name = aiContextDroppedTable(dragState.context, data, scope);
        if (name) onTable(name, true);
      }}
      disabled={!availability.available || (command !== "help" && (!usableChoice || !sameTarget)) || !chat.ready || chat.saveState === "saving" || historyPending}>
      <ModelPicker choice={choice} choices={models.choices} onChange={selectModel} onRefresh={models.refresh} loading={models.loading} disabled={chat.saveState === "saving" || historyPending} />
      {models.error && <p className="text-xs text-accent-error">{t("aiChat.modelsError")}</p>}
    </ChatComposer>
    </>}
  </section>;
}
