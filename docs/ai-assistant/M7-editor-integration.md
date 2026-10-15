# M7. Editor integration and modal migration

**Depends on:** M4, M6 · **Size:** M

## Goal

Make the sidebar the single entry point for AI in the editor, and delete the old modals.

## Current state

- `src/components/ui/AiDropdownButton.tsx` (items `ai.generateSql`, `ai.explain`), floating in `src/pages/Editor.tsx` (~line 5060).
- `src/components/modals/AiQueryModal.tsx`: Generate replaces the whole tab and runs it immediately.
- `src/components/modals/AiExplainModal.tsx`: plain-text output, hardcoded English strings, `bg-black/50` overlay.
- Notebook: `src/components/notebook/NotebookAiButtons.tsx` reuses both modals per SQL cell.
- Prompt overrides: `prompt_query.txt`, `prompt_explain.txt` (+ explain plan, cell name, tab rename) with get/save/reset commands and editors in `AiTab.tsx`.

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

- [ ] `impact` on `AiDropdownButton`, `AiQueryModal`, `AiExplainModal`, `NotebookAiButtons`, the `Editor.tsx` region, the error view.
- [ ] Slash command registry (frontend list + backend instruction lookup).
- [ ] New default prompts for optimize and fix in `config.rs` + override files + editors in `AiTab.tsx`.
- [ ] Rewire the dropdown and notebook buttons; extend the editor bridge to notebook cells.
- [ ] Fix with AI button.
- [ ] Context menu actions + palette entries.
- [ ] Delete the old modals and dead i18n keys.
- [ ] Changelog note.

## Tests

- Slash parser: command + args, unknown command, `/` mid-text is plain text.
- Each entry point sends the expected command and context spec (mock `useAiChat`).
- Notebook target: Insert writes into the right cell.

## Notebooks

M7 only migrates the per-cell buttons and makes the editor bridge able to target a notebook cell. Notebook-specific context, actions and commands are in M8.

## Done when

- No AI modal is left for Generate/Explain; every AI entry point in the editor and notebook goes through the sidebar.

## Good first issue candidates

- "Fix with AI" button on query errors.
- "Ask AI" entries in the editor and explorer context menus.
