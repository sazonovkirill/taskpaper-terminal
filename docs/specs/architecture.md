# Spec: Architecture

How the crate in `app/` is organized. Detailed behavior is in the other specs.

## 1. Modules and dependency direction

```
main ──► app ──┬──► ui ───────┐
               ├──► commands ─┤
               │              ▼
               │             view ──► query ──► model ◄── format
               │                              ▲
               ├──► keymap                    │
               ├──► storage ──► format ───────┘
               └──► config
```

`ui` and `commands` both depend on `view`; `view` depends on `query` and `model`; `model` depends on nothing in the crate.

Arrows point at dependencies. `model`, `format` and `query` have no dependency on the terminal and are tested without it.

| Module | Responsibility |
|---|---|
| `model` | Document arena of lines, derived attributes, the block layer, `Op`/`Transaction`, undo stack ([data-model.md](data-model.md)). |
| `format` | Parse bytes → `Document`, serialize `Document` → bytes, tag scanning and editing helpers ([file-format.md](file-format.md)). |
| `query` | Date parser now; the search query parser and matcher later ([search.md](search.md)). |
| `view` | Workspace, panes, shared folds, focus, selection, modes and edit-mode editor state, Today view, visible-row computation ([navigation.md](navigation.md)). Produces and applies the focus/selection restore tokens stored in transactions. |
| `commands` | Named commands: take `&mut Document` + `&mut Workspace` + args, produce a transaction and a message ([editing.md](editing.md)). |
| `keymap` | Key event → command resolution per mode and context, multi-key sequences, config overrides ([keybindings.md](keybindings.md)). |
| `ui` | Ratatui widgets: panes, blocks and inline editor, breadcrumbs, sidebar, shortcuts panel, pickers, prompts, status line, themes ([ui.md](ui.md)). |
| `storage` | Atomic save, Save As, backups, file watching, recovery, view-state persistence ([file-io.md](file-io.md)). |
| `config` | Load and validate `config.toml`. |
| `app` | Event loop; owns the `Document`, the `Workspace` and background workers. |

## 2. Event loop

The structure follows [testing-strategy.md](../quality/testing-strategy.md) (Accepted): `app/src/lib.rs` holds all logic and `app/src/main.rs` only sets up the terminal, runs the loop and restores the terminal. The `app` module uses the Elm Architecture split: input mapping (`KeyEvent` → message), `update(state, message) → effects`, and a view that renders state into a `Frame` without changing it. File I/O and the clock are passed in, never reached for directly, so every timing rule in these specs (autosave delay, debounces, relative dates) is testable without sleeping.

Messages come from one channel:
- terminal input (crossterm `EventStream` or a blocking reader thread),
- file-watcher notifications,
- save results from the storage worker,
- timer ticks from the injected clock (autosave debounce, filter debounce, midnight tick for relative dates).

The loop redraws only when state changed (N-PERF-03). Disk I/O runs on a storage worker thread that receives serialized snapshots (N-UX-01).

## 3. Dependencies

Existing: `ratatui` 0.30, `crossterm` 0.29. Proposed additions, each to be confirmed against its current version when first used:

| Crate | Use |
|---|---|
| `slotmap` | Generational arena for items. |
| `unicode-segmentation`, `unicode-width` | Grapheme cursor movement and display width. |
| `jiff` | Dates, time zones, arithmetic. |
| `nucleo-matcher` | Fuzzy matching for Go to Symbol, Go to Command and Move to. |
| `notify` / `notify-debouncer-full` | File watching. |
| `arboard` | System clipboard (OSC 52 fallback is hand-written). |
| `serde`, `toml` | Config. |
| `directories` | Config/state paths per platform. |
| `tempfile` | Atomic writes. |
| `thiserror`, `anyhow` | Library errors / top-level errors. |
| Dev: `insta`, `proptest` | Snapshot and property tests. |

The date and query parsers are hand-written for precise error positions.

## 4. Testing

[testing-strategy.md](../quality/testing-strategy.md) governs test kinds, layout, coverage and mutation testing. Per module, the specs add:

- `format`, `model`: property tests for the round-trip and canonical-outline invariants ([file-format.md §7](file-format.md#7-tests)) and for block grouping ([data-model.md §3](data-model.md#3-blocks)).
- `query`: table tests for dates and, later, queries ([search.md §5](search.md#5-tests)).
- `commands`, `view`: unit tests of the form "document + workspace + command → document + workspace".
- Every binding in [keybindings.md](keybindings.md) and every interaction in these specs gets an integration (UI) test.
