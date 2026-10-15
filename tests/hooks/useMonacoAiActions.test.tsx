import { act, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { editor } from "monaco-editor";
import type { AiEditorMenuActions } from "../../src/types/aiEditor";
import { useMonacoAiActions } from "../../src/hooks/useMonacoAiActions";

describe("Monaco AI menu availability", () => {
  it("registers after mounting, removes actions while busy/disabled and releases them on unmount", () => {
    const dispose = vi.fn();
    const addAction = vi.fn(() => ({ dispose }));
    const codeEditor = { addAction } as unknown as editor.IStandaloneCodeEditor;
    const actions: AiEditorMenuActions = { disabled: false, run: vi.fn() };
    const hook = renderHook(({ current }: { current?: AiEditorMenuActions }) => useMonacoAiActions(current),
      { initialProps: { current: undefined as AiEditorMenuActions | undefined } });
    act(() => hook.result.current(codeEditor));
    expect(addAction).not.toHaveBeenCalled();
    hook.rerender({ current: actions });
    expect(addAction).toHaveBeenCalledTimes(3);
    hook.rerender({ current: { ...actions, disabled: true } });
    expect(dispose).toHaveBeenCalledTimes(3);
    hook.rerender({ current: actions });
    expect(addAction).toHaveBeenCalledTimes(6);
    hook.unmount();
    expect(dispose).toHaveBeenCalledTimes(6);
  });
});
