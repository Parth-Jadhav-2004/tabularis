# M4. Chat panel in the right sidebar

**Depends on:** M1 (streaming), M2 (saving) · **Size:** L

## Goal

A working chat in the right sidebar: send a message, stream the answer, render markdown, act on SQL blocks, pick a model, and save the conversation.

## Current state

- `src/contexts/RightSidebarContext.ts`: `RightSidebarPanel = "row-editor" | null`, with `isOpen`, `isPinned`, `toggle`, `close`, `setActivePanel`, `togglePin`.
- `src/components/layout/RightSidebar.tsx` is mounted in `MainLayout.tsx`; width and resize in `useRightSidebarResize.ts` (localStorage).
- `react-markdown` ^10 is a dependency (unused by the AI features today).
- Command palette: `CommandPaletteProvider`; shortcuts in `src/config/shortcuts.json`.
- AI settings: `aiEnabled`, `aiProvider`, `aiModel`, `aiCustomOpenaiModel` in `SettingsContext.ts`; the model list comes from `get_ai_models`.

## Design

### Files

```
src/components/ai-chat/
  AiChatPanel.tsx          # header + transcript + composer
  ChatHeader.tsx           # new chat, history toggle (M5), mode switch (M9), close
  ChatTranscript.tsx       # virtualized only if needed
  ChatMessage.tsx          # renders blocks
  blocks/
    TextBlock.tsx          # react-markdown
    SqlBlock.tsx           # Monaco read-only (or highlighted pre) + actions
    ReasoningBlock.tsx     # collapsed by default
    ErrorBlock.tsx
  ChatComposer.tsx         # textarea, send/stop, model picker, attachments row (M10)
  ModelPicker.tsx
src/hooks/useAiChat.ts     # run lifecycle and event subscription
src/types/aiChat.ts        # TS mirrors of Message/Block/ChatEvent
src/utils/aiChat.ts        # pure helpers (block parsing, SQL fence detection)
```

React rule: files that export components don't export helpers. Put helpers in `src/utils/aiChat.ts`.

### Panel registration

- `RightSidebarPanel = "row-editor" | "ai-chat" | null`.
- Lazy-load `AiChatPanel` in `RightSidebar.tsx`.
- Opening the row editor while the chat is open switches panel; the chat state lives in a provider (`AiChatProvider`), so switching back restores it.
- Toggle from: a button in the editor toolbar (next to the current AI dropdown for now), the command palette ("Toggle AI assistant"), and a shortcut (for example `Mod+Shift+I`; check `shortcuts.json` for conflicts).
- Visibility: see "AI availability" below. When AI is unavailable, the toolbar button, palette command and shortcut are not registered at all.

### `useAiChat`

- State: `conversation`, `runState: idle | streaming | error`, `draft`.
- `send(text)`:
  1. append the user message locally
  2. `invoke("ai_chat_send", ...)` → `runId`
  3. `listen("ai-chat://" + runId)` and fold the events into a pending assistant message
  4. on `Done`, persist through `ai_chat_save` and unlisten
- `stop()` → `ai_chat_cancel(runId)`.
- Avoid a synchronous setState in effects (React rules); fold the events with a reducer.
- Unlisten on unmount and on conversation switch.

### SQL blocks

Detect fenced `sql` blocks (and untagged blocks that parse as SQL for the active dialect, best effort). Actions:

- **Copy**
- **Insert at cursor:** Monaco `executeEdits` on the active editor tab, so it is undoable. Without an active editor, open a new query tab.
- **Replace query:** `updateActiveTab({ query })`.
- **Run:** through the normal `runQuery` path, so production and read-only confirmations still apply. Never bypass them.

You need a way to reach the active Monaco instance from outside `Editor.tsx`. Check `EditorProvider.tsx` for an existing ref or registry; otherwise add a small "active editor commands" context (insert / replace / run) that `Editor.tsx` registers.

### AI availability

Every AI surface (panel, toolbar button, palette entries, shortcut, slash commands, context menu entries, "Fix with AI", notebook buttons, inline completions) is shown **only** when:

1. `settings.aiEnabled` is true, **and**
2. at least one provider is usable: a key is configured (keychain or env var, see `check_ai_key_status`), or a keyless provider is configured (Ollama with a port, custom OpenAI-compatible with a base URL).

- New backend command `get_ai_available_providers() -> [{ provider, label, source: keychain | env | keyless, models_cached: bool }]`, built on `check_ai_key_status` for each provider.
- A single frontend hook `useAiAvailability()` (one context, refreshed when AI settings change or a key is saved/deleted) that every AI surface reads. No component checks `aiEnabled` on its own.
- If the last provider becomes unusable while the panel is open, the panel shows an empty state with a link to Settings > AI instead of disappearing mid-conversation.

### Provider and model picker

Switching provider/model happens directly in the chat, under the composer, like a standard chat client.

- Lists **every usable provider** (not just the global one), each with its models from `get_ai_models` (cached; refresh button). Grouped by provider, searchable, with capability badges (tools, images, reasoning) from M1.
- New conversation: defaults to the last provider/model used in the chat; the first time, to the global `aiProvider` / `aiModel` from Settings.
- The choice is stored on the conversation and restored on resume. Changing it in the chat does **not** change the global setting (which keeps driving the other AI features: tab rename, cell names, ...).
- **Mid-conversation switch is allowed.** The next turn goes to the new model with the full history, converted by M1 into that provider's wire format. Each assistant message keeps the provider/model that produced it (shown subtly in the message footer).
- Things the new model can't take are handled explicitly, with a one-line notice in the transcript:
  - images, if the new model has no image support (dropped from the request, kept in the conversation)
  - tools, if the new model has no tool support (Agent/Ask tool use disabled for the conversation until switching back)
  - provider-specific reasoning signatures (dropped when the provider changes)
- Providers that are configured but not usable right now (for example Ollama not reachable) are shown disabled with the reason; unconfigured providers are listed at the bottom as "Set up in Settings".

### Accessibility

- Composer is a labelled textarea. Enter sends, Shift+Enter adds a newline, Esc returns focus to the editor.
- Streaming region with `aria-live="polite"`, throttled so screen readers aren't flooded (announce on completion, or per paragraph).
- All actions are buttons with labels; focus ring tokens.

## Steps

- [ ] `impact` on `RightSidebar`, `RightSidebarProvider`, `MainLayout`, `Editor.tsx` toolbar area.
- [ ] Types in `src/types/aiChat.ts` matching M1's serde shape (snake_case vs camelCase: decide once and stick to it).
- [ ] `AiChatProvider` + `useAiChat` with a mocked event source first (unit-testable).
- [ ] Panel, header, transcript, message, blocks.
- [ ] Active-editor command bridge + SQL block actions.
- [ ] `get_ai_available_providers` + `useAiAvailability` + gate every AI surface on it.
- [ ] Composer + provider/model picker (per conversation, mid-conversation switch with notices).
- [ ] Toolbar button, palette command, shortcut.
- [ ] i18n keys in every locale.

## Tests

- `useAiChat` reducer: deltas, done, error, cancel; tool events ignored for now.
- SQL fence extraction utilities.
- Component test: send → streaming → done renders markdown and SQL actions.
- Insert/Replace call the bridge with the right payload.

## Done when

- You can open the panel, chat with any configured provider, stop a run, insert/replace/run SQL, and the conversation is saved encrypted (resuming it comes in M5; the last one may simply reopen).

## Good first issue candidates

- Picker polish (capability badges, search, "Set up in Settings" entries) once the base exists.
