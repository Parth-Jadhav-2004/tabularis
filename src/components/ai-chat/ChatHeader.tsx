import { useTranslation } from "react-i18next";
import { History, SquarePen, X } from "lucide-react";
import type { ReactNode } from "react";

export function ChatHeader({ onNew, onClose, onHistory, historyOpen, disabled, subtitle }: {
  onNew: () => void; onClose: () => void; onHistory: () => void; historyOpen: boolean; disabled: boolean;
  subtitle?: ReactNode;
}) {
  const { t } = useTranslation();
  return <header className="flex min-h-14 items-center gap-1 border-b border-default px-4 py-3">
    <div className="min-w-0 flex-1">
      <h2 className="truncate text-sm font-medium text-primary">{t("aiChat.title")}</h2>
      {subtitle}
    </div>
    <button type="button" onClick={onHistory} aria-label={t("aiChat.history.title")} aria-expanded={historyOpen}
      title={t("aiChat.history.title")} className="rounded-lg p-2 text-secondary hover:bg-surface-secondary hover:text-primary focus-ring"><History size={16} /></button>
    <button type="button" disabled={disabled} onClick={onNew} aria-label={t("aiChat.newChat")}
      title={t("aiChat.newChat")} className="rounded-lg p-2 text-secondary hover:bg-surface-secondary hover:text-primary disabled:opacity-40 focus-ring"><SquarePen size={16} /></button>
    <button type="button" onClick={onClose} aria-label={t("aiChat.close")}
      title={t("aiChat.close")} className="rounded-lg p-2 text-secondary hover:bg-surface-secondary hover:text-primary focus-ring"><X size={16} /></button>
  </header>;
}
