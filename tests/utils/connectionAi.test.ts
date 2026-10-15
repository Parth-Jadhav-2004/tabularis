import { describe, expect, it } from "vitest";
import { limitAiInstructions, MAX_AI_INSTRUCTION_CHARS } from "../../src/utils/connectionAi";

describe("connection AI preferences", () => {
  it("preserves exact text, whitespace and complete Unicode code points", () => {
    expect(limitAiInstructions("  Prefer explicit joins.\n😀  ")).toBe("  Prefer explicit joins.\n😀  ");
    const text = "😀".repeat(MAX_AI_INSTRUCTION_CHARS);
    expect(limitAiInstructions(text)).toBe(text);
    expect(limitAiInstructions(`${text}a`)).toBe(text);
    expect(limitAiInstructions("a".repeat(1999) + "😀b")).toBe("a".repeat(1999) + "😀");
    expect(limitAiInstructions("")).toBe("");
  });
});
