import type {
  AiBlock, AiChatEvent, AiChatFailure, AiChatHistoryState, AiChatSaveResult, AiChatState,
  AiConversation, AiConversationSummary, AiMessage, AiModelChoice, AiNewConversation,
} from "../types/aiChat";
import type { AiContextSelection } from "../types/aiContext";
import { defaultAiContextSelection } from "./aiContext";

export function createAiChatState(conversation: AiConversation | null = null): AiChatState {
  return {
    contextInclude: defaultAiContextSelection(),
    ready: false, conversation, draft: "", runState: "idle", activeRun: null,
    finish: null, error: null, notices: [], saveState: "idle", saveResult: null,
    history: { scope: null, items: [], loading: false, busy: false, restoring: false, error: null, modelFallback: false },
  };
}

export function createAiConversation(options: AiNewConversation): AiConversation {
  const now = Date.now();
  return {
    schema_version: 1, id: crypto.randomUUID(), connection_id: options.connection_id,
    database: options.database ?? null, schema: options.schema ?? null,
    title: "", title_source: "first_message", mode: "ask", target: options.target ?? { type: "editor" },
    provider: options.provider, model: options.model, created_at: now, updated_at: now, messages: [],
  };
}

export function createAiMessage(role: AiMessage["role"], blocks: AiBlock[] = []): AiMessage {
  return {
    id: crypto.randomUUID(), role, blocks, created_at: Date.now(),
    usage: null, provider: null, model: null,
  };
}

export type AiChatAction =
  | { type: "context_include"; include: AiContextSelection }
  | { type: "ready"; ready: boolean }
  | { type: "select"; conversation: AiConversation | null }
  | { type: "draft"; draft: string }
  | { type: "model"; choice: AiModelChoice }
  | { type: "start"; runId: string; user: AiMessage; assistant: AiMessage }
  | { type: "event"; runId: string; event: AiChatEvent }
  | { type: "failure"; runId?: string; error: AiChatFailure; keepRun?: boolean }
  | { type: "saving" }
  | { type: "saved"; result: AiChatSaveResult }
  | { type: "save_failed" }
  | { type: "history"; patch: Partial<AiChatHistoryState> }
  | { type: "restore"; state: AiChatState }
  | { type: "generated_title"; summary: AiConversationSummary }
  | { type: "rename"; title: string };

function appendDelta(blocks: AiBlock[], type: "text" | "reasoning", text: string): AiBlock[] {
  if (!text) return blocks;
  const last = blocks.at(-1);
  if (last?.type === type && (last.type !== "reasoning" || last.signature === null)) {
    return [...blocks.slice(0, -1), { ...last, text: last.text + text }];
  }
  return [...blocks, type === "text" ? { type, text } : { type, text, signature: null }];
}

function foldMessage(message: AiMessage, event: AiChatEvent): AiMessage {
  switch (event.type) {
    case "text_delta":
      return { ...message, blocks: appendDelta(message.blocks, "text", event.text) };
    case "reasoning_delta":
      return { ...message, blocks: appendDelta(message.blocks, "reasoning", event.text) };
    case "reasoning_signature": {
      let index = message.blocks.length - 1;
      while (index >= 0 && message.blocks[index].type !== "reasoning") index--;
      return index < 0 ? message : {
        ...message,
        blocks: message.blocks.map((block, i) => i === index && block.type === "reasoning"
          ? { ...block, signature: (block.signature ?? "") + event.signature } : block),
      };
    }
    case "usage":
      return { ...message, usage: event.usage };
    case "error":
      return { ...message, blocks: [...message.blocks, { type: "error", message: event.error.message }] };
    default:
      return message;
  }
}

export function aiChatReducer(state: AiChatState, action: AiChatAction): AiChatState {
  switch (action.type) {
    case "context_include":
      return { ...state, contextInclude: action.include };
    case "generated_title": {
      const { summary } = action;
      const conversation = state.conversation;
      return conversation?.id === summary.id && conversation.connection_id === summary.connection_id
        && conversation.created_at === summary.created_at && conversation.title_source === "first_message"
        ? { ...state, conversation: { ...conversation, title: summary.title, title_source: "generated" } } : state;
    }
    case "history":
      return { ...state, history: { ...state.history, ...action.patch } };
    case "restore":
      return { ...action.state, ready: state.ready, history: { ...state.history, restoring: false, error: null, modelFallback: false } };
    case "rename":
      return state.conversation ? { ...state, conversation: {
        ...state.conversation, title: action.title, title_source: "user",
      } } : state;
    case "ready":
      return { ...state, ready: action.ready };
    case "select":
      return {
        ...createAiChatState(action.conversation), ready: state.ready,
        history: { ...state.history, restoring: false, modelFallback: false, error: null },
        error: state.error?.kind === "storage_subscription" ? state.error : null,
      };
    case "draft":
      return { ...state, draft: action.draft };
    case "model":
      return state.conversation ? {
        ...state, conversation: { ...state.conversation, ...action.choice },
      } : state;
    case "start": {
      if (!state.conversation || state.activeRun) return state;
      const text = action.user.blocks.find((block) => block.type === "text")?.text ?? "";
      const title = state.conversation.title || Array.from(text.replace(/\s+/g, " ").trim()).slice(0, 50).join("");
      return {
        ...state, draft: "", runState: "streaming", error: null, finish: null,
        contextInclude: defaultAiContextSelection(),
        activeRun: { id: action.runId, assistantId: action.assistant.id },
        saveState: "idle", saveResult: null,
        conversation: {
          ...state.conversation, title, updated_at: action.user.created_at,
          messages: [...state.conversation.messages, action.user, action.assistant],
        },
      };
    }
    case "event": {
      if (!state.conversation || state.activeRun?.id !== action.runId) return state;
      const { event } = action;
      if (event.type === "context") {
        const userIndex = state.conversation.messages.length - 2;
        const user = state.conversation.messages[userIndex];
        if (user?.role !== "user" || user.id !== event.message_id) return state;
        return { ...state, conversation: { ...state.conversation,
          messages: state.conversation.messages.map((message, index) => index === userIndex ? {
            ...message, blocks: [...message.blocks.filter((block) => block.type !== "context"), { type: "context", items: event.items }],
          } : message),
        } };
      }
      // Tool execution and approval rendering are introduced in M9.
      if (event.type === "tool_call_start" || event.type === "tool_call_args_delta" || event.type === "tool_call_end") return state;
      if (event.type === "notice") return {
        ...state, notices: [...state.notices, {
          assistantId: state.activeRun.assistantId, message_id: event.message_id, reason: event.reason,
        }],
      };
      if (event.type === "done") return {
        ...state, activeRun: null, finish: event.finish,
        runState: event.finish === "error" || state.error ? "error" : "idle",
        error: state.error ?? (event.finish === "error" ? { kind: "stream" } : null),
      };
      const messages = state.conversation.messages.map((message) =>
        message.id === state.activeRun?.assistantId ? foldMessage(message, event) : message);
      return {
        ...state, conversation: { ...state.conversation, messages },
        error: event.type === "error" ? { kind: "provider", error: event.error } : state.error,
      };
    }
    case "failure":
      if (action.runId && state.activeRun?.id !== action.runId) return state;
      return { ...state, activeRun: action.keepRun ? state.activeRun : null,
        runState: action.keepRun ? state.runState : "error", error: action.error };
    case "saving":
      return { ...state, saveState: "saving", saveResult: null };
    case "saved":
      return { ...state, saveState: "saved", saveResult: action.result };
    case "save_failed":
      return { ...state, saveState: "error", saveResult: null };
  }
}
