import { RefreshCw, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { AiContextPreview, AiContextSelection, AiContextTables } from "../../types/aiContext";
import { aiContextLabelKey, aiContextSize, aiContextTableCount } from "../../utils/aiContext";

export function ContextChips({ include, onChange, disabled, hasError, tables, onTable, preview, loading, error, onRefresh }: {
  include: AiContextSelection; onChange: (include: AiContextSelection) => void; disabled: boolean; hasError: boolean;
  tables?: AiContextTables; onTable?: (name: string, selected: boolean) => boolean;
  preview?: AiContextPreview | null; loading?: boolean; error?: boolean; onRefresh?: () => void;
}) {
  const { t } = useTranslation();
  const kinds = ["connection", "schema", "query", "selection", "cursor_statement", ...(hasError ? ["error" as const] : [])] as const;
  const details = preview?.items.filter((item) => item.kind === "table") ?? [];
  const names = [...new Set([...(tables?.tables ?? []), ...details.map((item) => item.id), ...(tables?.excluded_tables ?? [])])];
  const schema = preview?.items.find((item) => item.kind === "schema");
  const count = schema ? aiContextTableCount(schema) : null;
  return <div role="group" aria-label={t("aiChat.context.nextMessage")} className="space-y-1 border-t border-default px-3 py-2">
    <div className="flex items-center justify-between gap-2">
      <p className="text-xs text-muted">{t("aiChat.context.nextMessage")}</p>
      {onRefresh && <button type="button" onClick={onRefresh} disabled={disabled || loading} aria-label={t("aiChat.context.refreshPreview")}
        className="rounded p-1 text-secondary disabled:opacity-40 focus-ring"><RefreshCw size={12} aria-hidden="true" /></button>}
    </div>
    <div className="flex flex-wrap gap-1">
      {kinds.map((kind) => <button key={kind} type="button" aria-pressed={include[kind]} disabled={disabled}
        onClick={() => onChange({ ...include, [kind]: !include[kind] })}
        title={t(include[kind] ? "aiChat.context.remove" : "aiChat.context.include", { context: t(aiContextLabelKey(kind)) })}
        className={`inline-flex items-center gap-1 rounded border px-2 py-1 text-xs disabled:opacity-40 focus-ring ${include[kind] ? "border-strong bg-surface-secondary text-primary" : "border-default bg-base text-muted"}`}>
        {t(aiContextLabelKey(kind))}
        {kind === "schema" && count !== null && <span>({t("aiChat.context.tableCount", { count })})</span>}
        {kind === "schema" && schema?.truncated && <span className="text-accent-warning">{t("aiChat.context.truncated")}</span>}
        {include[kind] && <X size={10} aria-hidden="true" />}
      </button>)}
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
    {onRefresh && <p role="status" className={`text-xs ${error ? "text-accent-warning" : "text-muted"}`}>
      {loading ? t("aiChat.context.previewLoading") : error ? t("aiChat.context.previewError") : preview
        ? <>{t("aiChat.context.previewSize", { count: preview.total_chars })}{preview.truncated && <> · {t("aiChat.context.truncated")}</>}</> : null}
    </p>}
  </div>;
}
