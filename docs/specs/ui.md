# Spec: Screen layout and styling

Covers FR-BLK display, FR-TAG display, FR-FMT-01, FR-KEY-04–06, FR-UI-05–10, FR-SET-02–03 and N-PORT-02.

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

## 3. Themes (FR-UI-06–08)

- Built-in themes: at least a light and a dark theme that use the terminal's 16 ANSI colors (so they follow the user's palette), plus the twelve truecolor themes of FR-UI-08 ([palettes](../reference/themes.md)), used only when `COLORTERM` indicates support.
- A theme defines colors for: text, dim text, accent, selection, focus, headers, rule lines, code, links, each of the six tag colors, chips, overdue, errors.
- The theme picker in Go to Command previews each theme live as it is highlighted; `Enter` keeps it and saves it to the config (OQ-18), `Esc` restores the previous one.
- `NO_COLOR` set: no colors, only bold, dim, italic, underline, strikethrough and reverse.
- Unicode markers have ASCII fallbacks (`v`, `>`, `[ ]`, `[x]`, `-`) when the locale is not UTF-8.
- The settings window (§8) previews themes live in the same way.

## 4. Shortcuts panel (FR-KEY-04)

`?` toggles a panel on the left, in the sidebar's place, listing the most useful bindings for the current mode and context (navigate, edit, sidebar, picker), grouped as in [keybindings.md](keybindings.md) and showing user overrides. It notes when `Alt` keys have never been received (macOS Option setting) and whether the kitty keyboard protocol is active.

## 5. Context menu (FR-KEY-06)

A popup anchored at the focused element (or at the pointer for right-click) listing every command available for that element, grouped (Edit, Structure, Metadata, View, Panes), each with its key binding right-aligned. Commands that do not apply (for example Open URL on a block without one) are omitted. Entries come from the core's command list for that context, so the menu, the command palette and the `?` panel always agree.

## 6. Mouse (FR-KEY-05, D-16)

- Click a block: focus it (navigate mode); click inside the text of the block in edit mode: place the caret.
- Click a fold marker: toggle fold. Click a crumb: zoom the pane there. Click a sidebar entry: go there. Click a pane header: focus that pane.
- Alt+click a block, an internal link (once links exist, FR-OPS-10) or a crumb: open it in a new pane.
- Right-click a block, the selection, a sidebar entry, a crumb or a pane header: open its context menu (FR-KEY-06).
- Wheel: scroll the pane under the pointer without moving focus.
- No feature depends on the mouse; every action above has a key.

## 7. Icons (FR-UI-09, FR-UI-10)

- `icons = "nerd"` (default) draws Nerd Font glyphs; `icons = "none"` draws the Unicode markers of §2. When the locale is not UTF-8, both settings draw the ASCII fallbacks of §3.
- Every icon has exactly one fallback, defined next to it in one table in `ui::theme`, so the two sets cannot drift. Icons are drawn only by the UI; the core reports types and states, never glyphs.
- Icons cover: project fold state, task and done task, URL block, collapsed-children count, date chips (start, due, overdue), `@today`, bookmarks and the Today view in the sidebar, the status line's mode and save state, and the entries of pickers, the context menu and the settings window.
- Nerd Font glyphs are taken from the Nerd Fonts v3 code points (not the retired v2 Material Design range) and must be one cell wide in the terminals of N-PORT-01, so layout and alignment are identical with icons on and off; widths come from the same `unicode-width` measurement as text, and a glyph that measures two cells is not used.
- The app cannot detect whether the terminal font has the glyphs. The `?` panel and the settings window's icon entry say how to turn icons off, and the settings window's own labels are plain text, so it stays usable when glyphs render as boxes.

## 8. Settings window (FR-SET-02, FR-SET-03)

- Opened with `g,` in navigate mode or **Open Settings** in Go to Command; a centered overlay like the pickers, but up to 100×30.
- Left column: groups (Appearance, Editing, Files). Right: that group's settings, one per row, with name, current value and a one-line description; values that differ from the default are marked.
- Settings in v1: Appearance — theme (FR-UI-06), icons (§7), word wrap (FR-UI-05); Files — autosave delay ([file-io.md](file-io.md)). New settings are added to this window when they are added to the config file.
- Keys: `j`/`k`/arrows move, `Tab` switches between groups and settings, `Enter` or `Space` toggles a boolean or opens a value picker or number prompt, `h`/`l` cycle choices, `r` resets the focused setting to its default, typing `/` filters settings by name, `Esc` closes.
- Every change applies at once (theme and icons repaint the whole UI behind the overlay) and is written to `config.toml` immediately (FR-SET-04). A failed write shows an error in the window and the status line and keeps the value for the session.
- A footer shows the config file's path, so people know where to edit by hand.
