+++
title = "Selection and structure"
weight = 50
description = "Selecting several blocks, then indenting, moving, grouping, copying, deleting and undoing them."
+++

Every block carries its children with it. Indenting, moving, copying or deleting a block does the same to everything under it, its [attachments](@/reference/attachments.md) and hidden lines included. A block is always one line, so nothing ever splits or merges blocks.

## Selection

Next to its focus, each pane has a **selection**: the blocks the next command acts on. Selected blocks get the focus bar too, dimmer, so focus and selection read as one bar. With nothing selected, commands act on the focused block.

- `J` / `K` (or `Shift+↓` / `Shift+↑`) extend the selection down or up a row from where the run began. Going back shrinks the run. A new run adds to what was already selected.
- `v` adds the focused block to the selection, or takes it out. Plain motions move the focus and keep the selection, so you can pick blocks far apart one by one: `v`, `j j j`, `v`.
- `V` starts visual mode, as in vim: the focused block is selected, and every move, such as `j`, `k`, `gg` or `G`, selects all rows from there to the focus. The status line says `VISUAL`. `V` again or `Esc` ends it and clears the selection; a command such as `x`, `Tab` or `y` acts on the selected blocks and ends it, leaving them selected.
- `Ctrl+A` selects every row below the title, and `Esc` clears the selection.

A block that leaves the view, because it was deleted, folded away or is outside the zoom, leaves the selection. Entering edit mode clears it.

Every block command, such as [done, dates, star or color](@/reference/tags.md), goes to all selected blocks as one step to undo. A few act on the focused block alone, even with blocks selected: Edit Node, Open Link and the attachments.

{{ keys(topic="structure", group="Selection") }}

## Moving blocks

| Key | Does |
|---|---|
| `Tab` | **Indent:** the block becomes the last child of the sibling above it, which expands. Nothing happens on a first sibling. |
| `Shift+Tab` | **Outdent:** the block becomes the next sibling of its parent. The siblings after it stay where they are. Nothing happens at the top level of the pane. |
| `Alt+k` / `Alt+↑` | **Move Up:** swap with the sibling above. At the top of its list, the block moves to the end of the parent's previous sibling. |
| `Alt+j` / `Alt+↓` | **Move Down:** swap with the sibling below, or move to the start of the parent's next sibling. |

Selected blocks move together and keep their order. A selected block inside another selected block moves with it, not on its own. A block that cannot move stays, and so do the blocks that would have to pass it. No move takes a block out of the pane's zoom, and the zoom's title row never moves.

The moves go by what the pane shows. While the pane [hides completed blocks](@/reference/tags.md#showing-completed-blocks), a hidden done block is passed over and stays where it is. No move or indent puts a block under an empty line, which would then need placeholder text: the block looks past empty lines for a place, or stays.

### Group

**Group** (`gG`) wraps the selection, or the focused block, in a new text block placed where the first one was, and opens the new block in edit mode so you can name it. Creating and naming it is one step to undo.

### Move to…

**Move to…** (`gm`, or `Ctrl+M` with the kitty protocol) opens a finder over every block, as [Go to Symbol](@/reference/navigator.md#go-to-symbol) lists them, leaving out the blocks that move and what they hold. Type to filter, and `Enter` makes the selection the chosen block's last children. When every block that moves is a project, the finder also lists `⌂ Home` first, which moves them to the end of the document's top level.

The focus stays on a moved block. The destination expands, and a destination outside the pane's zoom becomes the zoom, a step in the [history](@/reference/navigation.md#history).

## Delete and duplicate

`dd` or `Delete` removes the blocks with everything under them. The focus moves to the next visible block, else the previous one. `D` duplicates each block with its subtree, right after the original, and focuses the copy of the last one.

## Copy, cut and paste

`y` or `Ctrl+C` copies, and `Ctrl+X` cuts, the selected blocks or the focused one. `p` or `Ctrl+V` pastes below the focused block, and `P` or `Alt+v` above it. More on the clipboard, also over SSH, is in [Copy and paste](@/reference/modes.md#copy-and-paste).

The clipboard gets TaskPaper text: each block's line with its tags, children and attachments, indented with tabs from the shallowest block. Pasting parses TaskPaper text into blocks at the focused block's depth, keeping their structure, so you can copy between tpt and any other TaskPaper app. One line of plain text pastes as a text block. On a zoomed pane's title row, the pasted blocks become its first children.

## Undo and redo

`z` or `Ctrl+Z` undoes, and `Z` or `Ctrl+Y` redoes. Each command is one step however many blocks it changed, and so is a whole edit-mode session. Undo restores attachments and hidden lines with their blocks. When the file is reloaded after an [outside change](@/reference/files.md#when-the-file-changes-on-disk), the undo history starts over.

{{ keys(topic="structure", group="Structure") }}

## Count

**Count Blocks and Words**, in [Go to Command](@/reference/navigator.md#go-to-command), says in the status line how many blocks and words are under the focused block, such as `12 blocks, 87 words`. On the title row it counts the whole zoom, and in the [Today view](@/reference/views.md#today) the focused entry. A divider's title words count, its dashes don't. Attachments and hidden lines are left out.

{{ keys(topic="structure", group="View and Panels") }}
