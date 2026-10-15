import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useSettings } from "../hooks/useSettings";
import type { AiAvailableProvider } from "../types/aiAvailability";
import { AiAvailabilityContext } from "./AiAvailabilityContext";

export function AiAvailabilityProvider({ children }: { children: ReactNode }) {
  const { settings } = useSettings();
  const [revision, setRevision] = useState(0);
  const refresh = useCallback(() => setRevision((value) => value + 1), []);
  const signature = JSON.stringify([
    settings.aiEnabled, settings.aiProvider, settings.aiOllamaPort,
    settings.aiCustomOpenaiUrl, settings.aiProviderProxies,
  ]);
  // An identity token also invalidates a result after disable/re-enable, even
  // when the serialized settings return to exactly the same values.
  const request = useMemo(() => ({ signature }), [signature]);
  const [result, setResult] = useState<{ request: { signature: string }; providers: AiAvailableProvider[] } | null>(null);
  const enabled = settings.aiEnabled === true;

  useEffect(() => {
    if (!enabled) return;
    let cancelled = false;
    void invoke<AiAvailableProvider[]>("get_ai_available_providers").then((providers) => {
      if (!cancelled) setResult({ request, providers });
    }).catch(() => {
      if (!cancelled) setResult({ request, providers: [] });
    });
    return () => { cancelled = true; };
  }, [enabled, request, revision]);

  useEffect(() => {
    window.addEventListener("focus", refresh);
    return () => window.removeEventListener("focus", refresh);
  }, [refresh]);

  const value = useMemo(() => {
    const current = enabled && result?.request === request;
    const providers = current ? result.providers : [];
    return { available: providers.some((provider) => provider.usable), providers, loading: enabled && !current, refresh };
  }, [enabled, refresh, result, request]);
  return <AiAvailabilityContext.Provider value={value}>{children}</AiAvailabilityContext.Provider>;
}
