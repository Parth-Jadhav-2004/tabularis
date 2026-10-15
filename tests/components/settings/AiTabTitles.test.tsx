import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { AiTab } from "../../../src/components/settings/AiTab";

vi.mock("lucide-react", () => vi.importActual("lucide-react"));
vi.mock("../../../src/hooks/useSettings", () => ({ useSettings: () => ({ settings: { aiEnabled: true }, updateSetting: vi.fn() }) }));
vi.mock("../../../src/hooks/useAiAvailability", () => ({ useAiAvailability: () => ({ refresh: vi.fn() }) }));
vi.mock("../../../src/hooks/useAlert", () => ({ useAlert: () => ({ showAlert: vi.fn() }) }));
beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(invoke).mockImplementation(async (command) => {
    if (command === "get_ai_models") return {};
    if (command === "check_ai_key_status") return { configured: false, fromEnv: false };
    if (command === "get_chattitle_prompt") return "Custom title instructions";
    if (command === "reset_chattitle_prompt") return "Default title instructions";
    return "";
  });
});

describe("conversation title prompt settings", () => {
  it("loads, edits, saves and resets the dedicated prompt", async () => {
    render(<AiTab />);
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("get_chattitle_prompt"));
    fireEvent.click(screen.getByRole("button", { name: /settings.ai.chattitlePrompt/ }));
    const editor = screen.getByRole("textbox", { name: "settings.ai.chattitlePrompt" });
    await waitFor(() => expect(editor).toHaveValue("Custom title instructions"));
    fireEvent.change(editor, { target: { value: "New title instructions" } });
    fireEvent.click(screen.getByRole("button", { name: "settings.ai.savePrompt" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("save_chattitle_prompt", { prompt: "New title instructions" }));
    fireEvent.click(screen.getByRole("button", { name: "settings.ai.resetDefault" }));
    await waitFor(() => expect(editor).toHaveValue("Default title instructions"));
  });
});
