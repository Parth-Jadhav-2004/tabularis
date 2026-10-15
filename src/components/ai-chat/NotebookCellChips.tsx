import { X } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { AiNotebookCellChoice } from "../../types/aiNotebookReferences";

export function NotebookCellChips({ ids, cells, disabled, onRemove }: {
  ids: readonly string[]; cells: readonly AiNotebookCellChoice[]; disabled: boolean; onRemove: (id: string) => void;
}) {
  const { t } = useTranslation();
  if (ids.length === 0) return null;
  return <div role="group" aria-label={t("aiChat.notebook.selectedCells")} className="flex max-h-28 flex-wrap gap-1 overflow-y-auto border-t border-default px-3 py-2">
    {ids.map((id) => {
      const cell = cells.find((item) => item.id === id);
      const label = cell ? `${t("aiChat.notebook.cell", { index: cell.index })}${cell.name ? ` · ${cell.name}` : ""}` : t("aiChat.notebook.cellUnavailable");
      return <button key={id} type="button" disabled={disabled} aria-label={t("aiChat.notebook.removeCell", { cell: label })}
        onClick={() => onRemove(id)} className={`inline-flex max-w-full items-center gap-1 rounded border border-strong bg-surface-secondary px-2 py-1 text-xs disabled:opacity-40 focus-ring ${cell ? "text-primary" : "text-accent-warning"}`}>
        <span className="truncate">{label}</span><X size={10} aria-hidden="true" className="shrink-0" />
      </button>;
    })}
  </div>;
}
