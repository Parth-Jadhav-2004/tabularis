import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { AiSqlActions } from "../../../types/aiEditor";
import { Check, Copy, CornerDownLeft, Play, Replace } from "lucide-react";

export function SqlBlock({ sql, actions }: { sql: string; actions?: AiSqlActions }) {
  const { t } = useTranslation();
  const [copyState, setCopyState] = useState<"idle" | "copied" | "error">("idle");
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(sql);
      setCopyState("copied");
    } catch { setCopyState("error"); }
  };
  return (
    <div className="my-3 overflow-hidden rounded-lg border border-default bg-base">
      <div className="flex items-center gap-0.5 border-b border-default px-3 py-1.5 text-xs">
        <span className="mr-auto font-mono text-muted">SQL</span>
        <button type="button" onClick={() => void copy()} aria-label={t(copyState === "copied" ? "aiChat.copied" : "aiChat.copy")}
          title={t(copyState === "copied" ? "aiChat.copied" : "aiChat.copy")} className="rounded p-1.5 text-secondary hover:bg-surface-secondary hover:text-primary focus-ring">
          {copyState === "copied" ? <Check size={14} aria-hidden="true" /> : <Copy size={14} aria-hidden="true" />}
        </button>
        {(["insert", "replace", "run"] as const).map((action) => (
          <button key={action} type="button" disabled={!actions} onClick={() => actions?.[action](sql)}
            aria-label={t(`aiChat.${action}`)} title={t(`aiChat.${action}`)}
            className="rounded p-1.5 text-secondary hover:bg-surface-secondary hover:text-primary disabled:opacity-40 focus-ring">
            {action === "insert" ? <CornerDownLeft size={14} aria-hidden="true" /> : action === "replace" ? <Replace size={14} aria-hidden="true" /> : <Play size={14} aria-hidden="true" />}
          </button>
        ))}
      </div>
      <pre className="overflow-x-auto p-3 font-mono text-xs text-primary"><code>{sql}</code></pre>
      {copyState === "error" && <p role="status" className="px-3 pb-2 text-xs text-accent-error">{t("aiChat.copyError")}</p>}
    </div>
  );
}
