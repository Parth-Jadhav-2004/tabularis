export type AiRemoteCommand = "generate" | "explain" | "optimize" | "fix";
export type AiSlashCommand = AiRemoteCommand | "help";

export type AiParsedCommand =
  | { kind: "text" }
  | { kind: "unknown"; name: string }
  | { kind: "command"; command: AiSlashCommand; args: string };
