import { useTranslation } from "react-i18next";
import { History, MessageSquare, Plus, X } from "lucide-react";

export function ChatHeader({ onNew, onClose, onHistory, historyOpen, disabled }: {
  onNew: () => void; onClose: () => void; onHistory: () => void; historyOpen: boolean; disabled: boolean;
}) {
  const { t } = useTranslation();
  return <header className="flex items-center gap-2 border-b border-default bg-base p-3">
    <MessageSquare size={16} className="shrink-0 text-accent" aria-hidden />
    <h2 className="min-w-0 flex-1 truncate text-sm font-semibold text-primary">{t("aiChat.title")}</h2>
    <button type="button" onClick={onHistory} aria-label={t("aiChat.history.title")} aria-expanded={historyOpen}
      className="rounded p-1.5 text-secondary hover:text-primary focus-ring"><History size={16} /></button>
    <button type="button" disabled={disabled} onClick={onNew} aria-label={t("aiChat.newChat")}
      className="rounded p-1.5 text-secondary hover:text-primary disabled:opacity-40 focus-ring"><Plus size={16} /></button>
    <button type="button" onClick={onClose} aria-label={t("aiChat.close")}
      className="rounded p-1.5 text-secondary hover:text-primary focus-ring"><X size={16} /></button>
  </header>;
}
