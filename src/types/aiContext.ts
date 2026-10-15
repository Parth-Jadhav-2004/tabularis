export interface AiContextSelection {
  connection: boolean;
  schema: boolean;
  query: boolean;
  selection: boolean;
  cursor_statement: boolean;
  error: boolean;
}

export interface AiEditorSnapshot {
  query?: string | null;
  selection?: string | null;
  cursor_statement?: string | null;
  last_error?: string | null;
}

export interface AiContextOptions {
  language: string;
  include: AiContextSelection;
  tables: string[];
  excluded_tables: string[];
  editor: Omit<AiEditorSnapshot, "last_error">;
  last_error: string | null;
}

export interface AiContextSpec extends AiContextOptions {
  connection_id: string;
  database: string | null;
  schema: string | null;
}
