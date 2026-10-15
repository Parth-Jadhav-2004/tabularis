import { X } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { AiContextSelection } from "../../types/aiContext";
import { aiContextLabelKey } from "../../utils/aiContext";

export function ContextChips({ include, onChange, disabled, hasError }: {
  include: AiContextSelection; onChange: (include: AiContextSelection) => void; disabled: boolean; hasError: boolean;
}) {
  const { t } = useTranslation();
  const kinds = ["connection", "schema", "query", "selection", "cursor_statement", ...(hasError ? ["error" as const] : [])] as const;
  return <div role="group" aria-label={t("aiChat.context.nextMessage")} className="space-y-1 border-t border-default px-3 py-2">
    <p className="text-xs text-muted">{t("aiChat.context.nextMessage")}</p>
    <div className="flex flex-wrap gap-1">
      {kinds.map((kind) => <button key={kind} type="button" aria-pressed={include[kind]} disabled={disabled}
        onClick={() => onChange({ ...include, [kind]: !include[kind] })}
        title={t(include[kind] ? "aiChat.context.remove" : "aiChat.context.include", { context: t(aiContextLabelKey(kind)) })}
        className={`inline-flex items-center gap-1 rounded border px-2 py-1 text-xs disabled:opacity-40 focus-ring ${include[kind] ? "border-strong bg-surface-secondary text-primary" : "border-default bg-base text-muted"}`}>
        {t(aiContextLabelKey(kind))}{include[kind] && <X size={10} aria-hidden="true" />}
      </button>)}
    </div>
  </div>;
}
