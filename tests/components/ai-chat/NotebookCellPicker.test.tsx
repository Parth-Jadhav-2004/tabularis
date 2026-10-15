import { useState } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ChatComposer } from "../../../src/components/ai-chat/ChatComposer";
import { aiNotebookCellChoices } from "../../../src/utils/aiNotebookMentions";

function setup(options: { disabled?: boolean; streaming?: boolean; accept?: boolean; cells?: boolean } = {}) {
  const onCell = vi.fn(() => options.accept ?? true);
  const onSend = vi.fn();
  const onEscape = vi.fn();
  function Fixture() {
    const [draft, setDraft] = useState("Compare #");
    return <ChatComposer draft={draft} onDraft={setDraft} onSend={onSend} onStop={() => {}} onEscape={onEscape}
      disabled={options.disabled ?? false} streaming={options.streaming ?? false} onCell={onCell}
      notebookCells={options.cells === false ? [] : aiNotebookCellChoices([
        { id: "a", type: "sql", name: "Revenue", content: "SELECT region" },
        { id: "b", type: "sql", name: "Revenue", content: "SELECT total" },
        { id: "notes", type: "markdown", name: "Notes", content: "Documentation" },
      ])}>{null}</ChatComposer>;
  }
  render(<Fixture />);
  const input = screen.getByRole("combobox") as HTMLTextAreaElement;
  input.setSelectionRange(input.value.length, input.value.length);
  fireEvent.focus(input);
  return { input, onCell, onSend, onEscape };
}

describe("notebook cell picker", () => {
  it("navigates by keyboard and turns a pick into context without sending", () => {
    const f = setup();
    expect(screen.getAllByRole("option")).toHaveLength(3);
    fireEvent.keyDown(f.input, { key: "ArrowDown" });
    expect(f.input).toHaveAttribute("aria-activedescendant", screen.getAllByRole("option")[1].id);
    fireEvent.keyDown(f.input, { key: "Enter" });
    expect(f.onCell).toHaveBeenCalledWith("b", true);
    expect(f.onSend).not.toHaveBeenCalled();
    expect(f.input).toHaveValue("Compare ");
  });

  it("filters by index, accepts Tab, and lets the pointer select Markdown cells", () => {
    const f = setup();
    fireEvent.change(f.input, { target: { value: "#2" } });
    expect(screen.getAllByRole("option")).toHaveLength(1);
    fireEvent.keyDown(f.input, { key: "Tab" });
    expect(f.onCell).toHaveBeenCalledWith("b", true);
    fireEvent.change(f.input, { target: { value: "#Notes" } });
    fireEvent.click(screen.getByRole("option", { name: /Notes/ }));
    expect(f.onCell).toHaveBeenLastCalledWith("notes", true);
  });

  it("handles Escape, composition and modifier Enter without selecting or sending", () => {
    const f = setup();
    fireEvent.keyDown(f.input, { key: "Enter", isComposing: true });
    fireEvent.keyDown(f.input, { key: "Enter", shiftKey: true });
    expect(f.onCell).not.toHaveBeenCalled();
    expect(f.onSend).not.toHaveBeenCalled();
    fireEvent.keyDown(f.input, { key: "Escape" });
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    expect(f.onEscape).not.toHaveBeenCalled();
    fireEvent.keyDown(f.input, { key: "Escape" });
    expect(f.onEscape).toHaveBeenCalledOnce();
  });

  it("announces the selection limit and preserves a rejected pick", () => {
    const f = setup({ accept: false });
    fireEvent.keyDown(f.input, { key: "Enter" });
    expect(f.input).toHaveValue("Compare #");
    expect(screen.getByText("aiChat.notebook.cellLimit")).toBeInTheDocument();
    expect(f.onSend).not.toHaveBeenCalled();
  });

  it("does not submit an unmatched cell marker", () => {
    const f = setup({ cells: false });
    expect(screen.getByText("aiChat.notebook.noCells")).toBeInTheDocument();
    fireEvent.keyDown(f.input, { key: "Enter" });
    expect(f.onSend).not.toHaveBeenCalled();
  });

  it.each([{ disabled: true }, { streaming: true }])("blocks picking while interaction is unavailable: %j", (options) => {
    const f = setup(options);
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    fireEvent.keyDown(f.input, { key: "ArrowDown" });
    expect(f.onCell).not.toHaveBeenCalled();
  });
});
