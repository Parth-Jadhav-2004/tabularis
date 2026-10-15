import type { AiProvider } from "../contexts/SettingsContext";

export interface AiAvailableProvider {
  provider: AiProvider;
  label: string;
  source: "keychain" | "env" | "keyless";
  models_cached: boolean;
  usable: boolean;
  reason: "unreachable" | "no_models" | null;
}

export interface AiAvailability {
  available: boolean;
  loading: boolean;
  providers: AiAvailableProvider[];
  refresh: () => void;
}
