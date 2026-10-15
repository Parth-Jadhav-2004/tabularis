import { useTranslation } from "react-i18next";
import type { AiContextItemRef } from "../../../types/aiChat";
import { aiContextLabelKey, aiContextSize, aiContextTableCount, aiContextViewCount } from "../../../utils/aiContext";

export function ContextBlock({ items }: { items: AiContextItemRef[] }) {
  const { t } = useTranslation();
  const visibleItems = items.filter((item) => item.kind !== "notebook_cell_ref");
  return <details className="mt-2 text-xs text-muted">
    <summary className="cursor-pointer rounded py-1 hover:text-primary focus-ring">{t("aiChat.context.sent")}</summary>
    {visibleItems.length === 0 ? <p className="mt-2">{t("aiChat.context.empty")}</p> : <ul className="mt-2 space-y-1">
      {visibleItems.map((item, index) => {
        const size = aiContextSize(item);
        const count = aiContextTableCount(item);
        const viewCount = aiContextViewCount(item);
        return <li key={`${item.kind}:${item.id}:${index}`} className="break-words">
          <span>{t(aiContextLabelKey(item.kind))}</span>
          {["table", "view", "saved_query", "connection", "notebook", "notebook_cell"].includes(item.kind) && <> · {item.label}</>}
          {count !== null && <> · {t("aiChat.context.tableCount", { count })}</>}
          {viewCount !== null && viewCount > 0 && <> · {t("aiChat.context.viewCount", { count: viewCount })}</>}
          {size !== null && <> · {t("aiChat.context.characters", { count: size })}</>}
          {item.truncated && <span className="ml-1 text-accent-warning">{t("aiChat.context.truncated")}</span>}
        </li>;
      })}
    </ul>}
  </details>;
}
