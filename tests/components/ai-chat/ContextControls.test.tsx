import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ContextChips } from "../../../src/components/ai-chat/ContextChips";
import { ContextBlock } from "../../../src/components/ai-chat/blocks/ContextBlock";
import { ContextReferenceChips } from "../../../src/components/ai-chat/ContextReferenceChips";
import { defaultAiContextSelection } from "../../../src/utils/aiContext";

vi.mock("lucide-react", () => vi.importActual("lucide-react"));

describe("AI context controls", () => {
  it("shows configured instructions, their actual size, and removal/restoration state", () => {
    const include = defaultAiContextSelection();
    const onChange = vi.fn();
    const { rerender } = render(<ContextChips include={include} onChange={onChange} disabled={false} hasError={false} hasInstructions
      preview={{ items: [{ kind: "instructions", id: "instructions", label: "Connection instructions", chars: 123, truncated: true }], total_chars: 1000, truncated: true }} />);
    const chip = screen.getByRole("button", { name: /aiChat.context.kinds.instructions/ });
    expect(chip).toHaveAttribute("aria-pressed", "true");
    expect(chip).toHaveTextContent("aiChat.context.characters");
    fireEvent.click(chip);
    expect(onChange).toHaveBeenCalledWith({ ...include, instructions: false });
    rerender(<ContextChips include={{ ...include, instructions: false }} onChange={onChange} disabled={false} hasError={false} hasInstructions />);
    expect(chip).toHaveAttribute("aria-pressed", "false");
    fireEvent.click(chip);
    expect(onChange).toHaveBeenLastCalledWith(include);
    rerender(<ContextChips include={include} onChange={onChange} disabled hasError={false} hasInstructions />);
    expect(chip).toBeDisabled();
    rerender(<ContextChips include={include} onChange={onChange} disabled={false} hasError={false} />);
    expect(screen.queryByRole("button", { name: /aiChat.context.kinds.instructions/ })).not.toBeInTheDocument();
  });
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
  it("shows table counts, sizes and truncation and lets inferred details be excluded", () => {
    const onTable = vi.fn(() => true);
    render(<ContextChips include={defaultAiContextSelection()} onChange={() => {}} disabled={false} hasError={false}
      tables={{ tables: [], excluded_tables: [] }} onTable={onTable} onRefresh={() => {}} preview={{
        items: [{ kind: "schema", id: "schema", label: "20000 tables", chars: 12000, table_count: 20000, truncated: true },
          { kind: "table", id: "users", label: "users", chars: 123, truncated: true }],
        truncated: true, total_chars: 14000,
      }} />);
    expect(screen.getByText(/aiChat.context.tableCount/)).toBeInTheDocument();
    expect(screen.getByText(/aiChat.context.previewSize/)).toBeInTheDocument();
    const table = screen.getByRole("button", { name: "aiChat.context.tableDetail" });
    expect(table).toHaveAttribute("aria-pressed", "true");
    fireEvent.click(table);
    expect(onTable).toHaveBeenCalledWith("users", false);
    expect(screen.getAllByText(/aiChat.context.truncated/)).toHaveLength(3);
  });
});

describe("AI view and saved-query controls", () => {
  it("removes view details and saved-query text independently using typed IDs", () => {
    const onMention = vi.fn(() => true);
    render(<ContextReferenceChips choices={{ tables: [], excluded_tables: [], views: ["users"], saved_queries: [{ id: "query-id", label: "users" }] }}
      preview={{ items: [{ kind: "view", id: "users", label: "users", chars: 123 }, { kind: "saved_query", id: "query-id", label: "Renamed query", chars: 456, truncated: true }], total_chars: 2000, truncated: true }} disabled={false} onMention={onMention} />);
    fireEvent.click(screen.getByRole("button", { name: "aiChat.context.viewDetail" }));
    expect(onMention).toHaveBeenCalledWith({ kind: "view", id: "users", label: "users" }, false);
    fireEvent.click(screen.getByRole("button", { name: "aiChat.context.savedQuery" }));
    expect(onMention).toHaveBeenLastCalledWith({ kind: "saved_query", id: "query-id", label: "users" }, false);
    expect(screen.getByText("Renamed query")).toBeInTheDocument();
    expect(screen.getByText("aiChat.context.truncated")).toBeInTheDocument();
  });
  it("shows unavailable saved queries without inserting label HTML and disables editing while busy", () => {
    render(<ContextReferenceChips choices={{ tables: [], excluded_tables: [], saved_queries: [{ id: "id", label: "<img src=x onerror=alert(1)>" }] }}
      preview={{ items: [], total_chars: 1000, truncated: false }} disabled onMention={() => true} />);
    expect(document.querySelector("img")).toBeNull();
    expect(screen.getByRole("button", { name: "aiChat.context.savedQuery" })).toBeDisabled();
    expect(screen.getByText("aiChat.context.notInPreview")).toBeInTheDocument();
  });
  it("retains both view names and saved-query names in historical summaries", () => {
    render(<ContextBlock items={[{ kind: "view", id: "v_users", label: "v_users", chars: 100 }, { kind: "saved_query", id: "query-id", label: "Revenue", chars: 200 }, { kind: "schema", id: "schema", label: "Schema", table_count: 5, view_count: 2 }]} />);
    expect(screen.getByText(/v_users/)).toBeInTheDocument();
    expect(screen.getByText(/Revenue/)).toBeInTheDocument();
    expect(screen.getByText(/aiChat.context.viewCount/)).toBeInTheDocument();
  });
});
