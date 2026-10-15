import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { TextBlock } from "../../../src/components/ai-chat/blocks/TextBlock";
import { ContextBlock } from "../../../src/components/ai-chat/blocks/ContextBlock";
import { ChatTranscript } from "../../../src/components/ai-chat/ChatTranscript";
import { createAiChatState, createAiConversation, createAiMessage } from "../../../src/utils/aiChat";
import type { AiEditorTarget } from "../../../src/types/aiEditor";
vi.mock("lucide-react", () => vi.importActual("lucide-react"));

describe("notebook references in answers", () => {
  it("links prose and formatted references while preserving code, external links and unknown cells", () => {
    const reveal = vi.fn(() => true);
    render(<TextBlock text={"See #2 and **#1**, but leave #999 and `#2` alone.\n\n```sql\nSELECT '#2';\n```\n\n[#2](https://example.test)"}
      notebookReferences={{ cells: new Map([[1, "a"], [2, "b"]]), reveal }} />);
    const links = screen.getAllByRole("button", { name: "aiChat.notebook.goToCell" });
    expect(links).toHaveLength(2);
    fireEvent.click(links[0]);
    expect(reveal).toHaveBeenCalledWith("b");
    expect(document.querySelector("pre code")).toHaveTextContent("SELECT '#2';");
    expect(screen.getByRole("link", { name: "#2" })).toHaveAttribute("href", "https://example.test");
    expect(screen.getByText(/#999/)).toBeInTheDocument();
  });

  it("resolves reopened answers to stable cell IDs after reordering and disables deleted or foreign targets", () => {
    const conversation = createAiConversation({ connection_id: "conn", provider: "openai", model: "model", target: { type: "notebook", notebook_id: "book" } });
    conversation.messages = [createAiMessage("user", [{ type: "text", text: "question" }, { type: "context", items: [
      { kind: "notebook", id: "book", label: "Book" }, { kind: "notebook_cell_ref", id: "a", label: "#1" }, { kind: "notebook_cell_ref", id: "b", label: "#2" },
    ] }]), createAiMessage("assistant", [{ type: "text", text: "Review cell #2." }])];
    const revealCell = vi.fn(() => true);
    const target: AiEditorTarget = { connectionId: "conn", database: null, schema: null,
      notebook: { id: "book", revealCell, readState: () => ({ id: "book", title: "Book", focusedCellId: "a", stopOnError: false, params: [], runAllResult: null,
        cells: [{ id: "b", type: "markdown", content: "Moved" }, { id: "a", type: "sql", content: "SELECT 1" }] }) },
      actions: { insert: vi.fn(), replace: vi.fn(), run: vi.fn(), focus: vi.fn() }, sqlActionsAvailable: false };
    const chat = createAiChatState(conversation);
    const f = render(<ChatTranscript chat={chat} target={target} canAct={false} canNavigateNotebook />);
    fireEvent.click(screen.getByRole("button", { name: "aiChat.notebook.goToCell" }));
    expect(revealCell).toHaveBeenCalledWith("b");
    expect(target.actions.run).not.toHaveBeenCalled();
    f.rerender(<ChatTranscript chat={chat} target={target} canAct={false} canNavigateNotebook={false} />);
    expect(screen.queryByRole("button", { name: "aiChat.notebook.goToCell" })).not.toBeInTheDocument();
    const missing = { ...target, notebook: { ...target.notebook!, readState: () => ({ ...target.notebook!.readState!(), cells: [{ id: "a", type: "sql" as const, content: "SELECT 1" }] }) } };
    f.rerender(<ChatTranscript chat={chat} target={missing} canAct={false} canNavigateNotebook />);
    expect(screen.queryByRole("button", { name: "aiChat.notebook.goToCell" })).not.toBeInTheDocument();
    expect(screen.getByText("Review cell #2.")).toBeInTheDocument();
  });

  it("hides identity metadata from the visible context summary", () => {
    render(<ContextBlock items={[{ kind: "notebook", id: "book", label: "Revenue", chars: 100 },
      { kind: "notebook_cell_ref", id: "private-id", label: "#42", chars: 0 }]} />);
    expect(screen.getAllByRole("listitem", { hidden: true })).toHaveLength(1);
    expect(screen.queryByText("#42")).not.toBeInTheDocument();
  });
});
