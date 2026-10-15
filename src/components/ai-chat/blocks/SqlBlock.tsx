import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { AiSqlActions } from "../../../types/aiEditor";

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
      <div className="flex flex-wrap items-center gap-1 border-b border-default p-2 text-xs">
        <span className="mr-auto font-mono text-muted">SQL</span>
        <button type="button" onClick={() => void copy()} className="rounded px-2 py-1 text-secondary hover:text-primary focus-ring">
          {t(copyState === "copied" ? "aiChat.copied" : "aiChat.copy")}
        </button>
        {(["insert", "replace", "run"] as const).map((action) => (
          <button key={action} type="button" disabled={!actions} onClick={() => actions?.[action](sql)}
            className="rounded px-2 py-1 text-secondary hover:text-primary disabled:opacity-40 focus-ring">
            {t(`aiChat.${action}`)}
          </button>
        ))}
      </div>
      <pre className="overflow-x-auto p-3 font-mono text-xs text-primary"><code>{sql}</code></pre>
      {copyState === "error" && <p role="status" className="px-3 pb-2 text-xs text-accent-error">{t("aiChat.copyError")}</p>}
    </div>
  );
}
