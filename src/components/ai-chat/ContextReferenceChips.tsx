import { X } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { AiContextPreview, AiContextTables, AiMentionItem } from "../../types/aiContext";
import { aiContextLabelKey, aiContextSize } from "../../utils/aiContext";

export function ContextReferenceChips({ choices, preview, disabled, onMention }: {
  choices: AiContextTables; preview?: AiContextPreview | null; disabled: boolean;
  onMention: (item: AiMentionItem, selected: boolean) => boolean;
}) {
  const { t } = useTranslation();
  const views = [...new Set([...(choices.views ?? []), ...(preview?.items.filter((item) => item.kind === "view").map((item) => item.id) ?? []), ...(choices.excluded_views ?? [])])];
  const references: AiMentionItem[] = [
    ...views.map((name) => ({ kind: "view" as const, id: name, label: name })),
    ...(choices.saved_queries ?? []).map((query) => ({ kind: "saved_query" as const, ...query })),
  ];
  if (references.length === 0) return null;
  return <div role="group" aria-label={t("aiChat.context.references")} className="flex max-h-28 flex-wrap gap-1 overflow-y-auto py-1">
    {references.map((reference) => {
      const item = preview?.items.find((item) => item.kind === reference.kind && item.id === reference.id);
      const label = item?.label ?? reference.label;
      const selected = reference.kind === "saved_query" || !choices.excluded_views?.includes(reference.id);
      const size = item ? aiContextSize(item) : null;
      return <button key={`${reference.kind}:${reference.id}`} type="button" disabled={disabled} aria-pressed={selected}
        aria-label={t(reference.kind === "view" ? "aiChat.context.viewDetail" : "aiChat.context.savedQuery", { name: label })}
        onClick={() => onMention(reference, !selected)} title={item ? label : t("aiChat.context.detailNotSent")}
        className={`inline-flex max-w-full items-center gap-1 rounded border px-2 py-1 text-xs disabled:opacity-40 focus-ring ${selected ? "border-strong bg-surface-secondary text-primary" : "border-default bg-base text-muted"}`}>
        <span className="shrink-0">{t(aiContextLabelKey(reference.kind))}:</span><span className="truncate font-mono">{label}</span>
        {size !== null && <span className="shrink-0 text-muted">{t("aiChat.context.characters", { count: size })}</span>}
        {item?.truncated && <span className="text-accent-warning">{t("aiChat.context.truncated")}</span>}
        {selected && preview && !item && <span className="text-accent-warning">{t("aiChat.context.notInPreview")}</span>}
        {selected && <X size={10} className="shrink-0" aria-hidden="true" />}
      </button>;
    })}
  </div>;
}
