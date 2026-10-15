import type { AiConversation, AiConversationSummary, AiHistoryError, AiResumeOptions } from "../types/aiChat";

export type AiHistoryBucket = "today" | "yesterday" | "week" | "older";

// Calendar boundaries use local time, including days that cross a DST change.
export function aiHistoryBucket(timestamp: number, now = new Date()): AiHistoryBucket {
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const yesterday = new Date(today);
  yesterday.setDate(yesterday.getDate() - 1);
  const week = new Date(today);
  week.setDate(week.getDate() - 6);
  if (timestamp >= today.getTime()) return "today";
  if (timestamp >= yesterday.getTime()) return "yesterday";
  if (timestamp >= week.getTime()) return "week";
  return "older";
}

export function groupAiHistory(items: AiConversationSummary[], search: string, now = new Date()) {
  const query = search.trim().toLocaleLowerCase();
  const groups: { bucket: AiHistoryBucket; items: AiConversationSummary[] }[] = [];
  const sorted = items.filter((item) => item.title.toLocaleLowerCase().includes(query))
    .sort((a, b) => b.updated_at - a.updated_at || a.id.localeCompare(b.id));
  for (const item of sorted) {
    const bucket = aiHistoryBucket(item.updated_at, now);
    const group = groups.at(-1);
    if (group?.bucket === bucket) group.items.push(item);
    else groups.push({ bucket, items: [item] });
  }
  return groups;
}

export function aiHistoryRelativeTime(timestamp: number, language: string, now = Date.now()): string {
  const minutes = Math.round((timestamp - now) / 60_000);
  const formatter = new Intl.RelativeTimeFormat(language, { numeric: "auto" });
  if (Math.abs(minutes) < 60) return formatter.format(minutes, "minute");
  const hours = Math.round(minutes / 60);
  if (Math.abs(hours) < 24) return formatter.format(hours, "hour");
  return formatter.format(Math.round(hours / 24), "day");
}

export function aiHistoryError(error: unknown): AiHistoryError {
  const kinds: AiHistoryError[] = ["key_unavailable", "corrupted", "wrong_context", "io",
    "invalid_input", "too_large", "unsupported_version"];
  if (error && typeof error === "object" && "kind" in error
    && kinds.includes(error.kind as AiHistoryError)) return error.kind as AiHistoryError;
  return "io";
}

export function restoreAiConversationModel(conversation: AiConversation, options: AiResumeOptions) {
  if (options.choices.some((choice) => choice.provider === conversation.provider && choice.model === conversation.model)
    || !options.fallback) return { conversation, modelFallback: false };
  return { conversation: { ...conversation, ...options.fallback }, modelFallback: true };
}

export function sameAiConversation(a: { connection_id: string; id: string } | null, b: { connection_id: string; id: string }) {
  return a?.connection_id === b.connection_id && a.id === b.id;
}
