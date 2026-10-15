import { useTranslation } from "react-i18next";
import type { AiChatTarget } from "../../types/aiChat";
import type { NotebookMetadata } from "../../types/notebook";

export function NotebookConversationTarget({ target, notebooks }: {
  target?: AiChatTarget | null; notebooks?: readonly NotebookMetadata[];
}) {
  const { t } = useTranslation();
  if (target?.type !== "notebook") return null;
  const notebook = notebooks?.find((item) => item.id === target.notebook_id);
  return <span className="block truncate text-xs text-secondary">
    {notebooks && !notebook ? t("aiChat.history.notebookUnavailable")
      : t("aiChat.history.notebook", { title: notebook?.title ?? target.notebook_id })}
  </span>;
}
