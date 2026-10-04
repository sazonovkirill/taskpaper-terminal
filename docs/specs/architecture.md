# Spec: Architecture

How the crate in `app/` is organized. Detailed behavior is in the other specs.

## 1. Layers and modules

[code-quality.md](../quality/code-quality.md) (Accepted) fixes three layers in one crate, with dependencies pointing inward only, and a UI-independent core (Document + App). The modules below map onto those layers.

```
UI (frontend)          ui::{keymap, widgets, layout, theme, effects, config}
   │                            │
   ▼                            ▼
App (core)             app::{state, update, workspace, commands, pickers, today}
   │
   ▼
Document (core)        document::{model, blocks, format, dates, query}
```

| Module | Layer | Responsibility |
|---|---|---|
| `document::model` | Document | Line arena, derived attributes, `Op`/`Transaction` and applying or inverting them ([data-model.md §1–5](data-model.md)). |
| `document::blocks` | Document | Block grouping and types derived from lines ([data-model.md §3](data-model.md#3-blocks)). |
| `document::format` | Document | Parse text → `Document`, serialize `Document` → text, tag scanning and editing helpers ([file-format.md](file-format.md)). Pure: takes and returns strings, never touches files. |
| `document::dates`, `document::query` | Document | Date parser now; search query parser and matcher later ([search.md](search.md)). Today's date is passed in. |
| `app::workspace` | App | Panes, shared folds, focus, selection, modes, edit-mode text and caret, Today view, visible blocks of each pane ([data-model.md §6](data-model.md#6-panes-and-view-state), [navigation.md](navigation.md)). |
| `app::commands` | App | Every user intent as a message handler: structural operations, block commands, mode changes ([editing.md](editing.md)). |
| `app::pickers` | App | Go to Symbol, Go to Command, Move to, context-menu entries: candidate lists and fuzzy ranking. |
| `app::update` | App | `update(state, message) -> effects`; the undo/redo history of transactions (code-quality.md assigns undo to App); autosave and debounce timing from clock-tick messages; canonical-outline repairs before save. |
| `ui::keymap` | UI | Key and mouse events → app messages per mode and context, multi-key sequences, config overrides ([keybindings.md](keybindings.md)). |
| `ui::widgets`, `ui::layout`, `ui::theme` | UI | Rendering panes, blocks and the inline editor, sidebar, panels, pickers, prompts, context menu, status line; wrapping, display width, scrolling, pane widths, themes ([ui.md](ui.md)). |
| `ui::effects` | UI | Executes the core's effects: atomic save, Save As, backups, recovery files, view-state files, file watching, clipboard, opening URLs ([file-io.md](file-io.md)). |
| `ui::config` | UI | Loads and writes `config.toml` (OQ-18). |

Following the core rules in code-quality.md:
- **Messages are intents** (`FocusNext`, `Indent`, `ZoomIn`, `SetDue(date)`, `InsertText(str)`), never key codes. Viewport-dependent intents carry the size (`PageDown { rows }`).
- **The core exposes what to show**: each pane's visible blocks (id, depth, type, display text with formatting spans, chips, fold state, selection, focus), the edit-mode text and caret, breadcrumbs, sidebar entries, picker results, mode and messages. Wrapping, display width, scroll offsets and pane widths are UI state.
- **Effects are data**: `Save { path, text }`, `Backup { path }`, `WriteRecovery { text }`, `CopyToClipboard(text)`, `OpenUrl(url)`, `PersistViewState(state)`, `Quit`. The UI adds its own presentation state (pane widths) to `PersistViewState` before writing the file, and passes it back on launch. Results come back as messages (`Saved { hash }`, `SaveFailed(err)`, `FileChangedOnDisk(text)`).

## 2. Event loop

`app/src/main.rs` only sets up the terminal, runs the loop and restores the terminal ([testing-strategy.md](../quality/testing-strategy.md)). The loop:

1. Receives events from one channel: terminal input, file-watcher notifications, effect results from worker threads, and clock ticks.
2. Maps them to app messages in `ui::keymap` (input) or directly (results, ticks with the current time).
3. Calls `update`, then executes the returned effects in `ui::effects` (disk I/O on a worker thread, N-UX-01).
4. Redraws only when state changed (N-PERF-03).

Because the clock and I/O arrive as messages, every timing rule in these specs (autosave delay, debounces, midnight) is testable without sleeping.

## 3. Dependencies

Existing: `ratatui` 0.30, `crossterm` 0.29 (UI only). Proposed additions, each added only when first needed and checked as code-quality.md requires:

| Crate | Use |
|---|---|
| `slotmap` | Generational arena for items (Document). |
| `unicode-segmentation` | Grapheme caret movement (App). |
| `unicode-width` | Display width (UI). |
| `jiff` | Date arithmetic (Document); reading the local date and time zone happens in the UI layer. |
| `nucleo-matcher` | Fuzzy matching for pickers (App). |
| `notify` / `notify-debouncer-full` | File watching (UI effects). |
| `arboard` | System clipboard (UI effects; OSC 52 fallback is hand-written). |
| `serde`, `toml`, `serde_json` | Config and view-state files (UI). |
| `directories` | Config/state paths per platform (UI). |
| `tempfile` | Atomic writes (UI effects). |
| `thiserror`, `anyhow` | Typed library errors / `main.rs` only (code-quality.md). |
| Dev: `insta`, `proptest` | Snapshot and property tests. |

The date and query parsers are hand-written for precise error positions.

## 4. Testing

[testing-strategy.md](../quality/testing-strategy.md) governs test kinds, layout, coverage and mutation testing. Per module, the specs add:

- `document::format`, `document::model`, `document::blocks`: property tests for the round-trip and canonical-outline invariants ([file-format.md §7](file-format.md#7-tests)) and for block grouping ([data-model.md §3](data-model.md#3-blocks)).
- `document::dates`, `document::query`: table tests for dates and, later, queries ([search.md §5](search.md#5-tests)).
- App: unit tests send messages to the core and check its queries and effects, with no UI (code-quality.md, "Test the core without the UI").
- Every binding in [keybindings.md](keybindings.md) and every interaction in these specs gets an integration (UI) test.
