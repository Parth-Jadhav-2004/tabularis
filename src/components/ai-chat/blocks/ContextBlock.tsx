import { useTranslation } from "react-i18next";
import type { AiContextItemRef } from "../../../types/aiChat";
import { aiContextLabelKey, aiContextSize } from "../../../utils/aiContext";

export function ContextBlock({ items }: { items: AiContextItemRef[] }) {
  const { t } = useTranslation();
  return <details className="mt-2 rounded border border-default bg-base p-2 text-xs text-secondary">
    <summary className="cursor-pointer rounded focus-ring">{t("aiChat.context.sent")}</summary>
    {items.length === 0 ? <p className="mt-2">{t("aiChat.context.empty")}</p> : <ul className="mt-2 space-y-1">
      {items.map((item, index) => {
        const size = aiContextSize(item);
        return <li key={`${item.kind}:${item.id}:${index}`} className="break-words">
          <span>{t(aiContextLabelKey(item.kind))}</span>
          {(item.kind === "table" || item.kind === "connection") && <> · {item.label}</>}
          {size !== null && <> · {t("aiChat.context.characters", { count: size })}</>}
          {item.truncated && <span className="ml-1 text-accent-warning">{t("aiChat.context.truncated")}</span>}
        </li>;
      })}
    </ul>}
  </details>;
}
