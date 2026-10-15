import { useId } from "react";
import { useTranslation } from "react-i18next";
import { limitAiInstructions, MAX_AI_INSTRUCTION_CHARS } from "../../../utils/connectionAi";

export function AiInstructionsSection({ value, onChange, disabled }: {
  value: string; onChange: (value: string) => void; disabled?: boolean;
}) {
  const { t } = useTranslation();
  const id = useId();
  return <div className="space-y-2 border-t border-default pt-4">
    <label htmlFor={id} className="block text-xs font-semibold uppercase tracking-wider text-muted">
      {t("newConnection.aiInstructions")}
    </label>
    <p id={`${id}-hint`} className="text-xs leading-snug text-secondary">{t("newConnection.aiInstructionsHint")}</p>
    <textarea id={id} value={value} disabled={disabled} rows={4}
      aria-describedby={`${id}-hint ${id}-count`}
      onChange={(event) => onChange(limitAiInstructions(event.target.value))}
      placeholder={t("newConnection.aiInstructionsPlaceholder")}
      className="w-full resize-y rounded-lg border border-strong bg-base px-3 py-2 text-sm text-primary placeholder:text-muted disabled:opacity-40 focus-ring" />
    <p id={`${id}-count`} className="text-right text-xs text-muted">
      {t("newConnection.aiInstructionsCount", { count: Array.from(value).length, max: MAX_AI_INSTRUCTION_CHARS })}
    </p>
  </div>;
}
