import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useSettings } from "./useSettings";
import { useAiAvailability } from "./useAiAvailability";
import { aiChatModelChoices } from "../utils/aiChatModels";

export function useAiChatModels() {
  const { settings } = useSettings();
  const { providers, available, refresh: refreshAvailability } = useAiAvailability();
  const [request, setRequest] = useState(0);
  const [result, setResult] = useState<{
    signature: string; models: Record<string, string[]>; error: boolean;
  } | null>(null);
  const signature = JSON.stringify([providers, settings.aiOllamaPort, settings.aiCustomOpenaiUrl, request]);
  useEffect(() => {
    if (!available) return;
    let cancelled = false;
    void invoke<Record<string, string[]>>("get_ai_models", { forceRefresh: request > 0 }).then((models) => {
      if (!cancelled) setResult({ signature, models, error: false });
    }).catch(() => {
      if (!cancelled) setResult({ signature, models: {}, error: true });
    });
    return () => { cancelled = true; };
  }, [available, request, signature]);
  const choices = useMemo(() => aiChatModelChoices(providers, result?.signature === signature ? result.models : {}, settings),
    [providers, result, settings, signature]);
  const refresh = useCallback(() => {
    setRequest((value) => value + 1);
    refreshAvailability();
  }, [refreshAvailability]);
  return { choices, loading: available && result?.signature !== signature, error: result?.error ?? false, refresh };
}
