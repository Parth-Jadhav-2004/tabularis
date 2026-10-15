import type { ReactNode } from "react";
import { renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { AiEditorContext } from "../../src/contexts/AiEditorContext";
import { CommandPaletteScopeContext } from "../../src/contexts/CommandPaletteScopeContext";
import { useAiEditor, useRegisterAiEditor } from "../../src/hooks/useAiEditor";
import { createAiEditorStore } from "../../src/utils/aiEditor";
import { createCommandScopeStore } from "../../src/utils/commandScopeStore";
import type { AiEditorTarget } from "../../src/types/aiEditor";

describe("active AI editor bridge", () => {
  it("follows the selected split pane and releases both registrations on unmount", () => {
    const store = createAiEditorStore();
    const paletteStore = createCommandScopeStore();
    let activeScopeId = "one";
    const one: AiEditorTarget = { connectionId: "one", database: "db1", schema: null,
      actions: { insert: vi.fn(), replace: vi.fn(), run: vi.fn(), focus: vi.fn() } };
    const two = { ...one, connectionId: "two", database: "db2" };
    const wrapper = ({ children }: { children: ReactNode }) => <AiEditorContext.Provider value={store}>
      <CommandPaletteScopeContext.Provider value={{ activeScopeId, store: paletteStore }}>{children}</CommandPaletteScopeContext.Provider>
    </AiEditorContext.Provider>;
    const { result, rerender, unmount } = renderHook(() => {
      useRegisterAiEditor("one", one);
      useRegisterAiEditor("two", two);
      return useAiEditor();
    }, { wrapper });
    expect(result.current).toBe(one);
    activeScopeId = "two";
    rerender();
    expect(result.current).toBe(two);
    unmount();
    expect(store.get("one")).toBeUndefined();
    expect(store.get("two")).toBeUndefined();
  });
});
