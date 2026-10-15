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

- [ ] List UI + grouping + filter.
- [ ] Resume / rename / delete wiring and the connection-switch behaviour.
- [ ] Title generation command + prompt + settings editor.
- [ ] Markdown export + notebook export.
- [ ] Error and empty states.
- [ ] i18n.

## Tests

- Grouping by date buckets (pure util).
- Title: not regenerated after a manual rename.
- Export renderer snapshot (pure function from `Conversation` to markdown).

## Done when

- Conversations survive restarts, are listed per connection, and can be resumed, renamed, deleted and exported.

## Good first issue candidates

- AI-generated conversation titles (once the store and the panel exist).
- Markdown and notebook export.
