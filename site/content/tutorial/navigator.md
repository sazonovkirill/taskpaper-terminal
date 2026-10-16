+++
title = "Navigator and finders"
weight = 7
description = "Jump anywhere in the file, and move lines across it."
+++

## The Navigator

Press <kbd>Ctrl+G</kbd> to open the **Navigator**, the list of places to go:

- **INBOX**, for things you haven't sorted yet: a top-level project named exactly `INBOX` is listed apart.
- The special views: **Today**, **Upcoming**, **Overdue**, **Colors**, **Logbook**, **Starred** and **Flagged**.
- **Bookmarks**, once you have some.
- **Projects**, as a tree, each with its number of open tasks.

{{ shot(src="tutorial/navigator", alt="The Navigator window over the outline, listing INBOX, the special views from Today to Flagged, and the projects as a tree with their open task counts", caption="The Navigator: the inbox, the special views and every project.") }}

Use <kbd>↓</kbd> and <kbd>↑</kbd> to move and <kbd>Enter</kbd> to go there, or type to filter. Type `gard`, press <kbd>Enter</kbd>, and the pane zooms into the garden. <kbd>Esc</kbd> closes the Navigator.

Open it again and choose **Logbook**: what you finished this week, the passports and the sunscreen. Since letters go into the filter, use the arrows here, not <kbd>j</kbd> and <kbd>k</kbd>.

## Bookmarks

Press <kbd>g</kbd> <kbd>h</kbd>, move to "Website redesign" under "Work" and press <kbd>b</kbd>. A bookmark icon appears after its name. Open the Navigator: the project now has an entry of its own under **Bookmarks**.

## Go to Symbol

To find any line at all, press <kbd>Ctrl+K</kbd> (or <kbd>/</kbd>) for **Go to Symbol** and type a few letters, such as `trm`: the match is loose. <kbd>Enter</kbd> takes you to the line.

<kbd>g</kbd> <kbd>P</kbd> does the same over projects alone.

## Move to

"Look up tram pass prices" is still in the INBOX, but it belongs with the trip.

Go to it, press <kbd>g</kbd> <kbd>m</kbd> for **Move to**, and type `before`. "Before we go" is at the top of the list. Press <kbd>Enter</kbd>. The task moves to the end of that list, and the pane follows it there.

That's the inbox habit: jot things down in the INBOX, then send each where it belongs.

Press <kbd>g</kbd> <kbd>h</kbd> to go home. The [Reference](@/reference/_index.md) has the rest of the Navigator's keys.
