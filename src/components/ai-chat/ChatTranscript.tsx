import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import type { AiChatState } from "../../types/aiChat";
import type { AiEditorTarget } from "../../types/aiEditor";
import { ChatMessage } from "./ChatMessage";

export function ChatTranscript({ chat, target, targetKey, canAct }: { chat: AiChatState; target?: AiEditorTarget; targetKey?: string; canAct: boolean }) {
  const { t } = useTranslation();
  const viewport = useRef<HTMLDivElement>(null);
  const follow = useRef(true);
  useEffect(() => {
    if (follow.current && viewport.current) viewport.current.scrollTop = viewport.current.scrollHeight;
  }, [chat.conversation?.messages]);
  return <div ref={viewport} className="min-h-0 flex-1 overflow-y-auto" role="region" aria-label={t("aiChat.transcript")}
    onScroll={() => {
      const element = viewport.current;
      if (element) follow.current = element.scrollHeight - element.scrollTop - element.clientHeight < 80;
    }}>
    {!chat.conversation?.messages.length && <p className="p-6 text-sm text-secondary">{t("aiChat.empty")}</p>}
    {chat.conversation?.messages.map((message) => <ChatMessage key={message.id} message={message}
      dialect={target?.dialect} actions={canAct && message.id !== chat.activeRun?.assistantId
        && (!chat.replyTargets[message.id] || chat.replyTargets[message.id] === targetKey) ? target?.actions : undefined} />)}
    {chat.notices.map((notice, index) => <p key={index} className="px-4 py-2 text-xs text-secondary">{t(`aiChat.notices.${notice.reason}`)}</p>)}
    {/* Announce terminal state instead of every streamed token. */}
    <p role="status" aria-live="polite" className="px-4 py-2 text-xs text-muted">
      {chat.activeRun ? t("aiChat.streaming") : chat.finish ? t(`aiChat.finish.${chat.finish}`) : ""}
    </p>
  </div>;
}
