# Spec: Navigation, panes and sidebar

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
- The UI scrolls each pane to keep the focused block visible, with a margin of 2 rows when space allows. Page up/down messages carry the pane's height in rows.

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
- Every zoom change pushes onto the pane's history; `back` / `forward` walk it, skipping blocks that no longer exist.
- The breadcrumb bar shows the zoom block's ancestors, truncated from the left with `…` when too wide. Each crumb is clickable (FR-KEY-05).

## 5. Panes (FR-UI-02 – FR-UI-04, D-13)

- Panes are columns, left to right. Each has its own zoom, breadcrumbs, history, focus, selection, mode, show-completed setting and, later, search. Folds are shared. Edits in one pane appear in all panes immediately.
- `open_in_new_pane`: open the focused block (or, from a picker or the sidebar, the chosen entry; or with Alt+click, the clicked block, link or crumb) zoomed in a new pane inserted right of the focused pane, and focus it. `open_location_in_new_pane` opens the focused pane's current location (its zoom block or Today) in a new pane, focusing the same block. A new pane starts with the focused pane's show-completed setting and an empty history, and focuses the first block under its title, or the title when it has none.
- `focus_pane N` focuses the Nth pane; `focus_pane_left/right` moves between neighbors; `close_pane` closes the focused pane and focuses its left neighbor, or its right one when it was first (the last pane cannot be closed). Switching panes ends editing, so only the focused pane is ever in edit mode.
- When a change removes the block a pane is zoomed into (deleted in any pane, or changed on disk), that pane zooms to the nearest ancestor that still exists and its focus and selection are repaired as in §2.
- Width (UI layer): panes share the width equally by default, with a minimum of 30 columns each, but none wider than the room for panes. `narrow`/`widen` change the focused pane's width by 4 columns. A narrowed or widened pane keeps its width, which the core stores with the pane as the frontend reports it, while the others share what is left. When the panes' total minimum width exceeds the terminal, the pane strip scrolls horizontally to keep the focused pane fully visible.
- The layout (number of panes, their zooms, histories, widths and show-completed settings) is restored on the next launch of the same file (FR-UI-04), stored per OQ-05 default A; fold state is not (D-26) ([file-io.md §5](file-io.md#5-view-state-persistence)).

## 6. Sidebar (FR-UI-01, D-7, D-11, D-28)

A panel on the left with three sections, top to bottom:
1. **Bookmarks**: every block tagged `@bookmark`, in document order; later also bookmarked searches (FR-SRCH-03).
2. **Today**: the built-in view (§7).
3. **Projects**: all project blocks as a tree, nested by project ancestry (non-project blocks in between are skipped), like TaskPaper.

`Enter` (or a click) zooms the focused pane to the entry; `Alt+Enter` (or Alt+click) opens it in a new pane. `toggle_sidebar` shows or hides it; it hides automatically below 80 columns. The `?` shortcuts panel opens in a secondary sidebar on the right instead ([ui.md §1](ui.md#1-layout), D-28).

## 7. Today view (FR-TAG-04, D-7)

A built-in list of every block that has `@today`, or `@due` or `@start` on or before today's local date (FR-TAG-04), in document order unless the user reordered it (D-57). Done blocks follow the pane's show-completed setting (FR-DONE-02), like any other view: while completed blocks are hidden, a done block leaves the list with every block inside it. Choosing Today in the sidebar, or Go to Today (`gt`, D-64), shows this list in the focused pane instead of an outline, under a header naming it ([ui.md §1](ui.md#1-layout)); it is a location in the pane's history like a zoom, entered focusing the block zoomed out of when it is an entry, and left focusing the focused entry when the new location shows it. Entries are flat, each with its breadcrumb path. Navigation, edit mode and block commands (done, dates, today, color) work on the entries directly, as do selection, delete, duplicate, copy and undo; an entry that stops matching leaves the list, its focus moving as in §2. `zoom_in` (`Alt+l`/`gz`, and `.` and `Alt+→`) on an entry goes to it in the outline: the pane zooms to its parent and focuses it, as Go to Symbol does (§8). The entries have no tree around them, so commands that need one do nothing in the Today view: folding, moving the focus to a parent, child or sibling, `zoom_out`, adding, grouping or pasting blocks outside edit mode, and indenting; `Enter` at the end of an entry being edited only ends editing. `move_up`/`move_down` reorder the list instead (D-57): the selected entries, else the focused one, move a step up or down past the entry next to them, runs of selected entries together, and one at the end it moves toward stays. The order is the app's, never the file's: the document does not change, nothing is saved and there is no step to undo. Every pane showing Today shares it, and an entry a pane hides keeps its place. Entries never moved follow the moved ones in document order, so a block that starts matching comes last, and a block that leaves the list keeps its place for when it returns. The order is kept with the layout ([file-io.md §5](file-io.md#5-view-state-persistence)). The list is recomputed on every document change and at local midnight.

## 8. Go to Symbol and Go to Command (FR-NAV-05, FR-NAV-06)

- **Go to Symbol**: a fuzzy picker (`nucleo-matcher`) over every block's display text, each shown with its breadcrumb path. Bookmarks rank first, in document order as the sidebar lists them, then projects and sections, then the rest, with an empty query and while filtering; within each group, better matches come first. Tasks show their outline markers and a bookmarked block's text is in the primary color (D-70). Choosing a block zooms the focused pane to its parent and focuses it; `Alt+Enter` opens it in a new pane.
- **Go to Project** (D-65): Go to Symbol over the project blocks alone, each with its breadcrumb path, bookmarked projects first, their text in the primary color (D-70), titled "Go to Project". Choosing one and `Alt+Enter` work as in Go to Symbol.
- **Go to Command**: a fuzzy picker over every command name with its current binding; `Enter` runs it. The theme picker opened from here previews themes live (FR-UI-06).
