# M11. Extras: diff walkthrough, inline completions, plugin slots

**Depends on:** M7 (slash commands), M3 (registry) · **Size:** L (three independent parts, any order)

## A. Review / optimize with a line-anchored diff

### Design

- For `/optimize` and a new `/review`, add an output contract to the prompt: after the explanation, emit a fenced ` ```tabularis-walkthrough ` JSON block:
  ```json
  {
    "after_sql": "...",
    "steps": [
      { "title": "...", "why": "...", "importance": "high|medium|low",
        "change": "rewrite|index|filter|join|other",
        "anchor": { "start_line": 3, "end_line": 5 } }
    ]
  }
  ```
- The app computes the diff locally (original = the SQL snapshot taken when the request was sent, stored on the message) and renders it side by side or unified, with the step list next to it. Clicking a step highlights its lines.
- **Apply to editor:** replace the original range only if the editor text in that range still equals the snapshot; otherwise open the result in a new tab. Undoable.
- If the block is missing or invalid, fall back to a normal markdown answer.
- Share the envelope parser and validation utilities with the M8 `tabularis-notebook` block.
- Diff library: Monaco's diff editor (`createDiffEditor`) is already available, so no new dependency is needed.

### Tests

- Envelope parser (valid, missing fields, invalid anchors).
- Apply: unchanged text → replaced; changed text → new tab.

## B. Inline AI completions in the SQL editor (opt-in)

### Design

- Monaco `registerInlineCompletionsProvider` for the SQL language, alongside the existing completion provider (`src/utils/autocomplete.ts`).
- Off by default. Settings > AI: enable, debounce (default 400 ms, 100–3000), model (can differ from the chat model; local models are a good fit).
- Request: up to 2000 chars before and 500 after the cursor with a `<CURSOR>` marker, a compact schema (M6 layer 5, small budget), and the dialect. Non-streaming, short max tokens, low temperature.
- Skip when the suggest widget is open, when the user is typing faster than the debounce, or inside comments/strings.
- Cancel in-flight requests on cursor move.
- Tab accepts, Esc dismisses.
- New backend command `ai_inline_complete` on top of M1 (no tools, no persistence, nothing written to the activity log beyond a counter, if anything).

### Tests

- Prompt builder (cursor marker, truncation).
- Skip conditions.

## C. Plugin slots for the AI assistant

### Design

- **Plugin tools:** a plugin manifest can declare read tools (name, description, JSON schema) served over its existing JSON-RPC channel. They are registered in `ai_tools` under a `plugin:<id>:` prefix, so a plugin can never shadow a built-in tool. Tools that write are always approval-gated.
- **Slash commands:** a plugin can contribute commands (name, description, instruction text, default context).
- **UI slot:** `ai-chat.header.actions` in `src/types/pluginSlots.ts` and `packages/plugin-api/src/slots.ts`.
- Document it in the plugin API docs and the manifest spec, with an example plugin.

### Tests

- Name collision rejection.
- A plugin write tool always produces an approval card.

## Good first issue candidates

- None until A/B/C exist; then small polish items (for example step filtering by importance in the walkthrough).
