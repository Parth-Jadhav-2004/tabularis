import { RefreshCw, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { AiContextPreview, AiContextSelection, AiContextTables, AiMentionItem } from "../../types/aiContext";
import { aiContextLabelKey, aiContextSize, aiContextTableCount, aiContextViewCount } from "../../utils/aiContext";
import { ContextReferenceChips } from "./ContextReferenceChips";

export function ContextChips({ include, onChange, disabled, hasError, hasInstructions, tables, onTable, preview, loading, error, onRefresh, onMention, notebook }: {
  include: AiContextSelection; onChange: (include: AiContextSelection) => void; disabled: boolean; hasError: boolean;
  tables?: AiContextTables; onTable?: (name: string, selected: boolean) => boolean;
  preview?: AiContextPreview | null; loading?: boolean; error?: boolean; onRefresh?: () => void;
  onMention?: (item: AiMentionItem, selected: boolean) => boolean;
  hasInstructions?: boolean;
  notebook?: boolean;
}) {
  const { t } = useTranslation();
  const instructions = preview?.items.find((item) => item.kind === "instructions");
  const kinds: (keyof AiContextSelection)[] = ["connection", ...(hasInstructions || instructions ? ["instructions" as const] : []), "schema",
    ...(notebook ? ["notebook", "selection", "notebook_results"] as const : ["query", "selection", "cursor_statement"] as const),
    ...(hasError ? ["error" as const] : [])];
  const details = preview?.items.filter((item) => item.kind === "table") ?? [];
  const names = [...new Set([...(tables?.tables ?? []), ...details.map((item) => item.id), ...(tables?.excluded_tables ?? [])])];
  const schema = preview?.items.find((item) => item.kind === "schema");
  const count = schema ? aiContextTableCount(schema) : null;
  const viewCount = schema ? aiContextViewCount(schema) : null;
  const notebookItems = preview?.items.filter((item) => item.kind.startsWith("notebook")) ?? [];
  return <div role="group" aria-label={t("aiChat.context.nextMessage")} className="space-y-1 border-t border-default px-3 py-2">
    <div className="flex items-center justify-between gap-2">
      <p className="text-xs text-muted">{t("aiChat.context.nextMessage")}</p>
      {onRefresh && <button type="button" onClick={onRefresh} disabled={disabled || loading} aria-label={t("aiChat.context.refreshPreview")}
        className="rounded p-1 text-secondary disabled:opacity-40 focus-ring"><RefreshCw size={12} aria-hidden="true" /></button>}
    </div>
    <div className="flex flex-wrap gap-1">
      {kinds.map((kind) => {
        const selected = kind === "notebook" ? include.notebook !== false : !!include[kind];
        return <button key={kind} type="button" aria-pressed={selected}
        disabled={disabled || (kind === "notebook_results" && include.notebook === false)}
        onClick={() => onChange({ ...include, [kind]: !selected, ...(kind === "notebook" && selected ? { notebook_results: false } : {}) })}
        title={kind === "notebook_results" ? t("aiChat.context.notebookResultsDescription")
          : t(selected ? "aiChat.context.remove" : "aiChat.context.include", { context: t(aiContextLabelKey(kind)) })}
        className={`inline-flex items-center gap-1 rounded border px-2 py-1 text-xs disabled:opacity-40 focus-ring ${selected ? "border-strong bg-surface-secondary text-primary" : "border-default bg-base text-muted"}`}>
        {t(aiContextLabelKey(kind))}
        {kind === "notebook" && selected && notebookItems.length > 0 && <span>{t("aiChat.context.characters", { count: notebookItems.reduce((sum, item) => sum + (aiContextSize(item) ?? 0), 0) })}</span>}
        {kind === "notebook" && notebookItems.some((item) => item.truncated) && <span className="text-accent-warning">{t("aiChat.context.truncated")}</span>}
        {kind === "instructions" && include.instructions && instructions && <span>{t("aiChat.context.characters", { count: aiContextSize(instructions) ?? 0 })}</span>}
        {kind === "instructions" && instructions?.truncated && <span className="text-accent-warning">{t("aiChat.context.truncated")}</span>}
        {kind === "schema" && count !== null && <span>({t("aiChat.context.tableCount", { count })})</span>}
        {kind === "schema" && viewCount !== null && viewCount > 0 && <span>· {t("aiChat.context.viewCount", { count: viewCount })}</span>}
        {kind === "schema" && schema?.truncated && <span className="text-accent-warning">{t("aiChat.context.truncated")}</span>}
        {selected && <X size={10} aria-hidden="true" />}
      </button>; })}
    </div>
    {onTable && names.length > 0 && <div role="group" aria-label={t("aiChat.context.tableDetails")} className="flex max-h-28 flex-wrap gap-1 overflow-y-auto py-1">
      {names.map((name) => {
        const selected = !tables?.excluded_tables.includes(name);
        const item = details.find((detail) => detail.id === name);
        const size = item ? aiContextSize(item) : null;
        return <button key={name} type="button" disabled={disabled} aria-pressed={selected}
          aria-label={t("aiChat.context.tableDetail", { table: name })} onClick={() => onTable(name, !selected)}
          title={item ? t("aiChat.context.characters", { count: size ?? 0 }) : t("aiChat.context.detailNotSent")}
          className={`inline-flex max-w-full items-center gap-1 rounded border px-2 py-1 text-xs disabled:opacity-40 focus-ring ${selected ? "border-strong bg-surface-secondary text-primary" : "border-default bg-base text-muted"}`}>
          <span className="truncate font-mono">{name}</span>
          {size !== null && <span className="shrink-0 text-muted">{t("aiChat.context.characters", { count: size })}</span>}
          {item?.truncated && <span className="text-accent-warning">{t("aiChat.context.truncated")}</span>}
          {selected && preview && !item && <span className="text-accent-warning">{t("aiChat.context.notInPreview")}</span>}
          {selected && <X size={10} className="shrink-0" aria-hidden="true" />}
        </button>;
      })}
    </div>}
    {onMention && tables && <ContextReferenceChips choices={tables} preview={preview} disabled={disabled} onMention={onMention} />}
    {onRefresh && <p role="status" className={`text-xs ${error ? "text-accent-warning" : "text-muted"}`}>
      {loading ? t("aiChat.context.previewLoading") : error ? t("aiChat.context.previewError") : preview
        ? <>{t("aiChat.context.previewSize", { count: preview.total_chars })}{preview.truncated && <> · {t("aiChat.context.truncated")}</>}</> : null}
    </p>}
  </div>;
}
