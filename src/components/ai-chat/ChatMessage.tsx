import { useTranslation } from "react-i18next";
import type { AiMessage } from "../../types/aiChat";
import type { AiSqlActions } from "../../types/aiEditor";
import { TextBlock } from "./blocks/TextBlock";
import { ReasoningBlock } from "./blocks/ReasoningBlock";
import { ErrorBlock } from "./blocks/ErrorBlock";
import { ContextBlock } from "./blocks/ContextBlock";
import { parseAiCommand } from "../../utils/aiCommands";
import type { AiNotebookReferences } from "../../types/aiNotebookReferences";

export function ChatMessage({ message, actions, dialect, notebookReferences }: { message: AiMessage; actions?: AiSqlActions; dialect?: string; notebookReferences?: AiNotebookReferences }) {
  const { t } = useTranslation();
  const assistant = message.role === "assistant";
  return <article className={assistant ? "px-5 py-5" : "px-4 pb-1 pt-5"} aria-label={t(assistant ? "aiChat.assistant" : "aiChat.you")}>
    <p className={assistant ? "mb-2 text-xs font-medium text-secondary" : "sr-only"}>{t(assistant ? "aiChat.assistant" : "aiChat.you")}</p>
    <div className={assistant ? "" : "ml-6 rounded-xl bg-surface-secondary px-3 py-2.5"}>
    {message.blocks.map((block, index) => {
      switch (block.type) {
        case "text": {
          if (assistant) return <TextBlock key={index} text={block.text} actions={actions} dialect={dialect} notebookReferences={notebookReferences} />;
          const parsed = message.role === "user" && index === 0 ? parseAiCommand(block.text) : null;
          return parsed?.kind === "command"
            ? <div key={index} className="space-y-2">
              <span className="inline-flex rounded border border-strong bg-surface-secondary px-2 py-0.5 font-mono text-xs text-primary"
                aria-label={t(`aiChat.commands.${parsed.command}`)}>/{parsed.command}</span>
              {parsed.args && <p className="whitespace-pre-wrap break-words text-sm text-primary">{parsed.args}</p>}
            </div>
            : <p key={index} className="whitespace-pre-wrap break-words text-sm text-primary">{block.text}</p>;
        }
        case "reasoning": return <ReasoningBlock key={index} text={block.text} />;
        case "error": return <ErrorBlock key={index} message={block.message} />;
        case "context": return <ContextBlock key={index} items={block.items} />;
        default: return null;
      }
    })}
    </div>
    {assistant && message.model && <p className="mt-3 text-xs text-muted">{message.provider} · {message.model}</p>}
  </article>;
}
