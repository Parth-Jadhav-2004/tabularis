import { useEffect, useState } from "react";
import type { NotebookMetadata } from "../types/notebook";
import { listNotebooks, NOTEBOOKS_CHANGED_EVENT } from "../utils/notebookStore";

// Only successful metadata reads can establish that a notebook was deleted.
export function useAiNotebookCatalog(connectionIds: readonly string[]) {
  const key = JSON.stringify([...new Set(connectionIds)].sort());
  const [catalog, setCatalog] = useState<Record<string, readonly NotebookMetadata[]>>({});
  useEffect(() => {
    const ids: string[] = JSON.parse(key);
    let cancelled = false;
    let version = 0;
    const refresh = async () => {
      const request = ++version;
      const entries = await Promise.all(ids.map(async (id) => {
        try { return [id, await listNotebooks(id)] as const; }
        catch { return [id, undefined] as const; }
      }));
      if (cancelled || request !== version) return;
      setCatalog(Object.fromEntries(entries.filter((entry): entry is readonly [string, NotebookMetadata[]] => entry[1] !== undefined)));
    };
    const changed = () => { setCatalog({}); void refresh(); };
    void refresh();
    window.addEventListener(NOTEBOOKS_CHANGED_EVENT, changed);
    return () => { cancelled = true; window.removeEventListener(NOTEBOOKS_CHANGED_EVENT, changed); };
  }, [key]);
  return catalog;
}
