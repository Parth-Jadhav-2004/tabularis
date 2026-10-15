import type { AiParsedCommand, AiSlashCommand } from "../types/aiCommands";
import type { AiContextSelection, AiEditorSnapshot } from "../types/aiContext";
import { defaultAiContextSelection } from "./aiContext";

export const AI_SLASH_COMMANDS: readonly AiSlashCommand[] = ["generate", "explain", "optimize", "fix", "help"];

// Only a slash at the very start is a command. Keep the original message text
// in encrypted history so exports and older clients retain the action as well.
export function parseAiCommand(text: string): AiParsedCommand {
  if (!text.startsWith("/")) return { kind: "text" };
  const match = /^\/([^\s]*)(?:\s+([\s\S]*))?$/.exec(text)!;
  const command = AI_SLASH_COMMANDS.find((name) => name === match[1]);
  return command ? { kind: "command", command, args: (match[2] ?? "").trim() }
    : { kind: "unknown", name: match[1] };
}

export function activeAiCommands(text: string, start: number, end: number): readonly AiSlashCommand[] | null {
  if (start !== end || end !== text.length || !/^\/[a-z]*$/.test(text)) return null;
  return AI_SLASH_COMMANDS.filter((command) => command.startsWith(text.slice(1)));
}

export function aiCommandContext(command: AiSlashCommand, snapshot: AiEditorSnapshot): AiContextSelection {
  const defaults = defaultAiContextSelection();
  if (command === "help") return { connection: false, instructions: false, schema: false, query: false,
    selection: false, cursor_statement: false, error: false };
  if (command === "generate") return defaults;
  return { ...defaults, query: !snapshot.selection?.trim(), selection: !!snapshot.selection?.trim(),
    cursor_statement: false, error: command === "fix" };
}
