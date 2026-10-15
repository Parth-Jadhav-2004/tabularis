import { describe, expect, it } from "vitest";
import { aiChatTargetKey, sameAiChatTarget } from "../../src/utils/aiChatTarget";

describe("AI conversation targets", () => {
  it("keeps legacy targets in the editor session", () => {
    expect(sameAiChatTarget(null, { type: "editor" })).toBe(true);
    expect(sameAiChatTarget(undefined, null)).toBe(true);
    expect(aiChatTargetKey("one", null)).toBe(aiChatTargetKey("one", { type: "editor" }));
  });

  it("isolates notebook IDs and connections without delimiter collisions", () => {
    const notebook = { type: "notebook" as const, notebook_id: "book" };
    expect(sameAiChatTarget(notebook, { ...notebook })).toBe(true);
    expect(sameAiChatTarget(notebook, { ...notebook, notebook_id: "other" })).toBe(false);
    expect(sameAiChatTarget(notebook, null)).toBe(false);
    expect(aiChatTargetKey("one", notebook)).not.toBe(aiChatTargetKey("two", notebook));
    expect(aiChatTargetKey("one:two", { ...notebook, notebook_id: "three" }))
      .not.toBe(aiChatTargetKey("one", { ...notebook, notebook_id: "two:three" }));
  });
});
