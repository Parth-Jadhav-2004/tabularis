# M9. Agent mode: tool loop and approvals

**Depends on:** M1 (tool calling), M3 (registry), M4 (panel) · **Size:** L

## Goal

Let the model explore the database through tools (Ask), and propose writes that only run after an explicit, per-statement approval (Agent).

## Design

### Modes

- A mode switch in the composer header: **Ask** / **Agent**, stored per conversation.
- Ask: `registry.tools_for(Read, conn)`.
- Agent: `registry.tools_for(Write, conn)`, so read + `run_statement`.
- Read-only connection: Agent is disabled, with a tooltip explaining why.
- A model without tool support: modes are disabled and the chat works with injected context only (M6).

### Loop: `src-tauri/src/ai_chat/agent_loop.rs`

```
loop round in 1..=max_rounds:
    stream model response
    if no tool calls: done
    for each tool call (sequentially):
        emit ToolCallStart
        if tool.mode == Write: await approval (see below)
        run tool via registry, with ToolContext { origin: Chat { conversation_id, run_id } }
        emit ToolCallEnd { status, result preview }
        append ToolResult to messages
if max_rounds reached: emit Paused { reason: round_limit } and wait for Continue / Stop
```

- `max_rounds` default 25, setting 5–200.
- Cancellation is checked between rounds, between tool calls and inside approval waits.
- Persist after every completed round, so a crash loses at most one round.
- Tool results go back to the model wrapped as untrusted data.

### Write approval

- `InAppGate` (M3) emits `ApprovalRequired { call_id, sql, connection, database, schema, preflight? }` on the run channel and awaits a oneshot.
- UI: an inline approval card in the transcript with the exact SQL (read-only Monaco), the target, the preflight EXPLAIN summary when available, and **Approve** / **Edit** / **Reject**.
- Command `ai_chat_decide(run_id, call_id, decision, edited_sql?)`.
- The grant covers that exact SQL and target, once. The backend re-checks the target before executing.
- An edited statement is what runs, and the tool result tells the model it was edited (include the final SQL).
- A rejection becomes a tool result `"rejected by user"` (with an optional reason) so the model can adapt.
- Every decision is written to `ai_activity` (sealed, M2) with origin `chat`.
- Keep the global `AiApprovalGate` modal for MCP requests; chat approvals stay inline in the chat.

### Tool call cards

- `src/components/ai-chat/blocks/ToolCallBlock.tsx`: name, short args summary, status icon (running / done / failed / rejected / awaiting approval), expandable args JSON, result preview.
- `run_query` / `sample_rows` results: a small table (first rows, truncated cells) + "Open in editor" (new tab with the SQL).
- Persisted as `ToolCall` / `ToolResult` blocks; same rendering on reopen.
- Status announced through `aria-live`.

### Long conversations

- Before each request, estimate tokens (chars/4 heuristic, or the provider's usage numbers from the previous turn) against the model's `context_window` minus an output reserve.
- Above the threshold, summarize the older turns with a fixed structured prompt (goal, decisions, relevant schema facts, open questions, next steps) and keep the last N turns verbatim.
- The summary is a special assistant message (`Text` with `kind: summary`) and the summarized messages are flagged `compacted: true`. They stay visible in the UI but are not sent anymore.
- A provider `context_length` error triggers a summary and one retry.

## Steps

- [ ] Mode switch UI + persistence.
- [ ] `agent_loop.rs` with a mocked provider and mocked tools first.
- [ ] `InAppGate` + `ai_chat_decide` + approval card.
- [ ] Tool call cards + "Open in editor".
- [ ] Round cap pause/continue.
- [ ] Token estimate + summarization + retry on context errors.
- [ ] Settings: max rounds.
- [ ] i18n.

## Tests

- Loop with a scripted provider: text only; one read tool; multiple tool calls; round cap.
- Ask mode never sends write tools (request snapshot).
- Approval: approve runs the exact SQL; edit runs the edited SQL; reject never touches the DB; timeout/cancel never touches the DB.
- Target re-check: a changed connection/database between request and approval is refused.
- Summarization picks the right turns and keeps the last N.

## Done when

- In Ask mode the model can list/describe/sample/query (read-only) on its own.
- In Agent mode every write shows an approval card and nothing runs without it.
- Long sessions keep working past the context window.
