import type { NotebookCell, NotebookParam, RunAllResult } from "./notebook";
export type { AiNotebookSnapshot } from "./aiNotebookSnapshot";

// Local, read-only source for the bounded snapshot built at send time.
// Reading this state does not include it in an IPC request or conversation.
export interface AiNotebookSource {
  id: string;
  title: string;
  cells: readonly Readonly<NotebookCell>[];
  params: readonly Readonly<NotebookParam>[];
  stopOnError: boolean;
  focusedCellId: string | null;
  runAllResult: Readonly<RunAllResult> | null;
}

export interface AiNotebookTarget {
  id: string;
  readState?: () => AiNotebookSource;
}
