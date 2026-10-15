import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { AiStorageSection } from "../../../src/components/settings/AiStorageSection";

const mocks = vi.hoisted(() => ({ invoke: vi.fn(), showAlert: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("lucide-react", async () => await vi.importActual("lucide-react"));
vi.mock("react-i18next", () => ({ useTranslation: () => ({ t: (key: string) => key }) }));
vi.mock("../../../src/hooks/useAlert", () => ({ useAlert: () => ({ showAlert: mocks.showAlert }) }));

beforeEach(() => { vi.clearAllMocks(); mocks.invoke.mockResolvedValue(undefined); });

describe("AI storage confirmation", () => {
  it("does not delete anything before confirmation or after cancelling", () => {
    render(<AiStorageSection />);
    fireEvent.click(screen.getByRole("button", { name: "settings.ai.storage.delete" }));
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(mocks.invoke).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "common.cancel" }));
    expect(mocks.invoke).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("deletes conversations only after the explicit confirm action", async () => {
    render(<AiStorageSection />);
    fireEvent.click(screen.getByRole("button", { name: "settings.ai.storage.delete" }));
    fireEvent.click(screen.getByRole("button", { name: "common.confirm" }));
    await waitFor(() => expect(mocks.invoke).toHaveBeenCalledExactlyOnceWith("ai_chat_delete_all"));
    expect(mocks.showAlert).toHaveBeenCalledWith("settings.ai.storage.deleteSuccess", expect.objectContaining({ kind: "info" }));
  });

  it("requires a separate confirmation for the encryption key reset", async () => {
    render(<AiStorageSection />);
    fireEvent.click(screen.getByRole("button", { name: "settings.ai.storage.reset" }));
    expect(screen.getByText("settings.ai.storage.resetConfirm")).toBeInTheDocument();
    expect(mocks.invoke).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "common.confirm" }));
    await waitFor(() => expect(mocks.invoke).toHaveBeenCalledExactlyOnceWith("ai_reset_encryption_key", { confirmed: true }));
  });

  it("keeps the confirmation open on failure and shows a translated error", async () => {
    mocks.invoke.mockRejectedValue({ kind: "key_unavailable" });
    render(<AiStorageSection />);
    fireEvent.click(screen.getByRole("button", { name: "settings.ai.storage.reset" }));
    fireEvent.click(screen.getByRole("button", { name: "common.confirm" }));
    await waitFor(() => expect(mocks.showAlert).toHaveBeenCalledWith("settings.ai.storage.error", expect.objectContaining({ kind: "error" })));
    expect(screen.getByRole("dialog")).toBeInTheDocument();
  });
});
