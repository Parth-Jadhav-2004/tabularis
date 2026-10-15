# M3. Shared tool registry

**Depends on:** none · **Size:** M

## Goal

One implementation of the database tools and their safety rules, used by both the MCP server and the in-app assistant.

## Baseline before M3

- `src-tauri/src/mcp/mod.rs` (~1370 lines): JSON-RPC over stdio. Tools `list_connections`, `list_databases`, `list_tables`, `describe_table`, `run_query` (limit, database, `output_format`). Resources `tabularis://connections`, `tabularis://{id}/schema`.
- Safety:
  - read-only connections block non-SELECT (`blocked_readonly`)
  - `mcp_approval_mode`: `off` / `all` / `writes_only` (default)
  - optional preflight EXPLAIN (`mcp/preflight.rs`)
  - file-based approval queue (`ai_approval.rs`, `ai_approval_watcher.rs` → `ai://pending_approval` → `AiApprovalGate.tsx` / `AiApprovalModal.tsx` → `decide_pending_approval`, which supports an edited query)
  - audit via `ai_activity` (`classify_query_kind`, `strip_strings_and_comments`)

## Design

```
src-tauri/src/ai_tools/
  mod.rs          # public exports and MCP compatibility entry points
  types.rs        # Tool trait, resolved ToolContext, output and errors
  registry.rs     # scoped definitions, validation, dispatch and audit
  database.rs     # driver operations shared with MCP adapters
  approval.rs     # ApprovalGate trait and FileQueueGate
  safety.rs       # statement classification, multi-statement check, read-only enforcement
  truncate.rs     # result size caps
  tools/
    list_connections.rs
    list_databases.rs
    list_tables.rs
    describe_table.rs
    sample_rows.rs        # new
    run_query.rs          # read-only
    explain_query.rs      # new
    run_statement.rs      # write, Agent mode only (used in M9)
  tests.rs
```

```rust
pub enum ToolMode { Read, Write }

// ToolContext::resolve fixes the connection, database, schema and access mode
// in Rust. Model arguments cannot override them; credentials are never exposed.

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn input_schema(&self) -> serde_json::Value;
    fn mode(&self) -> ToolMode;
    async fn run(&self, ctx: &ToolContext, args: serde_json::Value) -> Result<ToolOutput, ToolError>;
}

pub struct ToolOutput { pub content: String, pub truncated: bool, pub rows: Option<RowsPreview> }
```

- **`ApprovalGate` transport boundary:**
  - `FileQueueGate`: today's behaviour, for the MCP process
  - `InAppGate`: planned for M9; the GUI process emits an event and awaits a oneshot channel

  The gate transports approval requests; tool orchestration enforces SQL/mode policy before execution and records calls in encrypted `ai_activity`. MCP keeps its configured approval mode; chat writes always require a fresh approval. Approved edits are revalidated. The approval target includes the database and, when explicit, the schema.
- **Policy** (`safety.rs`):
  - classify statements as read / write / DDL / transaction
  - reject multi-statement payloads
  - write tools are never listed for read-only connections
  - `run_query` refuses anything that isn't read
- **Truncation:** chat results are capped at 12k Unicode characters, cells at 200 characters, with a default row limit of 50 and a hard cap of 1000. Text keeps head and tail; structured row previews keep a head/tail sample and remain valid JSON within the same cap. Driver truncation and omitted data always set `truncated`.
- **`sample_rows`:** `SELECT * ... LIMIT n` through the driver, with no free SQL.
- **`explain_query`:** reuse the existing EXPLAIN plumbing (Visual Explain / `mcp/preflight.rs`).
- **Scope and compatibility:** `ToolRegistry.tools_for(mode, &context)` produces M1 `ToolDef`s, with write tools omitted for Ask/read-only contexts and EXPLAIN omitted when unsupported. Chat `list_connections` describes only the bound connection. MCP still advertises its five existing tools, with unchanged JSON/TOON formats, argument schemas and row defaults; the chat caps apply only to the typed registry.
- **No driver-specific branching in the frontend.** Tools go through the driver trait, so plugin drivers work through `RpcDriver`.

## Steps

- [x] `impact` on the MCP tool handlers, `preflight`, the approval functions and `ai_activity` writers.
- [x] Extract the 5 existing tools and connection/output/audit helpers into `ai_tools` (pure move, MCP tests green).
- [x] Add the `Tool` trait, typed context and registry on top of the extracted implementations.
- [x] Extract `safety.rs` from the current read-only / approval checks, preserving legacy APIs and adding strict chat-tool policy tests.
- [x] `ApprovalGate` trait + `FileQueueGate`; the MCP server uses it.
- [x] Add `sample_rows`, `explain_query`, `run_statement` (not exposed to MCP unless policy allows it; keep MCP behaviour identical).
- [x] `truncate.rs` + tests.
- [x] `registry.tools_for(mode, &context)` returning `ToolDef`s for M1's `ChatRequest`.

## Tests

- Each tool against SQLite fixtures (list, describe, sample, run_query, explain).
- `run_query` rejects `DELETE`, `UPDATE`, `DROP`, a multi-statement payload, and a `WITH ... DELETE`.
- Read-only connection: `tools_for(Agent, ro_conn)` contains no write tool.
- Truncation keeps head and tail, respects Unicode/cell/serialized preview limits, and reports driver-side omissions.
- Approve, deny, timeout, mismatched IDs, transport errors and edited SQL against disposable SQLite fixtures.
- File-queue round trips use an isolated encrypted store with an in-memory keychain; optional schema fields preserve legacy serialization.

## Done when

- The MCP server runs entirely on `ai_tools`, with no behaviour change for external clients.
- The registry can produce tool definitions for a chat request.

## Validation status

Implementation and automated validation are complete; see [PROGRESS.md](./PROGRESS.md) for the session checks. Native MCP/GUI approval smoke validation remains pending. Chat execution, cancellation-aware inline approvals and the agent loop are wired in M9; M4 can consume the registry definitions when the chat orchestration is ready.
