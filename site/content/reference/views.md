+++
title = "Special views"
weight = 70
description = "Today, Upcoming, Overdue, Colors, Logbook, Starred and Flagged: flat lists of blocks gathered from the whole outline."
+++

A **special view** shows a list of blocks from all over the document in a pane, in place of the outline: everything due today, everything flagged, everything done this week. Each view has its own icon, a Go to key starting with `g`, and an entry in the [Navigator](@/reference/navigator.md), each after Today, with how many open tasks it lists.

Opening a view is a step in the pane's [history](@/reference/navigation.md#history), so `H` comes back. The pane's header shows the view's icon and name in place of the breadcrumbs.

## Working in a view

The entries are the blocks themselves, not copies. Each shows its path, dimmed, after its text. Moving, edit mode and block commands work on them as in the outline: done, dates, star, flag, color, Edit Node, selection, delete, duplicate, copy and undo. When an entry stops matching, for example after `x` in the Overdue view, it leaves the list, except while you are still editing it.

There is no tree around the entries, so commands that need one do nothing in a view: folding, moving to a parent, child or sibling, Zoom Out, adding, grouping or pasting blocks, and indenting. **Zoom In** on an entry takes you to it in the outline: the pane zooms to its parent and focuses it.

Completed blocks follow the pane's [show completed](@/reference/tags.md#showing-completed-blocks) setting, as in the outline. While they are hidden, a done block leaves the list with every block inside it. The Logbook is the exception.

## Today

**Today** (`gt` or `gT`) lists, in document order:

- every block with a start or due date on or before today,
- every starred block, while **Display starred items in Today view** is on, as it is by default.

The list changes at midnight, and whenever the document or the setting changes. The Navigator always lists Today.

### Reordering Today

In Today, and only there, **Move Up** and **Move Down** (`K` and `J`, or `Alt+k` and `Alt+j`) reorder the list. The selected entries, or the focused one, move a step past the entry next to them. The order belongs to the app, not the file: the document doesn't change, nothing is saved to it and there is nothing to undo. Every pane showing Today shares it, and it is kept with the [pane layout](@/reference/files.md#pane-layout). Entries you never moved come after the moved ones in document order, so a block that starts matching comes last, and one that leaves the list keeps its place for when it comes back.

## The other views

The other views can be grouped. A bold heading above each group names it. Headings are drawn, not blocks: the focus and paging skip them.

| View | Go to | Lists | Grouped by |
|---|---|---|---|
| <span id="upcoming">**Upcoming**</span> | `gU` | blocks with a start or due date from tomorrow to a week from today, once each, under the earlier date | day, earliest first: `Tomorrow · Thu 8 Oct`, `Wed 14 Oct` |
| <span id="overdue">**Overdue**</span> | `gO` | open blocks with a due date before today; done blocks and blocks inside them never | due date, oldest first: `3 days ago · Sun 4 Oct` |
| <span id="colors">**Colors**</span> | `gC` | blocks with a [color](@/reference/tags.md#colors) | color, red to blue |
| <span id="logbook">**Logbook**</span> | `gL` | blocks completed from six days ago to today, also inside done blocks, whatever show completed says | day of completion, newest first: `Yesterday · Tue 6 Oct` |
| <span id="starred">**Starred**</span> | `gS` | [starred](@/reference/tags.md#star-flag-and-bookmark) blocks | not grouped |
| <span id="flagged">**Flagged**</span> | `gF` | [flagged](@/reference/tags.md#star-flag-and-bookmark) blocks | not grouped |

A heading of a day in another year includes the year: `Fri 1 Jan 2027`. Within a group, entries are in document order. Move Up and Move Down do nothing in these views.

## Keys

{{ keys(topic="views") }}

## Settings

Each view but Today has a setting that leaves it out of the Navigator. Its Go to key still opens it.

{{ settings(topic="views") }}
