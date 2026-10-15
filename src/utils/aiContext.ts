import type { AiContextOptions, AiContextScope, AiContextSelection, AiContextTables, AiEditorSnapshot, AiMentionItem } from "../types/aiContext";
import type { AiContextItemRef } from "../types/aiChat";

export function defaultAiContextSelection(): AiContextSelection {
  return { connection: true, schema: true, query: true, selection: true, cursor_statement: true, error: false };
}

export function createAiContextOptions(snapshot: AiEditorSnapshot, include: AiContextSelection, language: string, tables?: AiContextTables): AiContextOptions {
  return {
    language, include, tables: tables?.tables ?? [], excluded_tables: tables?.excluded_tables ?? [],
    views: tables?.views ?? [], excluded_views: tables?.excluded_views ?? [], saved_queries: tables?.saved_queries?.map((query) => query.id) ?? [],
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
  if (tables.length + (choices.views?.length ?? 0) > 100 || excluded.length > 100) return choices;
  return { ...choices, tables, excluded_tables: excluded };
}

export function changeAiContextMention(choices: AiContextTables, item: AiMentionItem, selected: boolean): AiContextTables {
  if (item.kind === "table") return changeAiContextTable(choices, item.id, selected);
  if (item.kind === "view") {
    const views = (choices.views ?? []).filter((name) => name !== item.id);
    const excluded = (choices.excluded_views ?? []).filter((name) => name !== item.id);
    if (selected) views.push(item.id);
    else excluded.push(item.id);
    if (views.length + choices.tables.length > 100 || excluded.length > 100) return choices;
    return { ...choices, views, excluded_views: excluded };
  }
  const queries = (choices.saved_queries ?? []).filter((query) => query.id !== item.id);
  if (selected) queries.push({ id: item.id, label: item.label });
  return queries.length > 20 ? choices : { ...choices, saved_queries: queries };
}

export function aiContextViewCount(item: AiContextItemRef): number | null {
  return typeof item.view_count === "number" && Number.isSafeInteger(item.view_count) && item.view_count >= 0 ? item.view_count : null;
}

export function aiContextTableCount(item: AiContextItemRef): number | null {
  return typeof item.table_count === "number" && Number.isSafeInteger(item.table_count) && item.table_count >= 0 ? item.table_count : null;
}

export function aiContextLabelKey(kind: string): string {
  return ["connection", "instructions", "schema", "query", "selection", "cursor_statement", "error", "gaps", "table", "view", "saved_query"].includes(kind)
    ? `aiChat.context.kinds.${kind}` : "aiChat.context.kinds.other";
}

export function aiContextSize(item: AiContextItemRef): number | null {
  return typeof item.chars === "number" && Number.isSafeInteger(item.chars) && item.chars >= 0 ? item.chars : null;
}
