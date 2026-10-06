# Spec: Screen layout and styling

Covers FR-BLK display, FR-TAG display, FR-FMT-01, FR-KEY-04–06, FR-UI-05–11, FR-SET-02–03 and N-PORT-02.

## 1. Layout

```
┌ Bookmarks ─────────┬ ⌂ › Work ─────────────────────────┬ ⌂ › Ideas ────────────────────┐
│ ★ Launch plan      │ Work                              │ Ideas                         │
│ Today          3   │   Launch:                         │ • Terminal outliner           │
│ Projects           │   - Draft announcement  Fri 8 Oct │ • Plain-text sync             │
│ ▾ Work             │   ✔ Book venue                    │                               │
│   ▸ Launch         │   - Budget review  ▸ 3            │                               │
│ ▸ Ideas            │     Notes about the venue wrap    │                               │
│                    │     under the text like this.     │                               │
│                    │   Later                           │                               │
├────────────────────┴───────────────────────────────────┴───────────────────────────────┤
│ NAVIGATE · ~/notes.taskpaper · saved · ? shortcuts                                       │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

- **Sidebar** (FR-UI-01) on the left, 24–32 columns.
- **Panes** (FR-UI-02) to its right as columns, each with a breadcrumb header (its first row, a house icon `⌂` for the document root followed by the path to the zoom block, D-32, starting after the focus gutter of §2 so the root crumb lines up with top-level blocks, D-35) separated from the outline by a dim horizontal rule, like Workflowy (D-29), and, when zoomed, a title row showing the zoom block styled as in §2 for its type (marker, color, dimming, a project's `:`), in bold and without indent, so its type is clear (D-34). The focused pane's header is highlighted.
- **Status line** at the bottom: mode, file path (marked when it is a `/tmp` file not yet saved elsewhere), save state or error, pending key prefix, transient messages.
- **Secondary sidebar** on the right, 32–40 columns, shown only while it holds the `?` shortcuts panel (§4) or the comments panel (FR-SPC-02, [special-nodes.md §2.2](special-nodes.md#22-display)), one at a time; opening one replaces the other (D-28). It takes its width from the panes, which keep their minimum and scroll horizontally as in [navigation.md §5](navigation.md#5-panes-fr-ui-02--fr-ui-04-d-13). Below 80 columns it is drawn over the right part of the focused pane.
- **Pickers and prompts** (Go to Symbol, Go to Command, Move to, type, color, dates, Save As) are centered overlays at most 80×20.
- Below 80 columns the navigation sidebar hides; below 40×10 only the focused pane and the status line are drawn.

## 2. Blocks

Each row: indent, then the marker (if the type has one) and a space, then text, the `@today` and bookmark icons, and right-aligned chips. The indent is exactly the block's depth in the hierarchy, 2 columns per level, whatever its type; markers and icons come after it and never change it. Only tasks have markers; every other block starts with its text. So a task's text node, being its child, sits one level in and lines up with the task's text, as in TaskPaper (D-31).

| Block (FR-BLK) | Marker | Text |
|---|---|---|
| Project | none | bold heading in the accent color (Textual's `$primary`), ending in `:` as in the file |
| Task | `-`, done `✔` | normal; done tasks dimmed and struck through (falls back to dim), their `✔` dimmed too (D-36) |
| Text | none | slightly dimmed; less dim than done tasks and the other dimmed blocks (D-37). A line carrying `@code` is text like any other (D-43) |
| Section | none | bold label in the headers color (Textual's `$secondary`), no rule; children indented below |
| Divider | none | a horizontal rule across the pane in the rule-lines color, dimmed further so it is fainter than the header rule (D-33) |

- Tasks (normal text), projects and sections (their own colors) stand out; every other block type is dimmed (D-29).
- **Completed blocks** (FR-DONE-01, D-40): any block can be done. A done block keeps its type's marker and color and is dimmed and struck through (falls back to dim), its marker dimmed with it; a done task's marker is `✔` (D-36). Every block inside a done block, at any depth, is dimmed, marker included, but not struck through, since only the done block carries `@done`. The same holds when zoomed: the title row and the rows below it are dimmed when the zoom block or a block above it is done. A divider is already very dim and looks the same when done. The breadcrumb header keeps its style; the title row shows the zoom block's state.
- No fold marker is shown on expanded blocks, since any block can be folded. A block that has children and is collapsed shows `▸ n` (hidden child count) after its text and icons, projects included (D-29).
- **Tags are never shown** (FR-TAG-01). Their effects are: dates as compact chips (`󱙬 tomorrow`, FR-TAG-03, D-46), `@today` as a star and `@bookmark` as a bookmark icon right after the text, a space before each, the star first, both dimmed with a done block like the date chips and not shown on the block being edited (FR-TAG-04, D-47), a color tag tints the block's text but not a task's marker, keeping the text's other styles (FR-TAG-05). Bookmarks are also listed in the sidebar. Comments show as a count chip ([special-nodes.md §2.2](special-nodes.md#22-display)).
- **Date chips** (FR-TAG-03, D-46): right-aligned at the end of the row, the start date before the due date, each a calendar icon (§7) and the date named relative to today's local date: `today`, `tomorrow`, `yesterday`; the weekday (`Tue`) 2 to 6 days ahead; `next Tue` 7 to 13 days ahead; `in 15 days` further ahead; `3 days ago` further back. Start chips are green and due chips red. A due date before today's local date is overdue, unless the block is done or inside a done block, whose chips are dimmed with it; an overdue chip has its own icon and is bold and reversed. A tag whose value is not a date shows no chip. The block being edited shows none, and chips are drawn over text that reaches them. Chips follow the local date: the screen is redrawn at local midnight while a date chip is shown.
- **Inline formatting** (FR-FMT-01): `**bold**` renders bold and `` `code` `` in the code style with the markers hidden in navigate mode; edit mode shows the markers.
- **Links** (FR-BLK-06, FR-FMT-03, D-45), in every block but a divider and in navigate mode only; edit mode shows the raw text: bare `http(s)://` URLs, `<url>` and `[title](url)` show their title (or URL) in the link style, underlined in the links color over the block's own style, a color tag's tint included, and hide `[`, `](url)`, `<` and `>`. A line carrying `@url(…)` draws its whole title in the link style, its tag hidden like any other.
- **Markdown headings in text blocks** (FR-FMT-03, [editing.md §2.2](editing.md#22-markdown-in-text)), navigate mode only; edit mode shows the raw text: headings hide their `#` markers. Level 1 is bold in the accent color, level 2 bold in the headers color, levels 3 to 6 bold; a color tag's tint replaces the heading's color. Heading rows keep the block's indentation and get no rule, so they never look like a section.
- **Focus and selection**: the focused block is marked by a `▎` in the accent color in a 2-column gutter left of the outline, on its line (D-30). That line, gutter included and across the pane's full width, also gets the current-line background, a step brighter than the normal one, in navigate and edit mode alike; it changes no text color or style, so dimmed, done and colored blocks look the same as unfocused (D-39). Until themes arrive it is the 256-color grey 235, chosen for dark terminals. Selected blocks get a dimmer marker.
- **Edit mode**: the focused block's text becomes a one-line inline editor at the same position, with a visible caret and the markers shown (FR-MODE-02). Text wider than the pane scrolls to keep the caret in view.
- **Wrap** (FR-UI-05): on, long text wraps under its start column; off, every block takes one row and overflowing text ends in `…`. Widths use `unicode-width`; graphemes are never split.

## 3. Themes (FR-UI-06–08)

- Built-in themes: at least a light and a dark theme that use the terminal's 16 ANSI colors (so they follow the user's palette), plus the twelve truecolor themes of FR-UI-08 ([palettes](../reference/themes.md)), used only when `COLORTERM` indicates support.
- A theme defines colors for: text, dim text, accent, selection, focus, current line (the background of the focused block's lines, D-39), headers, rule lines, code, links, each of the six tag colors, start and due chips, overdue, errors.
- The theme picker in Go to Command previews each theme live as it is highlighted; `Enter` keeps it and saves it to the config (OQ-18), `Esc` restores the previous one.
- `NO_COLOR` set: no colors, only bold, dim, italic, underline, strikethrough and reverse.
- Unicode markers have ASCII fallbacks (`v`, `>`, `[x]`, `-`, `~` for `⌂`) when the locale is not UTF-8.
- The settings window (§8) previews themes live in the same way.

## 4. Shortcuts panel (FR-KEY-04)

`?` toggles a panel in the secondary sidebar on the right (§1, D-28), listing the most useful bindings for the current mode and context (navigate, edit, sidebar, picker), grouped as in [keybindings.md](keybindings.md) and showing user overrides. It notes when `Alt` keys have never been received (macOS Option setting) and whether the kitty keyboard protocol is active.

## 5. Context menu (FR-KEY-06)

A popup anchored at the focused element (or at the pointer for right-click) listing every command available for that element, grouped (Edit, Structure, Metadata, View, Panes), each with its key binding right-aligned. Commands that do not apply (for example Open URL on a block without one) are omitted. Entries come from the core's command list for that context, so the menu, the command palette and the `?` panel always agree.

The type menu of edit mode ([editing.md §2.3](editing.md#23-type-menu), D-44) is a smaller popup of the same kind: a bordered box titled "Node type:" on the line below the caret, starting at the caret's column, or above the caret's line when there is no room below, moved left to stay on screen. It lists the matching type names one per line, the highlighted one reversed, and shows `/` and the filter in its bottom border once one is typed.

## 6. Mouse (FR-KEY-05, D-16)

- Click a block: focus it (navigate mode); click inside the text of the block in edit mode: place the caret.
- Click a fold marker: toggle fold. Click a crumb: zoom the pane there. Click a sidebar entry: go there. Click a pane header: focus that pane.
- Alt+click a block, an internal link (once links exist, FR-OPS-10) or a crumb: open it in a new pane.
- Right-click a block, the selection, a sidebar entry, a crumb or a pane header: open its context menu (FR-KEY-06).
- Wheel: scroll the pane under the pointer without moving focus.
- No feature depends on the mouse; every action above has a key.

## 7. Icons (FR-UI-09, FR-UI-10)

- `icons = "nerd"` (default) draws Nerd Font glyphs; `icons = "none"` draws the Unicode markers of §2. When the locale is not UTF-8, both settings draw the ASCII fallbacks of §3.
- Every icon has exactly one fallback, defined next to it in one table in `ui::theme`, so the two sets cannot drift. Icons are drawn only by the UI; the core reports types and states, never glyphs. The table holds the icons drawn so far, Nerd Font glyphs only until the setting exists: the date chips' start `󱙭` (fallback `↦`), due `󱙬` (`⇥`) and overdue `󰨱` (`!`) (D-46), and the `@today` star `󰓎` (`★`) and the bookmark `󰃀` (`⚑`) after a block's text (D-47).
- Icons cover: the breadcrumb root (`⌂`), project fold state, task and done task, collapsed-children count, date chips (start, due, overdue), `@today`, bookmarks and the Today view in the sidebar, the status line's mode and save state, and the entries of pickers, the context menu and the settings window.
- Nerd Font glyphs are taken from the Nerd Fonts v3 code points (not the retired v2 Material Design range) and must be one cell wide in the terminals of N-PORT-01, so layout and alignment are identical with icons on and off; widths come from the same `unicode-width` measurement as text, and a glyph that measures two cells is not used.
- The app cannot detect whether the terminal font has the glyphs. The `?` panel and the settings window's icon entry say how to turn icons off, and the settings window's own labels are plain text, so it stays usable when glyphs render as boxes.

## 8. Settings window (FR-SET-02, FR-SET-03)

- Opened with `g,` in navigate mode or **Open Settings** in Go to Command; a centered overlay like the pickers, but up to 100×30.
- Left column: groups (Appearance, Editing, Files). Right: that group's settings, one per row, with name, current value and a one-line description; values that differ from the default are marked.
- Settings in v1: Appearance — theme (FR-UI-06), icons (§7), word wrap (FR-UI-05), animations (§9); Files — autosave delay and backups: on/off, folder, number kept ([file-io.md](file-io.md#2-saving)). New settings are added to this window when they are added to the config file.
- Keys: `j`/`k`/arrows move, `Tab` switches between groups and settings, `Enter` or `Space` toggles a boolean or opens a value picker or number prompt, `h`/`l` cycle choices, `r` resets the focused setting to its default, typing `/` filters settings by name, `Esc` closes.
- Every change applies at once (theme and icons repaint the whole UI behind the overlay) and is written to `config.toml` immediately (FR-SET-04). A failed write shows an error in the window and the status line and keeps the value for the session.
- A footer shows the config file's path, so people know where to edit by hand.

## 9. Animation (FR-UI-11)

Animation exists to show where focus landed, what a command changed and that an action was refused. It never makes the user wait. Research and sources: [reference/tui-animation.md](../reference/tui-animation.md).

### 9.1 Rules

- **Never in the way.** Every input is applied at once: state, layout, focus, scroll position and what the next key acts on change in the frame the input arrives (N-PERF-02). Effects are drawn over that final frame. Nothing waits for an effect to finish, and no input is queued, delayed or dropped because of one.
- **Cut short.** An input that changes a pane ends the effects running in it, drawn at their final frame. A new effect replaces a running one of the same kind on the same target.
- **Short and small.** 100–250 ms with an ease-out curve, except the status-message fade and the spinner. An effect covers only the cells it is about (a row, a pane body, the status line); only the theme crossfade covers the screen.
- **Frames only while needed.** While an effect runs, the UI redraws at up to 60 fps; with none running it requests no timer ticks (N-PERF-03). Processing effects fits in the N-PERF-02 frame budget.
- **Synchronized output.** Every frame, animated or not, is wrapped in synchronized output (DEC mode 2026, crossterm `BeginSynchronizedUpdate`/`EndSynchronizedUpdate`) so the terminal never shows half a frame. Terminals that do not support it ignore it.
- **Color depth.** Fades interpolate only between RGB colors. When the colors involved are ANSI or 256-color indexes, a flash shows its target style for its duration and then reverts in one step, and a fade is skipped. With `NO_COLOR`, flashes and fades are skipped; the status line still reports refusals.
- **Not animated:** single-step focus motion, scrolling of any kind (paging is not smooth-scrolled, since that would show offsets that are not final), typing and caret movement, entering and leaving edit mode, opening and closing pickers, the command palette, the context menu, the `?` panel and the sidebar, collapsing, closing or resizing panes, startup and quit. These must feel instant, and they are the most frequent actions.

### 9.2 Where the app animates

| ID | When | Effect | Duration | Reduced | Priority |
|---|---|---|---|---|---|
| UI-ANIM-01 | Focus **jumps**: any focus change other than a one-block motion, such as `gg`/`G`, page up/down, Go to Symbol, a sidebar entry, Move to, back/forward, zoom in/out, focusing another pane, or undo/redo moving focus. | Landing flash: the focused block's lines start with a faint accent-color background that fades out, leaving the gutter marker (D-30). | 250 ms | One-step flash | Must |
| UI-ANIM-02 | A command changes blocks other than just the focused one: paste, duplicate, Move to, an operation on a multi-block selection, undo, redo, or a reload after an external change (F-FILE-06). | Change highlight: every visible changed row is tinted in the accent color, fading out. | 250 ms | One-step flash | Must |
| UI-ANIM-03 | The core reports an intent as refused, such as typing `@` at the start of a word ([editing.md](editing.md)). Commands the specs define as no-ops are not refusals. | Error flash on the focused row, alongside the intent's status message when it has one. | 150 ms | One-step flash | Must |
| UI-ANIM-04 | A block is completed or uncompleted. | The new style (check mark, dim, strikethrough, and the dimming of the blocks inside it) applies at once; the text color eases from the old style to the new. When completed blocks are hidden, the block disappears at once with no exit effect. | 200 ms | None | Should |
| UI-ANIM-05 | A pane's root changes (zoom in/out, go home, back/forward) or a pane opens. | The pane body fades in from the background, from the top row down; the new last crumb gets a landing flash. | 120 ms | None | Should |
| UI-ANIM-06 | A block is expanded. | The newly shown rows fade in. | 120 ms | None | Should |
| UI-ANIM-07 | A transient status message reaches the end of its display time. | It fades out instead of vanishing; the UI keeps a copy of the message to draw while it fades. | 300 ms | Vanishes | Should |
| UI-ANIM-08 | The user waits longer than 300 ms on an operation, such as opening a large file, reloading or Save As. | A spinner in the status line until it finishes. | While waiting | Static `…` | Must |
| UI-ANIM-09 | The previewed theme changes, in the theme picker (FR-UI-06) or the settings window (§8). | Crossfade from the old theme to the previewed one when both are truecolor themes; otherwise instant. | 150 ms | None | Should |

### 9.3 Setting

- `animations = "full" | "reduced" | "off"` in `config.toml` (FR-SET-01). It is listed in the settings window's Appearance group (§8), and Go to Command has commands to switch it; both write it back to the config. There is no key binding.
- **Full** runs everything in §9.2. The settings window shows the default that applies to the current session. **Reduced** keeps only the informational flashes as one-step flashes (UI-ANIM-01–03) and a static spinner; no fades or crossfades. **Off** draws no effects and a static spinner.
- Default: `full`, or `reduced` when the config does not set it and the session runs over SSH (`SSH_CONNECTION` or `SSH_TTY` is set), because every animation frame crosses the network.

### 9.4 Implementation

- `ui::animation` owns a tachyonfx `EffectManager` and applies effects to the buffer after the widgets render ([architecture.md](architecture.md)). Effects are keyed by kind and target so a new one replaces the old.
- The core has no animation state, timing or durations (code-quality.md, UI-independent core). The UI derives triggers by comparing the view state before and after each update (focus, pane roots, fold state, theme), plus two facts the core reports: the blocks the last change touched, and that the last intent was refused.
- Effects advance by the elapsed time the event loop measures between frames, and tachyonfx output depends only on that time, so tests drive animations with a fixed clock.
- Tests: UI tests render through `TestBackend` at chosen elapsed times. They check that each effect's final frame equals the frame drawn with animations off, that input during an effect takes effect in the same frame, that no ticks are requested when nothing runs, and each fallback in §9.1. Tests not about animation use `off`.
