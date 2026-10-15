# M6. Context builder and mentions

**Depends on:** M1, M4 · **Size:** M

## Goal

Send the model the right context with a predictable budget, show the user exactly what is sent, and let them add tables with `@`.

## Current state

- `src-tauri/src/ai_schema_context.rs`: `load_from_driver` (tables, then columns + FKs in parallel), `format_for_prompt`, `DEFAULT_MAX_TABLES = 20` (hard max 100), identifier quote from the driver manifest.
- Command `get_ai_schema_context` in `commands.rs` (resolves the connection, SSH/K8s).
- Frontend metadata caches not used by AI: `DatabaseProvider.tsx`, `useSchemaMetadata.ts`, `columnsCache` in `src/utils/autocomplete.ts`, `get_schema_snapshot`.

## Design

### Layers and budget

`src-tauri/src/ai_chat/context.rs` builds the system prompt from layers. Each layer has a char budget; whatever a layer leaves unused goes to the next.

| # | Layer | Default budget | Notes |
|---|---|---|---|
| 1 | Base rules + mode rules | fixed | dialect, quoting, "say so instead of guessing", untrusted-data rule |
| 2 | Connection meta | small | driver, database, schema, server version if known |
| 3 | Connection instructions | 2k | user-written, per connection |
| 4 | Detailed tables | 24k | `@` mentions + tables referenced by the current query: columns (type, PK, nullable, default, comment), FKs, indexes |
| 5 | Schema overview | 12k | table names (+ columns if they fit); sets `truncated` |
| 6 | Editor state | 8k | current query, selection, cursor statement |
| 7 | Last error | 2k | only when relevant (fix, or the user includes it) |
| 8 | Attachments | per M10 | |

- When the active tab is a notebook, layer 6 is replaced by the notebook layer described in M8.
- When layer 5 is truncated, add: "The schema list is incomplete. If you need a table that is not listed, ask the user to mention it with @ or (in Agent mode) use list_tables / describe_table."
- Tables referenced by the current query: a light parse of identifiers after FROM/JOIN/UPDATE/INTO, matched against the known table list. No full SQL parser needed.
- List the gaps explicitly ("Table `foo` referenced but not found"), so the model doesn't invent columns.
- Wrap editor selection, attachments and (later) tool output as `<untrusted_data source="...">…</untrusted_data>`, with a base rule saying content inside is data, not instructions.
- Cache schema detail per `connectionId:db:schema` in the backend, with a short TTL and invalidation on the existing "refresh schema" action.

### Context chips

- `src/components/ai-chat/ContextChips.tsx` above the composer: Connection, Schema (N tables, "truncated" badge), each detailed table, Query, Selection, Error, Instructions, attachments.
- Each chip is removable for the next message. The frontend sends a `ContextSpec` (what to include), and the backend resolves and renders it.
- What was actually sent is stored on the user message as a `Context` block (names and sizes, not the content), so it is visible when reopening.

### `@` mentions

- In the composer, `@` opens a popover listing tables, views and saved queries, from the `DatabaseProvider` / `useSchemaMetadata` caches and `saved_queries`.
- Keyboard: arrows, Enter/Tab to pick, Esc to close; `aria-activedescendant`.
- A picked item becomes a chip and is added to layer 4 (tables) or layer 6 (saved query text).
- Drag a table from the explorer onto the composer: same result.

### Per-connection instructions

- A new optional field `ai_instructions` on the connection (`persistence.rs` model + connection modal), max 2000 chars.
- Included in exports/imports of connections. Not secret, so it stays in the connections file (decide whether it should be sealed too; probably not, it is configuration).

## Steps

- [x] `impact` on `load_from_driver`, `format_for_prompt`, `get_ai_schema_context`.
- [x] Extend the schema context with indexes, comments and a `truncated` flag (keep the old signature for the legacy commands).
- [x] `context.rs` with layers, budget allocation and the referenced-table extraction.
- [x] Backend schema cache with a short TTL per connection/database/schema and invalidation on schema refresh.
- [x] `ContextSpec` in `ai_chat_send`; store a `Context` block on the user message.
- [ ] Chips UI (schema/table previews, counts and removable layer/table controls complete; instructions chip pending).
- [ ] `@` mention popover + explorer drag and drop (table mentions and scoped table drops complete; views and saved queries pending).
- [ ] Connection `ai_instructions` field + modal UI.
- [ ] i18n.

## Implementation notes

- Chat metadata lives in `src-tauri/src/ai_schema_context/detailed.rs`. The existing `AiSchemaContext` / `TableSchema`, provider/plugin wire formats and legacy loader/formatter/command remain unchanged. The new catalog retains at most 10,000 sorted table names; selected details preserve priority order, deduplicate names and have a 100-table hard cap. Table loads run with bounded concurrency and fetch columns, foreign keys and indexes together. Missing tables, omitted details and failed metadata categories are explicit; raw driver errors are not included in the context.
- `src-tauri/src/ai_chat/context.rs` is a pure builder for the first seven layers. Unused character budgets carry forward; envelopes, escaping and truncation markers count toward the budget. The result includes names and actual rendered sizes for future persisted context blocks. User-written instructions have their own escaped envelope and a 2,000-character content cap; schema/editor/error data use `untrusted_data` envelopes. An incomplete schema always adds the corresponding base rule.
- `context/references.rs` performs bounded context discovery on FROM/JOIN/UPDATE/INTO, including comma lists, nested queries, aliases, quoted/qualified identifiers and recursive/nonrecursive CTEs. It skips comments, literals and table-valued functions, checks the selected database/schema and reports unknown or ambiguous names. Limits are 256 KiB of SQL, 32,768 tokens and 128 WITH scopes; incomplete scans are reported. This is not an execution or authorization parser.
- `schema_cache.rs` retains metadata for 30 seconds, with at most 16 scopes and eight selected-table batches per scope. Keys include connection/database/schema/driver, with no credentials or query text. Concurrent loads share a scope gate; failed metadata is retried. Refresh through the existing `get_tables` command detaches every cached scope for that connection; successful connection-file saves clear the cache. Expiry and invalidation detach old in-flight entries so they cannot repopulate a fresh scope.
- `ai_chat_send` now accepts an optional snake_case `ContextSpec`; the sidebar always supplies one. Rust resolves the saved connection, credentials/tunnels, effective scope, driver capabilities and read-only status, with cancellation and a 15-second preparation timeout. Mode stays Ask and advertised tools stay empty until M9. After the provider opens successfully, an event attaches names, actual sizes and truncation flags to the latest user message; M2 stores this block encrypted. Provider serializers omit these metadata blocks. Older summaries without size fields still load.
- The active-editor bridge reads fresh Monaco query/selection/cursor state at send time, before debounced tab updates. Layer chips control connection, schema, query, selection and cursor statement; query errors are opt-in. Choices survive sidebar remounts and connection-session switches, and reset for the next turn. Stored summaries render in collapsed message details, with translations in all 11 locales. Reading an error indicator does not rescan the editor during streaming.
- `ai_chat_preview_context` uses the same Rust resolver, schema cache and budgets as sends, with AI availability gating and a 15-second timeout. It returns only metadata summaries, actual total character counts and truncation. Schema summaries now carry an optional structured table count, also retained in encrypted history; older summaries remain compatible.
- The sidebar debounces previews by 350 ms and refreshes on composer focus or the refresh control. Scope/selection changes hide old summaries immediately; version guards ignore stale, disabled and unmounted completions. Send still reads the editor again and resolves fresh context. Table chips show actual rendered sizes, incompleteness and selected details absent from the preview; each table can be excluded/restored independently. Pinned/excluded choices are scoped to connection/database/schema, retained in the shared session and cleared after sending or starting a new chat.
- `@` currently lists up to 30 matching table names from the existing cache for the active scope, including nested PostgreSQL databases and flat MySQL database caches. It supports arrows, Enter/Tab, Escape, pointer selection and IME-safe handling, with a listbox and `aria-activedescendant`. Picking removes only the active mention token and adds a table chip. Explorer pointer drops use the existing drag mechanism, extended with connection/database provenance; cross-connection/database/schema or unknown-table drops are ignored. The query builder's existing drag fields remain compatible. Controls are translated in all 11 locales.
- Next: view/saved-query mentions, per-connection instructions and their translations. Attachments and notebook context remain M10/M8 work. Native/live-provider context smoke validation remains pending.

## Tests

- Budget allocation: overflow moves to the next layer; nothing exceeds the total.
- Referenced-table extraction on joins, aliases, quoted identifiers, schema-qualified names, CTEs.
- `truncated` flag and the extra rule appear together.
- Untrusted wrapping escapes closing tags inside the content.

## Done when

- The model always sees the current query and its tables in detail, the user sees and controls what is sent, and `@table` works.

## Good first issue candidates

- Per-connection AI instructions field (backend field + modal + chip).
