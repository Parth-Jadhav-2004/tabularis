import type { AiNotebookSource } from "../types/aiNotebook";
import type { AiNotebookCellChoice } from "../types/aiNotebookReferences";
import type { AiContextTables } from "../types/aiContext";
import type { AiMentionRange } from "./aiContextMentions";

export function activeAiCellMention(text: string, start: number, end = start): AiMentionRange | null {
  if (start !== end || start < 0 || start > text.length) return null;
  const match = /(?:^|\s)#([^\s#]{0,80})$/u.exec(text.slice(0, start));
  return match ? { start: start - match[1].length - 1, end: start, search: match[1] } : null;
}

export function aiNotebookCellChoices(cells: AiNotebookSource["cells"] | undefined): AiNotebookCellChoice[] {
  return cells?.map((cell, index) => ({ id: cell.id, index: index + 1, type: cell.type,
    name: Array.from(cell.name ?? "").slice(0, 128).join(""),
    preview: Array.from(cell.content.slice(0, 320).split(/\r?\n/, 1)[0]).slice(0, 80).join(""),
  })) ?? [];
}

export function filterAiNotebookCells(cells: readonly AiNotebookCellChoice[], search: string): AiNotebookCellChoice[] {
  const needle = search.toLocaleLowerCase();
  return cells.filter((cell) => `${cell.index}`.startsWith(needle) || cell.name.toLocaleLowerCase().includes(needle)
    || cell.preview.toLocaleLowerCase().includes(needle)).slice(0, 30);
}

export function selectedAiNotebookCells(choices: AiContextTables, notebookId: string | undefined): string[] {
  return notebookId && choices.notebook?.notebook_id === notebookId ? choices.notebook.cell_ids : [];
}

export function changeAiNotebookCell(choices: AiContextTables, notebookId: string, cellId: string, selected: boolean): AiContextTables {
  const previous = selectedAiNotebookCells(choices, notebookId);
  const ids = previous.filter((id) => id !== cellId);
  if (selected) ids.push(cellId);
  if (ids.length > 20) return choices;
  return { ...choices, notebook: { notebook_id: notebookId, cell_ids: ids } };
}
