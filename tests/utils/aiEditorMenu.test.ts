import { describe, expect, it, vi } from "vitest";
import type { editor } from "monaco-editor";
import { registerAiEditorMenu } from "../../src/utils/aiEditorMenu";

function setup(selected: string | null = "SELECT selected") {
  const registrations: editor.IActionDescriptor[] = [];
  const disposeAction = vi.fn();
  const codeEditor = { getValue: () => "SELECT whole query", getPosition: () => null,
    getModel: () => ({ getValueInRange: () => selected }),
    getSelection: () => selected === null ? null : { isEmpty: () => selected === "" },
    addAction: (action: editor.IActionDescriptor) => { registrations.push(action); return { dispose: disposeAction }; },
  } as unknown as editor.IStandaloneCodeEditor;
  const run = vi.fn();
  const dispose = registerAiEditorMenu(codeEditor, { disabled: false, run }, { explain: "Explain", optimize: "Optimize", ask: "Ask" });
  return { codeEditor, registrations, dispose, disposeAction, run };
}

describe("Monaco AI selection actions", () => {
  it.each(["explain", "optimize", "ask"] as const)("registers %s with a selection precondition and captures live SQL", (action) => {
    const fixture = setup();
    const registered = fixture.registrations.find((item) => item.id === `tabularis.ai.${action}`)!;
    expect(registered.precondition).toBe("editorHasSelection");
    registered.run(fixture.codeEditor);
    expect(fixture.run).toHaveBeenCalledWith(action, expect.objectContaining({ query: "SELECT whole query", selection: "SELECT selected" }));
  });

  it.each([null, "", "   "])("does not submit an empty selection: %s", (selected) => {
    const fixture = setup(selected);
    fixture.registrations.forEach((action) => action.run(fixture.codeEditor));
    expect(fixture.run).not.toHaveBeenCalled();
  });

  it("disposes all menu entries and rejects callbacks retained after cleanup", () => {
    const fixture = setup();
    fixture.dispose();
    expect(fixture.disposeAction).toHaveBeenCalledTimes(3);
    fixture.registrations[0].run(fixture.codeEditor);
    expect(fixture.run).not.toHaveBeenCalled();
  });
});
