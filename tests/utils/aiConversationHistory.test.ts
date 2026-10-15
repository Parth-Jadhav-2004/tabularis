import { describe, expect, it } from "vitest";
import type { AiConversationSummary } from "../../src/types/aiChat";
import { createAiConversation, createAiMessage } from "../../src/utils/aiChat";
import { aiHistoryBucket, aiHistoryError, aiHistoryRelativeTime, groupAiHistory, restoreAiConversationModel, sameAiConversation } from "../../src/utils/aiConversationHistory";

const now = new Date(2026, 9, 26, 12);
function daysAgo(days: number) {
  return new Date(now.getFullYear(), now.getMonth(), now.getDate() - days).getTime();
}
function summary(title: string, days: number): AiConversationSummary {
  const conversation = createAiConversation({ connection_id: "one", provider: "openai", model: "model" });
  return { ...conversation, title, updated_at: daysAgo(days), message_count: 2 };
}

describe("conversation history", () => {
  it("uses local calendar boundaries for Today, Yesterday, seven days and older", () => {
    expect(aiHistoryBucket(daysAgo(0), now)).toBe("today");
    expect(aiHistoryBucket(daysAgo(0) - 1, now)).toBe("yesterday");
    expect(aiHistoryBucket(daysAgo(1), now)).toBe("yesterday");
    expect(aiHistoryBucket(daysAgo(6), now)).toBe("week");
    expect(aiHistoryBucket(daysAgo(6) - 1, now)).toBe("older");
    expect(aiHistoryBucket(daysAgo(8), now)).toBe("older");
    expect(aiHistoryBucket(daysAgo(-1), now)).toBe("today");
  });

  it("sorts without mutating the input and searches only case-insensitive titles", () => {
    const items = [summary("Old", 10), summary("Città SQL", 1), summary("Newest", 0)];
    expect(groupAiHistory(items, "", now).map((group) => group.bucket)).toEqual(["today", "yesterday", "older"]);
    expect(groupAiHistory(items, " CITTÀ ", now)[0].items[0].title).toBe("Città SQL");
    expect(groupAiHistory(items, "missing", now)).toEqual([]);
    expect(items[0].title).toBe("Old");
  });

  it("formats relative time using the selected locale", () => {
    expect(aiHistoryRelativeTime(now.getTime() - 120_000, "en", now.getTime())).toBe("2 minutes ago");
    expect(aiHistoryRelativeTime(now.getTime() - 7_200_000, "it", now.getTime())).toBe("2 ore fa");
    expect(aiHistoryRelativeTime(now.getTime() - 86_400_000, "en", now.getTime())).toBe("yesterday");
  });

  it("restores an available model and otherwise changes only the next-turn choice", () => {
    const original = { ...createAiConversation({ connection_id: "one", provider: "openai", model: "old" }),
      mode: "agent" as const, messages: [{ ...createAiMessage("assistant"), provider: "openai", model: "old" }] };
    const fallback = { provider: "ollama", model: "local" };
    expect(restoreAiConversationModel(original, { choices: [{ provider: "openai", model: "old" }], fallback }).conversation).toBe(original);
    const restored = restoreAiConversationModel(original, { choices: [fallback], fallback });
    expect(restored).toMatchObject({ modelFallback: true, conversation: { ...fallback, mode: "agent" } });
    expect(restored.conversation.messages).toBe(original.messages);
    expect(restored.conversation.messages[0].model).toBe("old");
    expect(restoreAiConversationModel(original, { choices: [], fallback: null }).conversation).toBe(original);
  });

  it("sanitizes IPC errors and distinguishes equal conversation IDs on different connections", () => {
    expect(aiHistoryError({ kind: "key_unavailable", sensitive: "secret" })).toBe("key_unavailable");
    expect(aiHistoryError(new Error("secret"))).toBe("io");
    expect(aiHistoryError({ kind: "unexpected" })).toBe("io");
    expect(sameAiConversation({ connection_id: "one", id: "same" }, { connection_id: "two", id: "same" })).toBe(false);
    expect(sameAiConversation(null, { connection_id: "one", id: "same" })).toBe(false);
  });
});
