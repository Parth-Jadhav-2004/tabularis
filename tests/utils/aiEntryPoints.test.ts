import { describe, expect, it, vi } from "vitest";
import { aiFixEntry, aiQueryEntry, aiSelectionEntry, aiTableEntry, createAiQueryFailures, createAiTableTarget } from "../../src/utils/aiEntryPoints";
import { createAiContextOptions } from "../../src/utils/aiContext";
import { createAiEditorStore } from "../../src/utils/aiEditor";
import type { AiEditorTarget } from "../../src/types/aiEditor";

describe("AI entry-point context", () => {
  const snapshot = { query: "private whole query", selection: "SELECT selected", cursor_statement: "private cursor SQL", last_error: "unrelated error" };

  it("includes only selected SQL for selection actions", () => {
    const options = aiSelectionEntry(snapshot);
    const context = createAiContextOptions(options.snapshot!, options.include!, "en");
    expect(context.editor).toEqual({ query: null, selection: "SELECT selected", cursor_statement: null });
    expect(context.last_error).toBeNull();
    expect(JSON.stringify(context)).not.toContain("private");
  });

  it("palette query actions exclude a retained editor selection and unrelated errors", () => {
    const options = aiQueryEntry(snapshot);
    const context = createAiContextOptions(options.snapshot!, options.include!, "en");
    expect(context.editor).toEqual({ query: "private whole query", selection: null, cursor_statement: null });
    expect(context.last_error).toBeNull();
  });

  it("fixes the exact failed statement and complete error, without unrelated selection context", () => {
    const options = aiFixEntry("SELECT failed", "missing column\n\nserver detail");
    const context = createAiContextOptions(options.snapshot!, options.include!, "en");
    expect(context.editor).toEqual({ query: "SELECT failed", selection: null, cursor_statement: null });
    expect(context.last_error).toBe("missing column\n\nserver detail");
    expect(context.include.error).toBe(true);
  });

  it("selects a table by its exact name and excludes editor content", () => {
    const options = aiTableEntry("sales orders");
    expect(options.tables).toEqual(["sales orders"]);
    expect(options.include).toMatchObject({ schema: true, query: false, selection: false, cursor_statement: false, error: false });
  });
});

describe("transient failed-query context", () => {
  it("preserves executed SQL independently from drafts and ignores stale errors", () => {
    const failures = createAiQueryFailures();
    failures.record("tab", "SELECT :resolved_parameter", "failed");
    expect(failures.read("tab", "failed")).toBe("SELECT :resolved_parameter");
    expect(failures.read("tab", "other error")).toBeUndefined();
    failures.clear("tab");
    expect(failures.read("tab", "failed")).toBeUndefined();
  });

  it("bounds cached failures and refreshes an existing tab's recency", () => {
    const failures = createAiQueryFailures();
    for (let index = 0; index < 100; index++) failures.record(String(index), `SELECT ${index}`, "failed");
    failures.record("0", "SELECT new", "new error");
    failures.record("overflow", "SELECT 101", "failed");
    expect(failures.read("0", "new error")).toBe("SELECT new");
    expect(failures.read("1", "failed")).toBeUndefined();
  });
});

describe("explorer AI target", () => {
  const base: AiEditorTarget = { id: "console", connectionId: "connection", database: "primary", schema: "public",
    actions: { insert: vi.fn(), replace: vi.fn(), run: vi.fn(), focus: vi.fn() } };

  it("pins chat actions to the clicked table's database/schema and never auto-runs Insert/Replace", () => {
    const openEditor = vi.fn();
    const target = createAiTableTarget({ connectionId: "connection", tableName: "orders", database: "reporting", schema: "sales" }, base, openEditor);
    target.actions.insert("SELECT 1");
    target.actions.replace("SELECT 2");
    target.actions.run("SELECT 3");
    expect(openEditor.mock.calls.map(([request]) => request)).toEqual([1, 2, 3].map((index) => ({
      kind: "console", targetConnectionId: "connection", database: "reporting", schema: "sales",
      initialQuery: `SELECT ${index}`, preventAutoRun: index !== 3,
    })));
    expect(target.readContext?.()).toEqual({});
  });

  it("releases temporary table targets on editor changes and owner unmount", () => {
    const store = createAiEditorStore();
    const remove = store.register("left", base);
    const target = createAiTableTarget({ connectionId: "connection", tableName: "orders" }, base, vi.fn());
    store.setContextTarget("left", target);
    expect(store.get("left")).toBe(target);
    store.register("right", { ...base, connectionId: "other" });
    expect(store.get("left")).toBe(target);
    store.register("left", { ...base, id: "new console" });
    expect(store.get("left")?.id).toBe("new console");
    remove();
    expect(store.get("left")?.id).toBe("new console");
    const release = store.register("left", base);
    store.setContextTarget("left", target);
    release();
    expect(store.get("left")).toBeUndefined();
  });
});
