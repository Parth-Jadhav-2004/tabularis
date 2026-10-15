export interface AiNotebookCellChoice {
  id: string;
  index: number;
  name: string;
  preview: string;
  type: "sql" | "markdown";
}

export interface AiNotebookCellSelection {
  notebook_id: string;
  cell_ids: string[];
}

export interface AiNotebookReferences {
  cells: ReadonlyMap<number, string>;
  reveal: (cellId: string) => boolean;
}
