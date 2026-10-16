+++
title = "Panes"
weight = 110
description = "Side-by-side columns onto one outline: opening, focusing, moving, resizing and closing panes."
+++

tpt can show the outline in several **panes**, columns side by side. Each pane is a view of the same document: an edit in one shows in all of them at once. Each pane has its own zoom, breadcrumbs, [history](@/reference/navigation.md#history), focus, selection and [show completed](@/reference/tags.md#showing-completed-blocks) setting. Folds are shared: folding a block folds it in every pane.

Only the focused pane shows its focus marker, and the others' breadcrumbs are dimmed. While there is more than one pane, each header ends in the pane's number. Only the focused pane can be in edit mode: switching panes ends editing.

## Opening panes

- **Open in New Pane** (`Ctrl+W v`, or `Ctrl+\` with the kitty protocol) opens the focused block, zoomed in, in a new pane right of the focused one, and focuses it.
- **Open Location in New Pane** (`Ctrl+W s`) opens the place the focused pane shows, its zoom or a special view, in a new pane, on the same block.
- From the [Navigator](@/reference/navigator.md#the-navigator), Go to Symbol and Go to Project, `Alt+Enter`, `Ctrl+V` or `Ctrl+T` opens the chosen entry in a new pane.
- `Alt`+click opens a block or a breadcrumb in a new pane.

A new pane starts with the focused pane's show-completed setting and an empty history.

## Moving between panes

- `Alt+1` … `Alt+9`, or `g1` … `g9`, focus pane 1 to 9.
- `Ctrl+W h` / `Ctrl+W l`, `Ctrl+W ←` / `Ctrl+W →`, `Ctrl+W W` / `Ctrl+W w`, `gp` / `gn`, and `Ctrl+Shift+←` / `Ctrl+Shift+→` focus the pane to the left or right. They stop at either end.
- Clicking a pane focuses it.

`Ctrl+W H` and `Ctrl+W L` move the focused pane one place left or right, swapping it with its neighbor. The pane keeps the keys, so its number changes.

## Width

Panes share the screen's width equally, at least 30 columns each. `Ctrl+W <` / `Ctrl+W >`, or `Alt+Shift+H` / `Alt+Shift+L`, make the focused pane 4 columns narrower or wider. A pane you resized keeps its width, and the others share the rest. When the panes don't fit the screen, the row of panes scrolls sideways to keep the focused one in view.

## Closing

`Ctrl+W c` or `Ctrl+W q` closes the focused pane and focuses its left neighbor, or its right one when it was first. The last pane can't be closed.

When a block a pane is zoomed into goes away, deleted in any pane or changed on disk, that pane zooms out to the nearest ancestor that still exists.

The panes, with their zooms, histories, widths and show-completed settings, come back the next time you open the same file. See [Pane layout](@/reference/files.md#pane-layout).

{{ keys(topic="panes") }}
