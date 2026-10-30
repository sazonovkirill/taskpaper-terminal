+++
title = "Notes and comments"
weight = 8
description = "Keep longer text and a running log next to any line."
+++

## Text lines

The simplest note is a line of text, like "Four days in May…" under the trip: no dash, no colon, drawn a little dimmer than tasks.

To add one, press <kbd>o</kbd> on a text line, or press <kbd>Backspace</kbd> at the start of a new task to delete its dash. Text lines can hold some Markdown: `**bold**`, `` `code` ``, links and `#` headings.

## A note for any line

For more room, any line can have a **note** attached, of any length, out of sight until you open it.

Go to "Trip to Lisbon" and press <kbd>\\</kbd> (backslash). A panel opens over the right half of the pane, with an empty Note and Comments.

Press <kbd>e</kbd> (or <kbd>Enter</kbd>) to write the note. Type:

```
Hotel: somewhere in Alfama, with a terrace.
```

Press <kbd>Enter</kbd> for a new line and add "Budget: 120 a night". Press <kbd>Esc</kbd> to save the note.

## Comments

**Comments** are short, dated entries: a running log of what was decided and when.

With the panel still open, press <kbd>a</kbd> to add a comment. Type "Sam says May is busy, book soon" and press <kbd>Enter</kbd>. tpt stamps it with the date and time.

{{ shot(src="tutorial/notes", alt="The attachments panel on the right for Trip to Lisbon, with a two-line note about the hotel and one dated comment from Sam", caption="The note and comments of Trip to Lisbon, in the attachments panel.") }}

Press <kbd>\\</kbd> again to widen the panel to the whole pane, and once more to close it; <kbd>Esc</kbd> closes it at once. The trip's row now ends in two icons, for the note and for the comments with their count.

You don't need the panel to add a comment: press <kbd>c</kbd> on any line, type the comment in the prompt at the bottom of the screen and press <kbd>Enter</kbd>.

## In the file

They're saved as lines under their project, marked `@hidden`:

```
Trip to Lisbon:
	Four days in May. The flights are booked, everything else is open.
	NOTE @hidden
		Hotel: somewhere in Alfama, with a terrace.
		Budget: 120 a night
	COMMENTS @hidden
		[2026-04-12 18:27] Sam says May is busy, book soon
```

Other TaskPaper apps show them as ordinary lines, so nothing is lost. The [Reference](@/reference/_index.md) has the panel's other keys.
