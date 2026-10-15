export interface AiContextSelection {
  connection: boolean;
  instructions: boolean;
  schema: boolean;
  query: boolean;
  selection: boolean;
  cursor_statement: boolean;
  error: boolean;
  notebook?: boolean;
  notebook_results?: boolean;
}

export interface AiEditorSnapshot {
  query?: string | null;
  selection?: string | null;
  cursor_statement?: string | null;
  last_error?: string | null;
}

export interface AiContextOptions {
  notebook?: import("./aiNotebook").AiNotebookSnapshot;
  command?: import("./aiCommands").AiRemoteCommand;
  language: string;
  include: AiContextSelection;
  tables: string[];
  excluded_tables: string[];
  views: string[];
  excluded_views: string[];
  saved_queries: string[];
  editor: Omit<AiEditorSnapshot, "last_error">;
  last_error: string | null;
}

export interface AiContextSpec extends AiContextOptions {
  connection_id: string;
  database: string | null;
  schema: string | null;
}

export interface AiContextScope {
  connection_id: string;
  database: string | null;
  schema: string | null;
}

export interface AiContextTables {
  notebook?: import("./aiNotebookReferences").AiNotebookCellSelection;
  tables: string[];
  excluded_tables: string[];
  views?: string[];
  excluded_views?: string[];
  saved_queries?: { id: string; label: string }[];
}

export interface AiMentionItem {
  kind: "table" | "view" | "saved_query";
  id: string;
  label: string;
}

export interface AiContextTableChoices extends AiContextTables, AiContextScope {}

export interface AiContextPreview {
  items: import("./aiChat").AiContextItemRef[];
  truncated: boolean;
  total_chars: number;
}
