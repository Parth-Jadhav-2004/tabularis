import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { AiContextPreview, AiContextSpec } from "../types/aiContext";

export function useAiContextPreview(getContext: () => AiContextSpec | null, enabled: boolean) {
  const version = useRef(0);
  const scheduled = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [state, setState] = useState<{
    owner: typeof getContext; loading: boolean; error: boolean; preview: AiContextPreview | null;
  } | null>(null);
  const refresh = useCallback(async () => {
    if (scheduled.current !== null) clearTimeout(scheduled.current);
    if (!enabled) return;
    const context = getContext();
    if (!context) return;
    const request = ++version.current;
    setState({ owner: getContext, loading: true, error: false, preview: null });
    try {
      const preview = await invoke<AiContextPreview>("ai_chat_preview_context", { context });
      if (request === version.current) setState({ owner: getContext, loading: false, error: false, preview });
    } catch {
      if (request === version.current) setState({ owner: getContext, loading: false, error: true, preview: null });
    }
  }, [enabled, getContext]);
  const invalidate = useCallback(() => { version.current++; }, []);
  useEffect(() => {
    const timer = setTimeout(() => { void refresh(); }, 350);
    scheduled.current = timer;
    return () => { clearTimeout(timer); invalidate(); };
  }, [refresh, invalidate]);
  const current = enabled && state?.owner === getContext ? state : null;
  return { preview: current?.preview ?? null, loading: enabled && (!current || current.loading), error: current?.error ?? false, refresh };
}
