import { useState, useCallback, useContext, useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import type { OnMount } from "@monaco-editor/react";
import { SqlEditorWrapper } from "../ui/SqlEditorWrapper";
import { useAiAvailability } from "../../hooks/useAiAvailability";
import { NotebookAiButtons } from "./NotebookAiButtons";
import { CellSectionHeader } from "./CellSectionHeader";
import { AiNotebookContext } from "../../contexts/AiNotebookContext";
import { useOpenAiChat } from "../../hooks/useAiChatEntry";
import { aiSelectionEntry } from "../../utils/aiEntryPoints";
import type { AiEditorMenuAction } from "../../types/aiEditor";
import type { AiEditorSnapshot } from "../../types/aiContext";

interface SqlCellEditorProps {
  cellId: string;
  content: string;
  onContentChange: (content: string) => void;
  onRun: () => void;
  connectionId: string;
  schema?: string;
  collapsed?: boolean;
  onToggleCollapse: () => void;
}

export function SqlCellEditor({
  cellId,
  content,
  onContentChange,
  onRun,
  connectionId,
  collapsed,
  onToggleCollapse,
}: SqlCellEditorProps) {
  const { available } = useAiAvailability();
  const { t } = useTranslation();
  const [editorHeight, setEditorHeight] = useState(60);
  const aiNotebook = useContext(AiNotebookContext);
  const launcher = useOpenAiChat(aiNotebook?.scopeId);
  const runAiAction = useCallback((action: AiEditorMenuAction, snapshot: AiEditorSnapshot) => {
    const target = aiNotebook?.selectCell(cellId);
    if (target) launcher.open(action === "ask" ? null : action, target, aiSelectionEntry(snapshot));
  }, [aiNotebook, cellId, launcher]);
  const aiNotebookRef = useRef(aiNotebook);
  useEffect(() => { aiNotebookRef.current = aiNotebook; }, [aiNotebook]);
  const onMount: OnMount = useCallback((editor) => {
    const sync = () => setEditorHeight(Math.max(60, editor.getContentHeight()));
    editor.onDidContentSizeChange(sync);
    aiNotebookRef.current?.registerEditor(cellId, editor);
    const focus = editor.onDidFocusEditorText(() => { aiNotebookRef.current?.selectCell(cellId); });
    editor.onDidDispose(() => focus.dispose());
    sync();
  }, [cellId]);

  return (
    <div>
      <CellSectionHeader
        label={t("editor.notebook.sectionQuery")}
        collapsed={!!collapsed}
        onToggle={onToggleCollapse}
        divider={false}
      />
      {!collapsed && (
        <div className="relative" style={{ height: editorHeight }}>
          <SqlEditorWrapper
            aiActions={available ? { disabled: launcher.disabled, run: runAiAction } : undefined}
            height="100%"
            initialValue={content}
            onChange={onContentChange}
            onRun={onRun}
            onMount={onMount}
            editorKey={`notebook-${cellId}`}
            options={{
              padding: { top: 8, bottom: 8 },
              lineNumbers: "off",
              scrollbar: { alwaysConsumeMouseWheel: false },
            }}
          />
          {available && (
            <NotebookAiButtons
              cellId={cellId}
              content={content}
              connectionId={connectionId}
            />
          )}
        </div>
      )}
    </div>
  );
}
