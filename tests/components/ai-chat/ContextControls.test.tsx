import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ContextChips } from "../../../src/components/ai-chat/ContextChips";
import { ContextBlock } from "../../../src/components/ai-chat/blocks/ContextBlock";
import { defaultAiContextSelection } from "../../../src/utils/aiContext";

vi.mock("lucide-react", () => vi.importActual("lucide-react"));

describe("AI context controls", () => {
  it("exposes selected state, allows individual removal and keeps errors opt-in", () => {
    const include = defaultAiContextSelection();
    const onChange = vi.fn();
    const { rerender } = render(<ContextChips include={include} onChange={onChange} disabled={false} hasError />);
    const query = screen.getByRole("button", { name: "aiChat.context.kinds.query" });
    expect(query).toHaveAttribute("aria-pressed", "true");
    fireEvent.click(query);
    expect(onChange).toHaveBeenCalledWith({ ...include, query: false });
    expect(screen.getByRole("button", { name: "aiChat.context.kinds.error" })).toHaveAttribute("aria-pressed", "false");
    rerender(<ContextChips include={include} onChange={onChange} disabled hasError={false} />);
    expect(query).toBeDisabled();
    expect(screen.queryByRole("button", { name: "aiChat.context.kinds.error" })).not.toBeInTheDocument();
  });
  it("renders persisted summaries collapsed, escapes labels and accepts old summaries without sizes", () => {
    render(<ContextBlock items={[
      { kind: "table", id: "users", label: "<img src=x onerror=alert(1)>", chars: 123, truncated: true },
      { kind: "query", id: "query", label: "Query" },
    ]} />);
    expect(screen.getByText("aiChat.context.sent").closest("details")).not.toHaveAttribute("open");
    expect(document.querySelector("img")).toBeNull();
    expect(screen.getByText("aiChat.context.truncated")).toBeInTheDocument();
    expect(screen.getByText("aiChat.context.kinds.query")).toBeInTheDocument();
  });
});
