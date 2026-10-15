import { useEffect, useId, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { RefreshCw } from "lucide-react";
import type { AiCapabilities, AiModelChoice } from "../../types/aiChat";
import { useAiAvailability } from "../../hooks/useAiAvailability";
import { getProviderLabel } from "../../utils/settingsUI";

export function ModelPicker({ choice, choices, onChange, onRefresh, loading, disabled }: {
  choice: AiModelChoice | null; choices: AiModelChoice[]; onChange: (choice: AiModelChoice) => void;
  onRefresh: () => void; loading: boolean;
  disabled: boolean;
}) {
  const { t } = useTranslation();
  const id = useId();
  const { providers } = useAiAvailability();
  const [search, setSearch] = useState("");
  const [capabilities, setCapabilities] = useState<{ key: string; value: AiCapabilities } | null>(null);
  const key = choice ? JSON.stringify([choice.provider, choice.model]) : "";
  const provider = choice?.provider;
  const model = choice?.model;
  useEffect(() => {
    if (!provider || !model) return;
    let cancelled = false;
    void invoke<AiCapabilities>("get_ai_model_capabilities", { provider, model }).then((value) => {
      if (!cancelled) setCapabilities({ key, value });
    }).catch(() => {});
    return () => { cancelled = true; };
  }, [key, model, provider]);
  const filtered = choices.filter((item) => `${item.provider} ${item.model}`.toLowerCase().includes(search.toLowerCase()));
  // Retain the selection while filtering so the browser does not display a
  // different model from the one the next request will actually use.
  const visible = choice && !filtered.some((item) => item.provider === choice.provider && item.model === choice.model)
    ? [choice, ...filtered] : filtered;
  return <div className="space-y-2">
    <label htmlFor={id} className="text-xs text-secondary">{t("aiChat.model")}</label>
    <input aria-label={t("aiChat.searchModels")} value={search} onChange={(event) => setSearch(event.target.value)}
      placeholder={t("aiChat.searchModels")} className="w-full rounded border border-strong bg-base px-2 py-1 text-xs text-primary focus:border-focus focus-ring" />
    <div className="flex gap-2">
      <select id={id} value={key} disabled={disabled} onChange={(event) => {
        const next = choices.find((item) => JSON.stringify([item.provider, item.model]) === event.target.value);
        if (next) onChange(next);
      }} className="min-w-0 flex-1 rounded border border-strong bg-base p-2 text-xs text-primary focus:border-focus focus-ring">
        {!choice && <option value="">{t(loading ? "aiChat.loadingModels" : "aiChat.noModels")}</option>}
        {choice && !providers.some((item) => item.provider === choice.provider) &&
          <option value={key} disabled>{choice.provider} · {choice.model}</option>}
        {providers.map((item) => <optgroup key={item.provider} label={item.label} disabled={!item.usable}>
          {visible.filter((option) => option.provider === item.provider).map((option) =>
            <option key={option.model} value={JSON.stringify([option.provider, option.model])}>{option.model}</option>)}
          {!choices.some((option) => option.provider === item.provider) && <option disabled>{t(item.reason === "unreachable" ? "aiChat.unreachable" : "aiChat.noModels")}</option>}
        </optgroup>)}
        <optgroup label={t("aiChat.setupProviders")}>
          {(["openai", "anthropic", "minimax", "openrouter", "ollama", "custom-openai"] as const).filter((name) =>
            !providers.some((item) => item.provider === name)).map((name) => <option key={name} disabled>{getProviderLabel(name)}</option>)}
        </optgroup>
      </select>
      <button type="button" onClick={onRefresh} disabled={loading} aria-label={t("aiChat.refreshModels")}
        className="rounded border border-default p-2 text-secondary hover:text-primary disabled:opacity-40 focus-ring"><RefreshCw size={14} className={loading ? "animate-spin" : ""} /></button>
    </div>
    {capabilities?.key === key && <div className="flex flex-wrap gap-2 text-xs text-muted">
      {(["tools", "images", "reasoning"] as const).filter((name) => capabilities.value[name])
        .map((name) => <span key={name}>{t(`aiChat.capabilities.${name}`)}</span>)}
    </div>}
  </div>;
}
