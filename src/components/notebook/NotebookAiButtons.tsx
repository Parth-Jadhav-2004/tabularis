import { useContext } from "react";
import { AiNotebookContext } from "../../contexts/AiNotebookContext";
import { useOpenAiChat } from "../../hooks/useAiChatEntry";
import { AiDropdownButton } from "../ui/AiDropdownButton";

interface NotebookAiButtonsProps {
  cellId: string;
  content: string;
  connectionId: string;
}

export function NotebookAiButtons({
  cellId,
  content,
  connectionId,
}: NotebookAiButtonsProps) {
  const notebook = useContext(AiNotebookContext);
  const launcher = useOpenAiChat(notebook?.scopeId);
  const open = (command: "generate" | "explain") => {
    const target = notebook?.selectCell(cellId);
    if (target) launcher.open(command, target);
  };

  return (
      <div className="absolute bottom-1 right-2 z-10 flex items-center gap-1">
        <AiDropdownButton
          onGenerate={() => open("generate")}
          onExplain={() => open("explain")}
          disableAll={!connectionId || !notebook || launcher.disabled}
          disableExplain={!content.trim()}
          compact
        />
      </div>
  );
}
