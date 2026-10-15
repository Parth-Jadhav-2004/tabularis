import type { ConnectionData } from "../contexts/DatabaseContext";
import type { AiContextScope } from "../types/aiContext";

export interface AiMentionRange { start: number; end: number; search: string }
export interface AiTableDrag { name: string; connectionId: string; database: string | null; schema: string | null }
export type AiContextCatalogData = Pick<ConnectionData, "databaseName" | "capabilities" | "activeSchema" | "tables" | "schemaDataMap" | "nestedDatabaseDataMap" | "databaseDataMap" | "selectedDatabases">;

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

export function aiContextCatalogTables(data: AiContextCatalogData | undefined, scope: AiContextScope | null): string[] {
  if (!data || !scope) return [];
  const database = scope.database ?? data.databaseName;
  let tables;
  if (data.capabilities?.schemas) {
    const schema = scope.schema ?? (data.capabilities.sql_dialect === "postgres" ? "public" : data.activeSchema);
    const nested = data.nestedDatabaseDataMap[database];
    if (nested) tables = schema ? nested.schemaDataMap[schema]?.tables : undefined;
    else if (database === data.databaseName) tables = schema ? data.schemaDataMap[schema]?.tables : data.tables;
  } else if (data.selectedDatabases.length > 0 || (scope.schema && scope.schema !== data.databaseName)) {
    tables = data.databaseDataMap[scope.schema ?? database]?.tables;
  } else if (database === data.databaseName) tables = data.tables;
  return [...new Set((tables ?? []).map((table) => table.name))].sort((a, b) => a.localeCompare(b));
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
