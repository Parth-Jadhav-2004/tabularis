# AI Assistant: progress log

How to resume: read [README.md](./README.md) (decisions + milestone order), find the current milestone below, open its `M*.md`, continue from the first unchecked step. Update this file at the end of every session.

## Status

| Milestone | Status | Branch / PR | Notes |
|---|---|---|---|
| M1 Provider layer | in review | `feat/ai-assistant-provider-layer` | Implementation complete; automated checks pass. Live-provider/native smoke checks pending. |
| M2 Secure store | in progress | `feat/ai-assistant-provider-layer` | Encrypted conversations and command API verified; blobs next. |
| M3 Tool registry | not started | | |
| M4 Chat panel | not started | | |
| M5 Conversations | not started | | |
| M6 Context | not started | | |
| M7 Editor integration | not started | | |
| M8 Notebooks | not started | | |
| M9 Agent mode | not started | | |
| M10 Multimodal | not started | | |
| M11 Extras | not started | | |

Status values: `not started` · `in progress` · `in review` · `done` · `blocked`.

**Current focus:** M2, secure storage. M1 live-provider smoke checks remain pending in [M1-provider-layer.md](./M1-provider-layer.md).

## Open questions

- [ ] Key loss: accept unrecoverable history, or add a password-protected backup via `export_crypto`?
- [ ] Message body search (needs decrypting all files, or an encrypted index).
- [ ] Does the `--mcp` subprocess trigger a keychain prompt when it reads the AI store key on macOS? (M2)
- [x] Wire format: snake_case fields, snake_case enum values and a `type` discriminator. (M1/M4)
- [ ] Shortcut for toggling the AI panel (check `src/config/shortcuts.json`). (M4)

## Good first issues

Candidates are listed in README.md. Track opened ones here:

| Candidate | Milestone | Issue | Status |
|---|---|---|---|

## Session log

Newest first. Keep each entry short: what was done, what is next, anything surprising.

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
- Ran GitNexus PDG security explain on encryption, key lifecycle, approvals and storage paths: no findings reported; this does not prove safety (callback/property flows are not modeled). Manual source review continues.
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
