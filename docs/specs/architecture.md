# Spec: Architecture

How the crate in `app/` is organized. Detailed behavior is in the other specs.

## 1. Layers and modules

[code-quality.md](../quality/code-quality.md) (Accepted) fixes three layers in one crate, with dependencies pointing inward only, and a UI-independent core (Document + App). The modules below map onto those layers.

```
UI (frontend)          ui::{keymap, widgets, layout, theme, animation, effects, config}
   │                            │
   ▼                            ▼
App (core)             app::{state, update, history, workspace, commands, finder, today}
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
| `app::commands` | App | The command registry: every user intent as a command with its name, the modes it is given in and its default bindings, written in the config notation of [keybindings.md §5](keybindings.md#5-configuration), in one table that the keymap reads and the menus, Go to Command and the `?` panel list; the message each command sends; and the commands of one block that the command menu and the context menu offer. The messages are handled in `app::update` and the modules it calls. |
| `app::finder` and one module per finder (`app::go_to_symbol`, …) | App | The fuzzy finder shared by Go to Symbol, Go to Project, Go to Command and Move to: query, ranking with `nucleo-matcher` by tier and score, highlight; each finder module builds its candidates and acts on the choice. |
| `app::history` | App | The undo/redo history of committed transactions and the focus to restore with each (code-quality.md assigns undo to App). |
| `app::update` | App | `update(state, message) -> effects`, committing each command's transaction to the history; autosave and debounce timing from clock-tick messages; canonical-outline repairs before save. |
| `ui::keymap` | UI | Key events → app messages per mode and context, multi-key sequences, config overrides ([keybindings.md](keybindings.md)). |
| `ui::mouse` | UI | Mouse events → app messages: what the last frame drew under the pointer and what a click, Alt+click, right-click or the wheel there asks for ([ui.md §6](ui.md#6-mouse-fr-key-05-d-16)). |
| `ui::widgets`, `ui::layout`, `ui::theme` | UI | Rendering panes, blocks and the inline editor, the Navigator, panels, pickers, prompts, context menu, status line; wrapping, display width, scrolling, pane widths, themes ([ui.md](ui.md)). |
| `ui::animation` | UI | Animation effects drawn over rendered frames, their triggers and the animation setting ([ui.md §9](ui.md#9-animation-fr-ui-11)). |
| `ui::effects` | UI | Executes the core's effects: atomic save, Save As, backups, recovery files, view-state files, file watching, clipboard, opening URLs ([file-io.md](file-io.md)). |
| `ui::config` | UI | Loads and writes `config.toml` (OQ-18, FR-SET), keeping comments and order with `toml_edit`. |

Following the core rules in code-quality.md:
- **Messages are intents** (`FocusNext`, `Indent`, `ZoomIn`, `SetDue(date)`, `InsertText(str)`), never key codes. Viewport-dependent intents carry the size (`PageDown { rows }`).
- **The core exposes what to show**: each pane's visible blocks (id, depth, type, display text with formatting spans, chips, fold state, selection, focus), the edit-mode text and caret, breadcrumbs, sidebar entries, picker results, mode and messages, plus the blocks the last change touched and whether the last intent was refused. Wrapping, display width, scroll offsets and pane widths are UI state.
- **Effects are data**: `Save { path, text }`, `Backup { path }`, `WriteRecovery { text }`, `CopyToClipboard(text)`, `OpenUrl(url)`, `PersistViewState(state)`, `Quit`. The UI adds its own presentation state (pane widths) to `PersistViewState` before writing the file, and passes it back on launch. Results come back as messages (`Saved { hash }`, `SaveFailed(err)`, `FileChangedOnDisk(text)`).

## 2. Event loop

`app/src/main.rs` only sets up the terminal, runs the loop and restores the terminal ([testing-strategy.md](../quality/testing-strategy.md)). The loop:

1. Receives events from one channel: terminal input, file-watcher notifications, effect results from worker threads, and clock ticks.
2. Maps them to app messages in `ui::keymap` (input) or directly (results, ticks with the current time).
3. Calls `update`, then executes the returned effects in `ui::effects` (disk I/O on a worker thread, N-UX-01).
4. Redraws only when state changed or an animation effect is running; the UI requests frame ticks only while one runs ([ui.md §9](ui.md#9-animation-fr-ui-11), N-PERF-03).

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
| `similar` | Line diff of the external-change conflict prompt (App). |
| `arboard` | System clipboard, reached from `main.rs` through `ui::effects::SystemClipboard` so that tests never touch it; the OSC 52 fallback is crossterm's `CopyToClipboard` command, with its `osc52` feature (UI effects). |
| `serde`, `toml`, `toml_edit`, `serde_json` | Config and view-state files; `toml_edit` writes settings without losing comments (UI). The view state the core hands out derives `serde`'s traits, so any frontend can store it (App). |
| `directories` | State paths per platform (UI). The config, backup, recovery and view-state paths follow XDG on macOS too (`$XDG_CONFIG_HOME` or `~/.config`, `$XDG_DATA_HOME` or `~/.local/share`, `$XDG_STATE_HOME` or `~/.local/state`), not `~/Library` (FR-SET-01, FR-DOC-07, file-io.md §4–5). |
| `tachyonfx` | Animation effects (UI); 0.25 builds on `ratatui-core` 0.1, the core of Ratatui 0.30. |
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
