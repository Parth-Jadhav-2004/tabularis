import { act, renderHook } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useAiContextPreview } from "../../src/hooks/useAiContextPreview";
import { createAiContextOptions, defaultAiContextSelection } from "../../src/utils/aiContext";
import type { AiContextPreview } from "../../src/types/aiContext";

const preview: AiContextPreview = { items: [{ kind: "table", id: "users", label: "users", chars: 200 }], truncated: false, total_chars: 1400 };
function context(connection_id = "one") {
  return { connection_id, database: "app", schema: "public", ...createAiContextOptions({ query: "SELECT * FROM users" }, defaultAiContextSelection(), "en") };
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => { resolve = done; });
  return { promise, resolve };
}
describe("AI context preview lifecycle", () => {
  beforeEach(() => { vi.useFakeTimers(); vi.mocked(invoke).mockReset(); });
  afterEach(() => { vi.useRealTimers(); });
  it("debounces preparation, reads the latest editor state and refreshes without sending", async () => {
    vi.mocked(invoke).mockResolvedValue(preview);
    const snapshot = context();
    const getContext = vi.fn(() => snapshot);
    const { result } = renderHook(() => useAiContextPreview(getContext, true));
    expect(invoke).not.toHaveBeenCalled();
    snapshot.editor.query = "fresh query";
    await act(async () => { await vi.advanceTimersByTimeAsync(350); });
    expect(invoke).toHaveBeenCalledWith("ai_chat_preview_context", { context: snapshot });
    expect(getContext).toHaveBeenCalledTimes(1);
    expect(result.current.preview).toEqual(preview);
    await act(async () => { await result.current.refresh(); });
    expect(invoke).toHaveBeenCalledTimes(2);
  });
  it("hides old summaries immediately and ignores stale responses after a scope change", async () => {
    const first = deferred<AiContextPreview>();
    const second = deferred<AiContextPreview>();
    vi.mocked(invoke).mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);
    const { result, rerender } = renderHook(({ get }) => useAiContextPreview(get, true), { initialProps: { get: () => context() } });
    await act(async () => { await vi.advanceTimersByTimeAsync(350); });
    rerender({ get: () => context("two") });
    expect(result.current.preview).toBeNull();
    await act(async () => { await vi.advanceTimersByTimeAsync(350); });
    await act(async () => { second.resolve({ ...preview, total_chars: 2000 }); });
    await act(async () => { first.resolve(preview); });
    expect(result.current.preview?.total_chars).toBe(2000);
  });
  it("ignores responses after disabling or unmounting and makes failures retryable", async () => {
    vi.mocked(invoke).mockRejectedValueOnce(new Error("private-driver-error"));
    const getContext = () => context();
    const { result, rerender, unmount } = renderHook(({ enabled }) => useAiContextPreview(getContext, enabled), { initialProps: { enabled: true } });
    await act(async () => { await vi.advanceTimersByTimeAsync(350); });
    expect(result.current.error).toBe(true);
    const late = deferred<AiContextPreview>();
    vi.mocked(invoke).mockReturnValueOnce(late.promise);
    act(() => { void result.current.refresh(); });
    rerender({ enabled: false });
    await act(async () => { late.resolve(preview); });
    expect(result.current.preview).toBeNull();
    expect(result.current.error).toBe(false);
    vi.mocked(invoke).mockResolvedValueOnce(preview);
    rerender({ enabled: true });
    await act(async () => { await vi.advanceTimersByTimeAsync(350); });
    expect(result.current.preview).toEqual(preview);
    const last = deferred<AiContextPreview>();
    vi.mocked(invoke).mockReturnValueOnce(last.promise);
    act(() => { void result.current.refresh(); });
    unmount();
    await act(async () => { last.resolve(preview); });
  });
  it("manual refresh cancels pending debounce and disabled contexts never invoke", async () => {
    vi.mocked(invoke).mockResolvedValue(preview);
    const getContext = () => context();
    const { result, rerender } = renderHook(({ enabled }) => useAiContextPreview(getContext, enabled), { initialProps: { enabled: false } });
    await act(async () => { await result.current.refresh(); await vi.advanceTimersByTimeAsync(350); });
    expect(invoke).not.toHaveBeenCalled();
    rerender({ enabled: true });
    await act(async () => { await result.current.refresh(); await vi.advanceTimersByTimeAsync(350); });
    expect(invoke).toHaveBeenCalledTimes(1);
  });
});
