import { beforeEach, describe, expect, it } from "vitest";
import { aiChatModelChoices, defaultAiChatModel, readAiChatModel, rememberAiChatModel, AI_CHAT_MODEL_KEY } from "../../src/utils/aiChatModels";
import { DEFAULT_SETTINGS } from "../../src/contexts/SettingsContext";
import type { AiAvailableProvider } from "../../src/types/aiAvailability";

const providers: AiAvailableProvider[] = [
  { provider: "openai", label: "OpenAI", source: "env", models_cached: false, usable: true, reason: null },
  { provider: "custom-openai", label: "Custom", source: "keyless", models_cached: false, usable: true, reason: null },
];

describe("chat model choices", () => {
  beforeEach(() => localStorage.clear());
  it("lists only usable providers and merges explicit custom/default models without duplicates", () => {
    const settings = { ...DEFAULT_SETTINGS, aiProvider: "openai" as const, aiModel: "gpt-x", aiCustomOpenaiModel: "local" };
    expect(aiChatModelChoices(providers, { openai: ["gpt-x", "gpt-y"], anthropic: ["claude"] }, settings))
      .toEqual([{ provider: "openai", model: "gpt-x" }, { provider: "openai", model: "gpt-y" }, { provider: "custom-openai", model: "local" }]);
  });
  it("honors the user's custom model list", () => {
    expect(aiChatModelChoices(providers, { openai: ["remote"] }, { ...DEFAULT_SETTINGS, aiCustomModels: { openai: ["custom"] } }))
      .toEqual([{ provider: "openai", model: "custom" }]);
  });
  it("does not offer cached/default models from an offline provider", () => {
    expect(aiChatModelChoices([{ ...providers[0], usable: false, reason: "unreachable" }],
      { openai: ["cached"] }, { ...DEFAULT_SETTINGS, aiProvider: "openai", aiModel: "default" })).toEqual([]);
  });
  it("prefers the last chat model, then global settings, then a usable fallback", () => {
    const choices = [{ provider: "openai", model: "first" }, { provider: "openai", model: "second" }];
    const settings = { ...DEFAULT_SETTINGS, aiProvider: "openai" as const, aiModel: "first" };
    expect(defaultAiChatModel(choices, settings, choices[1])).toEqual(choices[1]);
    expect(defaultAiChatModel(choices, settings, { provider: "gone", model: "gone" })).toEqual(choices[0]);
    expect(defaultAiChatModel([], settings, null)).toBeNull();
  });
  it("persists only the chat model preference and tolerates malformed storage", () => {
    rememberAiChatModel({ provider: "openai", model: "example" });
    expect(readAiChatModel()).toEqual({ provider: "openai", model: "example" });
    for (const value of ["{", "null", "[]", '{"provider":false,"model":"x"}']) {
      localStorage.setItem(AI_CHAT_MODEL_KEY, value);
      expect(readAiChatModel()).toBeNull();
    }
  });
});
