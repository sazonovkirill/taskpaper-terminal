# Spec: Screen layout and styling

Covers F-VIEW-01–09, F-CMD-02–03 and N-PORT-02.

## 1. Layout

```
┌ Home › Work › Client A ─────────────────────────────── ● notes.taskpaper ┐  breadcrumbs + file
│ Client A                                                                  │  zoom title (editable)
│ ▾ Launch:                                                                 │
│     • Draft announcement @due(2026-10-08)                                 │
│     ✓ Book venue @done(2026-10-01)                                        │
│     ▸ Budget review                                             (3)       │
│       Notes about the venue go here and wrap under the text when the      │
│       line is longer than the terminal is wide.                           │
│ ▾ Ideas:                                                                  │
├───────────────────────────────────────────────────────────────────────────┤
│ / @due <[d] next week and not @done                          4 matches    │  query bar (when open)
│ filter: 4 matches · saved 12:04 · F1 help                                 │  status line
└───────────────────────────────────────────────────────────────────────────┘
```

- Rows: breadcrumbs (1), zoom title (1, hidden at home), outline (rest), query bar (1, when open), status line (1).
- Sidebar (F-VIEW-10) takes 24–32 columns on the left when toggled, hidden automatically below 80 columns.
- Pickers (Go to anything, Move to, Tag with, command palette) are centered overlays at most 80×20.
- Below 40×10 show only the outline and status line.

## 2. Rows

Each outline row: `indent` (2 columns per level, configurable) + `marker` + space + text.

| Item | Marker | Text style |
|---|---|---|
| Project, expanded / collapsed | `▾` / `▸` | bold; trailing `:` hidden by default |
| Task | `•` (collapsed with children: `▸`) | normal; `- ` hidden |
| Task done | `✓` | dim + strikethrough (falls back to dim when unsupported) |
| Note | none, aligned with sibling text | italic-dim (configurable) |
| Context-only ancestor (filter) | as above | dim |
| Collapsed with hidden children | count `(n)` right-aligned, optional | dim |

- Hidden syntax (`- `, `:`) is shown when the cursor is on that item so it can be edited; a setting shows it always.
- Tags are colored; `@done` dim, `@due` overdue red, due today yellow, `@today` accent. Tag colors are configurable by name.
- Long lines soft-wrap with continuation lines aligned to the text start. Widths use `unicode-width`; graphemes are never split.
- ASCII fallback markers (`v`, `>`, `-`, `x`) when `ascii = true` or the locale is not UTF-8.

## 3. Status line

Left to right: mode (when the vim preset is active), dirty marker `●` / save error, filter summary and match count, transient message (errors, "Reloaded", "Copied 3 items"), hint `F1 help`.

## 4. Theme

- Default theme uses the terminal's 16 ANSI colors so it follows the user's terminal palette in light and dark mode.
- Themes in config may use named, 256-color or `#rrggbb` colors; truecolor is used only when `COLORTERM` indicates support.
- `NO_COLOR` set: no colors, only bold/dim/italic/strikethrough/reverse.

## 5. Help and command palette

- `F1` opens a scrollable overlay listing commands grouped as in [keybindings.md](keybindings.md), showing the active bindings (including user overrides) and whether the kitty protocol is active.
- The command palette lists every command with its binding; fuzzy filtered; `Enter` runs it.

## 6. Mouse (F-NAV-07, OQ-19)

Click text to place the cursor; click a marker to toggle fold; Alt+click (or double-click) a marker to zoom; click a breadcrumb to zoom out; wheel scrolls the outline without moving the cursor. Disabled with `mouse = false`, which also lets the terminal do native selection.
