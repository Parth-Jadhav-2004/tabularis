# AI Assistant: implementation plan

Goal: replace the two one-shot AI modals in the query editor (Generate SQL, Explain) with a conversational assistant in the right sidebar. It uses the whole connection as context, keeps encrypted conversations you can resume, can read the database through tools, and can propose writes that need explicit approval.

Resume a session from [PROGRESS.md](./PROGRESS.md).

## Decisions (locked)

| Topic | Decision |
|---|---|
| Existing modals | Folded into the sidebar as `/generate` and `/explain`, then deleted (notebook cells included). |
| DB interaction | **Ask** mode: read tools only. **Agent** mode: write tools as well, every write approved once for that exact SQL and target. Never on read-only connections. |
| Multimodal | Image input, text/CSV/SQL file attachments, rich output (charts, Mermaid/ER). Result sets are not attachments; they reach the model through tools. |
| Storage | One file per conversation, scoped per connection, under `<config>/ai_chats/<connectionId>/`. No app-level SQLite. |
| Encryption | Everything AI-related on disk is encrypted at rest: conversations, index/titles, blobs, the `ai_activity` log, the approval queue, the MCP session state. AES-256-GCM, random master key in the OS keychain. Never a plaintext fallback. |
| Scope | Conversations belong to a connection (plus database/schema); the list has an "All connections" filter. |
| Extras | Diff walkthrough with safe apply, opt-in inline completions, plugin slots. |
| Notebooks | When a notebook tab is active the assistant works on the whole notebook (outline, params, focused and mentioned cells, errors; results only on request). Answers become cells; multi-cell proposals are reviewed before being applied. Conversations can be bound to a notebook. |
| AI availability | Every AI surface is shown only when AI is enabled **and** at least one provider is usable (key in keychain/env, or a configured keyless provider like Ollama). One `useAiAvailability()` hook drives all of them. |
| Provider/model choice | Switchable directly in the chat, per conversation, among all usable providers; mid-conversation switches allowed. The global setting stays the default and keeps driving the other AI features. |
| Who builds it | The maintainer. Small good first issues may be carved out later (candidates are listed in each milestone). |

## Open questions

- Key loss (keychain reset) makes history unreadable. Accept it, or add an optional password-protected backup through `export_crypto`?
- Full-text search across message bodies (needs decrypting every file, or an encrypted search index). For now, search titles only.

## Architecture

```
React  src/components/ai-chat/*
  AiChatPanel ── useAiChat ── invoke("ai_chat_send") / listen("ai-chat://<runId>")
                                   │
Rust   src-tauri/src/ai_chat/*     ▼
  run manager ── context builder ── providers (stream + tools)   [src-tauri/src/ai/]
       │                                   │
       ├── tool registry (shared with MCP) ┘                     [src-tauri/src/ai_tools/]
       ├── approval gate (shared with MCP)                       [ai_approval.rs]
       └── encrypted conversation store                          [src-tauri/src/secure_store/ + ai_chat/store.rs]
```

## Milestones

Order matters. Each milestone should be shippable on its own, behind the AI availability check (`useAiAvailability`, see M4), and the old modals keep working until M7.

| # | Milestone | Depends on | Size |
|---|---|---|---|
| M1 | [Provider layer: messages, streaming, tool calling](./M1-provider-layer.md) | none | L |
| M2 | [Secure store: encrypted conversations and AI logs](./M2-secure-store.md) | none | M |
| M3 | [Shared tool registry](./M3-tool-registry.md) | none | M |
| M4 | [Chat panel in the right sidebar](./M4-chat-panel.md) | M1, M2 | L |
| M5 | [Conversation history](./M5-conversations.md) | M2, M4 | M |
| M6 | [Context builder and mentions](./M6-context.md) | M1, M4 | M |
| M7 | [Editor integration and modal migration](./M7-editor-integration.md) | M4, M6 | M |
| M8 | [Notebook-aware assistant](./M8-notebooks.md) | M4, M6, M7 | M |
| M9 | [Agent mode: tool loop and approvals](./M9-agent-mode.md) | M1, M3, M4 | L |
| M10 | [Multimodal input and rich output](./M10-multimodal.md) | M2, M4, M9 | M |
| M11 | [Extras: diff walkthrough, inline completions, plugin slots](./M11-extras.md) | M7, M3 | L |

M1, M2 and M3 are independent and can be done in any order. A sensible first release is **M1 to M8** (chat with context, history, editor and notebook integration, modals removed). Agent mode (M9) is the second release.

## Cross-cutting rules (apply to every milestone)

- Follow `.rules/` (Rust module layout and sibling `tests.rs`, React rules, design tokens, accessibility, testing).
- Every UI string goes into every locale under `src/i18n/locales/`.
- Theme tokens only; `pnpm lint:theme` must pass.
- Run GitNexus `impact` before editing existing symbols and `detect_changes` before committing (see `AGENTS.md`).
- Before a PR: `pnpm lint`, `pnpm typecheck`, `pnpm test`, `cargo test` (known machine-specific failures aside).
- Nothing from a conversation is written to application logs.
- Selections, attachments and tool results are wrapped as untrusted data in prompts.

## Good first issue candidates (to define later)

Collected from the milestones; open them only once the code they build on has landed.

- M5: AI-generated conversation titles
- M5: export a conversation as Markdown / notebook
- M4: provider/model picker polish (badges, search, setup entries)
- M6: per-connection AI instructions field
- M7: "Fix with AI" button on query errors
- M7: "Ask AI" entries in the editor and explorer context menus
- M8: clickable `#cell` references in answers
- M8: `/document` notebook command
- M10: Mermaid rendering (once the block renderer registry exists)
