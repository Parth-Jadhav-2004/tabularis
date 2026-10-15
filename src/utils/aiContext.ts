import type { AiContextOptions, AiContextSelection, AiEditorSnapshot } from "../types/aiContext";
import type { AiContextItemRef } from "../types/aiChat";

export function defaultAiContextSelection(): AiContextSelection {
  return { connection: true, schema: true, query: true, selection: true, cursor_statement: true, error: false };
}

export function createAiContextOptions(snapshot: AiEditorSnapshot, include: AiContextSelection, language: string): AiContextOptions {
  return {
    language, include, tables: [], excluded_tables: [],
    editor: {
      query: include.query ? snapshot.query ?? null : null,
      selection: include.selection ? snapshot.selection ?? null : null,
      cursor_statement: include.cursor_statement ? snapshot.cursor_statement ?? null : null,
    },
    last_error: include.error ? snapshot.last_error ?? null : null,
  };
}

export function aiContextLabelKey(kind: string): string {
  return ["connection", "instructions", "schema", "query", "selection", "cursor_statement", "error", "gaps", "table"].includes(kind)
    ? `aiChat.context.kinds.${kind}` : "aiChat.context.kinds.other";
}

export function aiContextSize(item: AiContextItemRef): number | null {
  return typeof item.chars === "number" && Number.isSafeInteger(item.chars) && item.chars >= 0 ? item.chars : null;
}
