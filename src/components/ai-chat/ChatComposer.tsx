import { useEffect, useId, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { activeAiMention, filterAiMentions, removeAiMention } from "../../utils/aiContextMentions";
import type { AiMentionItem } from "../../types/aiContext";
import { aiContextLabelKey } from "../../utils/aiContext";
import { activeAiCommands } from "../../utils/aiCommands";
import type { AiSlashCommand } from "../../types/aiCommands";

export function ChatComposer({ draft, onDraft, onSend, onStop, onEscape, streaming, disabled, children, mentionTables = [], onTable, onDropTable, onFocus, mentionItems, onMention, focusRequest = 0 }: {
  draft: string; onDraft: (text: string) => void; onSend: () => void; onStop: () => void; onEscape: () => void;
  streaming: boolean; disabled: boolean; children: ReactNode;
  mentionTables?: string[]; onTable?: (name: string, selected: boolean) => boolean;
  onDropTable?: () => void; onFocus?: () => void;
  mentionItems?: AiMentionItem[]; onMention?: (item: AiMentionItem, selected: boolean) => boolean;
  focusRequest?: number;
}) {
  const { t } = useTranslation();
  const id = useId();
  const listId = `${id}-mentions`;
  const textarea = useRef<HTMLTextAreaElement>(null);
  const lastFocusRequest = useRef(0);
  useEffect(() => {
    if (!focusRequest || focusRequest === lastFocusRequest.current || disabled) return;
    lastFocusRequest.current = focusRequest;
    textarea.current?.focus();
    const length = textarea.current?.value.length ?? 0;
    textarea.current?.setSelectionRange(length, length);
  }, [focusRequest, disabled]);
  const [caret, setCaret] = useState({ text: draft, start: draft.length, end: draft.length });
  const [dismissed, setDismissed] = useState<string | null>(null);
  const [highlight, setHighlight] = useState({ key: "", index: 0 });
  const [limitReached, setLimitReached] = useState(false);
  const [focused, setFocused] = useState(false);
  const range = caret.text === draft ? activeAiMention(draft, caret.start, caret.end) : null;
  const commands = caret.text === draft ? activeAiCommands(draft, caret.start, caret.end) : null;
  const key = range ? `${range.start}:${range.end}:${range.search}` : commands ? `command:${draft}` : "";
  const commandOpen = focused && commands !== null && dismissed !== key && !disabled && !streaming;
  const mentionOpen = focused && !!range && !!(onMention || onTable) && dismissed !== key && !disabled && !streaming;
  const open = commandOpen || mentionOpen;
  const options = mentionOpen ? filterAiMentions(mentionItems ?? mentionTables.map((name) => ({ kind: "table", id: name, label: name })), range.search) : [];
  const optionCount = commandOpen ? commands.length : options.length;
  const index = highlight.key === key ? Math.min(highlight.index, Math.max(0, optionCount - 1)) : 0;
  useEffect(() => {
    if (open) document.getElementById(`${listId}-${index}`)?.scrollIntoView?.({ block: "nearest" });
  }, [open, listId, index]);
  const pick = (item: AiMentionItem) => {
    if (!range || !(onMention || onTable) || disabled || streaming) return;
    if (!(onMention ? onMention(item, true) : onTable?.(item.id, true))) { setLimitReached(true); return; }
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
  const pickCommand = (command: AiSlashCommand) => {
    const text = `/${command} `;
    onDraft(text);
    setCaret({ text, start: text.length, end: text.length });
    setDismissed(null);
    setHighlight({ key: "", index: 0 });
    requestAnimationFrame(() => {
      if (textarea.current?.value === text) {
        textarea.current.focus();
        textarea.current.setSelectionRange(text.length, text.length);
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
      <p className="px-2 py-1 text-xs text-muted">{t(commandOpen ? "aiChat.commands.title" : "aiChat.context.mentions")}</p>
      <div id={listId} role="listbox" aria-label={t(commandOpen ? "aiChat.commands.title" : "aiChat.context.mentionItems")} className="max-h-48 overflow-y-auto">
        {commandOpen && commands.map((command, option) => <button key={command} id={`${listId}-${option}`} type="button" role="option" aria-selected={index === option}
          aria-label={`/${command} · ${t(`aiChat.commands.${command}`)}`}
          onMouseDown={(event) => event.preventDefault()} onClick={() => pickCommand(command)}
          className={`flex w-full flex-col rounded px-2 py-1.5 text-left text-sm focus-ring ${index === option ? "bg-accent-primary text-inverse" : "text-primary hover:bg-surface-secondary"}`}>
          <span className="font-mono">/{command}</span>
          <span className="text-xs">{t(`aiChat.commands.${command}`)}</span>
        </button>)}
        {options.map((item, option) => <button key={`${item.kind}:${item.id}`} id={`${listId}-${option}`} type="button" role="option" aria-selected={index === option}
          aria-label={mentionItems ? `${item.label} · ${t(aiContextLabelKey(item.kind))}` : item.label}
          onMouseDown={(event) => event.preventDefault()} onClick={() => pick(item)}
          className={`flex w-full items-center gap-2 rounded px-2 py-1.5 text-left text-sm focus-ring ${index === option ? "bg-accent-primary text-inverse" : "text-primary hover:bg-surface-secondary"}`}>
          <span className="min-w-0 flex-1 truncate font-mono">{item.label}</span>
          {mentionItems && <span aria-hidden="true" className="shrink-0 text-xs">{t(aiContextLabelKey(item.kind))}</span>}
        </button>)}
      </div>
      {optionCount === 0 && <p role="status" className="px-2 py-2 text-xs text-secondary">{t(commandOpen ? "aiChat.commands.unknown" : mentionItems ? "aiChat.context.noMentions" : "aiChat.context.noMentionTables")}</p>}
      {limitReached && <p role="status" className="px-2 py-1 text-xs text-accent-warning">{t(mentionItems ? "aiChat.context.mentionLimit" : "aiChat.context.tableLimit")}</p>}
    </div>}
    <textarea ref={textarea} id={id} value={draft} role="combobox" aria-autocomplete="list" aria-haspopup="listbox"
      aria-expanded={open} aria-controls={open ? listId : undefined}
      aria-activedescendant={open && optionCount > 0 ? `${listId}-${index}` : undefined}
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
            if (optionCount) setHighlight({ key, index: (index + (event.key === "ArrowDown" ? 1 : optionCount - 1)) % optionCount });
            return;
          }
          if ((event.key === "Enter" || event.key === "Tab") && !event.shiftKey && !event.ctrlKey && !event.metaKey && !event.altKey && optionCount > 0) {
            event.preventDefault(); event.stopPropagation();
            if (commandOpen) pickCommand(commands[index]);
            else pick(options[index]);
            return;
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
    {(onMention || onTable) && <p className="text-xs text-muted">{t("aiChat.context.mentionHint")}</p>}
    <p className="text-xs text-muted">{t("aiChat.commands.hint")}</p>
    {children}
    <div className="flex items-center justify-between gap-2">
      <span className="text-xs text-muted">{t("aiChat.askMode")}</span>
      {streaming
        ? <button type="button" onClick={onStop} className="rounded-lg border border-strong px-3 py-1.5 text-sm text-primary focus-ring">{t("aiChat.stop")}</button>
        : <button type="submit" disabled={disabled || !draft.trim()} className="rounded-lg bg-accent-primary px-3 py-1.5 text-sm text-inverse disabled:opacity-40 focus-ring">{t("aiChat.send")}</button>}
    </div>
  </form>;
}
