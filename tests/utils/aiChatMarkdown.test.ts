import { describe, expect, it } from "vitest";
import { isAiSqlCode } from "../../src/utils/aiChatMarkdown";

describe("isAiSqlCode", () => {
  it("recognizes SQL dialect fences but leaves other languages alone", () => {
    for (const language of ["sql", "SQL", "postgresql", "mysql", "sqlite", "tsql", "plsql"]) {
      expect(isAiSqlCode(language, "SELECT 1;")).toBe(true);
    }
    expect(isAiSqlCode("javascript", "SELECT 1;")).toBe(false);
  });
  it("recognizes untagged SQL with comments and multiple statements", () => {
    expect(isAiSqlCode("", "-- example\nSELECT 1;\nSELECT 2;", "sqlite")).toBe(true);
    expect(isAiSqlCode("", "/* example */ UPDATE users SET name = 'Ada';")).toBe(true);
  });
  it("leaves prose, inline snippets, empty blocks and mixed languages alone", () => {
    for (const code of ["", "choose a table", "SELECT", "const x = 1;", "SELECT 1; console.log(1);"]) {
      expect(isAiSqlCode("", code)).toBe(false);
    }
  });
});
