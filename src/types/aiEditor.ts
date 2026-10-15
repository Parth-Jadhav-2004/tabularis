import type { AiContextSelection, AiEditorSnapshot } from "./aiContext";
import type { AiNotebookTarget } from "./aiNotebook";

export interface AiSqlActions {
  insert: (sql: string) => void;
  replace: (sql: string) => void;
  run: (sql: string) => void;
  focus: () => void;
}

export interface AiEditorTarget {
  id?: string;
  notebook?: AiNotebookTarget;
  sqlActionsAvailable?: boolean;
  readContext?: () => AiEditorSnapshot;
  readErrorContext?: () => AiEditorSnapshot;
  hasError?: boolean;
  connectionId: string;
  database: string | null;
  schema: string | null;
  dialect?: string;
  actions: AiSqlActions;
}

export interface AiChatEntryOptions {
  snapshot?: AiEditorSnapshot;
  include?: AiContextSelection;
  tables?: string[];
}

export type AiEditorMenuAction = "explain" | "optimize" | "ask";
export interface AiEditorMenuActions {
  disabled: boolean;
  run: (action: AiEditorMenuAction, snapshot: AiEditorSnapshot) => void;
}

export interface AiChatEntry extends AiChatEntryOptions {
  scopeId: string;
  target: AiEditorTarget;
  snapshot: AiEditorSnapshot;
  command: import("./aiCommands").AiRemoteCommand | null;
}

export interface AiEditorStore {
  register: (scopeId: string, target: AiEditorTarget) => () => void;
  get: (scopeId: string) => AiEditorTarget | undefined;
  subscribe: (listener: () => void) => () => void;
  requestEntry: (scopeId: string, target: AiEditorTarget, command: AiChatEntry["command"], options?: AiChatEntryOptions) => void;
  setContextTarget: (scopeId: string, target: AiEditorTarget) => void;
  getEntry: () => AiChatEntry | null;
  consumeEntry: (entry: AiChatEntry) => boolean;
}
