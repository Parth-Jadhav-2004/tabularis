import { useEffect, useRef, useState } from "react";
import { Link } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { ArrowLeft, Pencil, RefreshCw, Trash2 } from "lucide-react";
import type { AiChatContextValue, AiConversationSummary, AiResumeOptions } from "../../types/aiChat";
import { aiHistoryRelativeTime, groupAiHistory } from "../../utils/aiConversationHistory";
import { ConfirmModal } from "../modals/ConfirmModal";
import { ConversationExport } from "./ConversationExport";
import { useAiNotebookCatalog } from "../../hooks/useAiNotebookCatalog";
import { NotebookConversationTarget } from "./NotebookConversationTarget";
import type { NotebookMetadata } from "../../types/notebook";

interface ConversationListProps {
  chat: AiChatContextValue;
  connectionId: string | null;
  notebookId?: string;
  connectionNames: Record<string, string>;
  resumeOptions: AiResumeOptions;
  onClose: () => void;
}

export function ConversationList({ chat, connectionId, notebookId, connectionNames, resumeOptions, onClose }: ConversationListProps) {
  const { t, i18n } = useTranslation();
  const [filter, setFilter] = useState(notebookId ? "notebook" : "current");
  const [search, setSearch] = useState("");
  const [deleting, setDeleting] = useState<AiConversationSummary | null>(null);
  const all = filter === "all";
  const scope = all ? null : connectionId;
  const { refreshHistory } = chat;
  const requested = useRef(false);
  useEffect(() => {
    if (!chat.ready) return;
    if (!requested.current || chat.history.scope !== scope) {
      requested.current = true;
      void refreshHistory(scope);
    }
  }, [chat.ready, chat.history.scope, refreshHistory, scope]);
  const history = chat.history;
  const disabled = !!chat.activeRun || chat.saveState === "saving" || chat.saveState === "error" || history.busy || history.restoring;
  const items = history.scope === scope ? history.items : [];
  const catalog = useAiNotebookCatalog(items.filter((item) => item.target?.type === "notebook").map((item) => item.connection_id));
  const scopedItems = filter === "notebook" ? items.filter((item) => item.target?.type === "notebook"
    && item.target.notebook_id === notebookId && item.connection_id === connectionId) : items;
  const filtered = groupAiHistory(scopedItems, search).flatMap((group) => group.items);
  const sections = all ? [...new Set(filtered.map((item) => item.connection_id))] : [connectionId];
  const remove = async () => {
    if (deleting && await chat.deleteConversation(deleting)) setDeleting(null);
  };
  return <div className="flex min-h-0 flex-1 flex-col" aria-label={t("aiChat.history.title")}>
    <div className="space-y-3 border-b border-default p-3">
      <div className="flex items-center gap-2">
        <button type="button" onClick={onClose} aria-label={t("aiChat.history.back")} className="rounded p-1.5 text-secondary hover:text-primary focus-ring"><ArrowLeft size={16} /></button>
        <h3 className="flex-1 text-sm font-semibold text-primary">{t("aiChat.history.title")}</h3>
        <button type="button" disabled={history.loading || history.busy} onClick={() => void refreshHistory(scope)} aria-label={t("aiChat.history.refresh")}
          className="rounded p-1.5 text-secondary hover:text-primary disabled:opacity-40 focus-ring"><RefreshCw size={16} /></button>
      </div>
      <select aria-label={t("aiChat.history.filter")} value={filter} onChange={(event) => setFilter(event.target.value)}
        className="w-full rounded border border-strong bg-base p-2 text-sm text-primary focus-ring">
        {notebookId && <option value="notebook">{t("aiChat.history.thisNotebook")}</option>}
        <option value="current">{t("aiChat.history.thisConnection")}</option>
        <option value="all">{t("aiChat.history.allConnections")}</option>
      </select>
      <input type="search" aria-label={t("aiChat.history.search")} placeholder={t("aiChat.history.search")} value={search}
        onChange={(event) => setSearch(event.target.value)} className="w-full rounded border border-strong bg-base p-2 text-sm text-primary placeholder:text-muted focus-ring" />
    </div>
    <div className="min-h-0 flex-1 overflow-y-auto p-3" aria-busy={history.loading}>
      {history.loading && <p role="status" className="py-4 text-sm text-secondary">{t("common.loading")}</p>}
      {history.error && <div role="alert" className="space-y-2 py-3 text-sm text-accent-error">
        <p>{t(`aiChat.history.errors.${history.error}`)}</p>
        <Link to="/settings?tab=ai" className="rounded underline focus-ring">{t("settings.ai.storage.reset")}</Link>
      </div>}
      {!history.loading && !history.error && !filtered.length && <p className="py-6 text-sm text-secondary">{t(search.trim() ? "aiChat.history.noMatches" : "aiChat.history.empty")}</p>}
      {sections.map((id) => <div key={id ?? "none"} className="mb-4 space-y-4">
        {all && <h4 className="truncate text-sm font-semibold text-primary">{connectionNames[id ?? ""] ?? t("aiChat.history.unknownConnection")}</h4>}
        {groupAiHistory(filtered.filter((item) => item.connection_id === id), "").map((group) => <section key={group.bucket} aria-label={t(`aiChat.history.buckets.${group.bucket}`)}>
          <h5 className="mb-2 text-xs font-medium text-muted">{t(`aiChat.history.buckets.${group.bucket}`)}</h5>
          <ul className="space-y-2">
            {group.items.map((item) => <ConversationRow key={`${item.connection_id}:${item.id}`} item={item} disabled={disabled}
              notebooks={catalog[item.connection_id]}
              selected={chat.conversation?.id === item.id && chat.conversation.connection_id === item.connection_id}
              timestamp={aiHistoryRelativeTime(item.updated_at, i18n.resolvedLanguage ?? "en")}
              onResume={() => { void chat.resumeConversation(item, resumeOptions).then((resumed) => { if (resumed) onClose(); }); }}
              onRename={(title) => chat.renameConversation(item, title)} onDelete={() => setDeleting(item)} />)}
          </ul>
        </section>)}
      </div>)}
    </div>
    <ConfirmModal isOpen={deleting !== null} onClose={() => { if (!history.busy) setDeleting(null); }}
      title={t("aiChat.history.delete")} message={t("aiChat.history.deleteConfirm", { title: deleting?.title ?? "" })}
      confirmLabel={t("common.delete")} variant="danger" busy={history.busy} onConfirm={() => void remove()} />
  </div>;
}

function ConversationRow({ item, selected, disabled, timestamp, notebooks, onResume, onRename, onDelete }: {
  item: AiConversationSummary;
  selected: boolean;
  disabled: boolean;
  timestamp: string;
  notebooks?: readonly NotebookMetadata[];
  onResume: () => void;
  onRename: (title: string) => Promise<boolean>;
  onDelete: () => void;
}) {
  const { t } = useTranslation();
  const [editing, setEditing] = useState(false);
  const [title, setTitle] = useState(item.title);
  const rename = async () => { if (await onRename(title)) setEditing(false); };
  return <li className="rounded-lg border border-default bg-elevated p-2">
    {editing ? <form onSubmit={(event) => { event.preventDefault(); void rename(); }} className="space-y-2">
      <input aria-label={t("aiChat.history.rename")} value={title} autoFocus
        onChange={(event) => setTitle(event.target.value)} onKeyDown={(event) => { if (event.key === "Escape") setEditing(false); }}
        className="w-full rounded border border-strong bg-base p-2 text-sm text-primary focus-ring" />
      <div className="flex justify-end gap-2">
        <button type="button" disabled={disabled} onClick={() => setEditing(false)} className="rounded px-2 py-1 text-xs text-secondary focus-ring">{t("common.cancel")}</button>
        <button type="submit" disabled={disabled || !title.trim() || Array.from(title.trim()).length > 500}
          className="rounded bg-accent-primary px-2 py-1 text-xs text-inverse disabled:opacity-40 focus-ring">{t("common.save")}</button>
      </div>
    </form> : <div className="flex items-start gap-1">
      <button type="button" onClick={onResume} disabled={disabled} aria-current={selected ? "true" : undefined}
        className="min-w-0 flex-1 rounded p-1 text-left disabled:opacity-40 focus-ring">
        <span className="block truncate text-sm text-primary">{item.title}</span>
        <NotebookConversationTarget target={item.target} notebooks={notebooks} />
        <span className="block text-xs text-muted">{timestamp} · {t("aiChat.history.messages", { count: item.message_count })}</span>
      </button>
      <button type="button" disabled={disabled} onClick={() => { setTitle(item.title); setEditing(true); }} aria-label={t("aiChat.history.rename")}
        className="rounded p-1.5 text-secondary hover:text-primary disabled:opacity-40 focus-ring"><Pencil size={14} /></button>
      <button type="button" disabled={disabled} onClick={onDelete} aria-label={t("aiChat.history.delete")}
        className="rounded p-1.5 text-secondary hover:text-accent-error disabled:opacity-40 focus-ring"><Trash2 size={14} /></button>
    </div>}
    {!editing && <ConversationExport item={item} disabled={disabled} />}
  </li>;
}
