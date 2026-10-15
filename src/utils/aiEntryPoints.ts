import type { AiChatEntryOptions, AiEditorTarget } from "../types/aiEditor";
import type { AiEditorSnapshot } from "../types/aiContext";
import type { TableTarget } from "../types/databaseObjects";
import type { CommandRuntime } from "../types/commands";
import { defaultAiContextSelection } from "./aiContext";

// Execution metadata is transient and bounded; it is not conversation data.
export function createAiQueryFailures() {
  const failures = new Map<string, { query: string; error: string }>();
  return {
    record(tabId: string, query: string, error: string) {
      failures.delete(tabId);
      failures.set(tabId, { query, error });
      if (failures.size > 100) failures.delete(failures.keys().next().value!);
    },
    clear: (tabId: string) => { failures.delete(tabId); },
    read(tabId: string, error: string) {
      const failure = failures.get(tabId);
      return failure?.error === error ? failure.query : undefined;
    },
  };
}

export function aiSelectionEntry(snapshot: AiEditorSnapshot): AiChatEntryOptions {
  return { snapshot, include: { ...defaultAiContextSelection(), query: false, selection: true, cursor_statement: false } };
}

export function aiQueryEntry(snapshot: AiEditorSnapshot): AiChatEntryOptions {
  return { snapshot: { ...snapshot, selection: null, cursor_statement: null },
    include: { ...defaultAiContextSelection(), query: true, selection: false, cursor_statement: false } };
}

export function aiFixEntry(query: string, error: string): AiChatEntryOptions {
  return { ...aiQueryEntry({ query, last_error: error }), include: { ...defaultAiContextSelection(),
    query: true, selection: false, cursor_statement: false, error: true } };
}

export function aiTableEntry(tableName: string): AiChatEntryOptions {
  return { tables: [tableName], snapshot: {}, include: { ...defaultAiContextSelection(),
    query: false, selection: false, cursor_statement: false } };
}

export function createAiTableTarget(table: TableTarget, base: AiEditorTarget | undefined, openEditor: CommandRuntime["openEditor"]): AiEditorTarget {
  const database = table.database ?? base?.database ?? null;
  const schema = table.schema ?? base?.schema ?? null;
  const open = (sql: string, run: boolean) => openEditor({ kind: "console", targetConnectionId: table.connectionId,
    initialQuery: sql, preventAutoRun: !run, ...(database ? { database } : {}), ...(schema ? { schema } : {}) });
  return { id: JSON.stringify(["table", table.connectionId, database, schema, table.tableName]),
    connectionId: table.connectionId, database, schema, dialect: base?.dialect,
    readContext: () => ({}), actions: { insert: (sql) => open(sql, false), replace: (sql) => open(sql, false),
      run: (sql) => open(sql, true), focus: () => base?.actions.focus() } };
}
