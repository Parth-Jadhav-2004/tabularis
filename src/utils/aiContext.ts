import type { AiContextOptions, AiContextScope, AiContextSelection, AiContextTables, AiEditorSnapshot } from "../types/aiContext";
import type { AiContextItemRef } from "../types/aiChat";

export function defaultAiContextSelection(): AiContextSelection {
  return { connection: true, schema: true, query: true, selection: true, cursor_statement: true, error: false };
}

export function createAiContextOptions(snapshot: AiEditorSnapshot, include: AiContextSelection, language: string, tables?: AiContextTables): AiContextOptions {
  return {
    language, include, tables: tables?.tables ?? [], excluded_tables: tables?.excluded_tables ?? [],
    editor: {
      query: include.query ? snapshot.query ?? null : null,
      selection: include.selection ? snapshot.selection ?? null : null,
      cursor_statement: include.cursor_statement ? snapshot.cursor_statement ?? null : null,
    },
    last_error: include.error ? snapshot.last_error ?? null : null,
  };
}

export function sameAiContextScope(left: AiContextScope | null, right: AiContextScope | null): boolean {
  return !!left && !!right && left.connection_id === right.connection_id
    && left.database === right.database && left.schema === right.schema;
}

export function changeAiContextTable(choices: AiContextTables, name: string, selected: boolean): AiContextTables {
  const tables = choices.tables.filter((table) => table !== name);
  const excluded = choices.excluded_tables.filter((table) => table !== name);
  if (selected) tables.push(name);
  else excluded.push(name);
  if (tables.length > 100 || excluded.length > 100) return choices;
  return { tables, excluded_tables: excluded };
}

export function aiContextTableCount(item: AiContextItemRef): number | null {
  return typeof item.table_count === "number" && Number.isSafeInteger(item.table_count) && item.table_count >= 0 ? item.table_count : null;
}

export function aiContextLabelKey(kind: string): string {
  return ["connection", "instructions", "schema", "query", "selection", "cursor_statement", "error", "gaps", "table"].includes(kind)
    ? `aiChat.context.kinds.${kind}` : "aiChat.context.kinds.other";
}

export function aiContextSize(item: AiContextItemRef): number | null {
  return typeof item.chars === "number" && Number.isSafeInteger(item.chars) && item.chars >= 0 ? item.chars : null;
}
