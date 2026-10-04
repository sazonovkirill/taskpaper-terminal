# Spec: Screen layout and styling

Covers FR-BLK display, FR-TAG display, FR-FMT-01, FR-KEY-04, FR-KEY-05, FR-UI-05–07 and N-PORT-02.

## 1. Layout

```
┌ Bookmarks ─────────┬ Home › Work ──────────────────────┬ Home › Ideas ─────────────────┐
│ ★ Launch plan      │ Work                              │ Ideas                         │
│ Today          3   │ ▾ Launch                          │ • Terminal outliner           │
│ Projects           │   ☐ Draft announcement  Fri 8 Oct │ • Plain-text sync             │
│ ▾ Work             │   ☑ Book venue                    │                               │
│   ▸ Launch         │   ☐ Budget review  ▸ 3            │                               │
│ ▸ Ideas            │     Notes about the venue wrap    │                               │
│                    │     under the text like this.     │                               │
│                    │   ── Later ─────────────────────  │                               │
├────────────────────┴───────────────────────────────────┴───────────────────────────────┤
│ NAVIGATE · ~/notes.taskpaper · saved · ? shortcuts                                       │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

- **Sidebar** (FR-UI-01) on the left, 24–32 columns; replaced by the `?` panel while that is open (D-12).
- **Panes** (FR-UI-02) to its right as columns, each with a breadcrumb header (its first row) and, when zoomed, a title row. The focused pane's header is highlighted.
- **Status line** at the bottom: mode, file path (marked when it is a `/tmp` file not yet saved elsewhere), save state or error, pending key prefix, transient messages.
- **Pickers and prompts** (Go to Symbol, Go to Command, Move to, type, color, dates, Save As) are centered overlays at most 80×20.
- Below 80 columns the sidebar hides; below 40×10 only the focused pane and the status line are drawn.

## 2. Blocks

Each row: indent (2 columns per level) + marker + space + text, then right-aligned chips.

| Block (FR-BLK) | Marker | Text |
|---|---|---|
| Project | `▾` / `▸` (fold state) | bold heading |
| Task | `☐`, done `☑` | normal; done tasks dimmed and struck through (falls back to dim) |
| Note | none; aligned with sibling text | normal, several lines |
| Section | none | Things 3 style: bold label followed by a horizontal rule to the pane's edge; children indented below |
| Code | none | monospace-styled background band, no inline formatting |
| URL | `↗` | title styled as a link (underlined, accent color) |
| Divider | none | a horizontal rule across the pane |

- Blocks with children that are collapsed show `▸ n` (hidden child count) after the text; projects use their marker instead.
- **Tags are never shown** (FR-TAG-01). Their effects are: dates as compact chips (`Fri 8 Oct`; overdue `@due` in the error color, FR-TAG-03), `@today` as a `★`-style marker chip (FR-TAG-04), a color tag tints the block's text (FR-TAG-05), bookmarks as a marker in the sidebar only.
- **Inline formatting** (FR-FMT-01): `**bold**` renders bold and `` `code` `` in the code style with the markers hidden in navigate mode; edit mode shows the markers.
- **Bare URLs** in text are underlined (D-10).
- **Focus and selection**: the focused block has a full-width highlight; selected blocks a dimmer one.
- **Edit mode**: the focused block's text becomes an inline editor at the same position, with a visible caret and the markers shown (FR-MODE-02).
- **Wrap** (FR-UI-05): on, long text wraps under its start column; off, every block takes one row and overflowing text ends in `…`. Widths use `unicode-width`; graphemes are never split.

## 3. Themes (FR-UI-06, FR-UI-07)

- Built-in themes: at least a light and a dark theme that use the terminal's 16 ANSI colors (so they follow the user's palette), plus truecolor themes used only when `COLORTERM` indicates support.
- A theme defines colors for: text, dim text, accent, selection, focus, headers, rule lines, code, links, each of the six tag colors, chips, overdue, errors.
- The theme picker in Go to Command previews each theme live as it is highlighted; `Enter` keeps it and saves it to the config (OQ-18), `Esc` restores the previous one.
- `NO_COLOR` set: no colors, only bold, dim, italic, underline, strikethrough and reverse.
- Unicode markers have ASCII fallbacks (`v`, `>`, `[ ]`, `[x]`, `-`) when the locale is not UTF-8.

## 4. Shortcuts panel (FR-KEY-04)

`?` toggles a panel on the left, in the sidebar's place, listing the most useful bindings for the current mode and context (navigate, edit, sidebar, picker), grouped as in [keybindings.md](keybindings.md) and showing user overrides. It notes when `Alt` keys have never been received (macOS Option setting) and whether the kitty keyboard protocol is active.

## 5. Mouse (FR-KEY-05, D-16)

- Click a block: focus it (navigate mode); click inside the text of the block in edit mode: place the caret.
- Click a fold marker: toggle fold. Click a crumb: zoom the pane there. Click a sidebar entry: go there. Click a pane header: focus that pane.
- Alt+click a block, an internal link (once links exist, FR-OPS-10) or a crumb: open it in a new pane.
- Wheel: scroll the pane under the pointer without moving focus.
- No feature depends on the mouse; every action above has a key.
