+++
title = "Navigator and finders"
weight = 80
description = "The Navigator window of places to go, the inbox, the Go to Symbol, Go to Project and Go to Command finders, and the context menu."
+++

## The Navigator

`Ctrl+G` or `|` opens the **Navigator**: a window over the panes listing the places to go, top to bottom:

1. The **inbox**, when there is one, then **Today** and the other [special views](@/reference/views.md): Upcoming, Overdue, Colors, Logbook, Starred and Flagged, each unless its setting leaves it out.
2. **Bookmarks:** every block you [bookmarked](@/reference/tags.md#star-flag-and-bookmark), in document order.
3. **Projects:** every project as a tree, nested by the projects they are in. Blocks between them that are not projects are skipped.

Each entry shows how many open tasks it holds at any depth, or, for a view, how many it lists. A section with no entries has no heading.

The Navigator opens on the entry for where the focused pane is: its view, its zoom block, or else the closest block above the zoom that has an entry, such as the project around a zoomed task. That entry keeps a dim marker while you move to others, like a file tree following the open file.

- `↑`/`↓` or `Ctrl+P`/`Ctrl+N` move, and `←`/`→` fold and unfold project entries. Its folds are its own, kept while it is closed, and not saved.
- `Enter`, or a click, zooms the focused pane to the entry and closes the window.
- `Alt+Enter`, `Ctrl+V`, `Ctrl+T` or `Alt`+click opens the entry in a [new pane](@/reference/panes.md).
- `Esc` or `Ctrl+G` closes it and leaves the pane as it was; `|` types into its filter. A click outside it closes it too.

**Typing filters it.** Only the entries whose text holds what you typed, ignoring case, stay, with the projects around each matching project so the tree still reads. The filter looks through folds, and shows in the window's bottom border. `Backspace` erases. Closing the window drops the filter.

The Navigator keeps the keys while it is open: keys that are not its own do nothing, so letters go to the filter. That is why its context menu is on `Ctrl+R` and not `\`. `Ctrl+Q` still quits.

{{ keys(topic="navigator", mode="navigator") }}

### The inbox

The **inbox** is the first top-level project whose text is exactly `INBOX`, in capitals: `INBOX:` in the file, maybe after emoji and a space, as in `📥 INBOX:`. The Navigator lists it above Today, behind a tray icon, and leaves it and the projects inside it out of the Projects tree.

`gi` (**Go to Inbox**) zooms the focused pane to it from anywhere. When there is no inbox, the status line says "No INBOX project in the Navigator". [Quick Add to Inbox](@/reference/quick-add.md) adds tasks to it.

Turn off **Special handling of INBOX project** to list it among the other projects instead:

{{ settings(topic="navigator") }}

## Finders

The finders are windows that list blocks or commands, filtered as you type with fuzzy matching. They share their keys:

{{ context_keys(table="MENU") }}

The same keys drive the other pickers: [Move to…](@/reference/structure.md#move-to), the type and [color](@/reference/tags.md#colors) pickers and the [theme picker](@/reference/appearance.md#choosing-a-theme).

### Go to Symbol

**Go to Symbol** (`Ctrl+K`, `gs` or `/`) lists every block, each with its path. Bookmarks come first, then projects and sections, then the rest, and better matches come first within each group. Choosing a block zooms the focused pane to its parent and focuses the block.

### Go to Project

**Go to Project** (`gP`) is Go to Symbol over the projects alone, bookmarked ones first.

In both, a block can open in a [new pane](@/reference/panes.md) instead:

{{ context_keys(table="OPEN_IN_NEW_PANE") }}

### Go to Command

**Go to Command** (`Ctrl+P` or `:`) lists every command with its keys, as you bound them. `Enter` runs the chosen one. This is how you reach the commands without a key, such as **Count Blocks and Words**, **Save As**, **Start Weather Animation** and the **Animations** settings. Each key table in this Reference says "none; Go to Command" for those.

## Context menu

`\` or `Shift+F10` (with the kitty protocol also the `Menu` key) opens the **context menu**: every command for the focused block, or for the selection, grouped and each with its keys. A right-click opens it at the pointer, on whatever is under it. Type to filter it, `↑`/`↓` move, `Enter` runs and `Esc` closes. Commands that don't apply are left out.

- **On a block:** Edit, Edit Node, Open Note and Open Comments, New Block Below, Above and as Last Child, Copy, Cut, Paste Below and Above, Duplicate and Delete; then Indent, Outdent, Move Up, Move Down, Group and Move to…; status (Complete or Uncomplete, Toggle Star, Toggle Flag, Bookmark or Remove Bookmark); dates; colors; Convert Type, with Make Section on a text block and Remove Section on a section; Zoom In, Toggle Fold for a block with children, Count Blocks and Words, and Open Link for a block with a link; then Open in New Pane, Open Location in New Pane and, while there are other panes, Close Pane.
- **On the selection:** the same, without what acts on one block alone, labeled for all of them: Uncomplete only when all are done, Clear Due Date when any has a due date.
- **In a special view:** without what the view doesn't do.
- **On a Navigator entry:** Go to Entry, Open in New Pane, Collapse or Expand Entry for a project with sub-projects, and Return to Outline.
- **On a breadcrumb**, by right-click: Zoom Here, or Go Home on `⌂`, unless the pane is there already, and Open in New Pane; then Open Location in New Pane and Close Pane.
- **On a pane header**, by right-click: Zoom Out while zoomed and Go Home away from home, then the same pane commands.

{{ keys(topic="navigator", mode="navigate") }}
