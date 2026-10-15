import { describe, expect, it, vi } from "vitest";
import type { AiNotebookSource } from "../../src/types/aiNotebook";
import type { AiEditorTarget } from "../../src/types/aiEditor";
import { buildAiNotebookSnapshot, createAiTargetContextOptions } from "../../src/utils/aiNotebookSnapshot";
import { defaultAiContextSelection } from "../../src/utils/aiContext";

function source(): AiNotebookSource {
  return { id: "notebook", title: "Revenue", stopOnError: true, params: [{ name: "region", value: "'EU'" }], focusedCellId: "two", runAllResult: null,
    cells: [
      { id: "one", type: "markdown", content: "## Analysis" },
      { id: "two", type: "sql", content: "SELECT * FROM orders WHERE region = @region", schema: "sales", name: "Revenue" },
      { id: "three", type: "sql", content: "SELECT * FROM customers" },
      { id: "four", type: "markdown", content: "Conclusion" },
    ] };
}

describe("bounded notebook snapshots", () => {
  it("captures the outline, focused cell, neighbours and mentioned cells with original indexes", () => {
    const snapshot = buildAiNotebookSnapshot(source(), { mentionedCellIds: ["four", "two", "four"] });
    expect(snapshot).toMatchObject({ id: "notebook", title: "Revenue", cell_count: 4, stop_on_error: true, params: [{ name: "region", value: "'EU'" }],
      focused: { id: "two", index: 2, schema: "sales", name: "Revenue" } });
    expect(snapshot.outline.map((cell) => cell.id)).toEqual(["one", "two", "three", "four"]);
    expect(snapshot.neighbours.map((cell) => cell.id)).toEqual(["one", "three"]);
    expect(snapshot.mentioned.map((cell) => cell.id)).toEqual(["four"]);
    expect(snapshot.truncated).toBe(false);
  });

  it("bounds large Unicode content, outlines, params and mentioned content without splitting characters", () => {
    const original = source();
    original.cells = Array.from({ length: 90 }, (_, index) => ({ id: `cell${index}`, type: "sql", content: "😀".repeat(10_000) }));
    original.focusedCellId = "cell1";
    original.params = Array.from({ length: 45 }, () => ({ name: "界".repeat(200), value: "😀".repeat(300) }));
    const snapshot = buildAiNotebookSnapshot(original, { mentionedCellIds: ["cell4", "cell5", "cell6"] });
    expect(snapshot.cell_count).toBe(90);
    expect(snapshot.outline.length).toBeLessThanOrEqual(80);
    expect(snapshot.outline.length).toBeGreaterThan(0);
    expect(snapshot.outline.every((cell) => Array.from(cell.content).length === 150)).toBe(true);
    expect(Array.from(snapshot.focused!.content)).toHaveLength(6_000);
    expect(snapshot.neighbours.every((cell) => Array.from(cell.content).length === 2_000)).toBe(true);
    expect(snapshot.mentioned.reduce((total, cell) => total + Array.from(cell.content).length, 0)).toBe(8_000);
    expect(snapshot.params.length).toBeLessThanOrEqual(40);
    expect(snapshot.params.length).toBeGreaterThan(0);
    expect(Array.from(snapshot.params[0].value)).toHaveLength(256);
    expect(snapshot.truncated).toBe(true);
    expect(original.cells[1].content).toHaveLength(20_000);
  });

  it("never reads results without opt-in and excludes both cell and Run All errors by default", () => {
    const original = source();
    original.cells = original.cells.map((cell) => ({ ...cell, error: "private-error" }));
    original.runAllResult = { total: 4, executed: 1, succeeded: 0, failed: 1, skipped: 3,
      errors: [{ cellId: "two", cellIndex: 1, error: "private-run-all-error" }] };
    Object.defineProperty(original.cells[1], "result", { get: () => { throw new Error("Result was read"); } });
    expect(buildAiNotebookSnapshot(original)).toMatchObject({ errors: [], results: [] });
    const snapshot = buildAiNotebookSnapshot(original, { includeErrors: true, focusedError: "exact-failure" });
    expect(snapshot.errors.find((error) => error.cell_id === "two")).toEqual({ cell_id: "two", index: 2, error: "exact-failure" });
    expect(snapshot.errors).toHaveLength(4);
    expect(snapshot.results).toEqual([]);
  });

  it("samples only chosen cells, bounds values and retains known total row counts", () => {
    const original = source();
    original.cells = original.cells.map((cell) => ({ ...cell, result: { columns: ["id", "value"],
      rows: Array.from({ length: 20 }, (_, index) => [index, "😀".repeat(300)]), affected_rows: 0,
      pagination: { page: 1, page_size: 20, total_rows: 200, has_more: true } } }));
    const snapshot = buildAiNotebookSnapshot(original, { includeResults: true, mentionedCellIds: ["three"] });
    expect(snapshot.results.map((result) => result.cell_id)).toEqual(["two", "three"]);
    expect(snapshot.results.every((result) => result.rows.length === 5 && result.row_count === 200 && result.truncated)).toBe(true);
    expect(Array.from(snapshot.results[0].rows[0][1])).toHaveLength(200);
    expect(snapshot.truncated).toBe(true);
  });

  it("caps result text across cells and handles nested or circular values", () => {
    const original = source();
    const circular: Record<string, unknown> = {};
    circular.self = circular;
    original.cells = original.cells.map((cell) => ({ ...cell, result: { columns: Array.from({ length: 45 }, (_, index) => `c${index}`),
      rows: Array.from({ length: 6 }, () => [circular, ...Array.from({ length: 44 }, () => "界".repeat(300))]), affected_rows: 0 } }));
    const snapshot = buildAiNotebookSnapshot(original, { includeResults: true, mentionedCellIds: ["one", "three", "four"] });
    expect(snapshot.results.flatMap((result) => [...result.columns, ...result.rows.flat()])
      .reduce((total, value) => total + Array.from(value).length, 0)).toBeLessThanOrEqual(6_000);
    expect(snapshot.results[0].columns).toHaveLength(40);
    expect(snapshot.results[0].rows[0][0]).toContain("[…]");
    expect(snapshot.truncated).toBe(true);
  });

  it("preserves empty and Markdown-only notebooks without inventing a focused cell", () => {
    expect(buildAiNotebookSnapshot({ ...source(), cells: [], focusedCellId: null })).toMatchObject({ cell_count: 0, focused: null, neighbours: [] });
    expect(buildAiNotebookSnapshot({ ...source(), focusedCellId: "one" }).focused).toMatchObject({ type: "markdown", content: "## Analysis" });
    expect(buildAiNotebookSnapshot({ ...source(), focusedCellId: "deleted" }).focused).toBeNull();
  });
});

describe("notebook target context", () => {
  it("includes selected cells by ID only for the matching notebook and reports unavailable selections", () => {
    const original = source();
    const target: AiEditorTarget = { connectionId: "connection", database: null, schema: null,
      notebook: { id: "notebook", readState: () => original }, actions: { insert: vi.fn(), replace: vi.fn(), run: vi.fn(), focus: vi.fn() } };
    const choices = { tables: [], excluded_tables: [], notebook: { notebook_id: "notebook", cell_ids: ["three", "deleted"] } };
    const context = createAiTargetContextOptions(target, defaultAiContextSelection(), "en", choices);
    expect(context.notebook?.mentioned.map((cell) => cell.id)).toEqual(["three"]);
    expect(context.notebook?.unavailable_cells).toEqual(["deleted"]);
    expect(context.notebook?.truncated).toBe(true);
    expect(createAiTargetContextOptions(target, defaultAiContextSelection(), "en", { ...choices, notebook: { ...choices.notebook, notebook_id: "other" } }).notebook?.mentioned).toEqual([]);
  });
  it("reads fresh state for preview/send and replaces full editor context with the notebook layer", () => {
    let original = source();
    const target: AiEditorTarget = { connectionId: "connection", database: "app", schema: "sales",
      notebook: { id: "notebook", readState: () => original }, readContext: () => ({ query: "editor-marker", cursor_statement: "cursor-marker", selection: "selected SQL" }),
      actions: { insert: vi.fn(), replace: vi.fn(), run: vi.fn(), focus: vi.fn() } };
    const include = defaultAiContextSelection();
    const first = createAiTargetContextOptions(target, include, "en");
    original = { ...original, title: "Updated" };
    const second = createAiTargetContextOptions(target, include, "en");
    expect(first.notebook?.title).toBe("Revenue");
    expect(second.notebook?.title).toBe("Updated");
    expect(second.editor).toEqual({ query: null, cursor_statement: null, selection: "selected SQL" });
    expect(JSON.stringify(second)).not.toContain("editor-marker");
    expect(createAiTargetContextOptions(target, { ...include, notebook: false, notebook_results: true }, "en").notebook).toBeUndefined();
  });

  it("keeps entry-point query/error snapshots exact while results remain excluded", () => {
    const original = source();
    const target: AiEditorTarget = { connectionId: "connection", database: null, schema: null,
      notebook: { id: "notebook", readState: () => original }, actions: { insert: vi.fn(), replace: vi.fn(), run: vi.fn(), focus: vi.fn() } };
    const context = createAiTargetContextOptions(target, { ...defaultAiContextSelection(), error: true }, "it", undefined,
      { query: "SELECT exact_failed_query", last_error: "exact-error" });
    expect(context.notebook?.focused?.content).toBe("SELECT exact_failed_query");
    expect(context.notebook?.errors).toEqual([{ cell_id: "two", index: 2, error: "exact-error" }]);
    expect(context.notebook?.results).toEqual([]);
    expect(context.last_error).toBeNull();
  });
});
