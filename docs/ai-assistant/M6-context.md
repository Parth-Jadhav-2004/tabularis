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

- [ ] `impact` on `load_from_driver`, `format_for_prompt`, `get_ai_schema_context`.
- [ ] Extend the schema context with indexes, comments and a `truncated` flag (keep the old signature for the legacy commands).
- [ ] `context.rs` with layers, budget allocation and the referenced-table extraction.
- [ ] `ContextSpec` in `ai_chat_send`; store a `Context` block on the user message.
- [ ] Chips UI.
- [ ] `@` mention popover + explorer drag and drop.
- [ ] Connection `ai_instructions` field + modal UI.
- [ ] i18n.

## Tests

- Budget allocation: overflow moves to the next layer; nothing exceeds the total.
- Referenced-table extraction on joins, aliases, quoted identifiers, schema-qualified names, CTEs.
- `truncated` flag and the extra rule appear together.
- Untrusted wrapping escapes closing tags inside the content.

## Done when

- The model always sees the current query and its tables in detail, the user sees and controls what is sent, and `@table` works.

## Good first issue candidates

- Per-connection AI instructions field (backend field + modal + chip).
