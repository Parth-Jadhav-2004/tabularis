import { useId, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

export function ChatComposer({ draft, onDraft, onSend, onStop, onEscape, streaming, disabled, children }: {
  draft: string; onDraft: (text: string) => void; onSend: () => void; onStop: () => void; onEscape: () => void;
  streaming: boolean; disabled: boolean; children: ReactNode;
}) {
  const { t } = useTranslation();
  const id = useId();
  return <form className="space-y-3 border-t border-default bg-base p-3" onSubmit={(event) => {
    event.preventDefault();
    if (!disabled && !streaming && draft.trim()) onSend();
  }}>
    <label htmlFor={id} className="sr-only">{t("aiChat.message")}</label>
    <textarea id={id} value={draft} onChange={(event) => onDraft(event.target.value)} rows={3}
      placeholder={t("aiChat.placeholder")} className="w-full resize-y rounded-lg border border-strong bg-elevated p-3 text-sm text-primary placeholder:text-muted focus:border-focus focus-ring"
      onKeyDown={(event) => {
        if (event.nativeEvent.isComposing || event.nativeEvent.keyCode === 229) return;
        if (event.key === "Escape") { event.preventDefault(); event.stopPropagation(); onEscape(); }
        if (event.key === "Enter" && !event.shiftKey && !event.ctrlKey && !event.metaKey && !event.altKey) {
          event.preventDefault(); event.stopPropagation();
          if (!disabled && !streaming && draft.trim()) onSend();
        }
      }} />
    {children}
    <div className="flex items-center justify-between gap-2">
      <span className="text-xs text-muted">{t("aiChat.askMode")}</span>
      {streaming
        ? <button type="button" onClick={onStop} className="rounded-lg border border-strong px-3 py-1.5 text-sm text-primary focus-ring">{t("aiChat.stop")}</button>
        : <button type="submit" disabled={disabled || !draft.trim()} className="rounded-lg bg-accent-primary px-3 py-1.5 text-sm text-inverse disabled:opacity-40 focus-ring">{t("aiChat.send")}</button>}
    </div>
  </form>;
}
