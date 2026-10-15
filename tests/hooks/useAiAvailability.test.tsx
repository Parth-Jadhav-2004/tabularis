import { type ReactNode } from "react";
import { act, renderHook, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { AiAvailabilityProvider } from "../../src/contexts/AiAvailabilityProvider";
import { useAiAvailability } from "../../src/hooks/useAiAvailability";
import { DEFAULT_SETTINGS } from "../../src/contexts/SettingsContext";
import type { AiAvailableProvider } from "../../src/types/aiAvailability";

const state = vi.hoisted(() => ({ enabled: false, url: "" }));
vi.mock("../../src/hooks/useSettings", () => ({
  useSettings: () => ({ settings: { ...DEFAULT_SETTINGS, aiEnabled: state.enabled, aiCustomOpenaiUrl: state.url } }),
}));
const providers: AiAvailableProvider[] = [{ provider: "openai", label: "OpenAI", source: "env", models_cached: false, usable: true, reason: null }];
const wrapper = ({ children }: { children: ReactNode }) => <AiAvailabilityProvider>{children}</AiAvailabilityProvider>;

describe("shared AI availability", () => {
  beforeEach(() => {
    state.enabled = false;
    state.url = "";
    vi.mocked(invoke).mockReset().mockResolvedValue(providers);
  });
  it("does not inspect credentials when AI is disabled", () => {
    const { result } = renderHook(useAiAvailability, { wrapper });
    expect(result.current.available).toBe(false);
    expect(invoke).not.toHaveBeenCalled();
  });
  it("keeps configured offline providers visible in the picker without enabling AI surfaces", async () => {
    state.enabled = true;
    vi.mocked(invoke).mockResolvedValue([{ provider: "ollama", label: "Ollama", source: "keyless", models_cached: true, usable: false, reason: "unreachable" }]);
    const { result } = renderHook(useAiAvailability, { wrapper });
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.available).toBe(false);
    expect(result.current.providers[0].reason).toBe("unreachable");
  });
  it("shares one lookup between consumers and refreshes after credential changes", async () => {
    state.enabled = true;
    const { result } = renderHook(() => [useAiAvailability(), useAiAvailability()], { wrapper });
    await waitFor(() => expect(result.current[0].available).toBe(true));
    expect(result.current[0]).toBe(result.current[1]);
    expect(invoke).toHaveBeenCalledTimes(1);
    vi.mocked(invoke).mockResolvedValue([]);
    act(() => result.current[0].refresh());
    await waitFor(() => expect(result.current[0].available).toBe(false));
  });
  it("immediately hides stale providers after endpoint changes and ignores old responses", async () => {
    state.enabled = true;
    const { result, rerender } = renderHook(useAiAvailability, { wrapper });
    await waitFor(() => expect(result.current.available).toBe(true));
    let resolve!: (value: AiAvailableProvider[]) => void;
    vi.mocked(invoke).mockImplementationOnce(() => new Promise((done) => { resolve = done; }));
    state.url = "https://first.test";
    rerender();
    expect(result.current.available).toBe(false);
    vi.mocked(invoke).mockResolvedValue([]);
    state.url = "https://second.test";
    rerender();
    await waitFor(() => expect(result.current.loading).toBe(false));
    await act(async () => resolve(providers));
    expect(result.current.available).toBe(false);
  });
  it("fails closed on IPC errors and while disabled even with an earlier result", async () => {
    state.enabled = true;
    const { result, rerender } = renderHook(useAiAvailability, { wrapper });
    await waitFor(() => expect(result.current.available).toBe(true));
    state.enabled = false;
    rerender();
    expect(result.current.available).toBe(false);
    vi.mocked(invoke).mockRejectedValue(new Error("unavailable"));
    state.enabled = true;
    rerender();
    expect(result.current.available).toBe(false);
    await waitFor(() => expect(result.current.providers).toEqual([]));
  });
});
