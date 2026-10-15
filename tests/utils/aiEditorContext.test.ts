import { describe, expect, it } from "vitest";
import type { editor } from "monaco-editor";
import { readAiEditorContext } from "../../src/utils/aiEditor";

describe("AI active editor snapshots", () => {
  it("reads current Monaco text, selection and cursor statement before debounced tab state", () => {
    const query = "SELECT 1;\nSELECT name FROM users;";
    const instance = {
      getValue: () => query,
      getSelection: () => ({ isEmpty: () => false }),
      getPosition: () => ({ lineNumber: 2, column: 8 }),
      getModel: () => ({ getValueInRange: () => "name", getOffsetAt: () => query.indexOf("name") }),
    } as unknown as editor.IStandaloneCodeEditor;
    const result = readAiEditorContext(instance, "stale tab text", "postgres", "last failure");
    expect(result.query).toBe(query);
    expect(result.selection).toBe("name");
    expect(result.cursor_statement).toContain("SELECT name FROM users");
    expect(result.cursor_statement).not.toContain("SELECT 1");
    expect(result.last_error).toBe("last failure");
  });
  it("uses fallback text without inventing a selection or a cursor statement", () => {
    expect(readAiEditorContext(null, "SELECT 42", "sqlite", "")).toEqual({
      query: "SELECT 42", selection: null, cursor_statement: null, last_error: null,
    });
  });
});
