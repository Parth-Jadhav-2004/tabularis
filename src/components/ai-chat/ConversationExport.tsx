import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Download } from "lucide-react";
import type { AiConversationSummary } from "../../types/aiChat";
import { exportAiConversation, type AiConversationExportFormat } from "../../utils/aiConversationExport";

export function ConversationExport({ item, disabled }: { item: AiConversationSummary; disabled: boolean }) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [failed, setFailed] = useState(false);
  const exportFile = async (format: AiConversationExportFormat) => {
    if (busy || disabled) return;
    setBusy(true);
    setFailed(false);
    try {
      if (await exportAiConversation(item, format, t("aiChat.export.dialogTitle"))) setOpen(false);
    } catch { setFailed(true); }
    finally { setBusy(false); }
  };
  return <div className="mt-1 border-t border-default pt-1">
    <button type="button" disabled={disabled || busy} onClick={() => setOpen((value) => !value)}
      aria-expanded={open} className="flex items-center gap-1.5 rounded px-1 py-1 text-xs text-secondary hover:text-primary disabled:opacity-40 focus-ring">
      <Download size={12} aria-hidden />{t("aiChat.export.label")}
    </button>
    {open && <div className="space-y-2 p-1">
      <p className="text-xs text-secondary">{t("aiChat.export.unencrypted")}</p>
      <div className="flex flex-wrap gap-2">
        {(["markdown", "notebook"] as const).map((format) => <button type="button" key={format} disabled={disabled || busy}
          onClick={() => void exportFile(format)} className="rounded border border-strong px-2 py-1 text-xs text-primary hover:bg-surface-secondary disabled:opacity-40 focus-ring">
          {t(`aiChat.export.${format}`)}
        </button>)}
      </div>
      {busy && <p role="status" className="text-xs text-secondary">{t("aiChat.export.saving")}</p>}
      {failed && <p role="alert" className="text-xs text-accent-error">{t("aiChat.export.error")}</p>}
    </div>}
  </div>;
}
