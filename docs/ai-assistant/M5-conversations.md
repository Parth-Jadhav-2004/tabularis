# M5. Conversation history

**Depends on:** M2, M4 · **Size:** M

## Goal

Browse, resume, rename, delete, search and export conversations, scoped per connection.

## Design

### History view

- `src/components/ai-chat/ConversationList.tsx`, opened from the panel header, replacing the transcript while open.
- Data: `ai_chat_list(connectionId)`. The "All connections" filter calls `ai_chat_list(all)` and groups by connection name.
- When a notebook tab is active, a "This notebook" filter is selected by default (see M8).
- Sorted by `updated_at`, grouped by Today / Yesterday / Last 7 days / Older.
- Row: title, relative time, message count; actions rename (inline), export, delete (confirm).
- Search: client-side filter on decrypted titles. Body search is out of scope for now (see the open question in README).
- Remember the last open conversation per connection (an encrypted preference, or simply the most recent one).
- Switching the active connection switches the list and opens that connection's last conversation, or an empty one.
- Resuming restores mode, provider and model; if that model is no longer available, fall back to the default and say so.
- Error state: `KeyUnavailable` / `Corrupted` → message + "Reset AI history" (links to the M2 reset).

### Titles

- On creation: the first user message, cut to 50 chars (`title_source: FirstMessage`).
- After the first assistant reply, generate a title (max ~6 words) unless the user has renamed it. Follow the `generate_tab_rename` / `generate_cell_name` pattern: default prompt in `config.rs`, override file `prompt_chattitle.txt`, editor in Settings > AI next to the others.
- Use the conversation's provider/model, or a cheaper "utility model" setting if you add one.
- Failures are silent (keep the truncated title).

### Export

- **Markdown:** title, connection, date, then the messages; SQL as fenced blocks; tool calls as collapsed sections; images by filename. Save through the dialog plugin. The dialog must say the file is **not encrypted**.
- **Notebook:** SQL blocks become SQL cells, text becomes markdown cells. Reuse `src-tauri/src/ai_notebook_export.rs`.

## Steps

- [x] List UI + date grouping + connection/All filters + title search. Notebook filtering belongs to M8.
- [x] Resume / inline rename / confirmed delete wiring and connection-switch behaviour, with model fallback notices.
- [x] Title generation command + prompt + settings editor.
- [x] Markdown export + notebook export.
- [x] Error and empty states, retry and a link to the existing Settings recovery controls.
- [x] History, title prompt and export UI translations in all 11 locales.
- [ ] Native encrypted-history smoke check: restart, connection switching, resume, rename and delete; live title generation and Markdown/notebook save/import.

### Implementation notes

- The shared session uses the existing M2 list/load/rename/delete commands; titles and messages are never cached in browser storage. A new session loads the most recently updated conversation per connection. Within the running app, each connection keeps its selected conversation and draft in memory.
- Automatic connection switching waits for an active turn and its encrypted save to finish. Failed saves remain available in the connection's in-memory session for retry. Database/schema changes retain M4's target-mismatch guard.
- Load/list requests are versioned so an earlier connection/filter, reset or unmount cannot replace the current state. Rename/delete are blocked during streaming, saving or a failed save, and always address both connection and conversation ID.
- Resuming preserves scope, target and mode. An unavailable model falls back to the global default among usable models, with a visible notice; producing models on previous messages are preserved. Initial titles use the first 50 Unicode code points.
- After the first successful reply is saved, a background utility request generates a title using that reply's provider/model. Only bounded text from the first exchange is sent, wrapped as untrusted JSON; no tools or attachments are sent. Titles are limited to six words and 80 Unicode code points. A 30-second timeout or failure silently retains the initial title. The dedicated prompt uses `prompt_chattitle.txt` and the existing Settings load/save/reset pattern.
- Generated titles are applied under the encrypted store lock only while the same conversation and first exchange still exist with `first_message` title source. Late results cannot overwrite manual/generated titles or recreate deleted history. Older transcript saves retain stored manual/generated titles. The frontend also guards reset/unmount completions and updates in-memory connection sessions without replacing messages or drafts.
- Each history row offers Markdown and notebook export through the native save dialog, with an explicit unencrypted-file notice. Markdown preserves message order and SQL fences, collapses tools/reasoning and references images/attachments by filename without exporting their blobs. Notebook export reuses `ai_notebook_export` types/naming, preserves prose order and turns only explicitly tagged SQL fences into SQL cells; no cells are executed and no results are embedded. The per-cell database override uses the conversation database, while schema remains in the metadata. Filenames are sanitized and bounded in both code points and UTF-8 bytes.

## Tests

- Grouping by date buckets (pure util).
- Connection/filter races, reset/unmount, failed-save retention and model fallback.
- History list and panel integration, including StrictMode, title-only search, inline rename and deletion confirmation.
- Title: first-reply model, bounded input/output, silent failure, save retry, connection cache, manual rename, stale saves and reset/delete/unmount races.
- Pure Markdown/notebook renderers: metadata, message order, SQL fence lengths/languages, nested non-SQL fences, tool collapse and image/attachment references.
- Export dialog cancellation, chosen format/path, decryption errors and translated UI errors; Settings prompt load/save/reset.

Validation on 2026-10-15: full Vitest suite 6,182 passed (389 files), including 23 new history regressions; 12 existing Rust conversation-store tests passed. Application and scoped test typechecks, production build, source ESLint and theme-token checks passed. Full repository ESLint passed when excluding local `.claude/worktrees/**` and `.pnpm-store/**`; unfiltered `pnpm lint` encountered 517 errors in old nested worktrees. The Italian history UI was visually checked with an in-memory fixture in Light, Dark and High Contrast at sidebar width, including square corners and inline rename. Native persistence/restart validation remains pending.

Title/export validation on 2026-10-15: full Vitest suite 6,198 passed (393 files), including 16 additional frontend regressions; full Cargo suite 1,846 passed / 221 ignored, run serially, including seven new title/store/export regressions. Application/scoped test typechecks, production build, production cargo check, scoped rustfmt, theme-token checks and repository ESLint with local worktrees/cache excluded passed. The Italian export controls and title prompt editor were visually checked in Light, Dark and High Contrast, with square corners, focus and cancellation, using an in-memory fixture. Native restart, live-provider title generation and actual file save/import remain pending. M5 implementation is ready for review.

## Done when

- Conversations survive restarts, are listed per connection, and can be resumed, renamed, deleted and exported.

## Good first issue candidates

- AI-generated conversation titles (once the store and the panel exist).
- Markdown and notebook export.
