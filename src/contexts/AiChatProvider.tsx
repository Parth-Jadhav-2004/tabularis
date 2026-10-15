import { type ReactNode, useEffect, useMemo, useState, useSyncExternalStore } from "react";
import type { AiChatTransport } from "../types/aiChat";
import { createAiChatSession } from "../utils/aiChatSession";
import { aiChatTransport } from "../utils/aiChatTransport";
import { AiChatContext } from "./AiChatContext";

// Mounted above RightSidebar so panel switches preserve the shared session.
// Transport is fixed for the provider's lifetime, including StrictMode replay.
export const AiChatProvider = ({ children, transport = aiChatTransport }: {
  children: ReactNode;
  transport?: AiChatTransport;
}) => {
  const [session] = useState(() => createAiChatSession(transport));
  const state = useSyncExternalStore(session.subscribe, session.getSnapshot, session.getSnapshot);
  useEffect(() => session.mount(), [session]);
  const value = useMemo(() => ({
    ...state,
    setDraft: session.setDraft,
    setContextInclude: session.setContextInclude,
    newConversation: session.newConversation,
    selectConversation: session.selectConversation,
    selectModel: session.selectModel,
    send: session.send,
    stop: session.stop,
    retrySave: session.retrySave,
    activateConnection: session.activateConnection,
    refreshHistory: session.refreshHistory,
    resumeConversation: session.resumeConversation,
    renameConversation: session.renameConversation,
    deleteConversation: session.deleteConversation,
  }), [state, session]);

  return <AiChatContext.Provider value={value}>{children}</AiChatContext.Provider>;
};
