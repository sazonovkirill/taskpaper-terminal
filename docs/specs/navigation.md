# Spec: Navigation, panes and the Navigator

Covers FR-NAV, FR-DONE-02, FR-UI-01–04 and the navigation half of FR-KEY-05. Key bindings are in [keybindings.md](keybindings.md); drawing is in [ui.md](ui.md).

## 1. Visible rows of a pane

A pane's rows come from a depth-first walk over **blocks** ([data-model.md §3](data-model.md#3-blocks)) starting at the children of the pane's zoom block:

1. Don't descend into collapsed blocks. Fold state is shared by all panes (D-13).
2. If the pane hides completed blocks (FR-DONE-02), skip done blocks and their whole subtrees.
3. When search exists (Later, FR-SRCH-01): show matching blocks plus their ancestors up to the zoom block, ancestors shown only for context dimmed; non-matching descendants hidden; fold state ignored while searching but kept.

Each row carries: block id, display depth (relative to the zoom block) and, when it is collapsed with children, how many open tasks its fold hides (D-67). Wrapping and row heights are computed by the UI layer. The row list is recomputed only when the document revision, the folds or the pane change.

## 2. Focus and motion (FR-NAV-01)

- In navigate mode each pane has one **focused block**. Motions: next/previous visible block, parent, first/last child, next/previous sibling, first/last block, page up/down.
- If the focused block disappears (collapsed ancestor, hidden as done, deleted, changed in another pane), focus moves to the next visible block, else the previous, else the parent.
- The UI scrolls each pane to keep the focused block visible, with a margin of 2 rows when space allows. Page up/down messages carry how many rows a page passes: the rows it moves past, from the focused one down or from the one above it up, that fill no more than the pane's height in screen lines, and at least one, so a page of wrapped rows skips no line the pane did not show (FR-UI-05).

### 2.1 Selection (FR-OPS-01)

- Besides its focus, each pane has a **selection**: a set of blocks the next command acts on, drawn with a dimmer focus marker ([ui.md §2](ui.md#2-blocks)). Block and structural commands act on the selected blocks, or on the focused block when none is ([editing.md §4–5](editing.md#4-structural-operations-fr-ops)).
- `extend_down` / `extend_up` move the focus a row and select every row from where the run of extensions began to the focus, on top of what was selected before the run; moving back shrinks the run. A run begins at the focus unless the focus is still where the latest run ended. At the first or last row the focused row is selected.
- `toggle_selected` adds the focused block to the selection or takes it out. Plain motions move the focus and keep the selection, so blocks far apart can be picked one by one.
- `select_all` selects every row below the title; `clear_selection` selects nothing.
- A block no row shows, because it was deleted, folded away or left outside the zoom, leaves the selection. Entering edit mode clears it.

## 3. Folding (FR-NAV-02)

| Command | Effect |
|---|---|
| `collapse` | Collapse the focused block; if it is a leaf or already collapsed, move focus to its parent. |
| `expand` | Expand the focused block; if already expanded, move to its first child. |
| `toggle_fold` | Expand if collapsed, else collapse. |
| `expand_subtree` / `collapse_subtree` | The focused block's entire subtree. |
| `toggle_subtree` | `collapse_subtree` if any block in the focused block's subtree, the block included, is expanded and has children; otherwise `expand_subtree`. |
| `expand_all` / `collapse_all` | Every block under the pane's zoom block. |
| `expand_level` / `collapse_level` | Every block under the pane's zoom block, one level at a time, like TaskPaper's Expand/Collapse All By Level (D-26): expand every collapsed block at the shallowest depth that has one; collapse every expanded block with children at the deepest depth that has one. |

Fold state is kept in memory only and is shared by all panes (D-13); it is not saved, and a file opens with every block expanded (D-26).

## 4. Zoom and history (FR-NAV-03, FR-NAV-04)

- `zoom_in`: the focused block becomes the pane's root; its children are shown one level in (depth 1) under a title row at depth 0 showing the block itself (D-31) (editable like any block). Zooming into a leaf shows an empty page with focus on the title.
- `zoom_out`: zoom to the parent block and focus the block you were zoomed into.
- `go_home`: zoom to the document root.
- Every zoom change pushes onto the pane's history; `back` / `forward` walk it, skipping blocks that no longer exist and, so that every step goes somewhere, a visit of the place already shown, which that skipping brings next to it (as in a stored layout, [file-io.md §5](file-io.md#5-view-state-persistence)).
- The breadcrumb bar shows the zoom block's ancestors, truncated from the left with `…` when too wide. Each crumb is clickable (FR-KEY-05).

## 5. Panes (FR-UI-02 – FR-UI-04, D-13)

- Panes are columns, left to right. Each has its own zoom, breadcrumbs, history, focus, selection, mode, show-completed setting and, later, search. Folds are shared. Edits in one pane appear in all panes immediately.
- `open_in_new_pane`: open the focused block (or, from a picker or the Navigator, the chosen entry; or with Alt+click, the clicked block, link or crumb) zoomed in a new pane inserted right of the focused pane, and focus it. `open_location_in_new_pane` opens the focused pane's current location (its zoom block or Today) in a new pane, focusing the same block. A new pane starts with the focused pane's show-completed setting and an empty history, and focuses the first block under its title, or the title when it has none.
- `focus_pane N` focuses the Nth pane; `focus_pane_left/right` moves between neighbors, stopping at either end (D-71, D-79); `move_pane_left/right` swaps the focused pane with that neighbor, keeping the keys, so its number changes, and does nothing at that end (D-120); `close_pane` closes the focused pane and focuses its left neighbor, or its right one when it was first (the last pane cannot be closed). Switching panes ends editing, so only the focused pane is ever in edit mode.
- When a change removes the block a pane is zoomed into (deleted in any pane, or changed on disk), that pane zooms to the nearest ancestor that still exists and its focus and selection are repaired as in §2.
- Width (UI layer): panes share the width equally by default, with a minimum of 30 columns each, but none wider than the room for panes. `narrow`/`widen` change the focused pane's width by 4 columns. A narrowed or widened pane keeps its width, which the core stores with the pane as the frontend reports it, while the others share what is left. When the panes' total minimum width exceeds the terminal, the pane strip scrolls horizontally to keep the focused pane fully visible.
- The layout (number of panes, their zooms, histories, widths and show-completed settings) is restored on the next launch of the same file (FR-UI-04), stored per OQ-05 default A; fold state is not (D-26) ([file-io.md §5](file-io.md#5-view-state-persistence)).

## 6. Navigator (FR-UI-01, D-7, D-11, D-28, D-79)

The Navigator is the list of places to go, the inbox, the special views, bookmarks and projects, in a window that Open Navigator (`Ctrl+G`, `open_navigator` in `config.toml`) opens over the panes; there is no sidebar on the left (D-79). It was the sidebar until D-79, so the code and the settings' keys in `config.toml` still call it that. It lists three sections, top to bottom:
1. **Today**, then **Upcoming**, **Overdue**, **Colors**, **Logbook**, **Starred** and **Flagged** (§7.1, D-80, D-92, D-93), each with how many open tasks it lists (D-85): Today is the built-in view (§7), always listed (D-102), and each of the others is listed unless its setting leaves it out.
2. **Bookmarks**: every block tagged `@bookmark`, in document order, each with how many open tasks it holds at any depth, counted as for a project (D-85); later also bookmarked searches (FR-SRCH-03).
3. **Projects**: all project blocks as a tree, nested by project ancestry (non-project blocks in between are skipped), like TaskPaper, each with how many open tasks it holds at any depth, counted as for a collapsed block (D-67, D-73).

**Inbox** (D-75): above Today, the Navigator lists the first shown top-level project whose text is exactly `INBOX` (`INBOX:` in the file, in capitals), maybe after emoji and a space (`📥 INBOX:`, D-102), behind the inbox tray icon (ui.md §7) and with its open-task count, and leaves it and the projects inside it out of the Projects tree. It has no fold. `special_inbox = false` in `config.toml`, or **Special handling of INBOX project** in the settings window (ui.md §8), lists it among the projects instead; it is on by default. `gi` (Go to Inbox) zooms the focused pane to it from anywhere, as choosing its entry does; when the Navigator lists no inbox, it changes nothing and the status line says "No INBOX project in the Navigator" (D-78).

Typing filters the window (D-86): it lists only the entries whose text holds what was typed, ignoring case, with the project entries above each matching project so the tree stays readable, looking through folds, and focuses the first match; `Backspace` erases, and with no filter left the window lists everything again, focused as it opened. Folding does nothing while a filter is typed, so project entries show no fold marker then, and closing the window drops the filter. `Backspace` on an empty filter does nothing.

`Enter` (or a click) zooms the focused pane to the entry and closes the window; `Alt+Enter` (or Alt+click) opens it in a new pane and closes the window. The window keeps the keys while it is open, and `Esc` or Open Navigator again closes it, leaving the pane as it was. It opens on the entry of where the focused pane is, as an editor's file tree follows the open file (D-68): the entry of its Today view or zoom block, else the entry of the closest block above the zoom block that has one, such as the project holding a zoomed task or a folded entry over a zoomed project; a block with both a bookmark and a project or inbox entry is at the project or inbox entry. With no such entry it opens on the first, and with no entries at all (no inbox, Today left out, no bookmarks or projects) it says there is nothing to go to. The entry of where the pane is keeps a selected block's marker while the focus moves to other entries. Its folds are its own, kept while it is closed and not saved. The `?` shortcuts panel opens in a secondary sidebar on the right ([ui.md §1](ui.md#1-layout), D-28).

## 7. Today view (FR-TAG-04, D-7)

A built-in list of every block with `@due` or `@start` on or before today's local date, and of every starred block (one with `@star`) while **Display starred items in Today view** (`today_starred`, on by default, ui.md §8) is on (FR-TAG-04, D-88), and of every task inside the TODAY project, at any depth, while `special_today` (on by default, ui.md §8) is on (D-102), in document order unless the user reordered it (D-57). Done blocks follow the pane's show-completed setting (FR-DONE-02), like any other view: while completed blocks are hidden, a done block leaves the list with every block inside it. Choosing Today in the Navigator, or Go to Today (`gt` or `gT`, D-64, D-95), shows this list in the focused pane instead of an outline, under a header naming it ([ui.md §1](ui.md#1-layout)); it is a location in the pane's history like a zoom, entered from the outline focusing the block you were on when it is an entry, else the block zoomed out of when that is one (D-114), so `back` after `zoom_in` on an entry returns to that entry, and left focusing the focused entry when the new location shows it. Entries are flat, each with its breadcrumb path. Navigation, edit mode and block commands (done, dates, star, color) work on the entries directly, as do selection, delete, duplicate, copy and undo; an entry that stops matching leaves the list, its focus moving as in §2, though one being edited stays in its place until editing ends ([editing.md §2.3](editing.md#23-command-menu), D-105). `zoom_in` (`Alt+l`/`gz`, and `.` and `Alt+→`) on an entry goes to it in the outline: the pane zooms to its parent and focuses it, as Go to Symbol does (§8). The entries have no tree around them, so commands that need one do nothing in the Today view: folding, moving the focus to a parent, child or sibling, `zoom_out`, adding, grouping or pasting blocks outside edit mode, and indenting; `Enter` at the end of an entry being edited only ends editing. `move_up`/`move_down` reorder the list instead (D-57): the selected entries, else the focused one, move a step up or down past the entry next to them, runs of selected entries together, and one at the end it moves toward stays. The order is the app's, never the file's: the document does not change, nothing is saved and there is no step to undo. Every pane showing Today shares it, and an entry a pane hides keeps its place. Entries never moved follow the moved ones in document order, so a block that starts matching comes last, and a block that leaves the list keeps its place for when it returns. The order is kept with the layout ([file-io.md §5](file-io.md#5-view-state-persistence)). The list is recomputed on every document change, at local midnight and when either setting changes.

### 7.1 Other special views (FR-TAG-07, D-80)

Upcoming, Overdue, Colors, Logbook, Starred and Flagged are special views like Today: everything above holds for them except reordering, which only Today has, so `move_up`/`move_down` do nothing in them. Their entries may be grouped. A heading line above the first entry of each group names it; it is drawn, not a block, so focus, selection, motion and paging skip it, and an empty group has no heading. Each view has its own icon, shown before its name in its Navigator entry and its header, as Today's sun is ([ui.md §1, §7](ui.md#7-icons-fr-ui-09-fr-ui-10)). Each view's Go to key is `g` then a capital letter (D-95).

| View | Lists | Groups, in order | Go to | Setting (on by default) |
|---|---|---|---|---|
| Upcoming (D-81) | blocks with `@start` or `@due` from tomorrow to a week from today, once, under the earliest such date; done blocks per show-completed | days, earliest first; heading `Tomorrow · Thu 8 Oct`, or only `Wed 14 Oct` for a day the chip names by its weekday (D-111), with the year for a day of next year, `Fri 1 Jan 2027` | `gU` | `sidebar_upcoming` |
| Overdue (D-82) | open blocks (not done, not inside a done block) with `@due` before today | due dates, oldest first; heading `3 days ago · Sun 4 Oct`, with the year for a day of another year, `365 days ago · Thu 9 Oct 2025` | `gO` | `sidebar_overdue` |
| Colors (D-83) | blocks with a color tag; done blocks per show-completed | red, orange, yellow, green, blue, violet; heading the color's swatch and name | `gC` | `sidebar_colors` |
| Logbook (D-84) | blocks with `@done(DATE)` from six days ago to today, inside done blocks too, whatever show-completed says | days, newest first; heading `Yesterday · Tue 6 Oct` | `gL` | `sidebar_logbook` |
| Starred (D-92) | starred blocks (`@star`, D-88), ungrouped; done blocks per show-completed | none | `gS` | `sidebar_starred` |
| Flagged (D-93) | flagged blocks (`@flag`, D-91), ungrouped; done blocks per show-completed | none | `gF` | `sidebar_flagged` |

Within a group, entries are in document order. The Navigator lists the views after Today in the table's order (§6), each unless its setting is off; its Go to command still opens it.

## 8. Go to Symbol and Go to Command (FR-NAV-05, FR-NAV-06)

- **Go to Symbol**: a fuzzy picker (`nucleo-matcher`) over every block's display text, each shown with its breadcrumb path. Bookmarks rank first, in document order as the Navigator lists them, then projects and sections, then the rest, with an empty query and while filtering; within each group, better matches come first. Tasks show their outline markers and a bookmarked block's icon and text are in the primary color (D-70). Choosing a block zooms the focused pane to its parent and focuses it; `Alt+Enter` opens it in a new pane.
- **Go to Project** (D-65): Go to Symbol over the project blocks alone, each with its breadcrumb path, bookmarked projects first, their icon and text in the primary color (D-70), titled "Go to Project". Choosing one and `Alt+Enter` work as in Go to Symbol.
- **Go to Command**: a fuzzy picker over every command name with its current binding; `Enter` runs it. The theme picker opened from here previews themes live (FR-UI-06).
