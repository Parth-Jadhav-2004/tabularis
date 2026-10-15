import { useState } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ChatComposer } from "../../../src/components/ai-chat/ChatComposer";

function setup(options: { disabled?: boolean; streaming?: boolean; initial?: string; tables?: string[]; pick?: boolean } = {}) {
  const onTable = vi.fn(() => options.pick ?? true);
  const onSend = vi.fn();
  const onEscape = vi.fn();
  const onDropTable = vi.fn();
  function Harness() {
    const [draft, setDraft] = useState(options.initial ?? "Explain @");
    return <ChatComposer draft={draft} onDraft={setDraft} onTable={onTable} mentionTables={options.tables ?? ["orders", "users", "用户"]}
      disabled={options.disabled ?? false} streaming={options.streaming ?? false} onSend={onSend} onEscape={onEscape}
      onStop={() => {}} onDropTable={onDropTable}>{null}</ChatComposer>;
  }
  render(<Harness />);
  const input = screen.getByRole("combobox") as HTMLTextAreaElement;
  input.setSelectionRange(input.value.length, input.value.length);
  fireEvent.focus(input);
  return { input, onTable, onSend, onEscape, onDropTable };
}

describe("AI composer table mentions", () => {
  it("navigates with arrows and accepts Enter without sending the draft", () => {
    const { input, onTable, onSend } = setup();
    expect(input).toHaveAttribute("aria-expanded", "true");
    fireEvent.keyDown(input, { key: "ArrowUp" });
    const last = screen.getByRole("option", { name: "用户" });
    expect(input).toHaveAttribute("aria-activedescendant", last.id);
    expect(last).toHaveAttribute("aria-selected", "true");
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onTable).toHaveBeenCalledWith("用户", true);
    expect(onSend).not.toHaveBeenCalled();
    expect(input).toHaveValue("Explain ");
    expect(input).toHaveAttribute("aria-expanded", "false");
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onSend).toHaveBeenCalledTimes(1);
  });
  it("supports filtering, Tab and pointer selection", () => {
    const { input, onTable } = setup();
    fireEvent.change(input, { target: { value: "Explain @us" } });
    expect(screen.getAllByRole("option")).toHaveLength(1);
    fireEvent.keyDown(input, { key: "Tab" });
    expect(onTable).toHaveBeenCalledWith("users", true);
    fireEvent.change(input, { target: { value: "@ord" } });
    fireEvent.click(screen.getByRole("option", { name: "orders" }));
    expect(onTable).toHaveBeenLastCalledWith("orders", true);
    expect(input).toHaveValue("");
  });
  it("Escape closes the menu first, then returns focus to the editor", () => {
    const { input, onEscape, onTable } = setup();
    fireEvent.keyDown(input, { key: "Escape" });
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    expect(onEscape).not.toHaveBeenCalled();
    fireEvent.keyDown(input, { key: "Escape" });
    expect(onEscape).toHaveBeenCalledTimes(1);
    expect(onTable).not.toHaveBeenCalled();
  });
  it("does not accept IME composition or modifier Enter as a mention", () => {
    const { input, onTable, onSend } = setup();
    fireEvent.keyDown(input, { key: "Enter", isComposing: true });
    fireEvent.keyDown(input, { key: "Enter", shiftKey: true });
    fireEvent.keyDown(input, { key: "Enter", ctrlKey: true });
    expect(onTable).not.toHaveBeenCalled();
    expect(onSend).not.toHaveBeenCalled();
  });
  it("keeps a rejected pick editable and announces the table limit", () => {
    const { input, onSend } = setup({ pick: false });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(input).toHaveValue("Explain @");
    expect(screen.getByText("aiChat.context.tableLimit")).toBeInTheDocument();
    expect(onSend).not.toHaveBeenCalled();
  });
  it("shows no matches without sending Enter and closes on blur", () => {
    const { input, onSend } = setup({ tables: [] });
    expect(screen.getByText("aiChat.context.noMentionTables")).toBeInTheDocument();
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onSend).not.toHaveBeenCalled();
    fireEvent.blur(input);
    expect(input).toHaveAttribute("aria-expanded", "false");
  });
  it.each([{ disabled: true }, { streaming: true }])("does not offer mentions while interaction is blocked: %j", (options) => {
    const { input, onTable } = setup(options);
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    fireEvent.keyDown(input, { key: "ArrowDown" });
    expect(onTable).not.toHaveBeenCalled();
  });
  it("routes explorer pointer drops through the scoped callback", () => {
    const { input, onDropTable } = setup();
    fireEvent.pointerUp(input);
    expect(onDropTable).toHaveBeenCalledTimes(1);
  });
});
