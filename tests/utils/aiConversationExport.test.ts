import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/plugin-fs";
import { aiConversationExportFilename, exportAiConversation } from "../../src/utils/aiConversationExport";
import { createAiConversation } from "../../src/utils/aiChat";

const summary = { ...createAiConversation({ connection_id: "connection", provider: "openai", model: "model" }), title: "A / title", message_count: 2 };
beforeEach(() => { vi.clearAllMocks(); });

describe("conversation file export", () => {
  it("sanitizes portable filenames and preserves Unicode", () => {
    expect(aiConversationExportFilename("A / title", "markdown")).toBe("chat-A _ title.md");
    expect(aiConversationExportFilename("CON", "notebook")).toBe("chat-CON.tabularis-notebook");
    expect(aiConversationExportFilename("... ", "markdown")).toBe("chat-conversation.md");
    expect(aiConversationExportFilename("🦊".repeat(100), "markdown")).toBe(`chat-${"🦊".repeat(40)}.md`);
  });
  it("does not load or write plaintext when the save dialog is cancelled", async () => {
    vi.mocked(save).mockResolvedValueOnce(null);
    expect(await exportAiConversation(summary, "markdown", "Unencrypted export")).toBe(false);
    expect(invoke).not.toHaveBeenCalled();
    expect(writeTextFile).not.toHaveBeenCalled();
  });
  it.each(["markdown", "notebook"] as const)("saves %s only to the chosen path with an unencrypted dialog title", async (format) => {
    vi.mocked(save).mockResolvedValueOnce("/chosen/file");
    vi.mocked(invoke).mockResolvedValueOnce("exported content");
    expect(await exportAiConversation(summary, format, "Unencrypted export")).toBe(true);
    expect(save).toHaveBeenCalledWith(expect.objectContaining({ title: "Unencrypted export", defaultPath: aiConversationExportFilename(summary.title, format) }));
    expect(invoke).toHaveBeenCalledWith("ai_chat_export", { connectionId: "connection", id: summary.id, format });
    expect(writeTextFile).toHaveBeenCalledExactlyOnceWith("/chosen/file", "exported content");
  });
  it("does not write a file when decrypting the conversation fails", async () => {
    vi.mocked(save).mockResolvedValueOnce("/chosen/file");
    vi.mocked(invoke).mockRejectedValueOnce({ kind: "key_unavailable" });
    await expect(exportAiConversation(summary, "markdown", "Unencrypted export")).rejects.toMatchObject({ kind: "key_unavailable" });
    expect(writeTextFile).not.toHaveBeenCalled();
  });
});
