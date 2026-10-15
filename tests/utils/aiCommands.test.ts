import { describe, expect, it } from "vitest";
import { activeAiCommands, aiCommandContext, parseAiCommand } from "../../src/utils/aiCommands";

describe("AI slash commands", () => {
  it("parses arguments, whitespace and a command without arguments", () => {
    expect(parseAiCommand("/generate  Count users\nby country ")).toEqual({ kind: "command", command: "generate", args: "Count users\nby country" });
    expect(parseAiCommand("/explain")).toEqual({ kind: "command", command: "explain", args: "" });
    expect(parseAiCommand("/fix\tmissing column")).toEqual({ kind: "command", command: "fix", args: "missing column" });
    expect(parseAiCommand("/help ")).toEqual({ kind: "command", command: "help", args: "" });
  });
  it("recognizes only exact supported names at the beginning", () => {
    for (const text of ["", "explain /fix", " /explain", "https://example.test/help"]) expect(parseAiCommand(text)).toEqual({ kind: "text" });
    for (const name of ["", "unknown", "Explain", "explainMore", "tmp/query.sql"]) expect(parseAiCommand(`/${name}`)).toEqual({ kind: "unknown", name });
  });
  it("offers filtered commands only at a collapsed caret after the initial token", () => {
    expect(activeAiCommands("/", 1, 1)).toEqual(["generate", "explain", "optimize", "fix", "help"]);
    expect(activeAiCommands("/ex", 3, 3)).toEqual(["explain"]);
    expect(activeAiCommands("/unknown", 8, 8)).toEqual([]);
    expect(activeAiCommands("/ex", 0, 3)).toBeNull();
    expect(activeAiCommands("/explain ", 9, 9)).toBeNull();
    expect(activeAiCommands("text /ex", 8, 8)).toBeNull();
  });
  it("prefers a selection for SQL actions and uses the query when there is none", () => {
    for (const command of ["explain", "optimize", "fix"] as const) {
      expect(aiCommandContext(command, { selection: "SELECT 1" })).toMatchObject({ query: false, selection: true, schema: true, cursor_statement: false, error: command === "fix" });
      expect(aiCommandContext(command, { selection: "  " })).toMatchObject({ query: true, selection: false });
    }
    expect(aiCommandContext("generate", { selection: "SELECT 1" })).toMatchObject({ query: true, schema: true, error: false });
    expect(Object.values(aiCommandContext("help", {}))).toEqual(Array(7).fill(false));
  });
});
