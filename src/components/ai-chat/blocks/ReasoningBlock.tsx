import { useTranslation } from "react-i18next";

export function ReasoningBlock({ text }: { text: string }) {
  const { t } = useTranslation();
  return <details className="my-2 text-xs text-secondary">
    <summary className="cursor-pointer rounded py-1 focus-ring">{t("aiChat.reasoning")}</summary>
    <p className="whitespace-pre-wrap break-words py-2">{text}</p>
  </details>;
}
