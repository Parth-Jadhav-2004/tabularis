import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { AiChatEvent, AiChatTransport } from "../types/aiChat";

export const aiChatTransport: AiChatTransport = {
  generateTitle: (connectionId, id) => invoke("ai_chat_generate_title", { connectionId, id }),
  history: {
    list: (connectionId) => invoke("ai_chat_list", { connectionId }),
    load: (connectionId, id) => invoke("ai_chat_load", { connectionId, id }),
    rename: (connectionId, id, title) => invoke("ai_chat_rename", { connectionId, id, title }),
    delete: (connectionId, id) => invoke("ai_chat_delete", { connectionId, id }),
  },
  listen: (runId, onEvent) => listen<AiChatEvent>(`ai-chat://${runId}`, ({ payload }) => onEvent(payload)),
  listenStorageCleared: (onClear) => listen("ai-chat://storage-cleared", onClear),
  send: (req, runId, context) => invoke("ai_chat_send", { req, runId, ...(context ? { context } : {}) }),
  cancel: (runId) => invoke("ai_chat_cancel", { runId }),
  save: (conversation) => invoke("ai_chat_save", { conversation }),
};
