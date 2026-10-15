import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/plugin-fs";
import type { AiConversationSummary } from "../types/aiChat";

export type AiConversationExportFormat = "markdown" | "notebook";

export function aiConversationExportFilename(title: string, format: AiConversationExportFormat): string {
  const safe = Array.from(title, (char) => char.codePointAt(0)! < 32 || /[<>:"/\\|?*]/.test(char) ? "_" : char).join("").trim();
  let name = "";
  for (const char of Array.from(safe).slice(0, 80)) {
    if (new TextEncoder().encode(name + char).length > 160) break;
    name += char;
  }
  name = name.replace(/[. ]+$/, "");
  // A fixed prefix also avoids reserved Windows device filenames.
  return `chat-${name || "conversation"}.${format === "markdown" ? "md" : "tabularis-notebook"}`;
}

export async function exportAiConversation(summary: AiConversationSummary, format: AiConversationExportFormat, dialogTitle: string): Promise<boolean> {
  const path = await save({
    title: dialogTitle,
    defaultPath: aiConversationExportFilename(summary.title, format),
    filters: [{ name: format === "markdown" ? "Markdown" : "Tabularis Notebook", extensions: [format === "markdown" ? "md" : "tabularis-notebook"] }],
  });
  if (!path) return false;
  const content = await invoke<string>("ai_chat_export", { connectionId: summary.connection_id, id: summary.id, format });
  await writeTextFile(path, content);
  return true;
}
