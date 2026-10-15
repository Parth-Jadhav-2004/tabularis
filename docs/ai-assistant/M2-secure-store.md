# M2. Secure store: encrypted conversations and AI logs

**Depends on:** none (uses the `Message` type from M1 for conversations, but the crypto layer is independent) · **Size:** M

## Goal

A single encryption layer for every AI artifact on disk, and the conversation store built on top of it. Nothing AI-related is ever written in plain text.

## Current state

- `aes-gcm = "0.10"` is already a dependency, used with a password-derived key in `src-tauri/src/export_crypto/mod.rs`.
- `src-tauri/src/keychain_utils.rs`: `SERVICE_NAME = "tabularis"`, entries like `<id>:db`, `<id>:ssh`; AI keys via `set_ai_key` / `get_ai_key`. Reads go through an in-memory credential cache to avoid repeated macOS prompts.
- `persistence::write_private_file(path, contents)` writes with mode 0600.
- Plaintext AI files today:
  - `<config>/ai_activity.jsonl` + `.1`–`.5` (`ai_activity.rs`: `append_event_in`, `rotate_if_needed_in`, `read_events_in`, `read_sessions_in`, `clear_in`)
  - `<config>/.mcp_session_state.json` (`load_session_state_in`, `save_session_state_in`, client hint)
  - `<config>/pending_approvals/{id}.pending.json` / `{id}.decision.json` (`ai_approval.rs`, `ai_approval_watcher.rs`)
- The MCP server runs as a separate process of the same binary (`--mcp`, `lib.rs`).

## Design

### `src-tauri/src/secure_store/`

```
secure_store/
  mod.rs        # seal/open API
  key.rs        # master key lifecycle (keychain + cache)
  envelope.rs   # serialized format
  tests.rs
```

- **Master key:** 32 random bytes (`OsRng`), base64 in the keychain under `SERVICE_NAME`, account `ai_store_key`. Generated on first use. Cached in memory (`OnceLock` / the same cache pattern as other credentials). Zeroize on drop if practical.
- **Envelope:** `{ "v": 1, "alg": "A256GCM", "ctx": "<b64 SHA-256 of AAD>", "nonce": "<b64 12 bytes>", "ct": "<b64>" }`. A fresh random nonce per seal. The context digest distinguishes misplaced files from ciphertext/key authentication failures; the full AAD is still authenticated by AES-GCM.
- **AAD:** a caller-supplied context string, for example `chat:<connectionId>:<conversationId>`, `index:<connectionId>`, `blob:<sha256>`, `activity`, `approval:<id>`. A file copied into another slot fails to open.
- **API:**
  ```rust
  pub fn seal(plaintext: &[u8], aad: &str) -> Result<Vec<u8>, StoreError>;
  pub fn open(sealed: &[u8], aad: &str) -> Result<Vec<u8>, StoreError>;
  pub fn write_sealed(path: &Path, plaintext: &[u8], aad: &str) -> Result<(), StoreError>; // temp + rename, 0600
  pub fn read_sealed(path: &Path, aad: &str) -> Result<Vec<u8>, StoreError>;
  pub fn seal_line(plaintext: &[u8], aad: &str) -> Result<String, StoreError>;           // base64 envelope, one line
  pub fn open_line(line: &str, aad: &str) -> Result<Vec<u8>, StoreError>;
  ```
- **Errors:** `KeyUnavailable` (keychain denied/missing), `Corrupted`, `WrongContext`, `Io`. The UI maps them to "history cannot be decrypted" with a reset action. No plaintext fallback anywhere.
- **Key reset:** a command that deletes the key and every sealed AI file (conversations, blobs, activity, approvals). Behind a confirmation in Settings > AI.

### Conversation store: `src-tauri/src/ai_chat/store.rs`

```
<config>/ai_chats/
  <connectionId>/
    index.chat               # sealed JSON: [{ id, title, title_source, created_at, updated_at, message_count, mode, provider, model, database, schema }]
    <conversationId>.chat    # sealed JSON: Conversation
  blobs/
    <sha256>.bin             # sealed MIME header + bytes, AAD blob:<sha256>
```

```rust
pub struct Conversation {
    pub schema_version: u32,        // 1
    pub id: String,
    pub connection_id: String,
    pub database: Option<String>,
    pub schema: Option<String>,
    pub title: String,
    pub title_source: TitleSource,  // FirstMessage | Generated | User
    pub mode: ChatMode,             // Ask | Agent
    pub target: Option<ChatTarget>, // Editor | Notebook { notebook_id } (M8)
    pub provider: Option<String>,
    pub model: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub messages: Vec<Message>,     // from M1
}
```

- Commands: `ai_chat_list(connectionId | all)`, `ai_chat_load(id)`, `ai_chat_save(conversation)`, `ai_chat_rename(id, title)`, `ai_chat_delete(id)`, `ai_chat_delete_all()`, `ai_chat_put_blob(bytes, mime) -> blobId`, `ai_chat_get_blob(blobId)`.
- The index is rewritten on every save (small). If it fails to open but conversation files are fine, rebuild it.
- Size guard: warn above ~2 MB per conversation; images are blobs, not inline.
- **Deleting a connection** removes `ai_chats/<connectionId>/` (hook into the existing delete-connection path; run `impact` on it).
- **Blob GC:** on startup (debounced), delete blobs no conversation references.

### Migrating the existing AI files

- **`ai_activity.jsonl`:** each line becomes `seal_line(json, "activity")`. Appending and rotation keep the line-based logic. A line that fails to open is skipped and counted, not fatal.
- **`.mcp_session_state.json`:** `write_sealed` / `read_sealed` with AAD `mcp_session`.
- **`pending_approvals/*`:** keep the filenames (the watcher depends on them) and seal the contents with AAD `approval:<id>:pending` / `approval:<id>:decision`. Both the GUI and the MCP process read and write through `secure_store`.
- **One-time migration** at startup: for each known plaintext file, if it parses as plaintext JSON, seal it, verify it can be read back, then replace the original. Idempotent: sealed files are recognized by the envelope.
- **MCP subprocess:** it must load the same key. Check on macOS that the `--mcp` process doesn't trigger a keychain prompt (the binary is the same, so the item ACL should match; verify signed and unsigned dev builds). If it does prompt, document it, or have the GUI create the key before the MCP process is ever started.

## Steps

- [x] `secure_store` module + tests (round trip, wrong AAD, tampered ciphertext, wrong key, missing key).
- [x] Keychain helpers `get/set/delete_ai_store_key` in `keychain_utils.rs` with the cache.
- [x] `ai_chat/store.rs` + commands + tests (write, list, load, rename, delete, index rebuild).
- [x] Blob store + GC.
- [x] Delete-connection hook.
- [x] `impact` on `append_event_in`, `read_events_in`, `read_sessions_in`, `save_session_state_in`, `load_session_state_in` and the approval read/write functions, then switch them to `secure_store`.
- [x] Startup migration of plaintext files.
- [ ] Verify the MCP subprocess on macOS (signed + dev build), Windows, Linux (Secret Service).
- [ ] Settings > AI: "Delete all conversations" and "Reset encryption key" (danger zone).

## Tests

- No plaintext marker in any file under `ai_chats/`, `ai_activity*`, `pending_approvals/` after writes (a test writes a unique marker and greps the directory).
- A file copied to another conversation id fails with `WrongContext`.
- An activity log with one corrupted line still returns the other events.
- Migration: plaintext in, sealed out, readable, idempotent on second run.

## Done when

- The Activity panel, the approval modal and the MCP server work as before, on top of sealed files.
- Conversations can be saved, listed, loaded and deleted through commands (no UI yet).
- No AI-related plaintext remains in the config directory after first start.

## Good first issue candidates

None; this is security-sensitive.
