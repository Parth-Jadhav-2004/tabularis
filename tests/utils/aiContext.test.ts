import { describe, expect, it } from "vitest";
import { aiContextLabelKey, aiContextSize, createAiContextOptions, defaultAiContextSelection } from "../../src/utils/aiContext";

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
