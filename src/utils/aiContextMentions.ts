import type { ConnectionData, SchemaData } from "../contexts/DatabaseContext";
import type { SavedQuery } from "../contexts/SavedQueriesContext";
import type { AiContextScope, AiMentionItem } from "../types/aiContext";

export interface AiMentionRange { start: number; end: number; search: string }
export interface AiTableDrag { name: string; connectionId: string; database: string | null; schema: string | null }
export type AiContextCatalogData = Pick<ConnectionData, "databaseName" | "capabilities" | "activeSchema" | "tables" | "schemaDataMap" | "nestedDatabaseDataMap" | "databaseDataMap" | "selectedDatabases"> & Partial<Pick<ConnectionData, "views">>;

export function activeAiMention(text: string, start: number, end = start): AiMentionRange | null {
  if (start !== end || start < 0 || start > text.length) return null;
  const match = /(?:^|\s)@([^\s@]{0,80})$/u.exec(text.slice(0, start));
  return match ? { start: start - match[1].length - 1, end: start, search: match[1] } : null;
}

export function filterAiMentionTables(tables: string[], search: string): string[] {
  const needle = search.toLocaleLowerCase();
  return [...new Set(tables)].filter((name) => name.toLocaleLowerCase().includes(needle)).slice(0, 30);
}

export function removeAiMention(text: string, range: AiMentionRange): string {
  return text.slice(0, range.start) + text.slice(range.end);
}

function scopedCatalog(data: AiContextCatalogData | undefined, scope: AiContextScope | null): Pick<SchemaData, "tables" | "views"> | null {
  if (!data || !scope) return null;
  const database = scope.database ?? data.databaseName;
  let catalog;
  if (data.capabilities?.schemas) {
    const schema = scope.schema ?? (data.capabilities.sql_dialect === "postgres" ? "public" : data.activeSchema);
    const nested = data.nestedDatabaseDataMap[database];
    if (nested) catalog = schema ? nested.schemaDataMap[schema] : undefined;
    else if (database === data.databaseName) catalog = schema ? data.schemaDataMap[schema] : { tables: data.tables, views: data.views ?? [] };
  } else if (data.selectedDatabases.length > 0 || (scope.schema && scope.schema !== data.databaseName)) {
    catalog = data.databaseDataMap[scope.schema ?? database];
  } else if (database === data.databaseName) catalog = { tables: data.tables, views: data.views ?? [] };
  return catalog ?? null;
}

export function aiContextCatalogTables(data: AiContextCatalogData | undefined, scope: AiContextScope | null): string[] {
  return [...new Set((scopedCatalog(data, scope)?.tables ?? []).map((table) => table.name))].sort((a, b) => a.localeCompare(b));
}

export function aiContextMentionItems(data: AiContextCatalogData | undefined, scope: AiContextScope | null, saved: SavedQuery[]): AiMentionItem[] {
  if (!scope) return [];
  const catalog = scopedCatalog(data, scope);
  const capabilities = data?.capabilities;
  const flat = capabilities && !capabilities.schemas && !capabilities.file_based && !capabilities.folder_based && !capabilities.single_database;
  const database = (flat ? scope.schema : null) ?? scope.database ?? data?.databaseName ?? null;
  return [
    ...(catalog?.tables ?? []).map((table) => ({ kind: "table" as const, id: table.name, label: table.name })),
    ...(catalog?.views ?? []).map((view) => ({ kind: "view" as const, id: view.name, label: view.name })),
    ...saved.filter((query) => query.connection_id === scope.connection_id && (query.database === null || query.database === database))
      .map((query) => ({ kind: "saved_query" as const, id: query.id, label: query.name })),
  ];
}

export function filterAiMentions(items: AiMentionItem[], search: string): AiMentionItem[] {
  const seen = new Set<string>();
  const needle = search.toLocaleLowerCase();
  const matches = items.filter((item) => {
    const key = `${item.kind}:${item.id}`;
    if (seen.has(key) || !item.label.toLocaleLowerCase().includes(needle)) return false;
    seen.add(key);
    return true;
  });
  const groups = ["table", "view", "saved_query"].map((kind) => matches.filter((item) => item.kind === kind));
  const result: AiMentionItem[] = [];
  for (let index = 0; result.length < 30; index++) {
    let found = false;
    for (const group of groups) if (group[index] && result.length < 30) { result.push(group[index]); found = true; }
    if (!found) break;
  }
  return result;
}

export function aiContextDroppedTable(drag: AiTableDrag | null, data: AiContextCatalogData | undefined, scope: AiContextScope | null): string | null {
  if (!drag || !data || !scope || drag.connectionId !== scope.connection_id) return null;
  const schemaBased = !!data.capabilities?.schemas;
  const database = schemaBased ? scope.database ?? data.databaseName : scope.schema ?? scope.database ?? data.databaseName;
  const draggedDatabase = drag.database ?? (schemaBased ? data.databaseName : drag.schema ?? data.databaseName);
  const schema = schemaBased ? scope.schema ?? (data.capabilities?.sql_dialect === "postgres" ? "public" : data.activeSchema) : null;
  const draggedSchema = schemaBased ? drag.schema ?? data.activeSchema : null;
  if (database !== draggedDatabase || schema !== draggedSchema) return null;
  return aiContextCatalogTables(data, scope).includes(drag.name) ? drag.name : null;
}
