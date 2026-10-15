import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ChatContextMenu } from "../../../src/components/ai-chat/ChatContextMenu";
import { ModelPicker } from "../../../src/components/ai-chat/ModelPicker";
import { AiAvailabilityContext } from "../../../src/contexts/AiAvailabilityContext";
import { invoke } from "@tauri-apps/api/core";
vi.mock("lucide-react", () => vi.importActual("lucide-react"));

describe("compact sidebar controls", () => {
  it("keeps context details collapsed and restores focus on Escape", () => {
    render(<ChatContextMenu count={2}><button type="button">Included query</button></ChatContextMenu>);
    const trigger = screen.getByRole("button", { name: "aiChat.context.options" });
    expect(trigger).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByRole("button", { name: "Included query" })).not.toBeInTheDocument();
    fireEvent.click(trigger);
    const control = screen.getByRole("button", { name: "Included query" });
    control.focus();
    fireEvent.keyDown(control, { key: "Escape" });
    expect(trigger).toHaveFocus();
    expect(trigger).toHaveAttribute("aria-expanded", "false");
    fireEvent.click(trigger);
    fireEvent.pointerDown(document.body);
    expect(screen.queryByRole("button", { name: "Included query" })).not.toBeInTheDocument();
  });

  it("shows the selected model without exposing search and setup until opened", () => {
    vi.mocked(invoke).mockResolvedValue({ tools: false, reasoning: false, images: false, context_window: null });
    const onChange = vi.fn();
    render(<AiAvailabilityContext.Provider value={{ available: true, loading: false, refresh: vi.fn(), providers: [
      { provider: "openai", label: "OpenAI", source: "env", models_cached: true, usable: true, reason: null },
    ] }}><ModelPicker choice={{ provider: "openai", model: "one" }} choices={[{ provider: "openai", model: "one" }, { provider: "openai", model: "two" }]}
      onChange={onChange} onRefresh={vi.fn()} disabled={false} loading={false} /></AiAvailabilityContext.Provider>);
    const summary = screen.getByLabelText("aiChat.modelOptions");
    expect(summary).toHaveTextContent("one");
    expect(screen.getByLabelText("aiChat.searchModels")).not.toBeVisible();
    fireEvent.click(summary);
    expect(screen.getByLabelText("aiChat.searchModels")).toBeVisible();
    fireEvent.change(screen.getByLabelText("aiChat.model"), { target: { value: JSON.stringify(["openai", "two"]) } });
    expect(onChange).toHaveBeenCalledWith({ provider: "openai", model: "two" });
    expect(summary.closest("details")).not.toHaveAttribute("open");
    expect(summary).toHaveFocus();
  });
});
