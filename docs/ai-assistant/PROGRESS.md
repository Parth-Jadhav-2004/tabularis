# AI Assistant: progress log

How to resume: read [README.md](./README.md) (decisions + milestone order), find the current milestone below, open its `M*.md`, continue from the first unchecked step. Update this file at the end of every session.

Keep detailed security reviews and audit findings outside the repository. This log records implementation progress and validation only.

## Status

| Milestone | Status | Branch / PR | Notes |
|---|---|---|---|
| M1 Provider layer | in review | `feat/ai-assistant-provider-layer` | Implementation complete; automated checks pass. Live-provider/native smoke checks pending. |
| M2 Secure store | in review | `feat/ai-assistant-provider-layer` | Implementation and automated checks complete. Native keychain/platform validation remains pending. |
| M3 Tool registry | in review | `feat/ai-assistant-provider-layer` | Typed registry, shared approval transport, eight chat tools and bounded results implemented. Automated checks pass; native MCP smoke check pending. |
| M4 Chat panel | in review | `feat/ai-assistant-provider-layer` | Calmer sidebar with compact composer and collapsed context/model controls; Markdown/SQL actions, editor bridge and entry points implemented. Automated and in-memory visual checks pass; native/provider smoke validation pending. |
| M5 Conversations | in review | `feat/ai-assistant-provider-layer` | History lifecycle, generated titles, prompt editor and Markdown/notebook exports implemented. Automated and in-memory visual checks pass; native persistence/provider/export smoke validation pending. |
| M6 Context | in review | `feat/ai-assistant-provider-layer` | Schema cache, Rust-resolved sends/previews, typed mentions, scoped drops, encrypted summaries and per-connection instructions implemented. Visual/native/live-provider validation pending. |
| M7 Editor integration | in review | `feat/ai-assistant-provider-layer` | Slash commands, prompt customization, sidebar migration, cell targets, Fix with AI and context-menu/palette entry points implemented; old modals removed. Automated checks pass; visual/native/provider validation pending. |
| M8 Notebooks | in progress | `feat/ai-assistant-provider-layer` | Notebook-bound conversations, bounded context/results opt-in, `#` picker/chips and stable clickable cell references implemented. Reviewed cell actions, multi-cell proposals and notebook commands pending. |
| M9 Agent mode | not started | | |
| M10 Multimodal | not started | | |
| M11 Extras | not started | | |

Status values: `not started` · `in progress` · `in review` · `done` · `blocked`.

**Current focus:** M8 context and stable cell references are implemented, alongside the M4 sidebar redesign. Next: reviewed notebook cell actions, the multi-cell envelope and notebook commands. In-memory sidebar visual checks pass; M8 native/provider checks, M7 visual/native/provider checks, M6 mixed-mention/instructions visual and native/live-provider context checks, M5 native restart/history/title/export checks, M4 native/provider/editor smoke validation, M3 native MCP smoke validation, M2 native keychain/platform validation and M1 live-provider smoke checks remain pending.

## Open questions

- [ ] Key loss: accept unrecoverable history, or add a password-protected backup via `export_crypto`?
- [ ] Message body search (needs decrypting all files, or an encrypted index).
- [ ] Does the `--mcp` subprocess trigger a keychain prompt when it reads the AI store key on macOS? (M2)
- [x] Wire format: snake_case fields, snake_case enum values and a `type` discriminator. (M1/M4)
- [x] Shortcut for toggling the AI panel: configurable `Mod+Shift+I`, no existing conflict. (M4)

## Good first issues

Candidates are listed in README.md. Track opened ones here:

| Candidate | Milestone | Issue | Status |
|---|---|---|---|

## Session log

Newest first. Keep each entry short: what was done, what is next, anything surprising.

### 2026-10-15: Calmer sidebar and stable notebook cell references
- Published the bounded notebook context/results work as `f6bd1fe4` on `feat/ai-assistant-provider-layer`, without a co-author trailer.
- Redesigned the sidebar following the requested ChatGPT/Claude direction: quiet header, subtle user bubbles, flowing assistant replies, compact auto-growing composer and icon SQL actions. Context layers/previews and model search/capabilities are hidden behind compact disclosures; only explicit `@`/`#` choices stay in the composer. Successful save/completion notices remain accessible without adding permanent visual rows. Errors remain visible.
- Added the `#` picker with index/name/first-line search, arrows, Enter/Tab, pointer and IME support, removable chips and a 20-cell limit. Choices use stable IDs in the scoped conversation cache, reset after send and report deleted choices without substituting another cell. Rust validates unavailable IDs and persists bounded ID/index metadata with the existing encrypted context summaries.
- Assistant prose references now scroll to, focus and highlight the corresponding cell, including Markdown cells. Per-turn mappings survive reorder and conversation resume; deleted/unknown references remain text, and code/existing links are excluded. Navigation respects the active notebook and does not edit or execute SQL. Added all new strings in 11 locales.
- Validation: 21 new frontend regressions and two new Rust regressions. Complete Vitest suite: 6,375 passed (420 files); complete Cargo suite: 1,925 passed / 221 ignored, run serially. Production frontend build/cargo check, application/scoped-test typechecks, repository ESLint with local worktrees/cache excluded, theme-token/contrast checks, AI-chat locale key parity, scoped Rust formatting and diff checks pass.
- Visual validation: real chat components in an Italian, in-memory browser fixture at 384/320 px; Light, Dark and High Contrast reviewed, including context disclosure and compact model controls. Removed the temporary fixture/server afterward. Native Tauri/live-provider/notebook-history validation remains pending; no real provider, saved notebook/history or OS keychain was used.
- Pre-edit GitNexus impact: HIGH for the notebook hook (one direct caller / three process groups), shared context choices (17 dependents) and Rust notebook renderer (one direct caller / resolver group); LOW/MEDIUM for other resolved application symbols. Unresolved graph boundaries were checked against source references. A full index rebuild with PDG retained avoids stale incremental symbol data. Final staged `detect_changes`: 49 files, 137 symbols and 11 flows (HIGH), confined to expected chat/context sends, notebook navigation and panel dependencies; the source diff and complete suites confirm this scope.
- M8 remains in progress; the fourth implementation step is checked. Next: reviewed notebook cell actions, multi-cell proposals and notebook commands.

### 2026-10-15: M8 bounded notebook context and results opt-in
- Added fresh notebook snapshots for previews and sends, replacing full editor-query/cursor context while preserving explicit selections and exact entry-point failure SQL. Snapshots include title, stop-on-error, bounded parameters, outline, focused/neighbour/mentioned cells and optional errors/results; cells are not copied on each keystroke. Empty and Markdown notebooks remain valid. Mentioned-ID support is ready for the next step's picker.
- Rust validates snapshot shape, section limits and a 256 KiB packet cap, then renders notebook data in separate untrusted-data envelopes within 36k characters. Independent budgets preserve space for focused cells, errors and results when the outline is large. Focused/mentioned SQL selects detailed metadata only in the matching schema; parameter guidance covers both `@name` and `${name}` with verbatim substitution. Preview/history metadata contains names/indexes, sizes and truncation, never SQL, parameter values, errors or rows.
- Added Notebook and Include results chips. Results require explicit opt-in for every message, cover only focused/mentioned cells, and share a 6k-character cap (five rows, 40 columns, 200 characters per value). Removing Notebook also disables results. Errors reuse the opt-in Error chip and `/fix` defaults. Added context labels/descriptions in all 11 locales.
- Validation: 11 new frontend regressions and 11 new Rust regressions. Complete Vitest suite: 6,354 passed (416 files); complete Cargo suite: 1,923 passed / 221 ignored, run serially. Production frontend build/cargo check, application/scoped-test typechecks, repository ESLint with local worktrees/cache excluded, scoped-test ESLint, theme-token/contrast checks, AI-chat locale key parity, scoped Rust formatting and diff checks pass.
- Visual/native/live-provider validation remains pending: computer-use inventory exposes no enabled browser automation surface. Tests use in-memory chat fixtures and disposable SQLite databases; no real provider, saved notebook/history or OS keychain was used.
- Pre-edit GitNexus impact: CRITICAL for shared context defaults (10 direct callers, 14 process groups), HIGH for the shared builder/validator (one/two direct callers, resolver flows) and optional context interfaces (16–17 direct dependents); remaining UI helpers are LOW or have unresolved graph boundaries checked against source references. Rebuilt GitNexus with PDG retained. Final disposable-index `detect_changes` covers 33 files, 96 symbols and 47 indexed flows (CRITICAL). The broad frontend report includes name-based matches to local `text`/`value` helpers; the source diff is confined to notebook context, chat controls and shared context defaults. The real staging area is unchanged; no commit or push was made.
- M8 remains in progress; its third implementation step is checked. Next: `#` cell picker/chips and clickable answer references, then reviewed cell actions, multi-cell proposals and notebook commands.

### 2026-10-15: M8 notebook conversations and state bridge
- Published the completed M6–M7 changes as `417e1395` on `feat/ai-assistant-provider-layer`, without a co-author trailer.
- New chats persist their notebook target. Reopening restores the latest conversation for that notebook; editor/legacy conversations and each notebook have separate transcript, draft and failed-save caches. Target switches wait for streaming/persistence; late list/load results cannot replace another notebook's chat. Rename/delete and delayed generated-title updates address the corresponding cached target.
- Added the default **This notebook** history filter and notebook labels in the list/transcript. Metadata refreshes after notebook changes; confirmed deletion keeps the conversation readable and reports **Notebook no longer available**. Foreign or unavailable targets disable sends/SQL actions; users can start a new chat on an available active target. Added three labels in all 11 locales.
- Added a local, read-only notebook selector for current title, cells, parameters, focus and Run All errors without copying the cells on each keystroke. Focus includes Markdown cells; empty/Markdown-only notebooks keep their target with SQL actions disabled. The source is not sent to IPC/providers or stored in history yet; bounded snapshots/results opt-in and the Rust context layer are next.
- Validation: 23 new frontend regressions; complete Vitest suite: 6,343 passed (415 files). Production frontend build, application/scoped-test typechecks, repository ESLint with local worktrees/cache excluded, theme-token/contrast checks, AI-chat locale key parity and diff checks pass. Visual/native/live-provider checks remain pending; backend source is unchanged in this step.
- Pre-edit impact: HIGH for the notebook hook (one direct caller, two process groups) and shared editor/resume interfaces (17–18 direct dependents); LOW for the other edited application functions. Rebuilt GitNexus with PDG retained. Pre-commit `detect_changes`: 31 files, 58 symbols and five affected flows (MEDIUM), matching chat restoration and notebook scope. M8 remains in progress; the first two implementation steps are checked.

### 2026-10-15: M7 Fix with AI and contextual entry points completed — ready for review
- Added Fix with AI to single-query errors and individual failed statements in both multi-result layouts. Single-query execution retains the exact failed SQL in a bounded, transient cache, so later draft edits cannot replace it. Fix sends the complete error with query/error context enabled and unrelated selection/cursor context excluded. Missing or evicted failure context hides Fix instead of substituting the current draft; generic and unavailable/busy surfaces retain their existing behavior.
- Added Explain selection, Optimize selection and Ask AI to Monaco in the editor and notebook. Selection preconditions, fresh snapshots, per-cell targeting and disposal/availability handling share the sidebar launcher. Explain/Optimize send once; Ask opens an empty, focused composer without contacting the provider.
- Added Ask AI about this table to explorer menus, with an explicit table chip and database/schema. Temporary table targets survive composer edits, reject foreign connections and are released on editor changes/unmount. SQL actions open a correctly scoped console. Palette Explain/Optimize actions use the whole current query in the active split pane. Added seven labels in all 11 locales and extended the changelog.
- Validation: 35 new frontend regressions. Complete Vitest suite: 6,320 passed (411 files). Production frontend build, application/scoped-test typechecks, repository ESLint with local worktrees/cache excluded, theme-token/contrast checks, locale key parity and diff checks pass. Updated existing touched test fixtures so they also typecheck against current editor/settings interfaces.
- Visual/native/live-provider validation remains pending; no real provider, database/history or OS keychain was used in this step. Rust source is unchanged by these entry points; backend checks were not repeated. Persistent notebook/conversation binding remains M8 work.
- Pre-edit impact: CRITICAL for the shared error view (three direct callers), Monaco editor (one direct caller) and query runner (13 direct callers / seven process groups); HIGH for the launcher, result content, query-error wrapper and palette hook, MEDIUM for editor interfaces, LOW for the remaining application entry points. The query runner change only retains transient failure context. Rebuilt GitNexus with PDG retained. Final staged `detect_changes` covers the combined uncommitted M6–M7 work: 103 files, 303 symbols and 78 flows (CRITICAL), including context resolution, connection lifecycle, editor/query execution, notebook and explorer entry points. Used a disposable Git index; the real staging area is unchanged.
- All M7 implementation steps are checked. M7 remains in review until visual/native/provider validation passes. Next implementation: M8 notebook context and reviewed cell actions.

### 2026-10-15: M7 editor/notebook sidebar migration and modal removal
- Rewired the editor dropdown and notebook buttons to the sidebar. Generate focuses `/generate ` and waits for a request; Explain sends `/explain` using the selection captured at invocation. Entry requests wait for model discovery/history restoration, are claimed once under StrictMode and are discarded after a target/pane change. Availability, streaming and pending/failed-save guards protect the shared session.
- Added focused-cell notebook targets with per-cell scope/errors, Monaco selection/undo support and collapsed-cell edits through notebook history. Run uses the existing console navigation/execution pipeline. Individual replies retain their original editor/cell/scope binding in memory, preventing actions from being redirected after a cell/pane switch. These bindings are omitted from provider requests and encrypted history; persistent notebook binding remains M8 work.
- Removed both legacy Generate/Explain modals, six superseded tests, unused mocks and dead keys in all 11 locales. Added visible dropdown focus states and a changelog note: generating SQL no longer replaces or executes a query automatically. Visual Explain is unchanged.
- Added 15 frontend regressions covering sidebar entry points, asynchronous restoration, target isolation, notebook edits and Monaco focus/disposal. Complete Vitest suite: 6,285 passed (406 files). Production frontend build, application/scoped-test typechecks, repository ESLint with local worktrees/cache excluded, theme-token/contrast checks and diff checks pass.
- Visual/native/live-provider validation remains pending; UI inventory exposes no browser automation surface. No real provider, database/history or OS keychain was used in this step. Backend source is unchanged by this migration; Rust checks were not repeated.
- Pre-edit impact: HIGH for the dropdown and old modals (two direct callers, split-pane layout flow), CRITICAL for the shared state initializer (three direct callers, no indexed flows), HIGH for chat state/send-option interfaces (18 direct dependents), MEDIUM for editor interfaces and LOW for the remaining application targets. Refreshed GitNexus with PDG retained after an incremental UTF-8 error. Final staged `detect_changes` covers the combined uncommitted M6–M7 work: 86 files, 217 symbols and 40 flows (CRITICAL), including context resolution, connection save/update/duplication, editor/notebook entry points and startup registration. Used a disposable Git index; the real staging area is unchanged.
- M7 remains in progress. Next: Fix with AI on query errors, then editor/explorer context menus and command-palette entries.

### 2026-10-15: M7 slash commands and prompt overrides implemented
- Added `/generate`, `/explain`, `/optimize`, `/fix` and local `/help`, with a translated, accessible composer picker supporting arrows, Enter/Tab, Escape, pointer selection and IME. Unknown commands stay editable without starting a run. Command defaults prefer selected SQL where appropriate; `/fix` includes the last error by default and lets users remove it.
- Encrypted history retains the original slash-prefixed user text and renders a command chip. Rust checks the typed action against the latest user message and sends only its arguments (or an editor-context fallback) to the provider. Actions apply to one turn. Existing query/explain overrides are reused, with schema context supplied by the bounded builder and language placeholders expanded in Rust.
- Added `prompt_optimize.txt` / `prompt_fix.txt`, native get/save/reset commands and Settings editors. All new controls and descriptions are translated in all 11 locales. Selection-only requests now resolve referenced-table details and missing-table gaps without including the excluded full query.
- Validation: 21 new frontend regressions and seven new Rust regressions. Complete Vitest suite: 6,276 passed (404 files); complete Rust library suite: 1,908 passed / 11 ignored, run serially. Production frontend build, production cargo check, application/scoped-test typechecks, repository ESLint with local worktrees/cache excluded, theme-token checks, scoped Rust formatting and diff checks pass.
- Visual verification remains pending: the UI tools expose no available browser, including the in-app browser. Removed the temporary fixture/server. No live provider, real database/history or OS keychain was used for this step; backend regressions use disposable SQLite fixtures.
- Pre-edit impact: HIGH for the shared context-layer helper (one direct caller, resolver process group), MEDIUM for selected-metadata resolution (nine direct callers, resolver flows) and the optional frontend context field; other edited application entry points are LOW. Preliminary migration analysis also flags the dropdown, both old modals and the shared error view HIGH; those are unchanged in this step. Refreshed GitNexus with PDG retained. Final staged `detect_changes` covers the combined uncommitted M6–M7 work: 64 files, 69 symbols and 24 flows (CRITICAL), including connection save/update/duplication and startup registration. The real staging area is unchanged.
- M7 remains in progress. Next: editor dropdown/notebook migration and the notebook-cell target bridge, followed by Fix with AI, context menus/palette entries, modal removal and the changelog note.

### 2026-10-15: M6 per-connection instructions completed — ready for review
- Added optional `SavedConnection.ai_instructions` as non-secret configuration. Native save/update and deserialization enforce a 2,000-Unicode-code-point limit; empty preferences are omitted. Existing connections remain compatible, native export/import and duplication preserve instructions, and clearing a saved value survives unknown-field preservation.
- Added an accessible Advanced-tab textarea with Unicode-safe input limiting, a character counter and an explicit unencrypted-storage/export description. New form sessions reset preferences. Added all strings in all 11 locales.
- Rust resolves saved instructions afresh for both previews and sends, independently of schema caching and the connection-metadata toggle. Added a default-on `include.instructions` choice and a removable/restorable chip with actual preview size and truncation. IPC requests carry the toggle only; preview and encrypted-history summaries carry names/sizes, never the instruction text.
- Validation: nine new frontend regressions and five new Rust regressions. Complete Vitest suite: 6,255 passed (402 files); complete Cargo suite: 1,905 passed / 221 ignored, run serially. Production build/cargo check, application/scoped-test typechecks, repository ESLint with local worktrees/cache excluded, theme-token/contrast checks, scoped Rust formatting and diff checks pass. Removed redundant unsupported `exact` options from existing role queries so the touched modal tests typecheck.
- Visual verification of mixed mentions and instructions remains pending: no browser automation surface is available in this session. Native/live-provider validation also remains pending; tests use disposable files/databases and existing in-memory fixtures.
- Pre-edit GitNexus impact: CRITICAL for `SavedConnection` (13 direct dependents; three save/update/duplicate process groups) and the context initializer (two direct callers, no indexed flows); MEDIUM for the internal resolver (eight direct callers, resolver flows). Other edited application targets are LOW. Refreshed the index with PDG retained. Final staged `detect_changes`: 45 files, 52 symbols and 15 affected flows (HIGH), matching the expected connection-settings/context scope. Used a disposable Git index; the real staging area is unchanged.
- M6 implementation is complete and remains in review until visual/native/live-provider checks pass. Next implementation: M7 editor actions and modal migration.

### 2026-10-15: M6 view and saved-query mentions implemented
- Extended scoped catalogs and cached details to regular views, using the view-specific columns/definition APIs. Explicit typed mentions share the default 20-detail limit and take priority over inferred references. Typed cache keys keep equal table/view names separate; failed view catalogs are retried and the existing view refresh invalidates AI metadata.
- Added saved-query ID selection, resolved afresh in Rust against the current connection/database. Sources are read with bounded metadata/SQL sizes and UTF-8-safe truncation; unavailable selections become explicit gaps. Saved SQL shares the editor budget and untrusted-data envelope. Requests carry IDs only; preview/history retain names, sizes and optional view counts.
- Added mixed `@` suggestions, type badges/accessibility labels, view/saved-query chips and all 11 locale translations. Equal labels remain distinct, and a large table list cannot hide every other matching kind. Model changes before the first send now preserve context choices.
- Validation: 11 new frontend regressions and 12 new Rust regressions. Complete Vitest suite: 6,246 passed (400 files); complete Cargo suite: 1,900 passed / 221 ignored, run serially. Production cargo check/build, application/scoped test typechecks, repository ESLint with local worktrees/cache excluded, theme-token/contrast checks, scoped Rust formatting and diff checks pass.
- Mixed-mention visual verification remains pending: Chrome changed during the local fixture attempt, so UI automation was stopped and its tab/server/files were removed to avoid interfering with the user's session. No real provider, database, history or keychain was used for UI verification.
- Pre-edit GitNexus impact: HIGH for catalog/detail loaders and the builder (one to ten direct callers, resolver flows); MEDIUM for the internal resolver (seven direct callers); other edited application targets are LOW. Refreshed the index with PDG retained. Final staged `detect_changes`: 46 files, 126 symbols and nine resolver flows (HIGH), matching the expected context/mention scope.
- M6 remains in progress. Next: per-connection instructions and their chip/translations, then visual/native/live-provider validation.

### 2026-10-15: M6 table previews, mentions and explorer drops implemented
- Added a read-only context-preview command using the same Rust resolver, cache and character budgets as sends. IPC exposes only names, sizes, counts and truncation. Structured schema table counts also survive encrypted history, while older summaries remain compatible.
- Added debounced, refreshable previews and per-table inclusion/exclusion controls. Table chips show actual sizes, incomplete metadata and details absent from the preview. Scoped choices survive panel remounts and connection switches, then reset after sending or starting a new chat. Late preview responses cannot replace newer scope/selection state; sends still read the editor and resolve context afresh.
- Added `@` table suggestions from the active cache, with arrows, Enter/Tab, Escape, pointer selection, IME handling and accessible active-option state. Added explorer table drops through the existing pointer-drag mechanism, preserving query-builder compatibility and rejecting cross-connection/database/schema or unknown-table drops. Translated controls in all 11 locales.
- Validation: 26 new frontend regressions and three new Rust regressions pass. Complete Vitest suite: 6,235 passed (400 files); complete Cargo suite: 1,888 passed / 221 ignored, run serially. Production cargo check/build, application/scoped test typechecks, repository ESLint with local worktrees/cache excluded, theme-token checks and scoped Rust formatting pass (pre-existing module ordering retained).
- Visually verified Italian counts, removal/restoration, keyboard picking, real explorer pointer drops and stored summaries in Light, Dark and High Contrast, including square corners and visible focus. Fixed the active mention option to use the primary/inverse contrast pair. Removed the temporary in-memory fixture/server; no real provider, database, history or keychain was used for UI verification.
- Pre-edit GitNexus impact: HIGH for the shared builder (nine direct callers, resolver flows), CRITICAL for the state initializer (three direct callers, no indexed flows); both changes only add metadata/next-turn choices. Other edited application targets are LOW. Refreshed the index with PDG retained. Final staged `detect_changes`: 44 files, 88 symbols and nine startup flows (HIGH); the startup function changes only register the preview command. Scope matches the expected context/UI changes.
- M6 remains in progress. Next: view/saved-query mentions and per-connection instructions, followed by native/live-provider context validation.

### 2026-10-15: M6 schema cache, chat context and layer controls connected
- Added a 30-second scoped schema cache with bounded scope/batch counts, shared concurrent loads and retry of unavailable metadata. Schema refresh and successful connection-file saves detach old entries; cancelled or stale loads cannot repopulate refreshed scopes.
- Wired `ContextSpec` into `ai_chat_send`. Rust resolves the connection, scope, credentials/tunnels, capabilities and read-only state, builds the bounded prompt and keeps Ask/tools policy fixed. Preparation supports cancellation and a 15-second timeout. Once the provider opens, metadata is attached to the producing user message and saved through M2; all provider serializers omit context-summary blocks.
- Added fresh editor snapshots and removable layer controls, with errors opt-in. Choices survive sidebar remounts and connection-session switches, then reset for the next message. Encrypted history shows collapsed context names/sizes/truncation; old blocks without sizes remain compatible. Translated controls and summaries in all 11 locales.
- Validation: 16 new Rust regressions and 11 new frontend regressions pass. Complete Cargo suite: 1,885 passed / 221 ignored, run serially; complete Vitest suite: 6,209 passed (397 files). Production cargo check/build, application/scoped test typechecks, repository ESLint with local worktrees/cache excluded, theme-token checks and scoped formatting pass.
- Visually checked Italian controls, removal/restoration state, visible focus and stored summaries in Light, Dark and High Contrast (including square corners), using an in-memory browser fixture. Removed the fixture/server. No real provider, database, conversation history or OS keychain was used for UI verification; backend fixtures use disposable SQLite databases and in-memory keychains.
- GitNexus pre-edit scope is LOW for send/reducer/editor/panel targets. Shared connection invalidation is HIGH (20 direct callers / four reported process groups); its change only clears the AI metadata cache after a successful save. The state initializer is CRITICAL (three direct callers, no indexed flows); its change only adds per-turn context choices. Refreshed the index with PDG retained. Final staged `detect_changes`: 154 files, 777 symbols and 29 flows across the combined M3–M6 work (CRITICAL); the nine new flows start at the context resolver, alongside the previous 20 tools/startup flows.
- M6 remains in progress. Next: schema/table preview counts and individual table chips, mentions/drag and drop, per-connection instructions and native/live-provider context validation.

### 2026-10-15: M6 backend context foundation implemented
- Added a separate bounded schema catalog and selected-table detail loader, including table/column comments, indexes and explicit missing/omitted/unavailable metadata. Loads retain selection priority and use bounded concurrency. Legacy schema types, provider/plugin wire formats and commands remain unchanged.
- Added a pure layered prompt builder with fixed mode rules, scoped connection metadata, connection preferences, detailed tables, overview, editor state and relevant errors. Unicode character budgets include escaping/envelopes and pass unused space forward. Context summaries contain names and actual sizes; incomplete schemas add the missing-schema rule. Untrusted content cannot close its data envelope.
- Added bounded SQL reference discovery for joins, comma lists, nested queries, quoted/qualified identifiers and recursive/nonrecursive CTEs. Unknown, ambiguous and cross-scope names are explicit gaps; the scanner is separate from execution policy.
- Validation: 23 new Rust regressions pass, including real metadata-to-prompt integration against a disposable SQLite database, budget boundaries, truncation, scope/CTE handling and data-envelope escaping. Complete Cargo suite: 1,869 passed / 221 ignored, run serially. Production cargo check, `pnpm typecheck`, theme-token checks, scoped rustfmt and diff checks pass.
- Pre-edit GitNexus impact is LOW for the three legacy schema targets (0–2 direct callers, no indexed flows). Parser refinements are HIGH through the new builder and its tests, with no indexed application flow. Rebuilt the index with PDG retained after an incremental FTS error. Final `detect_changes` includes new source/tests through a disposable index: 137 files, 650 symbols and the same 20 affected flows across the combined M3–M6 work (CRITICAL). The real staging area is unchanged.
- The context foundation is not wired into chat sends yet. Next: cache/refresh invalidation, `ContextSpec` resolution and persisted summaries, then chips, mentions/drag and drop, connection instructions and i18n. M6 remains in progress.

### 2026-10-15: M5 generated titles and conversation exports completed — ready for review
- Added background title generation after the first successful reply is saved, using that reply's provider/model and bounded text from the first exchange. Titles are limited to six words / 80 Unicode code points; failures and a 30-second timeout silently retain the first-message title. Added `prompt_chattitle.txt` load/save/reset and its Settings editor, with translations in all 11 locales.
- Generated titles use a conditional update under the encrypted store lock. Manual renames and stored generated titles survive stale saves; late title results cannot recreate deleted/replaced history. Frontend reset/unmount guards and connection-cache updates preserve current messages, drafts and streaming state.
- Added Markdown and notebook export to history rows, using the native save dialog with an explicit unencrypted-file notice. Markdown retains SQL fences and collapsed tools/reasoning; images/attachments are filename references only. Notebook export reuses the existing export types/naming, converts explicitly tagged SQL fences to SQL cells, preserves prose order and the database override, and embeds no results. Export filenames are bounded in Unicode code points and UTF-8 bytes.
- Validation: full Vitest suite 6,198 passed (393 files), including 16 new frontend regressions; complete Cargo suite 1,846 passed / 221 ignored, run serially, including seven new Rust regressions. Application/scoped test typechecks, production build, production cargo check, scoped rustfmt, theme-token checks and repository ESLint with local worktrees/cache excluded pass.
- Visually checked the Italian export controls and prompt editor in Light, Dark and High Contrast, including square corners, focus and export cancellation, using an in-memory browser fixture. Removed the temporary fixture/server. No real provider, database, conversation history or OS keychain was used for that check.
- Pre-edit GitNexus impact is LOW for session/reducer/history/Settings/command registration. The shared store save is CRITICAL (three direct callers and two reported process groups); its change only preserves title precedence. Refreshed the index with PDG retained. Final `detect_changes` includes new source/tests through a disposable index: 125 files, 498 symbols and the same 20 affected flows across the combined M3–M5 work (CRITICAL). The real staging area is unchanged.
- M5 remains pending native restart/history, live title generation and file save/import smoke validation. Next implementation: M6 schema context, layered budgets and referenced-table extraction, then chips and mentions.

### 2026-10-15: M5 history browsing and conversation lifecycle completed
- Added header history navigation, title-only search, local-calendar date buckets, relative times and connection/All filters grouped by connection name. Added inline rename, explicit delete confirmation, loading/empty/error states and links to the existing Settings recovery controls. Translated history controls in all 11 locales.
- Reused M2's encrypted list/load/rename/delete commands. New app sessions restore the most recent conversation per connection; running sessions retain each connection's selected transcript and draft in memory, including failed saves for retry. No transcript/title is written to browser storage.
- Connection switching waits for streaming and saving to settle. Versioned loads/lists ignore stale connection/filter responses and reset/unmount completions. Resume preserves scope/target/mode, falls back from unavailable models with a notice, and leaves the producing models on past messages intact. Initial titles now use 50 Unicode code points.
- Validation: full Vitest suite 6,182 passed (389 files), with 23 new history regressions; 12 existing Rust conversation-store tests passed serially. Application/scoped test typechecks, production build, source ESLint and theme-token checks pass. Repository ESLint passes with local worktrees/cache excluded; unfiltered `pnpm lint` fails on 517 errors in pre-existing `.claude/worktrees` checkouts.
- Visually checked the Italian history UI at sidebar width in Light, Dark and High Contrast, including square corners, All grouping, focus and inline rename, using an in-memory fixture. No real provider, database, conversation history or OS keychain was used for that check. Temporary fixture/server removed.
- Pre-edit GitNexus impact is LOW for panel/header/provider/session targets. The state initializer is CRITICAL (two direct callers, 13 reachable symbols, no indexed flows); its change only adds history state. Existing streaming/cancellation/save regressions pass. Next: generated titles and prompt customization, Markdown/notebook exports, then native restart/history smoke validation. M5 remains in progress.
- Rebuilt the GitNexus index with PDG retained after an incremental-index error. Final `detect_changes` includes untracked source/tests through a disposable Git index: 111 files, 435 symbols and 20 affected flows across the combined M3–M5 work (CRITICAL). The real staging area is unchanged.

### 2026-10-15: M4 sidebar, SQL actions and provider selection completed — ready for review
- Mounted the shared chat session above a lazy sidebar panel. Added Markdown/SQL blocks, collapsed reasoning, send/stop, model attribution, conversion notices and encrypted-save status/retry. Raw HTML and remote images are omitted.
- Added a dedicated active-editor command registry following the selected split-pane scope. Insert/replace use undoable Monaco edits and immediate tab synchronization; non-editable targets open a console. Run delegates to the existing console-navigation/query-guard pipeline. A changed connection/database/schema disables sends and SQL actions until a new chat is started.
- Added Rust-resolved provider availability and a shared frontend gate for editor, result, notebook, import and Visual Explain AI controls. Keychain/env credentials stay in Rust; custom HTTP(S) OpenAI endpoints may be keyless. Ollama availability uses a bounded loopback probe and reports offline/no-model reasons.
- Added grouped/searchable model selection, capability badges and a separate last-chat-model preference. Global model settings are unchanged by chat selection. Added toolbar/palette entry points, configurable `Mod+Shift+I` and translations in all 11 locales.
- Validation: complete Vitest suite 6,159 passed (385 files); complete Cargo suite 1,839 passed / 221 ignored, run serially. Application/test typechecks, source ESLint, production build, theme-token checks and scoped rustfmt pass. Six new Rust availability regressions use disposable loopback fixtures; 29 additional frontend regressions cover the panel, model choices, availability, split-pane registry and gated shortcuts.
- Visually verified the Italian panel in Light, Dark and High Contrast with an in-memory browser fixture, including square corners, visible focus and SQL insertion. Removed the temporary fixture/server. No real provider, database, conversation store or OS keychain was used for UI verification.
- GitNexus pre-edit scope is LOW for sidebar/layout/editor and the new availability/model modules. The existing palette bridge was HIGH and left unchanged; Visual Explain was CRITICAL, with changes limited to AI-control availability. Native/provider smoke checks remain pending. Next: M5 history and resume.
- Refreshed the GitNexus index with PDG retained. Final `detect_changes` includes new files through a disposable index: 104 files, 361 symbols, 20 affected flows (CRITICAL across the combined M3/M4 work). All reported flow anchors are the existing `tool_run_query` or Tauri startup `run`; the latter only adds command registration. The real staging area is unchanged.

### 2026-10-15: M4 chat state and event lifecycle completed
- Added snake_case TypeScript mirrors of M1 messages/blocks/events/requests/capabilities and M2 conversations/summaries/save results.
- Added a reducer-backed `AiChatProvider` / `useAiChat` session with an injectable Tauri transport. Transcripts and drafts survive panel consumer remounts; messages retain their producing provider/model while the next turn can use another model with the full history.
- Subscribe before invoking with a frontend UUID. Duplicate sends are blocked, cancellation handles pending subscriptions/invokes, terminal events save once through the encrypted M2 command and stale callbacks cannot affect another conversation. Tool events are ignored and no tools are advertised until M9.
- Separate save status/result and retry preserve the transcript on storage errors. Reset/delete-all notifications clear the session and cancel active work; subscriptions are released on switch/unmount, including StrictMode replay.
- Validation: 40 new frontend regressions pass; complete Vitest suite 6,130 passed (379 files). Application/test TypeScript checks, scoped ESLint and theme-token checks pass. Pre-integration GitNexus impact is LOW for all four sidebar/layout/editor targets (0–1 direct callers each, one indexed editor flow).
- Refreshed the GitNexus index with PDG retained. `detect_changes --scope all` includes new files through a disposable Git index: 39 files, 216 symbols, 11 affected flows (HIGH, including the existing M3 changes; no additional affected flows). The real staging area is unchanged.
- The provider is not mounted in the app yet. Next: sidebar rendering, SQL actions/command bridge, shared AI availability, provider/model picker, toolbar/palette/shortcut and translations. Native/provider smoke checks from M1–M3 remain pending.

### 2026-10-15: M3 typed tool registry completed — ready for review
- Added the `Tool` trait, Rust-resolved connection context, typed errors/output and M1-compatible tool definitions. Ask/read-only contexts omit write tools; model arguments cannot override the connection/database/schema.
- MCP and chat now share driver operations and `FileQueueGate`. MCP retains its five tools, JSON/TOON formats, schemas, approval settings and row defaults. The typed registry adds `sample_rows`, non-ANALYZE `explain_query` and approval-only `run_statement`; no chat execution command is exposed yet.
- Added Unicode-safe result caps: 12k characters, 200-character cells, 50 rows by default and a 1000-row hard cap. Head/tail row previews remain valid bounded JSON; omitted driver data is reported as truncated.
- Chat writes use fresh request IDs and revalidate approved edits. Approval targets carry an optional schema, displayed by the existing modal without changing legacy MCP payloads. Inline `InAppGate` and the agent loop remain M9 work.
- Validation: 24 new Rust regressions pass; full Cargo command passes serially (1,833 passed / 221 ignored). The parallel run encountered the known theme-operation contention; that test passes alone and in the serial suite. Frontend approval utilities: 55 passed. Production cargo check, typecheck, scoped ESLint, theme-token lint, scoped rustfmt and diff checks pass.
- GitNexus `detect_changes --scope all` includes the new files and confirms the expected tools/approval scope (28 files, 145 indexed symbols, 11 affected flows; HIGH). Tests use disposable databases and directory-scoped in-memory keychains. Native MCP/GUI approval smoke validation remains pending. Next: M4 chat state, sidebar and provider/model selection.

### 2026-10-15: documentation privacy policy
- Keep detailed review material outside version control; public milestone logs contain implementation progress and validation only.
- Application source and milestone implementation state are unchanged.

### 2026-10-15: M3 shared SQL classification and safety policy completed
- Validation: complete Cargo command 1,809 passed / 221 ignored; production cargo check passes with the existing sandbox warning. Scoped formatting and diff checks pass.
- Extracted the existing SQL classifier/scanner to `ai_tools/safety.rs`; all 22 original function bodies are unchanged except whitespace. Re-exports preserve the existing `ai_activity` API used by audit, SQLite/PostgreSQL protections and tests.
- MCP now uses shared read-only/approval policy helpers for both original and edited queries, preserving existing mode defaults and wire behaviour.
- Added strict policy functions for upcoming chat tools: one statement under both escaping interpretations, bounded SQL input, typed read/write/DDL/transaction/unknown kinds, read-only enforcement and rejection of transaction/unknown execution.
- Seven new regression tests cover CTE mutations, SELECT INTO/file writes, quoted side-effect functions, EXPLAIN ANALYZE, PRAGMA mutations, ambiguous escaping, transaction control, size limits and equivalent MCP policy behaviour.
- Pre-edit scanner impact is HIGH (9 direct callers and 3 affected processes); preserved all old paths. The stricter policy is not yet exposed by a chat execution command: registry/gate and SQLite fixtures remain next.

### 2026-10-15: M3 existing MCP tools extracted
- Moved the five existing tool handlers into `ai_tools/tools/`, with shared connection resolution, discovery, dispatch, audit, output, cooldown and preflight modules. MCP keeps transport/resources and re-exports its existing public preflight path.
- Compared all 28 original top-level function bodies: unchanged except whitespace. JSON/TOON schemas, errors, limits, database overrides, approval/editing behaviour and audit remain unchanged; no new tool or write access is exposed.
- Pre-edit impact: HIGH for dispatch, connection resolution, schema defaults and audit; CRITICAL for output encoding (with over-approximated references to other `encode` methods). Reviewed direct callers and preserved their interfaces.
- Validation: 22 MCP tests pass; complete Cargo command 1,802 passed / 221 ignored, including the moved output/cooldown tests. Scoped formatting and diff checks pass.
- Next: shared statement classification/read-only policy, bounded results, typed registry and approval gate, followed by SQLite fixture coverage.

### 2026-10-15: provider HTTP/TLS validation completed
- Updated provider transport dependencies before starting M3.
- Updated five resolved TLS/HTTP packages, with a patched minimum for the direct `rustls` dependency. Kept unrelated Windows dependency edges intact and verified the locked Windows graph resolves.
- Three disposable local TLS regression tests pass, checking untrusted CAs, trusted matching hosts and hostname mismatch. Full Cargo command: 1,802 passed / 221 ignored, including 1,798 library tests passed / 11 ignored. No external providers or OS trust/keychain entries used.
- Production cargo check passes. Shared HTTP-client upstream risk is CRITICAL for 0.13 (3 direct callers, 26 symbols across AI/updater/plugins) and LOW for 0.12; production client implementations are unchanged.
- Native keychain/platform checks remain pending. Next: M3 shared tools.

### 2026-10-15: M2 validation and private MCP diagnostics completed
- Added a metadata-only MCP logger: dependency/driver diagnostics never format message arguments or dynamic targets, preventing debug SQL/parameter leakage. Installation failure aborts MCP startup instead of falling back to ordinary logging.
- Three logger regressions pass, including a sensitive argument whose formatting would panic. Final full Rust suite: 1,795 passed / 11 ignored; full frontend suite: 6,089 passed. Production cargo check, typecheck, scoped ESLint, theme-token lint and scoped formatting pass.
- Detailed review notes are maintained privately outside the repository.
- M2 implementation is ready for review on the common integration branch. Signed/dev macOS MCP keychain access and Windows/Linux permissions/keychain checks remain unchecked; no real user key/history was migrated or reset during development. Next implementation: M3 shared tools and safety policy.

### 2026-10-15: M2 confirmed recovery controls and path guards completed
- Added Settings > AI deletion/reset actions with separate explicit confirmations, busy/error handling, and translations in all 11 locales. Reset requires backend confirmation, cancels active chat runs, clears AI artifacts/key/cache and preserves the watched approval directory.
- Added managed-path preflight and symlink guards for activity/approval writes, migration and reset. Named encrypted staging files are identifiable and removed by recovery after crashes.
- The global keychain account is now protected by a lock in the default config directory even if processes use different active storage roots.
- GC and overwrite operations stop on unknown future blocks; a late save validates the current connection under the AI storage lock so deleted connections' histories cannot be recreated.
- Validation: full Rust suite 1,792 passed / 11 ignored; full frontend suite 6,089 passed; typecheck, scoped ESLint and theme-token lint pass. Production cargo check passes (one pre-existing sandbox dead-code warning).
- Added regression tests for symlink redirection, reset preflight/staging cleanup, unknown-block retention, delayed saves and cancellation, plus four confirmation UI tests.
- Native signed/dev and other-platform keychain verification is still pending; no real user key or history was reset.

### 2026-10-15: M2 startup migration completed
- GUI migration runs on the blocking pool before starting approval consumers/GC; standalone MCP migrates and verifies key availability before handling requests.
- Legacy activity/rotated logs, session state and approvals are converted in place, with encrypted staging files read back and verified before replacing originals. Mixed logs preserve existing sealed lines; malformed legacy lines are retained encrypted and skipped by readers.
- Sealed JSON files are recognized and verified; repeated migration makes no changes. Denied keys, malformed JSON and symlinks fail without replacing those original files.
- Five migration tests pass, including idempotence, no plaintext markers, mixed logs, keychain denial and symlink rejection. No real user configuration was migrated during development.
- Next: confirmed Settings actions, then a final security/concurrency review and full validation. Native signed/dev, Windows and Linux keychain/ACL checks remain pending.

### 2026-10-15: M2 existing AI artifact I/O encrypted
- Activity entries are individually sealed; session state and pending/decision payloads use authenticated atomic files. Filenames and watcher/polling protocol are preserved.
- Approval AAD binds both request ID and pending/decision phase; ID/decision validation and corruption tests prevent copied or tampered files from authorizing a request.
- Activity reads skip/count corrupted lines without logging their contents. Rotation and clear operations share the cross-process lock with reset/migration.
- Removed complete MCP request/response logging and content-bearing JSON parse errors from stderr.
- Validation: 133 existing AI-related tests and six new artifact/security tests pass; the additional activity/rotation regression run passes 76 tests.
- Next: verified, idempotent startup migration before GUI/MCP consumers, followed by Settings controls and final security review.

### 2026-10-15: M2 connection deletion hooks completed
- Single-connection and group/subgroup deletion now remove every affected connection's encrypted chat directory after connection persistence succeeds.
- Cleanup runs on the blocking pool and reports failures; other connections and shared blobs remain intact (orphan blobs are collected conservatively on startup).
- GitNexus impact is LOW for both Tauri entry points. Ten conversation-store regression tests pass; reviewed the group-subtree ID capture before connections are removed.
- Next: encrypted activity/session/approval I/O and startup migration.

### 2026-10-15: M2 encrypted blobs and GC completed
- Added content-addressed encrypted blobs (MIME and bytes sealed together), put/get commands, SHA-256 integrity checks, deduplication and a 20 MiB limit.
- Added debounced startup GC. Referenced image/attachment blobs are retained; uploads younger than one hour are protected; unreadable conversations abort collection without deleting anything.
- Five blob tests pass: round trips/deduplication, no plaintext payload or MIME, invalid input/size guards, copied/tampered files and conservative collection.
- Next: connection/group deletion hooks, then seal the existing activity/session/approval files and migrate them.

### 2026-10-15: M2 encrypted conversations completed
- Added versioned conversations, Ask/Agent mode, editor/notebook targets, summaries and Tauri list/load/save/rename/delete/delete-all commands.
- Each command addressing a conversation requires its connection ID as well as its conversation ID; equal IDs on different connections remain isolated.
- Encrypted indexes recover from missing/corrupt/stale state. Unreadable or newer-version conversations cannot be silently overwritten. Saves report a warning above 2 MiB and reject payloads above 16 MiB.
- Added path traversal/reserved-name/symlink guards and private directories. Cross-process transactions serialize index updates.
- Ten conversation tests pass, covering storage, index recovery, corruption, unsupported versions, concurrency, size guards and encryption of titles/messages/indexes.
- Next: encrypted blob storage and conservative garbage collection.

### 2026-10-15: M2 master key lifecycle completed
- Added `get/set/delete_ai_store_key` helpers without secret logging, a zeroized in-memory key cache and the public seal/open/file/line APIs.
- Serialized encryption transactions and key creation across threads and processes with an OS file lock. A non-secret key fingerprint detects resets in other processes and invalidates cached keys.
- Missing/denied/malformed keys never fall back to plaintext. Existing sealed artifacts or an initialized-store marker prevent silent replacement of a lost key.
- Added explicit directory-scoped in-memory keychain fixtures for tests; tests never access the real OS keychain. Eighteen crypto/lifecycle tests pass.
- Kept one integration branch after discussing the branch strategy: the M2 foundation commit `af68ce0` is also pushed on `feat/ai-assistant-provider-layer`; the separate M2 branch is no longer used.
- Pre-migration impact: approval writing CRITICAL (14 direct callers), activity writing/reading and approval decision reading HIGH. Preserve the protocol and fail closed on authentication errors.
- Next: encrypted conversations, index recovery and commands.

### 2026-10-15: M2 encryption primitives completed
- Started `feat/ai-assistant-secure-store` on top of M1; unrelated local files remain untouched.
- Added AES-256-GCM envelopes with fresh 96-bit nonces and authenticated caller context, ciphertext-only atomic file replacement (0600 on Unix), line encoding and zeroized key buffers.
- The envelope adds a SHA-256 `ctx` digest of AAD (never the context itself) so misplaced files produce `WrongContext`; AES-GCM still authenticates the complete AAD. Tampering with the digest cannot bypass authentication.
- Nine focused tests pass: binary/Unicode/empty round trips, nonce uniqueness, wrong AAD/key, tampering, malformed/versioned envelopes, line encoding, permissions and failed atomic replacement.
- Next: keychain lifecycle, cross-process locking, missing-key tests and the public storage API. No user data or real keychain entries were changed.

### 2026-10-15: M1 cancellable runs completed — ready for review
- Registered `RunManager`, `ai_chat_send`, `ai_chat_cancel` and model-capability lookup in Tauri. Runs clean up on completion/drop, have independent cancellation tokens and emit exactly one terminal event.
- Added 50 ms ordered text/reasoning coalescing with immediate metadata/error flushing. Frontends can subscribe using a canonical UUID before starting a run.
- Validation: 41 focused tests; full Rust suite 1,737 passed / 11 ignored; Vitest 6,085 passed; typecheck passes; theme-token check passes; scoped rustfmt passes.
- `pnpm lint` traverses pre-existing `.claude/worktrees` copies and reports 517 errors there. ESLint passes with `--ignore-pattern '.claude/worktrees/**'`; no unrelated worktree/config changes made.
- GitNexus's inconsistent FTS index was rebuilt. `detect_changes --scope staged` was run before every commit; source changes stay within AI modules and Tauri registration. Shared `run` names produce over-approximated execution-flow matches; unrelated command implementations are unchanged.
- Commits are pushed per completed step without co-author trailers: `f09d87d` (split + plan), `f1be9cd` (types/parser), `c4fde04` (adapters/legacy), followed by this run-lifecycle step.
- No live cloud calls or native UI smoke checks were performed. M1 stays **in review** until those checks are done. Next implementation: M2, starting with the master key/envelope and crypto tests.

### 2026-10-15: M1 streaming adapters completed
- Implemented OpenAI-compatible, Anthropic and Ollama streaming for text, reasoning, usage and tool calls, including fragmented tool arguments and typed/sanitized errors.
- Legacy commands now collect streamed text and retain code-fence cleanup, proxy settings, local Ollama proxy bypass and MiniMax region failover.
- Added cross-provider history mapping, stable provider-safe tool IDs, image serialization and explicit notices for omitted/flattened blocks. Blob resolution remains internal to Rust, ready for M2/M10.
- Added YAML capabilities and successful-only cached Ollama `/api/show` probes; failed probes can be retried.
- Validation: 34 focused AI tests, including local HTTP fixtures for all protocols, cancellation, errors and region failover. No live cloud requests made.
- Next: cancellable run state and Tauri event coalescing.

### 2026-10-15: M1 messages and framing completed
- Added provider-neutral messages, blocks, usage, requests, capabilities and typed errors.
- Locked snake_case field names with a `type` discriminator for blocks/events; unknown block types are skipped without logging their content.
- Added bounded SSE/NDJSON framing that preserves UTF-8 across arbitrary chunk boundaries and handles CR/LF, comments and multiline data.
- Added optional YAML model capabilities while preserving the model-list API, and the cancellation-token dependency.
- Focused protocol/type tests and legacy tests pass; next: streaming adapters and legacy collection.

### 2026-10-15: M1 resumed — module split verified
- Continued the existing work on `feat/ai-assistant-provider-layer`; preserved unrelated local changes.
- Verified the behaviour-preserving `ai.rs` split: all four existing AI unit tests pass.
- Refreshed GitNexus using the installed CLI directly with the local OpenSSL 3 library path. The runner through pnpm strips the library environment.
- Impact analysis: dispatcher HIGH (5 direct callers, 11 affected symbols); model catalog HIGH (3 direct callers, model discovery flow). Public command signatures stay unchanged.
- Verified and committed the module split; next: provider-neutral wire types and parsers.
- The assistant plan is now tracked explicitly despite the repository-wide `/docs` ignore rule. Future steps update this log in their own commits; no co-author trailers.

### 2026-10-15: M1 implementation started
- Read the plan and repository rules; started with the first milestone.
- GitNexus's native module needs OpenSSL 3, removed from Homebrew on this machine. Running its CLI directly with the compatible local toolchain libraries works; refreshing the stale index before impact analysis.
- Baseline Rust tests need access to the shared build cache outside the workspace.
- Next: impact analysis, behaviour-preserving module split, streaming adapters and run lifecycle tests.

### 2026-10-15: availability and provider switch
- All AI surfaces gated on `useAiAvailability()` (AI enabled + at least one usable provider). Provider/model switchable in the chat per conversation, mid-conversation included (M4, M1).

### 2026-10-15: notebooks
- Added M8 (notebook-aware assistant) and renumbered: Agent mode is now M9, Multimodal M10, Extras M11.

### 2026-10-15: planning
- Reviewed the current AI integration (single-shot modals, no streaming/tools, plaintext AI logs) and the existing infrastructure (right sidebar, MCP tools and approvals, keychain, `aes-gcm`).
- Locked the decisions listed in README.md, including encryption at rest for conversations **and** AI logs/approvals.
- Wrote milestones M1–M11.
- Next: pick the first milestone and create its branch.
