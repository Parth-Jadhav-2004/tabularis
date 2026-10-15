# M4. Chat panel in the right sidebar

**Depends on:** M1 (streaming), M2 (saving) · **Size:** L

## Goal

A working chat in the right sidebar: send a message, stream the answer, render markdown, act on SQL blocks, pick a model, and save the conversation.

## Current state

- Frontend wire types, the shared `AiChatProvider` / `useAiChat` session, lazy sidebar panel, Markdown/SQL rendering, composer and model picker are implemented. `MainLayout` mounts the session above the sidebar; transcripts and drafts survive panel switches.
- `src/contexts/RightSidebarContext.ts`: `RightSidebarPanel = "row-editor" | "ai-chat" | null`, with `isOpen`, `isPinned`, `toggle`, `close`, `setActivePanel`, `togglePin`.
- `src/components/layout/RightSidebar.tsx` is mounted in `MainLayout.tsx`; width and resize in `useRightSidebarResize.ts` (localStorage).
- `react-markdown` renders assistant text with raw HTML and remote images omitted.
- Command palette: `CommandPaletteProvider`; shortcuts in `src/config/shortcuts.json`.
- AI settings: `aiEnabled`, `aiProvider`, `aiModel`, `aiCustomOpenaiModel` in `SettingsContext.ts`; the model list comes from `get_ai_models`.
- `AiAvailabilityProvider` gates existing editor, result, notebook, import and Visual Explain AI surfaces. Configured offline Ollama instances remain in the picker with a disabled reason; the loopback probe has a two-second timeout.
- Automated implementation checks and an in-memory browser fixture pass. Native provider/keychain and real Monaco/production-confirmation smoke checks remain pending.

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
src/contexts/AiChatContext.ts
src/contexts/AiChatProvider.tsx  # shared session, independent of panel mounting
src/types/aiChat.ts        # TS mirrors of Message/Block/ChatEvent
src/utils/aiChat.ts        # pure reducer and message/conversation creation
src/utils/aiChatSession.ts # event lifecycle, cancellation and persistence
src/utils/aiChatTransport.ts # injectable Tauri IPC/event adapter
src/contexts/AiEditorContext.ts
src/contexts/AiEditorProvider.tsx
src/hooks/useAiEditor.ts   # active-scope registration/lookup, including split panes
src/utils/aiEditor.ts      # undoable edits and normal console navigation
src/contexts/AiAvailabilityContext.ts
src/contexts/AiAvailabilityProvider.tsx
src/hooks/useAiAvailability.ts
src/hooks/useAiChatModels.ts
src/utils/aiChatModels.ts
src/utils/aiChatMarkdown.ts
```

React rule: files that export components don't export helpers. Put helpers in `src/utils/aiChat.ts`.

### Panel registration

- `RightSidebarPanel = "row-editor" | "ai-chat" | null`.
- Lazy-load `AiChatPanel` in `RightSidebar.tsx`.
- Opening the row editor while the chat is open switches panel; the chat state lives in a provider (`AiChatProvider`), so switching back restores it.
- Toggle from: a button next to the current editor AI dropdown, the command palette ("Toggle AI assistant"), and the conflict-free, configurable `Mod+Shift+I` shortcut.
- Visibility: see "AI availability" below. When AI is unavailable, the toolbar button, palette command and shortcut are not registered at all.

### `useAiChat`

- State: `conversation`, `runState: idle | streaming | error`, `draft`, plus readiness, notices and a separate save status/result.
- `AiChatProvider` owns a reducer-backed session through `useSyncExternalStore`; `useAiChat` reads its context. The panel can unmount without losing the transcript or draft.
- `send(text)`:
  1. generate a canonical UUID and append the user plus pending assistant locally
  2. await `listen("ai-chat://" + runId)` before starting the backend run
  3. `invoke("ai_chat_send", { req, runId })` and fold events with the reducer
  4. on `Done`, unlisten and persist exactly once through `ai_chat_save`, including partial cancelled/failed responses
- `stop()` → `ai_chat_cancel(runId)`.
- Stop before subscription completes never starts a backend request. Cancellation while `invoke` is pending is repeated after acknowledgement so it cannot precede backend run registration unnoticed.
- New turns wait for an in-flight save; a failed save keeps the transcript and exposes `retrySave()`. Model changes affect the next request while assistant messages retain their producing model.
- Avoid a synchronous setState in effects (React rules); fold the events with a reducer.
- Unlisten/cancel on provider unmount and conversation switch; ignore stale events and save completions. Subscribe to `ai-chat://storage-cleared` before allowing sends, and clear the session on reset/delete-all.
- Tool events are ignored and requests advertise no tools until M9.

### SQL blocks

Detect fenced SQL/dialect blocks and conservatively recognize common SQL statements in untagged fences. Recognition is for presentation; execution still uses the normal SQL guards. Actions:

- **Copy**
- **Insert at cursor:** Monaco `executeEdits` on the active editor tab, so it is undoable. Without an active editor, open a new query tab.
- **Replace query:** an undoable Monaco edit of the full model, immediately synchronized through `updateTab`.
- **Run:** open a console in the active connection/database/schema through `openEditorInScope`; its existing single/batch execution, parameter and query-guard pipeline applies production and read-only protections.

`Editor.tsx` registers commands in a dedicated `AiEditorProvider` store. The selected command scope identifies the active split pane without changing the shared command-palette bridge. Read-only definition tabs and non-console tabs open a fresh console instead of being modified. Actions are disabled during the current assistant response and whenever the active connection/database/schema differs from the conversation target.

### AI availability

Every AI surface (panel, toolbar button, palette entries, shortcut, slash commands, context menu entries, "Fix with AI", notebook buttons, inline completions) is shown **only** when:

1. `settings.aiEnabled` is true, **and**
2. at least one provider is usable: a key is configured (keychain or env var, see `check_ai_key_status`), or a keyless provider is configured (Ollama with a port, custom OpenAI-compatible with a base URL).

- Backend command `get_ai_available_providers() -> [{ provider, label, source: keychain | env | keyless, models_cached, usable, reason }]`, built on the shared key-status helper. Credentials stay in Rust. Ollama requires an explicit valid port and a reachable instance with models; custom OpenAI requires a valid HTTP(S) base URL and may be keyless. Configured unavailable providers carry `unreachable` or `no_models` reasons.
- A single frontend hook `useAiAvailability()` (one context, refreshed when AI settings change or a key is saved/deleted) that every AI surface reads. No component checks `aiEnabled` on its own.
- If the last provider becomes unusable while the panel is open, the panel shows an empty state with a link to Settings > AI instead of disappearing mid-conversation.

### Provider and model picker

Switching provider/model happens through a compact selected-model disclosure in the composer toolbar. Search, the grouped selector, capabilities, refresh and setup entries appear only when it is opened. Selection and Escape close it and restore focus; clicking outside also dismisses it.

- Lists **every usable provider** (not just the global one), each with its models from `get_ai_models` (cached; refresh button). Grouped by provider, searchable, with capability badges (tools, images, reasoning) from M1.
- New conversation: defaults to the last provider/model used in the chat; the first time, to the global `aiProvider` / `aiModel` from Settings.
- The choice is stored on the conversation and restored on resume. Changing it in the chat does **not** change the global setting (which keeps driving the other AI features: tab rename, cell names, ...).
- **Mid-conversation switch is allowed.** The next turn goes to the new model with the full history, converted by M1 into that provider's wire format. Each assistant message keeps the provider/model that produced it (shown subtly in the message footer).
- Things the new model can't take are handled explicitly, with a one-line notice in the transcript:
  - images, if the new model has no image support (dropped from the request, kept in the conversation)
  - tools, if the new model has no tool support (Agent/Ask tool use disabled for the conversation until switching back)
  - provider-specific reasoning signatures (dropped when the provider changes)
- Providers that are configured but not usable right now (for example Ollama not reachable) are shown disabled with the reason; unconfigured providers are listed at the bottom as "Set up in Settings".

### Sidebar layout

- A quiet header contains the title, an optional notebook subtitle and icon controls for history, new chat and close.
- User turns use a subtle bubble; assistant content flows directly in the transcript. Sent-context summaries stay collapsed, model provenance appears in a muted footer and successful saves/completions are announced to screen readers. Errors remain visible.
- One auto-growing composer contains explicit `@`/`#` reference chips, the Context/model controls and a circular send/stop button. Context layers and preview details live in a collapsed, keyboard-accessible Context disclosure. Hints remain available inside it and to screen readers.
- SQL block actions use labelled icon buttons. Theme tokens preserve light, dark, high-contrast and square-corner appearances.

### Accessibility

- Composer is a labelled textarea. Enter sends, Shift+Enter adds a newline, Esc returns focus to the editor.
- Streaming region with `aria-live="polite"`, throttled so screen readers aren't flooded (announce on completion, or per paragraph).
- All actions are buttons with labels; focus ring tokens.

## Steps

- [x] `impact` on `RightSidebar`, `RightSidebarProvider`, `MainLayout`, `Editor.tsx` toolbar area. Re-run before editing those symbols for integration.
- [x] Types in `src/types/aiChat.ts` matching M1's serde shape (snake_case fields/values and `type` discriminator).
- [x] `AiChatProvider` + `useAiChat` with a mocked event source first (unit-testable).
- [x] Panel, header, transcript, message, blocks.
- [x] Active-editor command bridge + SQL block actions.
- [x] `get_ai_available_providers` + `useAiAvailability` + gate every AI surface on it.
- [x] Composer + provider/model picker (per conversation, mid-conversation switch with notices).
- [x] Toolbar button, palette command, shortcut.
- [x] i18n keys in every locale.
- [ ] Native smoke: real provider streaming/cancellation and encrypted save; Monaco cursor/undo, split panes and production/read-only execution confirmations.

## Tests

- Covered: reducer deltas, reasoning signatures, usage/notices, done, error, cancel; tool events ignored for now.
- Covered: early events, subscription/invoke failures, duplicate sends, cancellation during subscription/invoke, panel consumer remount, conversation switches, provider unmount, StrictMode replay, storage reset and save retry/isolation.
- Covered: SQL fence recognition, Markdown rendering, HTML/remote-image omission and exact SQL action payloads after completion.
- Covered: undoable insert/replace, no-editor fallback, guarded console-navigation delegation and active split-scope registration/cleanup.
- Covered: model preference/defaults, provider switching with full history, credential/settings refresh isolation, disabled/offline providers, keyboard/IME handling and gated shortcuts.
- Covered: component streaming/stop/save retry/reset, panel remount, changed targets and loss of the final provider.
- Covered: collapsed Context/model controls, keyboard dismissal and focus restoration, explicit-reference chips and unchanged context/send behavior.
- Visual fixture: Italian UI, Light/Dark/High Contrast themes, square corners, keyboard focus and SQL insertion; transport, storage and editor actions are in-memory fixtures.
- Sidebar redesign checked with real components at 384/320 px in Light, Dark and High Contrast; native/provider validation remains pending.

## Done when

- You can open the panel, chat with any configured provider, stop a run, insert/replace/run SQL, and the conversation is saved encrypted (resuming it comes in M5; the last one may simply reopen).

## Good first issue candidates

- Picker polish (capability badges, search, "Set up in Settings" entries) once the base exists.
