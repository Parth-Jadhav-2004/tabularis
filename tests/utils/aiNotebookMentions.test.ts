import { describe, expect, it } from "vitest";
import { activeAiCellMention, aiNotebookCellChoices, changeAiNotebookCell, filterAiNotebookCells, selectedAiNotebookCells } from "../../src/utils/aiNotebookMentions";
import { aiNotebookMessageReferences } from "../../src/utils/aiNotebookReferences";
import { createAiMessage } from "../../src/utils/aiChat";

describe("notebook cell mentions", () => {
  it("recognizes a cell marker at the caret without confusing embedded hashes or selections", () => {
    expect(activeAiCellMention("Compare #12", 11)).toEqual({ start: 8, end: 11, search: "12" });
    expect(activeAiCellMention("#", 1)?.search).toBe("");
    for (const [text, start, end] of [["C#1", 3, 3], ["##1", 3, 3], ["#1", 1, 2], ["#1", -1, -1], ["#1", 3, 3]] as const) {
      expect(activeAiCellMention(text, start, end)).toBeNull();
    }
  });

  it("searches names, indexes and first lines, retains duplicate names and caps suggestions", () => {
    const cells = aiNotebookCellChoices([
      { id: "a", type: "sql", name: "Revenue", content: "SELECT region\nFROM orders" },
      { id: "b", type: "sql", name: "Revenue", content: "SELECT total" },
      { id: "notes", type: "markdown", content: "## 説明" },
    ]);
    expect(filterAiNotebookCells(cells, "revenue").map((cell) => cell.id)).toEqual(["a", "b"]);
    expect(filterAiNotebookCells(cells, "2").map((cell) => cell.id)).toEqual(["b"]);
    expect(filterAiNotebookCells(cells, "説明").map((cell) => cell.id)).toEqual(["notes"]);
    expect(cells[0].preview).toBe("SELECT region");
    const many = aiNotebookCellChoices(Array.from({ length: 100 }, (_, index) => ({ id: `${index}`, type: "sql", content: "😀".repeat(1000) })));
    expect(filterAiNotebookCells(many, "")).toHaveLength(30);
    expect(Array.from(many[0].preview)).toHaveLength(80);
  });

  it("keeps choices scoped to one notebook, independently caps them and preserves table selections", () => {
    const start = { tables: ["orders"], excluded_tables: ["users"] };
    const chosen = changeAiNotebookCell(start, "one", "a", true);
    expect(selectedAiNotebookCells(chosen, "one")).toEqual(["a"]);
    expect(selectedAiNotebookCells(chosen, "two")).toEqual([]);
    expect(start).not.toHaveProperty("notebook");
    expect(chosen.tables).toEqual(["orders"]);
    expect(changeAiNotebookCell(chosen, "two", "b", true).notebook).toEqual({ notebook_id: "two", cell_ids: ["b"] });
    expect(changeAiNotebookCell(chosen, "one", "a", false).notebook?.cell_ids).toEqual([]);
    const full = { ...start, notebook: { notebook_id: "one", cell_ids: Array.from({ length: 20 }, (_, index) => `${index}`) } };
    expect(changeAiNotebookCell(full, "one", "overflow", true)).toBe(full);
    expect(changeAiNotebookCell(full, "one", "0", true).notebook?.cell_ids).toHaveLength(20);
  });
});

describe("notebook answer reference identities", () => {
  it("uses only the preceding turn's matching notebook metadata and available stable IDs", () => {
    const user = createAiMessage("user", [{ type: "context", items: [
      { kind: "notebook", id: "book", label: "Notebook" },
      { kind: "notebook_cell_ref", id: "a", label: "#1" },
      { kind: "notebook_cell_ref", id: "b", label: "#2" },
      { kind: "notebook_cell_ref", id: "deleted", label: "#3" },
    ] }]);
    const assistant = createAiMessage("assistant");
    const available = new Set(["b", "a"]);
    expect([...aiNotebookMessageReferences([user, assistant], 1, "book", available)]).toEqual([[1, "a"], [2, "b"]]);
    expect(aiNotebookMessageReferences([user, assistant], 1, "other", available).size).toBe(0);
    const next = createAiMessage("user", [{ type: "text", text: "No notebook context" }]);
    expect(aiNotebookMessageReferences([user, assistant, next, assistant], 3, "book", available).size).toBe(0);
    expect(aiNotebookMessageReferences([assistant], 0, "book", available).size).toBe(0);
  });

  it("supports older focused-cell summaries without redirecting grouped or invalid indexes", () => {
    const user = createAiMessage("user", [{ type: "context", items: [
      { kind: "notebook", id: "book", label: "Notebook" },
      { kind: "notebook_cell", id: "a", label: "#2 · Revenue" },
      { kind: "notebook_cell", id: "neighbours", label: "#1, #3" },
      { kind: "notebook_cell_ref", id: "b", label: "#0" },
    ] }]);
    expect([...aiNotebookMessageReferences([user, createAiMessage("assistant")], 1, "book", new Set(["a", "b", "neighbours"]))]).toEqual([[2, "a"]]);
  });
});
