import type { editor } from "monaco-editor";
import type { AiEditorMenuAction, AiEditorMenuActions } from "../types/aiEditor";
import { readAiEditorContext } from "./aiEditor";

export function registerAiEditorMenu(codeEditor: editor.IStandaloneCodeEditor, actions: AiEditorMenuActions,
  labels: Record<AiEditorMenuAction, string>, dialect?: string) {
  let disposed = false;
  const registrations = (["explain", "optimize", "ask"] as const).map((action, index) => codeEditor.addAction({
    id: `tabularis.ai.${action}`, label: labels[action], precondition: "editorHasSelection",
    contextMenuGroupId: "8_ai", contextMenuOrder: index,
    run: () => {
      if (disposed || actions.disabled) return;
      const snapshot = readAiEditorContext(codeEditor, null, dialect);
      if (snapshot.selection?.trim()) actions.run(action, snapshot);
    },
  }));
  return () => { disposed = true; registrations.forEach((registration) => registration.dispose()); };
}
