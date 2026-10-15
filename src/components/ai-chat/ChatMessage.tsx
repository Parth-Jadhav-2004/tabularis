import { useTranslation } from "react-i18next";
import type { AiMessage } from "../../types/aiChat";
import type { AiSqlActions } from "../../types/aiEditor";
import { TextBlock } from "./blocks/TextBlock";
import { ReasoningBlock } from "./blocks/ReasoningBlock";
import { ErrorBlock } from "./blocks/ErrorBlock";
import { ContextBlock } from "./blocks/ContextBlock";

export function ChatMessage({ message, actions, dialect }: { message: AiMessage; actions?: AiSqlActions; dialect?: string }) {
  const { t } = useTranslation();
  const assistant = message.role === "assistant";
  return <article className="border-b border-default px-4 py-4" aria-label={t(assistant ? "aiChat.assistant" : "aiChat.you")}>
    <p className="mb-2 text-xs font-semibold text-secondary">{t(assistant ? "aiChat.assistant" : "aiChat.you")}</p>
    {message.blocks.map((block, index) => {
      switch (block.type) {
        case "text": return assistant
          ? <TextBlock key={index} text={block.text} actions={actions} dialect={dialect} />
          : <p key={index} className="whitespace-pre-wrap break-words text-sm text-primary">{block.text}</p>;
        case "reasoning": return <ReasoningBlock key={index} text={block.text} />;
        case "error": return <ErrorBlock key={index} message={block.message} />;
        case "context": return <ContextBlock key={index} items={block.items} />;
        default: return null;
      }
    })}
    {assistant && message.model && <p className="mt-2 text-xs text-muted">{message.provider} · {message.model}</p>}
  </article>;
}
