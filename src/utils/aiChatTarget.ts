import type { AiChatTarget } from "../types/aiChat";

// Legacy conversations without a target belong to the editor session.
export function aiChatTargetKey(connectionId: string | null, target?: AiChatTarget | null): string {
  return JSON.stringify([connectionId, target?.type === "notebook" ? target.notebook_id : null]);
}

export function sameAiChatTarget(a?: AiChatTarget | null, b?: AiChatTarget | null): boolean {
  return aiChatTargetKey(null, a) === aiChatTargetKey(null, b);
}
