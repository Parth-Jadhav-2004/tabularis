import type { AiNotebookSource } from "../types/aiNotebook";
import type { AiNotebookCellSnapshot, AiNotebookSnapshot } from "../types/aiNotebookSnapshot";
import type { AiContextOptions, AiContextSelection, AiContextTables, AiEditorSnapshot } from "../types/aiContext";
import type { AiEditorTarget } from "../types/aiEditor";
import { createAiContextOptions } from "./aiContext";

// Bound before IPC, without first allocating an array of every code point.
function cut(text: string, limit: number): { text: string; truncated: boolean } {
  let count = 0;
  let end = 0;
  for (const character of text) {
    if (count++ === limit) return { text: text.slice(0, end), truncated: true };
    end += character.length;
  }
  return { text, truncated: false };
}

function resultValue(value: unknown, depth = 0): unknown {
  if (typeof value === "string") return cut(value, 200).text;
  if (value === null || typeof value === "number" || typeof value === "boolean") return value;
  if (typeof value !== "object") return String(value);
  if (depth >= 2) return "[…]";
  if (Array.isArray(value)) return value.slice(0, 8).map((item) => resultValue(item, depth + 1));
  return Object.fromEntries(Object.keys(value).slice(0, 8).map((key) => [cut(key, 40).text,
    resultValue((value as Record<string, unknown>)[key], depth + 1)]));
}

export function buildAiNotebookSnapshot(source: AiNotebookSource, options: {
  mentionedCellIds?: readonly string[]; includeResults?: boolean; includeErrors?: boolean;
  focusedContent?: string | null; focusedError?: string | null;
} = {}): AiNotebookSnapshot {
  let truncated = false;
  const text = (value: string, limit: number) => {
    const part = cut(value, limit);
    truncated ||= part.truncated;
    return part.text;
  };
  const cell = (index: number, limit: number, content?: string | null): AiNotebookCellSnapshot => {
    const original = source.cells[index];
    const part = cut(content ?? original.content, limit);
    const name = original.name ? cut(original.name, 128) : null;
    const schema = original.schema ? cut(original.schema, 128) : null;
    const incomplete = part.truncated || !!name?.truncated || !!schema?.truncated;
    truncated ||= incomplete;
    return { id: original.id, index: index + 1, type: original.type, name: name?.text ?? null,
      schema: schema?.text ?? null, content: part.text, truncated: incomplete };
  };
  const focusedIndex = source.cells.findIndex((item) => item.id === source.focusedCellId);
  const focused = focusedIndex < 0 ? null : cell(focusedIndex, 6_000, options.focusedContent);
  const mentionedIds = new Set(options.mentionedCellIds?.slice(0, 20));
  const mentioned: AiNotebookCellSnapshot[] = [];
  let mentionedRemaining = 8_000;
  for (let index = 0; index < source.cells.length; index++) {
    if (index === focusedIndex || !mentionedIds.has(source.cells[index].id)) continue;
    const item = cell(index, Math.min(6_000, mentionedRemaining));
    mentionedRemaining -= Array.from(item.content).length;
    mentioned.push(item);
  }
  const selectedIds = new Set([...(focused ? [focused.id] : []), ...mentioned.map((item) => item.id)]);
  const neighbours = focusedIndex < 0 ? [] : [focusedIndex - 1, focusedIndex + 1]
    .filter((index) => index >= 0 && index < source.cells.length && !selectedIds.has(source.cells[index].id))
    .map((index) => cell(index, 2_000));
  const errors: AiNotebookSnapshot["errors"] = [];
  if (options.includeErrors) {
    let remaining = 2_000;
    const add = (id: string, error: string) => {
      const index = source.cells.findIndex((item) => item.id === id);
      if (index < 0 || errors.some((item) => item.cell_id === id)) return;
      if (errors.length >= 40 || remaining === 0) { truncated = true; return; }
      const value = text(error, remaining);
      remaining -= Array.from(value).length;
      errors.push({ cell_id: id, index: index + 1, error: value });
    };
    if (focused && options.focusedError) add(focused.id, options.focusedError);
    source.runAllResult?.errors.forEach((error) => add(error.cellId, error.error));
    source.cells.forEach((item) => { if (item.error) add(item.id, item.error); });
  }
  const results: AiNotebookSnapshot["results"] = [];
  if (options.includeResults) {
    let remaining = 6_000;
    for (const original of source.cells) {
      if (!selectedIds.has(original.id) || !original.result) continue;
      if (remaining === 0) { truncated = true; break; }
      const result = original.result;
      let incomplete = !!result.truncated || !!result.pagination?.has_more || !!result.additional_results?.length
        || result.columns.length > 40 || result.rows.length > 5;
      const value = (raw: string) => {
        const part = cut(raw, Math.min(200, remaining));
        remaining -= Array.from(part.text).length;
        incomplete ||= part.truncated;
        return part.text;
      };
      const columns = result.columns.slice(0, 40).map(value);
      const rows = result.rows.slice(0, 5).map((row) => row.slice(0, columns.length).map((raw) => {
        incomplete ||= raw !== null && typeof raw === "object";
        const bounded = typeof raw === "string" ? raw : JSON.stringify(resultValue(raw));
        return value(bounded ?? "null");
      }));
      truncated ||= incomplete;
      const total = result.pagination?.total_rows;
      const rowCount = typeof total === "number" && Number.isSafeInteger(total) && total >= result.rows.length ? total : result.rows.length;
      results.push({ cell_id: original.id, columns, row_count: rowCount, rows, truncated: incomplete });
    }
  }
  const title = text(source.title, 200);
  const params: AiNotebookSnapshot["params"] = [];
  let paramsRemaining = 2_000;
  for (const param of source.params.slice(0, 40)) {
    const name = text(param.name, Math.min(128, paramsRemaining));
    paramsRemaining -= Array.from(name).length;
    const value = text(param.value, Math.min(256, paramsRemaining));
    paramsRemaining -= Array.from(value).length;
    params.push({ name, value });
    if (paramsRemaining === 0) break;
  }
  const outline: AiNotebookCellSnapshot[] = [];
  let outlineRemaining = 8_000;
  for (let index = 0; index < Math.min(source.cells.length, 80); index++) {
    const item = cell(index, 150, index === focusedIndex ? options.focusedContent : undefined);
    const size = Array.from(JSON.stringify(item)).length;
    if (size > outlineRemaining) break;
    outlineRemaining -= size;
    outline.push(item);
  }
  truncated ||= source.cells.length > outline.length || source.params.length > params.length || (options.mentionedCellIds?.length ?? 0) > 20;
  return { id: source.id, title, cell_count: source.cells.length, stop_on_error: source.stopOnError,
    params, outline, focused, neighbours, mentioned, errors, results, truncated };
}

// Capture current state for both preview and send; the transcript retains only
// Rust-produced context summaries, never this local snapshot.
export function createAiTargetContextOptions(target: AiEditorTarget | undefined, include: AiContextSelection,
  language: string, tables?: AiContextTables, override?: AiEditorSnapshot): AiContextOptions {
  const editor = override ?? target?.readContext?.() ?? {};
  const options = createAiContextOptions(editor, include, language, tables);
  const source = target?.notebook?.readState?.();
  if (!source || source.id !== target?.notebook?.id) return options;
  options.editor.query = null;
  options.editor.cursor_statement = null;
  options.last_error = null;
  if (include.notebook !== false) options.notebook = buildAiNotebookSnapshot(source, {
    includeResults: include.notebook_results === true, includeErrors: include.error,
    focusedContent: override?.query, focusedError: override?.last_error,
  });
  return options;
}
