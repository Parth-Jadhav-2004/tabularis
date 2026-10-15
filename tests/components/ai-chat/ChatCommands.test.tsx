import { useState } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ChatComposer } from "../../../src/components/ai-chat/ChatComposer";
import { ChatMessage } from "../../../src/components/ai-chat/ChatMessage";
import { createAiMessage } from "../../../src/utils/aiChat";

function setup(initial = "/", blocked = false) {
  const onSend = vi.fn();
  const onEscape = vi.fn();
  function Harness() {
    const [draft, onDraft] = useState(initial);
    return <ChatComposer draft={draft} onDraft={onDraft} onSend={onSend} onStop={() => {}} onEscape={onEscape}
      disabled={blocked} streaming={false}>{null}</ChatComposer>;
  }
  render(<Harness />);
  const input = screen.getByRole("combobox") as HTMLTextAreaElement;
  input.setSelectionRange(input.value.length, input.value.length);
  fireEvent.focus(input);
  return { input, onSend, onEscape };
}

describe("AI command picker", () => {
  it("navigates and picks a command without submitting, then sends arguments", () => {
    const { input, onSend } = setup();
    expect(screen.getAllByRole("option")).toHaveLength(5);
    fireEvent.keyDown(input, { key: "ArrowDown" });
    const selected = screen.getByRole("option", { name: "/explain · aiChat.commands.explain" });
    expect(input).toHaveAttribute("aria-activedescendant", selected.id);
    fireEvent.keyDown(input, { key: "Enter" });
    expect(input).toHaveValue("/explain ");
    expect(onSend).not.toHaveBeenCalled();
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onSend).toHaveBeenCalledTimes(1);
  });
  it("filters, accepts Tab and pointer picking, and wraps to the last command", () => {
    const { input } = setup("/op");
    expect(screen.getAllByRole("option")).toHaveLength(1);
    fireEvent.keyDown(input, { key: "Tab" });
    expect(input).toHaveValue("/optimize ");
    fireEvent.change(input, { target: { value: "/f" } });
    fireEvent.click(screen.getByRole("option", { name: "/fix · aiChat.commands.fix" }));
    expect(input).toHaveValue("/fix ");
    fireEvent.change(input, { target: { value: "/" } });
    fireEvent.keyDown(input, { key: "ArrowUp" });
    expect(screen.getByRole("option", { name: "/help · aiChat.commands.help" })).toHaveAttribute("aria-selected", "true");
  });
  it("dismisses Escape before returning focus and ignores composition", () => {
    const { input, onSend, onEscape } = setup();
    fireEvent.keyDown(input, { key: "Enter", isComposing: true });
    fireEvent.keyDown(input, { key: "Enter", shiftKey: true });
    expect(input).toHaveValue("/");
    expect(onSend).not.toHaveBeenCalled();
    fireEvent.keyDown(input, { key: "Escape" });
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    expect(onEscape).not.toHaveBeenCalled();
    fireEvent.keyDown(input, { key: "Escape" });
    expect(onEscape).toHaveBeenCalledTimes(1);
  });
  it("keeps unknown tokens editable without submitting from the empty list", () => {
    const { input, onSend } = setup("/unknown");
    expect(screen.queryByRole("option")).not.toBeInTheDocument();
    expect(screen.getByText("aiChat.commands.unknown")).toBeInTheDocument();
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onSend).not.toHaveBeenCalled();
    expect(input).toHaveValue("/unknown");
  });
  it.each(["SQL /explain", "/explain query"])("does not offer commands inside text or arguments: %s", (draft) => {
    setup(draft);
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
  });
  it("does not offer commands while interaction is blocked", () => {
    setup("/", true);
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
  });
  it("renders persisted user command chips, arguments and plain assistant text", () => {
    render(<ChatMessage message={createAiMessage("user", [{ type: "text", text: "/fix missing column" }])} />);
    expect(screen.getByLabelText("aiChat.commands.fix")).toHaveTextContent("/fix");
    expect(screen.getByText("missing column")).toBeInTheDocument();
    render(<ChatMessage message={createAiMessage("assistant", [{ type: "text", text: "/fix is available" }])} />);
    expect(screen.getByText("/fix is available")).toBeInTheDocument();
  });
});
