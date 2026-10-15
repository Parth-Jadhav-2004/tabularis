import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ConversationExport } from "../../../src/components/ai-chat/ConversationExport";
import { exportAiConversation } from "../../../src/utils/aiConversationExport";
import { createAiConversation } from "../../../src/utils/aiChat";

vi.mock("../../../src/utils/aiConversationExport", () => ({ exportAiConversation: vi.fn() }));
const item = { ...createAiConversation({ connection_id: "connection", provider: "openai", model: "model" }), title: "Title", message_count: 2 };
beforeEach(() => { vi.clearAllMocks(); });

describe("conversation export controls", () => {
  it("shows the encryption notice and exports the selected format", async () => {
    vi.mocked(exportAiConversation).mockResolvedValueOnce(true);
    render(<ConversationExport item={item} disabled={false} />);
    fireEvent.click(screen.getByRole("button", { name: "aiChat.export.label" }));
    expect(screen.getByText("aiChat.export.unencrypted")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "aiChat.export.notebook" }));
    await waitFor(() => expect(exportAiConversation).toHaveBeenCalledExactlyOnceWith(item, "notebook", "aiChat.export.dialogTitle"));
    await waitFor(() => expect(screen.queryByText("aiChat.export.unencrypted")).not.toBeInTheDocument());
  });
  it("keeps controls open after cancellation and shows only a translated error on failure", async () => {
    vi.mocked(exportAiConversation).mockResolvedValueOnce(false).mockRejectedValueOnce(new Error("private content"));
    render(<ConversationExport item={item} disabled={false} />);
    fireEvent.click(screen.getByRole("button", { name: "aiChat.export.label" }));
    fireEvent.click(screen.getByRole("button", { name: "aiChat.export.markdown" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "aiChat.export.markdown" })).toBeEnabled());
    fireEvent.click(screen.getByRole("button", { name: "aiChat.export.markdown" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("aiChat.export.error");
    expect(screen.queryByText("private content")).not.toBeInTheDocument();
  });
});
