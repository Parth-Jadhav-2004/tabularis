import { fireEvent, render, screen } from "@testing-library/react";
import type { TFunction } from "i18next";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { QueryErrorDisplay } from "../../../src/components/ai-chat/QueryErrorDisplay";

const state = vi.hoisted(() => ({ available: true, disabled: false, open: vi.fn(), target: {
  id: "tab", connectionId: "connection", database: "db", schema: "public", readContext: () => ({ query: "current draft" }),
  readErrorContext: vi.fn<() => { query: string | null }>(() => ({ query: "SELECT last_failure" })),
} }));
vi.mock("../../../src/hooks/useAiAvailability", () => ({ useAiAvailability: () => ({ available: state.available }) }));
vi.mock("../../../src/hooks/useAiEditor", () => ({ useAiEditor: () => state.target }));
vi.mock("../../../src/hooks/useAiChatEntry", () => ({ useOpenAiChat: () => ({ open: state.open, disabled: state.disabled }) }));

const t = ((key: string) => key) as TFunction;

describe("query error Fix with AI", () => {
  beforeEach(() => { state.available = true; state.disabled = false; state.open.mockClear();
    state.target.readErrorContext.mockReturnValue({ query: "SELECT last_failure" }); });

  it("does not substitute the current draft when the failed execution context is unavailable", () => {
    state.target.readErrorContext.mockReturnValue({ query: null });
    render(<QueryErrorDisplay error="old failure" connectionId="connection" t={t} />);
    expect(screen.queryByRole("button", { name: "aiChat.entryPoints.fix" })).not.toBeInTheDocument();
    expect(state.open).not.toHaveBeenCalled();
  });

  it("uses the selected result entry's failed SQL and complete error instead of the current editor", () => {
    render(<QueryErrorDisplay error={"failed\n\nserver detail"} query="SELECT batch_failure" connectionId="connection" t={t} />);
    fireEvent.click(screen.getByRole("button", { name: "aiChat.entryPoints.fix" }));
    expect(state.open).toHaveBeenCalledWith("fix", state.target, expect.objectContaining({
      snapshot: { query: "SELECT batch_failure", selection: null, cursor_statement: null, last_error: "failed\n\nserver detail" },
      include: expect.objectContaining({ error: true }),
    }));
  });

  it.each(["unavailable", "foreign", "empty"])("does not offer Fix for %s context", (kind) => {
    state.available = kind !== "unavailable";
    render(<QueryErrorDisplay error="failed" query={kind === "empty" ? "  " : "SELECT failed"}
      connectionId={kind === "foreign" ? "other connection" : "connection"} t={t} />);
    expect(screen.queryByRole("button", { name: "aiChat.entryPoints.fix" })).not.toBeInTheDocument();
  });

  it("disables Fix while the shared session is busy", () => {
    state.disabled = true;
    render(<QueryErrorDisplay error="failed" query="SELECT failed" connectionId="connection" t={t} />);
    const button = screen.getByRole("button", { name: "aiChat.entryPoints.fix" });
    expect(button).toBeDisabled();
    fireEvent.click(button);
    expect(state.open).not.toHaveBeenCalled();
  });
});
