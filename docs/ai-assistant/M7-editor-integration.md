# M7. Editor integration and modal migration

**Depends on:** M4, M6 · **Size:** M

## Goal

Make the sidebar the single entry point for AI in the editor, and delete the old modals.

## Current state

- `AiDropdownButton` in `Editor.tsx` opens the sidebar. Generate focuses `/generate ` for the user's request; Explain sends `/explain` on the SQL selection or current query.
- `NotebookAiButtons` uses the same sidebar launcher and selects its cell explicitly. `NotebookView` registers the focused SQL cell as the active editor target, including its database/schema override and error. Insert/Replace use Monaco undo stops or update collapsed cells through notebook history; Run opens a console through the existing guarded execution pipeline.
- Pending entry points wait for model discovery and connection-history restoration, are claimed once, and are discarded after a target/pane change. SQL replies retain their original editor/cell target in memory, so switching cells cannot redirect an earlier reply's actions. These bindings are not serialized into history or provider requests; persistent notebook binding belongs to M8.
- `AiQueryModal.tsx`, `AiExplainModal.tsx`, their superseded tests/mocks and unused translations have been removed. Visual Explain's `ExplainAiAnalysis.tsx` remains unchanged.
- Prompt overrides include `prompt_query.txt`, `prompt_explain.txt`, `prompt_optimize.txt` and `prompt_fix.txt` (+ explain plan, cell name, tab rename), with get/save/reset commands and Settings editors.
- Query errors offer Fix with AI, including individual statements in both multi-result layouts. A bounded, transient cache retains single-query execution failures independently of later draft edits; `/fix` sends the failed SQL and complete error, excluding unrelated selection/cursor context. Missing or evicted failure context hides Fix instead of substituting the current draft.
- Monaco selection menus in the editor and notebook offer Explain, Optimize and Ask AI. Explain/Optimize send immediately with selection-only editor context; Ask AI focuses an empty composer without a model call. Registration follows editor mounting, AI availability, busy state and cleanup.
- Explorer tables offer Ask AI with an explicit table chip and database/schema scope. A temporary target survives composer edits and opens SQL actions in a console scoped to that table; editor changes or owner unmount release it. Cross-connection requests are rejected. The palette offers Explain/Optimize for the whole current query in the active pane.
- All entry-point labels are translated in all 11 locales. Implementation and automated checks are complete; visual/native/live-provider validation remains pending.

## Design

### Slash commands

- `/` at the start of an empty composer opens the command list: `/generate`, `/explain`, `/optimize`, `/fix`, `/help`.
- A command = an action id + an instruction appended to the system prompt for that turn + a default context spec:

| Command | Default context | Instruction source |
|---|---|---|
| `/generate <request>` | schema + current query | `prompt_query.txt` (existing override, `{{SCHEMA}}` handled by the context builder) |
| `/explain` | selection or current query + its tables | `prompt_explain.txt` (`{{LANGUAGE}}` kept) |
| `/optimize` | query + tables + indexes + EXPLAIN if available | new default prompt, overridable |
| `/fix` | query + last error + tables | new default prompt, overridable |
| `/help` | none | static, local (no model call) |

- Text after the command is the user request; with no text, the command works on the selection or current query.
- The command is stored on the user message, so the transcript shows "/explain" as a chip.

Implementation note: the original slash prefix is retained in the encrypted user-message text and rendered as a command chip. This also preserves commands in exports and older clients without a message-schema change. `ContextSpec.command` carries the typed action for the current turn; Rust checks it against the latest user message and sends only the arguments to the provider. Prompt overrides are read afresh for both sends and previews. `/help` stays local and is not added to history.

### Migrating the modals

- `AiDropdownButton` items open the sidebar and send `/generate` (with the composer focused, waiting for the request) or `/explain` (sent immediately on the selection or query).
- Notebook buttons do the same, with the cell as the target for Insert/Replace (extend the active-editor bridge from M4 to target a notebook cell).
- Generate no longer auto-runs: the result arrives as a SQL block with Insert / Replace / Run. Behaviour change, worth a note in the changelog.
- Delete `AiQueryModal.tsx`, `AiExplainModal.tsx` and their unused i18n keys. Keep `ExplainAiAnalysis.tsx` (Visual Explain) as is, or add a "Continue in chat" button there.

### Fix with AI

- In the query error view (`MultiResultPanel.tsx` and the error component it renders), a "Fix with AI" button when AI is enabled: open the sidebar, send `/fix` with query + error chips.

### Context menus

- Monaco context menu actions: "Explain selection", "Optimize selection", "Ask AI about selection" (the last one opens the panel with the selection chip and focuses the composer).
- Explorer table context menu (`ExplorerSidebar.tsx`): "Ask AI about this table" (opens the panel with an `@table` chip).
- Command palette entries for `/explain` and `/optimize` on the current query.

## Steps

- [x] `impact` on `AiDropdownButton`, `AiQueryModal`, `AiExplainModal`, `NotebookAiButtons`, the `Editor.tsx` region, the error view. Repeat before the migration edits if the index or callers change.
- [x] Slash command registry (frontend list + backend instruction lookup).
- [x] New default prompts for optimize and fix in `config.rs` + override files + editors in `AiTab.tsx`.
- [x] Rewire the dropdown and notebook buttons; extend the editor bridge to notebook cells.
- [x] Fix with AI button.
- [x] Context menu actions + palette entries.
- [x] Delete the old modals and dead i18n keys.
- [x] Changelog note.

## Tests

- Slash parser: command + args, unknown command, `/` mid-text is plain text.
- Each entry point sends the expected command and context spec (mock `useAiChat`).
- Notebook target: Insert writes into the right cell.

Implemented coverage: parser/picker keyboard and IME behavior, unknown commands, local help without model/history calls, selected-SQL defaults, removable error context, command persistence and turn isolation, prompt editor load/save/reset, Rust action/request validation and placeholder expansion, and selected-table metadata/gaps with the full query excluded. Migration regressions cover Generate without automatic sends/edits/execution, invocation-time Explain selections, lazy mounting/history restoration and StrictMode, pane switches, availability/streaming guards, per-cell scope and action binding, reordering/deletion/collapsed cells, Monaco undo/disposal/focus callbacks and reply-binding reset. Final entry-point regressions cover failed executed SQL after draft edits, full error details, selection-only menus and notebook-cell routing, empty selections, availability/disposal, Ask without a provider call, scoped table chips/SQL actions and overlay cleanup, foreign-connection rejection and whole-query palette actions in split panes.

## Notebooks

M7 only migrates the per-cell buttons and makes the editor bridge able to target a notebook cell. Notebook-specific context, actions and commands are in M8.

## Done when

- No AI modal is left for Generate/Explain; every AI entry point in the editor and notebook goes through the sidebar.

## Good first issue candidates

- "Fix with AI" button on query errors.
- "Ask AI" entries in the editor and explorer context menus.
