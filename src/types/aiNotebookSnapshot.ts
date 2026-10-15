export interface AiNotebookCellSnapshot {
  id: string;
  index: number;
  type: "sql" | "markdown";
  name: string | null;
  schema: string | null;
  content: string;
  truncated: boolean;
}

export interface AiNotebookResultSnapshot {
  cell_id: string;
  columns: string[];
  row_count: number;
  rows: string[][];
  truncated: boolean;
}

export interface AiNotebookSnapshot {
  unavailable_cells?: string[];
  id: string;
  title: string;
  cell_count: number;
  stop_on_error: boolean;
  params: { name: string; value: string }[];
  outline: AiNotebookCellSnapshot[];
  focused: AiNotebookCellSnapshot | null;
  neighbours: AiNotebookCellSnapshot[];
  mentioned: AiNotebookCellSnapshot[];
  errors: { cell_id: string; index: number; error: string }[];
  results: AiNotebookResultSnapshot[];
  truncated: boolean;
}
