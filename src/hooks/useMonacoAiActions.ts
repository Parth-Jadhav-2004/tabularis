import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type { editor } from "monaco-editor";
import type { AiEditorMenuActions } from "../types/aiEditor";
import { registerAiEditorMenu } from "../utils/aiEditorMenu";

export function useMonacoAiActions(actions?: AiEditorMenuActions, dialect?: string) {
  const { t } = useTranslation();
  const [codeEditor, setCodeEditor] = useState<editor.IStandaloneCodeEditor | null>(null);
  useEffect(() => {
    if (!codeEditor || !actions || actions.disabled) return;
    return registerAiEditorMenu(codeEditor, actions, { explain: t("aiChat.entryPoints.explainSelection"),
      optimize: t("aiChat.entryPoints.optimizeSelection"), ask: t("aiChat.entryPoints.askSelection") }, dialect);
  }, [codeEditor, actions, dialect, t]);
  return setCodeEditor;
}
