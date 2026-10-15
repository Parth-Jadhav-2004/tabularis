import { X } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { AiContextTables, AiMentionItem } from "../../types/aiContext";

export function ComposerReferenceChips({ choices, disabled, onRemove }: {
  choices: AiContextTables; disabled: boolean; onRemove: (item: AiMentionItem) => void;
}) {
  const { t } = useTranslation();
  const items: AiMentionItem[] = [
    ...choices.tables.map((id) => ({ kind: "table" as const, id, label: id })),
    ...(choices.views ?? []).map((id) => ({ kind: "view" as const, id, label: id })),
    ...(choices.saved_queries ?? []).map((query) => ({ kind: "saved_query" as const, id: query.id, label: query.label })),
  ];
  if (!items.length) return null;
  return <div className="flex max-h-20 flex-wrap gap-1 overflow-y-auto px-1 pt-1">
    {items.map((item) => <button key={`${item.kind}:${item.id}`} type="button" disabled={disabled}
      aria-label={t("aiChat.context.removeReference", { name: item.label })} onClick={() => onRemove(item)}
      className="inline-flex max-w-full items-center gap-1 rounded-lg bg-surface-secondary px-2 py-1 text-xs text-secondary disabled:opacity-40 focus-ring">
      <span className="truncate">@{item.label}</span><X size={10} aria-hidden="true" className="shrink-0" />
    </button>)}
  </div>;
}
