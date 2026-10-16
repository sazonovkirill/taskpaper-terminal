+++
title = "Navigation and zoom"
weight = 40
description = "Moving the focus, folding blocks, zooming into a block and walking back through where you have been."
+++

In navigate mode each pane has one **focused block**, marked by a bar in the gutter on its left and a brighter band across the pane. Every navigate-mode command acts on it, or on the [selection](@/reference/structure.md#selection) when there is one.

## Moving

`j` and `k` (or `↓` and `↑`) move to the next and previous visible row, whatever its depth. The other motions follow the outline's structure:

- `h` collapses the focused block. On a block that is already collapsed, or has no children, it moves to the parent instead. `l` expands the block, and on an expanded block it moves to its first child. So `h` and `l` walk up and down the tree as well as folding it.
- `Ctrl+↓` / `Ctrl+↑`, or `gj` / `gk`, jump to the next and previous sibling, skipping the children between.
- `^` and `$` go to the first and last sibling, and `#` to the parent.
- `gg` / `Home` and `G` / `End` go to the first and last visible row.
- `Ctrl+D` / `PgDn` and `Ctrl+U` / `PgUp` move a page, as many rows as fill the pane. With word wrap on, a page never skips a line the pane did not show.

When the focused block disappears, because it was folded away, hidden as done, deleted or changed in another pane, the focus moves to the next visible block, else the previous one, else the parent. The pane scrolls to keep the focus in view, with 2 rows of margin when there is room.

{{ keys(topic="navigation", group="Moving") }}

## Folding

Any block with children can be folded, not only projects. Expanded blocks show no marker. A collapsed block shows `▸` after its text, with the number of **open tasks** it hides: every task below it, at any depth, that is neither done nor inside a done block. Projects, notes, sections and dividers don't count. With no open tasks inside, it shows `▸` alone. A block whose only children are [attachments](@/reference/attachments.md) or other hidden lines has no fold at all.

| Key | Folds |
|---|---|
| `'` | the focused block: open it if it is collapsed, close it otherwise |
| `"` | the focused block's whole subtree: collapses it if anything in it is expanded, otherwise expands all of it |
| `=` / `-` | the pane, one level at a time: expands every collapsed block at the shallowest level that has one, or collapses every expanded block at the deepest level |
| `+` / `_` | everything under the pane's zoom block |

Folds belong to the document, not the pane: a block folded in one pane is folded in every pane. They are not saved, so a file always opens fully expanded.

{{ keys(topic="navigation", group="Folding") }}

## Zoom

**Zoom In** (`.` or `gz`) makes the focused block the pane's root. The pane shows it as a bold **title row** at the top, in its type's style, with its children one level in below it. The title row is a block like any other: you can focus it, edit it and run commands on it. Zooming into a block without children shows its title and nothing else. New blocks you add on the title row become its children (see [Creating blocks](@/reference/modes.md#creating-blocks)).

**Zoom Out** (`,` or `gu`) zooms to the parent and focuses the block you were zoomed into. **Go Home** (`gh`, or `Ctrl+'` with the kitty protocol) zooms back to the whole document.

When a block that a pane is zoomed into goes away, deleted in any pane or changed on disk, that pane zooms out to its nearest ancestor that still exists.

### Breadcrumbs

The first row of each pane is its **breadcrumb bar**: a house icon `⌂` for the whole document, then the path from the top of the document to the zoom block, joined by `›`. When the path is wider than the pane, its start gives way to `…`. Click a crumb to zoom there, `Alt`+click one to open it in a new pane, and right-click it for a [context menu](@/reference/navigator.md#context-menu) with Zoom Here and Open in New Pane. In a [special view](@/reference/views.md) the bar shows the view's icon and name instead.

## History

Every change of a pane's root, by zooming, going home, a [Navigator](@/reference/navigator.md) entry, a finder or a [special view](@/reference/views.md), is a step in that pane's history. **Back** (`H`, `Alt+←`, `Ctrl+O`) and **Forward** (`L`, `Alt+→`, or `Ctrl+I` with the kitty protocol) walk it, like a browser. Steps to blocks that no longer exist are skipped, and so is a step to the place already shown, so every press goes somewhere. Each pane has a history of its own, and the history is saved with the [pane layout](@/reference/files.md#pane-layout).

{{ keys(topic="navigation", group="Zoom and History") }}

The Go to keys for the special views are on [Special views](@/reference/views.md#keys), and Go to Inbox is with the [Navigator](@/reference/navigator.md#the-inbox).
