// Wire types mirror ai/message.rs, ai/provider.rs and ai_chat/types.rs.
// Fields and discriminators intentionally retain Rust's snake_case format.
import type { AiContextOptions, AiContextSelection, AiContextSpec, AiContextTableChoices } from "./aiContext";
export type AiJsonValue = null | boolean | number | string | AiJsonValue[] | {
  [key: string]: AiJsonValue;
};

export interface AiUsage {
  input_tokens: number;
  output_tokens: number;
  cache_read_tokens: number;
  cache_write_tokens: number;
}

export interface AiContextItemRef {
  kind: string;
  id: string;
  label: string;
  chars?: number | null;
  table_count?: number | null;
  view_count?: number | null;
  truncated?: boolean;
}

export type AiBlock =
  | { type: "text"; text: string }
  | { type: "reasoning"; text: string; signature: string | null }
  | { type: "tool_call"; id: string; name: string; args: AiJsonValue;
      status: "pending" | "running" | "completed" | "rejected" | "failed" }
  | { type: "tool_result"; call_id: string; content: string; truncated: boolean; is_error: boolean }
  | { type: "image"; blob_id: string; mime: string; width: number; height: number }
  | { type: "attachment"; kind: "text" | "csv" | "sql"; name: string; blob_id: string | null; size: number }
  | { type: "context"; items: AiContextItemRef[] }
  | { type: "error"; message: string };

export interface AiMessage {
  id: string;
  role: "user" | "assistant" | "tool";
  blocks: AiBlock[];
  created_at: number;
  usage: AiUsage | null;
  provider: string | null;
  model: string | null;
}

export type AiFinishReason = "stop" | "length" | "tool_use" | "cancelled" | "error" | "content_filter";
export type AiDropReason = "images_unsupported" | "image_unavailable" | "reasoning_omitted"
  | "tools_flattened" | "tools_unsupported" | "reasoning_unsupported";

export interface AiProviderError {
  kind: "auth" | "rate_limit" | "context_length" | "network" | "other";
  message: string;
}

export type AiChatEvent =
  | { type: "context"; message_id: string; items: AiContextItemRef[] }
  | { type: "text_delta"; text: string }
  | { type: "reasoning_delta"; text: string }
  | { type: "reasoning_signature"; signature: string }
  | { type: "tool_call_start"; id: string; name: string }
  | { type: "tool_call_args_delta"; id: string; delta: string }
  | { type: "tool_call_end"; id: string; args: AiJsonValue }
  | { type: "usage"; usage: AiUsage }
  | { type: "notice"; message_id: string | null; reason: AiDropReason }
  | { type: "done"; finish: AiFinishReason }
  | { type: "error"; error: AiProviderError };

export interface AiCapabilities {
  tools: boolean;
  images: boolean;
  reasoning: boolean;
  context_window: number | null;
}

export interface AiChatRequest {
  provider: string;
  model: string;
  system: string;
  messages: AiMessage[];
  tools: { name: string; description: string; parameters: AiJsonValue }[];
  max_output_tokens?: number | null;
  reasoning?: "low" | "medium" | "high" | null;
}

export type AiChatTarget = { type: "editor" } | { type: "notebook"; notebook_id: string };
export type AiChatMode = "ask" | "agent";
export type AiTitleSource = "first_message" | "generated" | "user";

export interface AiConversation {
  schema_version: number;
  id: string;
  connection_id: string;
  database: string | null;
  schema: string | null;
  title: string;
  title_source: AiTitleSource;
  mode: AiChatMode;
  target: AiChatTarget | null;
  provider: string | null;
  model: string | null;
  created_at: number;
  updated_at: number;
  messages: AiMessage[];
}

export interface AiConversationSummary extends Omit<AiConversation, "schema_version" | "messages"> {
  message_count: number;
}

export interface AiChatSaveResult {
  bytes: number;
  large: boolean;
}

export interface AiModelChoice {
  provider: string;
  model: string;
}

export interface AiNewConversation extends AiModelChoice {
  connection_id: string;
  database?: string | null;
  schema?: string | null;
  target?: AiChatTarget | null;
}

export interface AiSendOptions {
  editorTarget?: string;
  context?: AiContextOptions;
  system?: string;
  max_output_tokens?: number;
  reasoning?: "low" | "medium" | "high";
}

export type AiHistoryError = "key_unavailable" | "corrupted" | "wrong_context" | "io"
  | "invalid_input" | "too_large" | "unsupported_version";

export interface AiResumeOptions {
  choices: AiModelChoice[];
  fallback: AiModelChoice | null;
}

export interface AiChatHistoryState {
  scope: string | null;
  items: AiConversationSummary[];
  loading: boolean;
  busy: boolean;
  restoring: boolean;
  error: AiHistoryError | null;
  modelFallback: boolean;
}

export interface AiChatHistoryTransport {
  list: (connectionId: string | null) => Promise<AiConversationSummary[]>;
  load: (connectionId: string, id: string) => Promise<AiConversation>;
  rename: (connectionId: string, id: string, title: string) => Promise<void>;
  delete: (connectionId: string, id: string) => Promise<void>;
}

// Local failures are translation identifiers, never raw IPC exceptions or request data.
export type AiChatFailure =
  | { kind: "subscription" | "send" | "cancel" | "storage_subscription" }
  | { kind: "provider"; error: AiProviderError }
  | { kind: "stream" };

export interface AiChatState {
  replyTargets: Record<string, string>;
  contextInclude: AiContextSelection;
  contextTables: AiContextTableChoices | null;
  ready: boolean;
  conversation: AiConversation | null;
  draft: string;
  runState: "idle" | "streaming" | "error";
  activeRun: { id: string; assistantId: string } | null;
  finish: AiFinishReason | null;
  error: AiChatFailure | null;
  notices: { assistantId: string; message_id: string | null; reason: AiDropReason }[];
  saveState: "idle" | "saving" | "saved" | "error";
  saveResult: AiChatSaveResult | null;
  history: AiChatHistoryState;
}

export interface AiChatActions {
  setContextInclude: (include: AiContextSelection) => void;
  setContextTables: (tables: AiContextTableChoices | null) => void;
  setDraft: (draft: string) => void;
  newConversation: (options: AiNewConversation) => void;
  selectConversation: (conversation: AiConversation | null) => void;
  selectModel: (choice: AiModelChoice) => void;
  send: (text: string, options?: AiSendOptions) => Promise<boolean>;
  stop: () => Promise<void>;
  retrySave: () => Promise<void>;
  activateConnection: (connectionId: string | null, options: AiResumeOptions, force?: boolean) => Promise<void>;
  refreshHistory: (connectionId: string | null) => Promise<void>;
  resumeConversation: (summary: AiConversationSummary, options: AiResumeOptions) => Promise<boolean>;
  renameConversation: (summary: AiConversationSummary, title: string) => Promise<boolean>;
  deleteConversation: (summary: AiConversationSummary) => Promise<boolean>;
}

export type AiChatContextValue = AiChatState & AiChatActions;

export interface AiChatTransport {
  generateTitle?: (connectionId: string, id: string) => Promise<AiConversationSummary | null>;
  history?: AiChatHistoryTransport;
  listen: (runId: string, onEvent: (event: AiChatEvent) => void) => Promise<() => void>;
  listenStorageCleared: (onClear: () => void) => Promise<() => void>;
  send: (request: AiChatRequest, runId: string, context?: AiContextSpec) => Promise<string>;
  cancel: (runId: string) => Promise<void>;
  save: (conversation: AiConversation) => Promise<AiChatSaveResult>;
}
