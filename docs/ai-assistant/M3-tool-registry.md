# M3. Shared tool registry

**Depends on:** none · **Size:** M

## Goal

One implementation of the database tools and their safety rules, used by both the MCP server and the in-app assistant.

## Current state

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
  mod.rs          # registry, Tool trait, ToolContext
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

pub struct ToolContext {
    pub connection_id: String,
    pub database: Option<String>,
    pub schema: Option<String>,
    pub origin: ToolOrigin,          // Mcp { session_id } | Chat { conversation_id, run_id }
    pub approval: Arc<dyn ApprovalGate>,
}

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

- **`ApprovalGate` trait** with two implementations:
  - `FileQueueGate`: today's behaviour, for the MCP process
  - `InAppGate`: the GUI process emits an event and awaits a oneshot channel (used by M9)

  Both apply the same policy and both write to `ai_activity`.
- **Policy** (`safety.rs`):
  - classify statements as read / write / DDL / transaction
  - reject multi-statement payloads
  - write tools are never listed for read-only connections
  - `run_query` refuses anything that isn't read
- **Truncation:** default cap 12k chars per result (head + tail + row sample), cells cut at 200 chars, row limit default 50, hard cap 1000. Always report `truncated`.
- **`sample_rows`:** `SELECT * ... LIMIT n` through the driver, with no free SQL.
- **`explain_query`:** reuse the existing EXPLAIN plumbing (Visual Explain / `mcp/preflight.rs`).
- **No driver-specific branching in the frontend.** Tools go through the driver trait, so plugin drivers work through `RpcDriver`.

## Steps

- [ ] `impact` on the MCP tool handlers, `preflight`, the approval functions and `ai_activity` writers.
- [ ] Create `ai_tools` with the trait and registry, and move the 5 existing tools (pure move, MCP tests green).
- [ ] Extract `safety.rs` from the current read-only / approval checks.
- [ ] `ApprovalGate` trait + `FileQueueGate`; the MCP server uses it.
- [ ] Add `sample_rows`, `explain_query`, `run_statement` (not exposed to MCP unless policy allows it; keep MCP behaviour identical).
- [ ] `truncate.rs` + tests.
- [ ] `registry.tools_for(mode, connection)` returning `ToolDef`s for M1's `ChatRequest`.

## Tests

- Each tool against SQLite fixtures (list, describe, sample, run_query, explain).
- `run_query` rejects `DELETE`, `UPDATE`, `DROP`, a multi-statement payload, and a `WITH ... DELETE`.
- Read-only connection: `tools_for(Agent, ro_conn)` contains no write tool.
- Truncation keeps head and tail and sets `truncated`.

## Done when

- The MCP server runs entirely on `ai_tools`, with no behaviour change for external clients.
- The registry can produce tool definitions for a chat request.
