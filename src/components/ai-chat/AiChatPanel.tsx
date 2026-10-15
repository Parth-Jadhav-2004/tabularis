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
import { defaultAiChatModel, readAiChatModel, rememberAiChatModel } from "../../utils/aiChatModels";
import { ChatHeader } from "./ChatHeader";
import { ChatTranscript } from "./ChatTranscript";
import { ChatComposer } from "./ChatComposer";
import { ModelPicker } from "./ModelPicker";
import { ConversationList } from "./ConversationList";
import { ContextChips } from "./ContextChips";
import { changeAiContextTable, createAiContextOptions, sameAiContextScope } from "../../utils/aiContext";
import { aiContextCatalogTables, aiContextDroppedTable } from "../../utils/aiContextMentions";
import { useAiContextPreview } from "../../hooks/useAiContextPreview";
import { dragState } from "../../utils/dragState";

export function AiChatPanel() {
  const { t, i18n } = useTranslation();
  const chat = useAiChat();
  const availability = useAiAvailability();
  const models = useAiChatModels();
  const target = useAiEditor();
  const { settings } = useSettings();
  const { close } = useRightSidebar();
  const database = useContext(DatabaseContext);
  const [historyOpen, setHistoryOpen] = useState(false);
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
  const getContext = useCallback(() => scope ? { ...scope,
    ...createAiContextOptions(target?.readContext?.() ?? {}, chat.contextInclude, i18n.resolvedLanguage ?? "en", tables),
  } : null, [scope, target, chat.contextInclude, i18n.resolvedLanguage, tables]);
  const preview = useAiContextPreview(getContext, contextEnabled && !historyOpen);
  const data = scope ? database?.connectionDataMap[scope.connection_id] : undefined;
  const mentionTables = useMemo(() => aiContextCatalogTables(data, scope), [data, scope]);
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
    }
    rememberAiChatModel(next);
  };
  const send = () => {
    if (!availability.available || !usableChoice || !sameTarget || !chat.ready || chat.activeRun || chat.saveState === "saving" || historyPending) return;
    const draft = chat.draft;
    const context = getContext();
    if (!context) return;
    if (!conversation) createConversation();
    void chat.send(draft, { context });
  };
  return <section className="flex min-h-0 flex-1 flex-col" aria-label={t("aiChat.title")}>
    <ChatHeader onNew={() => createConversation()} onClose={close} onHistory={() => setHistoryOpen((open) => !open)} historyOpen={historyOpen}
      disabled={!target || !usableChoice || !!chat.activeRun || chat.saveState === "saving" || chat.saveState === "error" || historyPending} />
    {historyOpen ? <ConversationList key={connectionId} chat={chat} connectionId={connectionId} resumeOptions={resumeOptions}
      connectionNames={Object.fromEntries(database?.connections.map((connection) => [connection.id, connection.name]) ?? [])}
      onClose={() => setHistoryOpen(false)} /> : <>
    <ChatTranscript chat={chat} target={target} canAct={sameTarget && !historyPending} />
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
    <div role="status" className="px-4 text-xs text-muted">
      {chat.saveState === "saving" && t("aiChat.saving")}
      {chat.saveState === "saved" && t(chat.saveResult?.large ? "aiChat.savedLarge" : "aiChat.saved")}
      {chat.saveState === "error" && <p className="py-2 text-accent-error">{t("aiChat.saveError")}{" "}
        <button type="button" onClick={() => void chat.retrySave()} className="rounded underline focus-ring">{t("aiChat.retrySave")}</button>
      </p>}
    </div>
    <ContextChips include={chat.contextInclude} onChange={chat.setContextInclude}
      hasError={!!target?.hasError} disabled={!contextEnabled} tables={tables} onTable={onTable}
      preview={preview.preview} loading={preview.loading} error={preview.error} onRefresh={() => void preview.refresh()} />
    <ChatComposer draft={chat.draft} onDraft={chat.setDraft} onSend={send} onStop={() => void chat.stop()}
      onEscape={() => target?.actions.focus()} streaming={!!chat.activeRun}
      mentionTables={mentionTables} onTable={onTable} onFocus={() => void preview.refresh()}
      onDropTable={() => {
        const name = aiContextDroppedTable(dragState.context, data, scope);
        if (name) onTable(name, true);
      }}
      disabled={!availability.available || !usableChoice || !sameTarget || !chat.ready || chat.saveState === "saving" || historyPending}>
      <ModelPicker choice={choice} choices={models.choices} onChange={selectModel} onRefresh={models.refresh} loading={models.loading} disabled={chat.saveState === "saving" || historyPending} />
      {models.error && <p className="text-xs text-accent-error">{t("aiChat.modelsError")}</p>}
    </ChatComposer>
    </>}
  </section>;
}
