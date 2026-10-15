import { useEffect, useId, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { activeAiMention, filterAiMentionTables, removeAiMention } from "../../utils/aiContextMentions";

export function ChatComposer({ draft, onDraft, onSend, onStop, onEscape, streaming, disabled, children, mentionTables = [], onTable, onDropTable, onFocus }: {
  draft: string; onDraft: (text: string) => void; onSend: () => void; onStop: () => void; onEscape: () => void;
  streaming: boolean; disabled: boolean; children: ReactNode;
  mentionTables?: string[]; onTable?: (name: string, selected: boolean) => boolean;
  onDropTable?: () => void; onFocus?: () => void;
}) {
  const { t } = useTranslation();
  const id = useId();
  const listId = `${id}-mentions`;
  const textarea = useRef<HTMLTextAreaElement>(null);
  const [caret, setCaret] = useState({ text: draft, start: draft.length, end: draft.length });
  const [dismissed, setDismissed] = useState<string | null>(null);
  const [highlight, setHighlight] = useState({ key: "", index: 0 });
  const [limitReached, setLimitReached] = useState(false);
  const [focused, setFocused] = useState(false);
  const range = caret.text === draft ? activeAiMention(draft, caret.start, caret.end) : null;
  const key = range ? `${range.start}:${range.end}:${range.search}` : "";
  const open = focused && !!range && !!onTable && dismissed !== key && !disabled && !streaming;
  const options = open ? filterAiMentionTables(mentionTables, range.search) : [];
  const index = highlight.key === key ? Math.min(highlight.index, Math.max(0, options.length - 1)) : 0;
  useEffect(() => {
    if (open) document.getElementById(`${listId}-${index}`)?.scrollIntoView?.({ block: "nearest" });
  }, [open, listId, index]);
  const pick = (name: string) => {
    if (!range || !onTable || disabled || streaming) return;
    if (!onTable(name, true)) { setLimitReached(true); return; }
    const text = removeAiMention(draft, range);
    onDraft(text);
    setCaret({ text, start: range.start, end: range.start });
    setDismissed(null);
    setLimitReached(false);
    setHighlight({ key: "", index: 0 });
    requestAnimationFrame(() => {
      if (textarea.current?.value === text) {
        textarea.current.focus();
        textarea.current.setSelectionRange(range.start, range.start);
      }
    });
  };
  return <form className="space-y-3 border-t border-default bg-base p-3" onSubmit={(event) => {
    event.preventDefault();
    if (!disabled && !streaming && draft.trim()) onSend();
  }}>
    <label htmlFor={id} className="sr-only">{t("aiChat.message")}</label>
    <div className="relative">
    {open && <div className="absolute bottom-full left-0 right-0 z-10 mb-1 rounded-lg border border-strong bg-elevated p-1 shadow-lg">
      <p className="px-2 py-1 text-xs text-muted">{t("aiChat.context.tableDetails")}</p>
      <div id={listId} role="listbox" aria-label={t("aiChat.context.mentionTables")} className="max-h-48 overflow-y-auto">
        {options.map((name, option) => <button key={name} id={`${listId}-${option}`} type="button" role="option" aria-selected={index === option}
          onMouseDown={(event) => event.preventDefault()} onClick={() => pick(name)}
          className={`block w-full truncate rounded px-2 py-1.5 text-left font-mono text-sm focus-ring ${index === option ? "bg-accent-primary text-inverse" : "text-primary hover:bg-surface-secondary"}`}>
          {name}
        </button>)}
      </div>
      {options.length === 0 && <p role="status" className="px-2 py-2 text-xs text-secondary">{t("aiChat.context.noMentionTables")}</p>}
      {limitReached && <p role="status" className="px-2 py-1 text-xs text-accent-warning">{t("aiChat.context.tableLimit")}</p>}
    </div>}
    <textarea ref={textarea} id={id} value={draft} role="combobox" aria-autocomplete="list" aria-haspopup="listbox"
      aria-expanded={open} aria-controls={open ? listId : undefined}
      aria-activedescendant={open && options.length > 0 ? `${listId}-${index}` : undefined}
      onFocus={(event) => {
        setFocused(true);
        setDismissed(null);
        setCaret({ text: event.currentTarget.value, start: event.currentTarget.selectionStart, end: event.currentTarget.selectionEnd });
        onFocus?.();
      }} onPointerUp={onDropTable}
      onBlur={() => { setFocused(false); setDismissed(key); }}
      onSelect={(event) => setCaret({ text: event.currentTarget.value, start: event.currentTarget.selectionStart, end: event.currentTarget.selectionEnd })}
      onChange={(event) => {
        onDraft(event.target.value);
        setCaret({ text: event.target.value, start: event.target.selectionStart, end: event.target.selectionEnd });
        setDismissed(null);
        setLimitReached(false);
        setHighlight({ key: "", index: 0 });
      }} rows={3}
      placeholder={t("aiChat.placeholder")} className="w-full resize-y rounded-lg border border-strong bg-elevated p-3 text-sm text-primary placeholder:text-muted focus:border-focus focus-ring"
      onKeyDown={(event) => {
        if (event.nativeEvent.isComposing || event.nativeEvent.keyCode === 229) return;
        if (open) {
          if (event.key === "Escape") { event.preventDefault(); event.stopPropagation(); setDismissed(key); return; }
          if (event.key === "ArrowDown" || event.key === "ArrowUp") {
            event.preventDefault(); event.stopPropagation();
            if (options.length) setHighlight({ key, index: (index + (event.key === "ArrowDown" ? 1 : options.length - 1)) % options.length });
            return;
          }
          if ((event.key === "Enter" || event.key === "Tab") && !event.shiftKey && !event.ctrlKey && !event.metaKey && !event.altKey && options.length > 0) {
            event.preventDefault(); event.stopPropagation(); pick(options[index]); return;
          }
          if (event.key === "Enter" && !event.shiftKey && !event.ctrlKey && !event.metaKey && !event.altKey) {
            event.preventDefault(); event.stopPropagation(); return;
          }
        }
        if (event.key === "Escape") { event.preventDefault(); event.stopPropagation(); onEscape(); }
        if (event.key === "Enter" && !event.shiftKey && !event.ctrlKey && !event.metaKey && !event.altKey) {
          event.preventDefault(); event.stopPropagation();
          if (!disabled && !streaming && draft.trim()) onSend();
        }
      }} />
    </div>
    {onTable && <p className="text-xs text-muted">{t("aiChat.context.mentionHint")}</p>}
    {children}
    <div className="flex items-center justify-between gap-2">
      <span className="text-xs text-muted">{t("aiChat.askMode")}</span>
      {streaming
        ? <button type="button" onClick={onStop} className="rounded-lg border border-strong px-3 py-1.5 text-sm text-primary focus-ring">{t("aiChat.stop")}</button>
        : <button type="submit" disabled={disabled || !draft.trim()} className="rounded-lg bg-accent-primary px-3 py-1.5 text-sm text-inverse disabled:opacity-40 focus-ring">{t("aiChat.send")}</button>}
    </div>
  </form>;
}
