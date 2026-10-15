export const MAX_AI_INSTRUCTION_CHARS = 2_000;

export function limitAiInstructions(value: string): string {
  return Array.from(value).slice(0, MAX_AI_INSTRUCTION_CHARS).join("");
}
