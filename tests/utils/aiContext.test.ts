import { describe, expect, it } from "vitest";
import { aiContextLabelKey, aiContextSize, aiContextTableCount, changeAiContextTable, createAiContextOptions, defaultAiContextSelection, sameAiContextScope } from "../../src/utils/aiContext";

describe("AI context choices", () => {
  it("omits excluded editor data and includes errors only by explicit choice", () => {
    const include = { ...defaultAiContextSelection(), query: false, selection: false, cursor_statement: false };
    const snapshot = { query: "private-query", selection: "private-selection", cursor_statement: "private-cursor", last_error: "private-error" };
    const options = createAiContextOptions(snapshot, include, "it");
    expect(options.editor).toEqual({ query: null, selection: null, cursor_statement: null });
    expect(options.last_error).toBeNull();
    expect(JSON.stringify(options)).not.toContain("private-");
    expect(createAiContextOptions(snapshot, { ...include, error: true }, "it").last_error).toBe("private-error");
  });
  it("keeps query text exact and has independent defaults for each new turn", () => {
    const include = defaultAiContextSelection();
    const query = " SELECT '界';\n";
    expect(createAiContextOptions({ query }, include, "ja").editor.query).toBe(query);
    include.schema = false;
    expect(defaultAiContextSelection().schema).toBe(true);
  });
  it("uses known translation keys and renders only valid optional sizes", () => {
    expect(aiContextLabelKey("table")).toBe("aiChat.context.kinds.table");
    expect(aiContextLabelKey("unknown.kind")).toBe("aiChat.context.kinds.other");
    const item = { kind: "table", id: "users", label: "users" };
    expect(aiContextSize(item)).toBeNull();
    expect(aiContextSize({ ...item, chars: 0 })).toBe(0);
    expect(aiContextSize({ ...item, chars: 123 })).toBe(123);
    for (const chars of [-1, 1.2, Infinity, NaN]) expect(aiContextSize({ ...item, chars })).toBeNull();
  });
});

describe("AI table context choices", () => {
  it("pins, excludes and restores exact names without mutating the previous choices", () => {
    const start = { tables: ["orders"], excluded_tables: ["users"] };
    const pinned = changeAiContextTable(start, "users", true);
    expect(pinned).toEqual({ tables: ["orders", "users"], excluded_tables: [] });
    expect(start).toEqual({ tables: ["orders"], excluded_tables: ["users"] });
    const excluded = changeAiContextTable(pinned, "orders", false);
    expect(excluded).toEqual({ tables: ["users"], excluded_tables: ["orders"] });
    expect(createAiContextOptions({}, defaultAiContextSelection(), "it", excluded)).toMatchObject(excluded);
  });
  it("bounds both choices and matches the complete scope", () => {
    const full = { tables: Array.from({ length: 100 }, (_, i) => `table${i}`), excluded_tables: [] };
    expect(changeAiContextTable(full, "overflow", true)).toBe(full);
    expect(changeAiContextTable(full, "table0", true).tables).toHaveLength(100);
    const excluded = { tables: [], excluded_tables: full.tables };
    expect(changeAiContextTable(excluded, "overflow", false)).toBe(excluded);
    const scope = { connection_id: "one", database: "app", schema: "public" };
    expect(sameAiContextScope(scope, { ...scope })).toBe(true);
    for (const change of [{ connection_id: "two" }, { database: null }, { schema: "other" }]) expect(sameAiContextScope(scope, { ...scope, ...change })).toBe(false);
    expect(sameAiContextScope(null, scope)).toBe(false);
    expect(sameAiContextScope(scope, null)).toBe(false);
  });
  it("accepts only safe optional table counts", () => {
    const item = { kind: "schema", id: "schema", label: "Schema" };
    expect(aiContextTableCount(item)).toBeNull();
    expect(aiContextTableCount({ ...item, table_count: 0 })).toBe(0);
    expect(aiContextTableCount({ ...item, table_count: 10000 })).toBe(10000);
    for (const table_count of [-1, 0.5, Infinity, NaN]) expect(aiContextTableCount({ ...item, table_count })).toBeNull();
  });
});
