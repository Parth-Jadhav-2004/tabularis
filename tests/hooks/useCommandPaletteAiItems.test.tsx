import type { ReactNode } from "react";
import { act, renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { AiChatProvider } from "../../src/contexts/AiChatProvider";
import { AiAvailabilityContext } from "../../src/contexts/AiAvailabilityContext";
import { AiEditorContext } from "../../src/contexts/AiEditorContext";
import { CommandPaletteScopeContext } from "../../src/contexts/CommandPaletteScopeContext";
import { RightSidebarProvider } from "../../src/contexts/RightSidebarProvider";
import { useCommandPaletteActionItems } from "../../src/hooks/useCommandPaletteActionItems";
import { createAiEditorStore } from "../../src/utils/aiEditor";
import { createCommandScopeStore } from "../../src/utils/commandScopeStore";
import type { AiChatTransport } from "../../src/types/aiChat";
import type { AiEditorTarget } from "../../src/types/aiEditor";

vi.mock("../../src/hooks/useKeybindings", () => ({ useKeybindings: () => ({ shortcuts: [], overrides: {}, isMac: false }) }));
vi.mock("../../src/hooks/useCommandPalette", () => ({ useCommandPaletteState: () => ({ focusContext: null }) }));

function setup() {
  const editors = createAiEditorStore();
  const scopes = createCommandScopeStore();
  const actions = { insert: vi.fn(), replace: vi.fn(), run: vi.fn(), focus: vi.fn() };
  const target: AiEditorTarget = { id: "left-query", connectionId: "left", database: "db", schema: "public", actions,
    readContext: () => ({ query: "SELECT whole query", selection: "private selection", last_error: "private error" }) };
  editors.register("left", target);
  editors.register("right", { ...target, id: "right-query", connectionId: "right" });
  for (const connectionId of ["left", "right"]) scopes.registerScope(connectionId, { connectionId, driver: "postgres", table: null,
    runtime: { navigate: vi.fn(), openEditor: vi.fn() } });
  const transport: AiChatTransport = { listen: async () => () => {}, listenStorageCleared: async () => () => {},
    send: async (_request, id) => id, save: async () => ({ bytes: 0, large: false }), cancel: async () => {} };
  let activeScopeId = "left";
  let available = true;
  const wrapper = ({ children }: { children: ReactNode }) => <AiAvailabilityContext.Provider value={{ available,
    loading: false, providers: [], refresh: vi.fn() }}><AiEditorContext.Provider value={editors}>
    <CommandPaletteScopeContext.Provider value={{ activeScopeId, store: scopes }}>
      <RightSidebarProvider><AiChatProvider transport={transport}>{children}</AiChatProvider></RightSidebarProvider>
    </CommandPaletteScopeContext.Provider>
  </AiEditorContext.Provider></AiAvailabilityContext.Provider>;
  const hook = renderHook(() => useCommandPaletteActionItems(vi.fn(), vi.fn()), { wrapper });
  return { ...hook, editors, target, setScope: (id: string) => { activeScopeId = id; hook.rerender(); },
    setAvailable: (value: boolean) => { available = value; hook.rerender(); } };
}

describe("AI command-palette query entries", () => {
  it.each(["explain", "optimize"] as const)("launches /%s on the whole current query, with the active split-pane scope", async (command) => {
    const fixture = setup();
    await waitFor(() => expect(fixture.result.current.some((item) => item.id === `ai.${command}-query`)).toBe(true));
    fixture.setScope("right");
    const item = fixture.result.current.find((entry) => entry.id === `ai.${command}-query`)!;
    act(() => { void item.primaryAction.execute(); });
    expect(fixture.editors.getEntry()).toMatchObject({ scopeId: "right", command, target: { connectionId: "right" },
      snapshot: { query: "SELECT whole query", selection: null, cursor_statement: null },
      include: { query: true, selection: false, error: false } });
  });

  it("hides query actions without SQL or a usable AI provider", async () => {
    const fixture = setup();
    await waitFor(() => expect(fixture.result.current.some((item) => item.id === "ai.explain-query")).toBe(true));
    act(() => { fixture.editors.register("left", { ...fixture.target, readContext: () => ({ query: "  " }) }); });
    expect(fixture.result.current.some((item) => item.id === "ai.explain-query")).toBe(false);
    fixture.setAvailable(false);
    expect(fixture.result.current.some((item) => item.id.startsWith("ai."))).toBe(false);
  });
});
