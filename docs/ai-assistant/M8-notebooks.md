# M8. Notebook-aware assistant

**Depends on:** M4, M6, M7 (the editor bridge must already target notebook cells) · **Size:** M

## Goal

When the active tab is a notebook, the assistant works on the **notebook as a whole**: it sees its structure, parameters, cells and errors, and its answers turn into cells rather than query replacements.

## Current state

- Types in `src/types/notebook.ts`:
  - `NotebookCell { id, type: "sql" | "markdown", content, name?, schema?, result?, error?, chartConfig?, history?, ... }`
  - `NotebookState { cells, stopOnError?, params? }`
  - `NotebookParam { name, value }`
  - `RunAllResult { errors: [{ cellId, cellIndex, error }] }`
  - `CellChartConfig { type: bar | line | pie, labelColumn, valueColumns }`
- UI: `src/components/notebook/*` (`NotebookView`, `SqlCell`, `MarkdownCell`, `NotebookOutline`, `ParamsPanel`, `RunAllSummary`, `CellChart`, ...).
- Storage: `src-tauri/src/notebooks.rs`, `<config>/notebooks/<connectionId>/`, so notebooks are already per connection. That matches the conversation scope.
- Existing AI in notebooks:
  - `NotebookAiButtons` (Generate/Explain per cell; moved to the sidebar in M7)
  - `CellNameAiButton` / `generate_cell_name` (stays as is)
  - `ai_notebook_export.rs` (AI session to notebook)

## Design

### Conversation target

- Add `target: Option<ChatTarget>` to `Conversation` (M2), with `ChatTarget = Editor | Notebook { notebook_id }`.
- The conversation list (M5) gets a "This notebook" filter, selected by default when a notebook tab is active. Opening a notebook reopens its last conversation.
- A conversation started on a notebook stays bound to it. If the notebook is deleted, the conversation remains and is shown as "notebook no longer available".

### Notebook context layer

When the active tab is a notebook, M6's "Editor state" layer (6) is replaced by a **Notebook** layer:

| Part | Content | Budget |
|---|---|---|
| Header | title, cell count, `stopOnError`, params (`name = value`) | small |
| Outline | one line per cell: `#index [id] type name: first ~150 chars` (max 80 cells; then "... N more") | 8k |
| Focused cell | full content + schema override + last error | 6k |
| Neighbours | full content of the previous and next cells | 4k |
| Mentioned cells | full content of cells referenced with `#` | 8k |
| Errors | latest `RunAllResult.errors` or per-cell `error` | 2k |
| Results (opt-in chip) | for the chosen cells: columns + row count + first 5 rows, cells cut at 200 chars | 6k |

- Results are **off by default** (same principle as the editor: data leaves only when the user asks). The chip "Include results" turns them on per message, or `#cell` with results.
- Parameters: tell the model how params are referenced in SQL (use the same syntax the notebook uses; check `ParamsPanel.tsx` / the substitution code) so generated cells use them instead of literals.
- Tables referenced by the focused and mentioned cells feed M6's "Detailed tables" layer, the same way the current query does in the editor.
- Markdown cells count as documentation: they are useful context ("this notebook analyses churn by region").

### Cell references

- `#` in the composer opens a picker of cells (by name, or `#index` + first line). The pick becomes a chip: "Cell 4 · monthly_revenue".
- Clicking a `#<index>` reference in an assistant answer scrolls to, focuses and highlights that cell. References resolve through the IDs captured for that turn, so reordering cells or resuming a conversation preserves their meaning. Deleted cells and references without a known ID remain plain text; code and existing Markdown links are excluded.

### Actions on answers

SQL blocks in notebook mode get notebook-specific actions instead of the editor ones:

- **Add as cell below** (focused cell) / **Add at end**: the main action
- **Replace cell** (focused or referenced cell), undoable
- **Run** after insert (uses the normal cell run path, so safety checks apply)

Markdown text gets **Add as markdown cell**. A chart block (M10) gets **Add as cell with chart**, mapping the chart spec to `CellChartConfig` when the data came from a query (keep the M10 chart spec compatible with `CellChartConfig`: `type`, label column, value columns).

### Multi-cell proposals

For requests like "build an analysis of orders by month and region", the model can propose several cells at once with a fenced ` ```tabularis-notebook ` JSON block:

```json
{
  "ops": [
    { "op": "insert", "after": "<cellId>|null", "cell": { "type": "markdown", "content": "## Orders by month" } },
    { "op": "insert", "after": "$prev", "cell": { "type": "sql", "name": "orders_by_month", "content": "SELECT ..." } },
    { "op": "replace", "cell_id": "<cellId>", "content": "SELECT ..." }
  ]
}
```

- Rendered as a review card: the list of operations, with a preview of each new or changed cell (diff for replace).
- **Apply all**, or apply/skip per operation. Nothing is ever applied automatically and nothing runs automatically. After applying, offer "Run new cells".
- `replace` only applies if the target cell content still equals what was sent as context; otherwise it is turned into an insert.
- `delete` and `move` are not allowed in v1 (the model can suggest them in text). This keeps the blast radius small.
- An invalid block falls back to plain markdown with normal SQL blocks.
- A fenced block (not a tool) works with every model, including those without tool support. The same envelope approach is used by the M11 diff walkthrough; share the parser and validation utilities.

### Notebook commands

Added to the slash command registry (M7), only shown in notebook context:

| Command | What it does |
|---|---|
| `/cell <request>` | generate one new SQL cell after the focused cell |
| `/fix` | in a notebook: fix the focused cell, or every failing cell from the last Run All (one `replace` op per cell) |
| `/explain` | explain the focused cell, or the whole notebook if none is focused |
| `/document` | write markdown cells that describe what the notebook does (inserted before the relevant SQL cells) |
| `/summarize` | summarise the results of the chosen cells into a markdown cell (requires the results chip) |

### Agent mode in notebooks (with M9)

- Read tools work the same way (the model can explore tables before proposing cells).
- Notebook edits always go through the review card above, never through a tool that edits silently.
- Write statements inside proposed cells are not executed by the assistant; running them goes through the normal cell run path and its confirmations.

## Steps

- [x] `ChatTarget` on `Conversation` + "This notebook" filter + reopen behaviour.
- [x] Expose the active notebook state to the AI provider (a read-only selector from `NotebookView` state; avoid copying cells on every keystroke: snapshot at send time).
- [x] Notebook context layer in `context.rs` (the frontend sends a `NotebookSnapshot`: outline + focused/neighbour/mentioned cells + errors + optional results).
- [x] `#` cell picker + chips + clickable cell references in answers.
- [ ] Notebook actions on SQL/markdown/chart blocks (cell insert/replace via the bridge from M7).
- [ ] `tabularis-notebook` envelope: parser, validation, review card, apply with the stale-content check.
- [ ] Notebook slash commands + default prompts (overridable like the others).
- [ ] i18n.

### Implementation notes

- Conversation persistence already supports notebook targets. Creation, restoration and in-memory draft/transcript caches now use the connection and target together; legacy null targets remain editor conversations. Loading another notebook's chat from history preserves its binding and disables sends/SQL actions until the matching target is active or a new conversation is created.
- History defaults to **This notebook** in notebook tabs. Notebook metadata supplies titles and confirms deletion; failed metadata reads do not imply deletion. Deleted notebooks retain their conversations and display **Notebook no longer available**. These labels are translated in all 11 locales; later M8 controls still need translations.
- The bridge exposes a local, read-only `readState()` source with title, cells, parameters, focused cell and latest Run All result. It reads current arrays without copying them on each edit. Empty and Markdown-only notebooks retain their own target; SQL actions are disabled without a focused SQL cell.
- Preview and send now capture a fresh, bounded `NotebookSnapshot`. Rust validates section sizes and the total packet, renders separate untrusted-data envelopes within the notebook's 36k-character allocation, and replaces full-query/cursor context with the notebook layer while retaining explicit editor selections. Header/parameters, outline, focused cell, neighbours, mentioned cells, errors and results have independent caps; previews and encrypted message summaries retain names, indexes, sizes and truncation, never cell contents, parameter values or rows.
- Notebook and Include results chips control each message. Results remain off by default, reset after sending and include only the focused/mentioned cells (up to five rows, 40 columns and 200 characters per value, sharing a 6k-character allocation). Errors use the existing opt-in Error chip, including `/fix` defaults. Entry-point snapshots preserve the exact failed/selected SQL. Parameter instructions document both `@name` and `${name}` syntax and verbatim substitution.
- Focused and mentioned SQL feeds the detailed-schema resolver; cells with a different schema override do not select metadata from the current scope. The `#` picker searches indexes, names and bounded first lines, with keyboard, pointer and IME handling. Up to 20 explicit cell choices are stored by ID in the existing scoped conversation cache and reset after sending. Missing choices remain removable warning chips and are reported in the bounded snapshot; choosing a cell also enables Notebook context.
- Rust adds bounded, zero-character `notebook_cell_ref` summaries containing only IDs and the indexes used by that turn. Existing encrypted history preserves these mappings without sending context summaries to providers. Answer navigation checks the conversation/notebook scope and current IDs, supports Markdown cells, and never edits or runs a cell. Legacy focused-cell summaries remain readable. New controls and labels are translated in all 11 locales; later M8 controls still need translations.
- The calmer M4 sidebar keeps notebook choices next to explicit `@` references in the composer. Notebook/results switches and preview details are available through the collapsed Context menu.

## Tests

- Snapshot builder: outline truncation, focused/neighbour selection, results excluded unless opted in.
- Covered: `#` search/picker keyboard and pointer selection, IME, limits, missing IDs, per-notebook choice isolation and reset after send.
- Covered: persisted/legacy reference mappings, reorder/delete handling, Markdown exclusions, notebook/scope gating and actual notebook scroll/focus/highlight without execution.
- Visual fixture: real chat components in Italian at 384/320 px with Light, Dark and High Contrast themes; transport, history and notebook data are in memory. Native notebook/provider checks remain pending.
- Envelope parser: valid ops, `$prev` chaining, unknown op rejected, stale `replace` becomes an insert.
- Apply: ops applied in order, undo restores the notebook.
- `/fix` after a Run All with 2 failing cells proposes 2 `replace` ops with the right ids.

## Done when

- Opening the assistant on a notebook shows notebook-scoped conversations and a notebook context chip.
- "Add a cell that breaks this down by region" inserts a correct cell below the focused one.
- A multi-cell proposal can be reviewed and applied selectively; nothing runs without the user.

## Good first issue candidates

- `/document` command prompt and wiring, once the command registry and envelope exist.
