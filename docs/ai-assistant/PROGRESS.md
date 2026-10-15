# AI Assistant: progress log

How to resume: read [README.md](./README.md) (decisions + milestone order), find the current milestone below, open its `M*.md`, continue from the first unchecked step. Update this file at the end of every session.

## Status

| Milestone | Status | Branch / PR | Notes |
|---|---|---|---|
| M1 Provider layer | in progress | `feat/ai-assistant-provider-layer` | Provider-neutral messages, streaming adapters and cancellable runs. |
| M2 Secure store | not started | | |
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

**Current focus:** M1. Preserve the existing commands while introducing the provider-neutral streaming layer.

## Open questions

- [ ] Key loss: accept unrecoverable history, or add a password-protected backup via `export_crypto`?
- [ ] Message body search (needs decrypting all files, or an encrypted index).
- [ ] Does the `--mcp` subprocess trigger a keychain prompt when it reads the AI store key on macOS? (M2)
- [ ] Wire format casing between Rust and TS for `Message`/`Block` (snake_case vs camelCase). (M1/M4)
- [ ] Shortcut for toggling the AI panel (check `src/config/shortcuts.json`). (M4)

## Good first issues

Candidates are listed in README.md. Track opened ones here:

| Candidate | Milestone | Issue | Status |
|---|---|---|---|

## Session log

Newest first. Keep each entry short: what was done, what is next, anything surprising.

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
