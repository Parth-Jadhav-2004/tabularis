import { useState } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { AiInstructionsSection } from "../../../src/components/modals/NewConnectionModal/AiInstructionsSection";

function Form() {
  const [value, setValue] = useState("");
  return <AiInstructionsSection value={value} onChange={setValue} />;
}

describe("connection instructions form", () => {
  it("labels and describes the field and caps pasted Unicode without splitting emoji", () => {
    render(<Form />);
    const field = screen.getByRole("textbox", { name: "newConnection.aiInstructions" });
    expect(field).toHaveAccessibleDescription(/newConnection.aiInstructionsHint/);
    fireEvent.change(field, { target: { value: "😀".repeat(2000) + "extra" } });
    expect(field).toHaveValue("😀".repeat(2000));
    expect(screen.getByText("newConnection.aiInstructionsCount")).toBeInTheDocument();
    fireEvent.change(field, { target: { value: "" } });
    expect(field).toHaveValue("");
  });
  it("disables editing while saving", () => {
    render(<AiInstructionsSection value="Prefer explicit JOINs" onChange={() => {}} disabled />);
    expect(screen.getByRole("textbox")).toBeDisabled();
  });
});
