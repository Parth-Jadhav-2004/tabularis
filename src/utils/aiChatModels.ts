import type { Settings } from "../contexts/SettingsContext";
import type { AiAvailableProvider } from "../types/aiAvailability";
import type { AiModelChoice } from "../types/aiChat";

export const AI_CHAT_MODEL_KEY = "tabularis_ai_chat_model";

export function readAiChatModel(): AiModelChoice | null {
  try {
    const value: unknown = JSON.parse(localStorage.getItem(AI_CHAT_MODEL_KEY) ?? "null");
    if (value && typeof value === "object" && "provider" in value && "model" in value
      && typeof value.provider === "string" && typeof value.model === "string") {
      return { provider: value.provider, model: value.model };
    }
  } catch { /* A malformed preference must not prevent opening the chat. */ }
  return null;
}

export function rememberAiChatModel(choice: AiModelChoice) {
  try { localStorage.setItem(AI_CHAT_MODEL_KEY, JSON.stringify(choice)); }
  catch { /* The in-memory conversation remains usable without preferences. */ }
}

export function aiChatModelChoices(
  providers: AiAvailableProvider[],
  models: Record<string, string[]>,
  settings: Pick<Settings, "aiCustomModels" | "aiProvider" | "aiModel" | "aiCustomOpenaiModel">,
): AiModelChoice[] {
  return providers.filter((item) => item.usable).flatMap(({ provider }) => {
    const names = settings.aiCustomModels?.[provider] ?? models[provider] ?? [];
    const configured = provider === "custom-openai" ? settings.aiCustomOpenaiModel : "";
    const defaultModel = settings.aiProvider === provider ? settings.aiModel : "";
    return [...new Set([...names, configured, defaultModel].filter((name): name is string => !!name?.trim()))]
      .map((model) => ({ provider, model }));
  });
}

export function defaultAiChatModel(
  choices: AiModelChoice[],
  settings: Pick<Settings, "aiProvider" | "aiModel">,
  last: AiModelChoice | null,
): AiModelChoice | null {
  return choices.find((choice) => choice.provider === last?.provider && choice.model === last.model)
    ?? choices.find((choice) => choice.provider === settings.aiProvider && choice.model === settings.aiModel)
    ?? choices.find((choice) => choice.provider === settings.aiProvider)
    ?? choices[0] ?? null;
}
