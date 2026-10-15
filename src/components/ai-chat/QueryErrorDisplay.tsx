import type { TFunction } from "i18next";
import { useAiAvailability } from "../../hooks/useAiAvailability";
import { useAiEditor } from "../../hooks/useAiEditor";
import { useOpenAiChat } from "../../hooks/useAiChatEntry";
import { aiFixEntry } from "../../utils/aiEntryPoints";
import { ErrorDisplay } from "../ui/ErrorDisplay";

export function QueryErrorDisplay({ error, query, connectionId, t }: {
  error: string; query?: string; connectionId: string | null; t: TFunction;
}) {
  const { available } = useAiAvailability();
  const target = useAiEditor();
  const launcher = useOpenAiChat();
  const getSql = () => query ?? (target?.readErrorContext?.() ?? target?.readContext?.())?.query;
  const canFix = available && !!connectionId && target?.connectionId === connectionId
    && !!getSql()?.trim();
  const fix = () => {
    if (!canFix || !target || launcher.disabled) return;
    const sql = getSql();
    if (sql?.trim()) launcher.open("fix", target, aiFixEntry(sql, error));
  };
  return <ErrorDisplay error={error} t={t} onFixWithAi={canFix ? fix : undefined} aiDisabled={launcher.disabled} />;
}
