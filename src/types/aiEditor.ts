import type { AiEditorSnapshot } from "./aiContext";

export interface AiSqlActions {
  insert: (sql: string) => void;
  replace: (sql: string) => void;
  run: (sql: string) => void;
  focus: () => void;
}

export interface AiEditorTarget {
  readContext?: () => AiEditorSnapshot;
  hasError?: boolean;
  connectionId: string;
  database: string | null;
  schema: string | null;
  dialect?: string;
  actions: AiSqlActions;
}

export interface AiEditorStore {
  register: (scopeId: string, target: AiEditorTarget) => () => void;
  get: (scopeId: string) => AiEditorTarget | undefined;
  subscribe: (listener: () => void) => () => void;
}
