import { createContext } from "react";
import type { editor } from "monaco-editor";
import type { AiEditorTarget } from "../types/aiEditor";

export interface AiNotebookContextValue {
  scopeId: string;
  selectCell: (cellId: string) => AiEditorTarget | null;
  registerEditor: (cellId: string, codeEditor: editor.IStandaloneCodeEditor) => void;
}

export const AiNotebookContext = createContext<AiNotebookContextValue | null>(null);
