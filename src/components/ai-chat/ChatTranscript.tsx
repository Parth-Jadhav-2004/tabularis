import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import type { AiChatState } from "../../types/aiChat";
import type { AiEditorTarget } from "../../types/aiEditor";
import { ChatMessage } from "./ChatMessage";
import { aiNotebookMessageReferences } from "../../utils/aiNotebookReferences";
import { MessageSquare } from "lucide-react";

export function ChatTranscript({ chat, target, targetKey, canAct, canNavigateNotebook = false }: { chat: AiChatState; target?: AiEditorTarget; targetKey?: string; canAct: boolean; canNavigateNotebook?: boolean }) {
  const { t } = useTranslation();
  const viewport = useRef<HTMLDivElement>(null);
  const follow = useRef(true);
  const notebook = canNavigateNotebook ? target?.notebook : undefined;
  const availableIds = new Set(notebook?.readState?.().cells.map((cell) => cell.id));
  useEffect(() => {
    if (follow.current && viewport.current) viewport.current.scrollTop = viewport.current.scrollHeight;
  }, [chat.conversation?.messages]);
  return <div ref={viewport} className="min-h-0 flex-1 overflow-y-auto" role="region" aria-label={t("aiChat.transcript")}
    onScroll={() => {
      const element = viewport.current;
      if (element) follow.current = element.scrollHeight - element.scrollTop - element.clientHeight < 80;
    }}>
    {!chat.conversation?.messages.length && <div className="flex min-h-64 flex-col items-center justify-center gap-4 px-8 py-12 text-center">
      <MessageSquare size={24} strokeWidth={1.5} aria-hidden="true" className="text-muted" />
      <div className="space-y-2">
        <h3 className="text-base font-medium text-primary">{t("aiChat.welcome")}</h3>
        <p className="text-sm leading-relaxed text-secondary">{t("aiChat.empty")}</p>
      </div>
    </div>}
    {chat.conversation?.messages.map((message, index, messages) => <ChatMessage key={message.id} message={message}
      notebookReferences={notebook?.revealCell ? { cells: aiNotebookMessageReferences(messages, index, notebook.id, availableIds), reveal: notebook.revealCell } : undefined}
      dialect={target?.dialect} actions={canAct && message.id !== chat.activeRun?.assistantId
        && (!chat.replyTargets[message.id] || chat.replyTargets[message.id] === targetKey) ? target?.actions : undefined} />)}
    {chat.notices.map((notice, index) => <p key={index} className="px-4 py-2 text-xs text-secondary">{t(`aiChat.notices.${notice.reason}`)}</p>)}
    {/* Announce terminal state instead of every streamed token. */}
    <p role="status" aria-live="polite" className={chat.activeRun ? "px-5 pb-3 text-xs text-muted" : "sr-only"}>
      {chat.activeRun ? t("aiChat.streaming") : chat.finish ? t(`aiChat.finish.${chat.finish}`) : ""}
    </p>
  </div>;
}
