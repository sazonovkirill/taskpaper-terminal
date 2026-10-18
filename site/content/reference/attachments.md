+++
title = "Attachments"
weight = 90
description = "A Markdown note and dated comments on any block, read and edited in the attachments panel, and kept in the file as ordinary lines."
+++

Any block but a divider can carry two **attachments**: a **note**, Markdown text of any length, and **comments**, dated one-line entries. They never show as rows in the outline. A block with a note shows a note icon at the right of its row, before its date chips, and a block with comments a comments icon with their number.

Attachments stay out of the Navigator, the finders, Move to, the special views and counts. They travel with their block wherever it goes: moving, copying, pasting, duplicating, deleting and undo all take them along.

## The attachments panel

`\` or `&` (**Open Attachments**) opens the **attachments panel** on the right side of the screen, for the focused block, with the note selected, or the newest comment while notes are off. It works whether or not the block has an attachment yet. It is also in Go to Command, the context menu and the edit-mode [command menu](@/reference/modes.md#the-command-menu), from which closing the panel returns you to editing.

The panel shows a **Note** section above a **Comments** section, under a title naming the block. It takes the keys while it is open. One item is selected at a time: the note, or one comment.

{{ context_keys(table="ATTACHMENTS") }}

The motion keys scroll through the selected item while it runs past the panel's edge, and otherwise select the next or previous item:

{{ context_keys(table="SCROLL") }}

Each change the panel makes is one step to undo, with the document's undo once the panel is closed. In a read-only file the edit keys refuse with its notice.

If the file is [reloaded from disk](@/reference/files.md#when-the-file-changes-on-disk), the panel stays open on its block, with what you were typing, and closes only when the block is gone.

## Notes

The note shows rendered as Markdown, wrapped to the panel: headings, emphasis, inline code, links, lists, block quotes and fenced code blocks, without syntax highlighting. A block without one shows "No note".

`e` or `Enter` turns the note into a plain-text editor over its Markdown. The [text keys](@/reference/modes.md#text-keys) work as in edit mode. `Backspace` at the start of a line and `Delete` at its end join it to its neighbor, and `Ctrl+V` pastes. The whole editing session is one step to undo. Emptying the note deletes it. Closing the panel or quitting saves the note too.

{{ context_keys(table="NOTE_EDITOR") }}

## Comments

Comments are listed newest first, each with its timestamp dimmed and its text rendering bold, inline code and links. A block without any shows "No comments".

- `a` adds a comment, whatever is selected: type it in the line that opens at the top of the list, then `Enter` adds it, stamped with the current local time. `Esc` cancels.
- `e` or `Enter` rewords the selected comment in its place, keeping its timestamp.
- `d` or `Delete` deletes it. Deleting the last one removes the comments.

A comment is one line: a paste's line breaks become spaces. Text of only spaces adds nothing.

## In the file

Attachments are child lines of their block, so any TaskPaper app shows them as plain text:

```
Launch:
	Ship the docs first.
	NOTE @hidden
		# Plan
		Write the **guide**, then the website.
	COMMENTS @hidden
		[2026-10-08 00:40] Moved the date
		[2026-10-06 09:15] Started
	- Write the guide
```

- A note is a line reading exactly `NOTE`, and comments a line reading exactly `COMMENTS`, each with `@hidden`. The content is the lines one level deeper.
- tpt writes them right after the block's leading text children and before its first task or project child. It leaves an attachment another app put elsewhere where it is.
- A comment line starts with its timestamp, `[YYYY-MM-DD HH:MM]`. A line without one is shown as a comment without a date and kept as it is.
- So that no note line reads as a task or a project in another app, a line that would start with `-` or with whitespace, or end in `:`, gets an invisible zero-width space (U+200B) at that place. tpt removes it again on reading, so you always see what you typed, and code indentation survives.
- A second `NOTE` or `COMMENTS` under one block is kept as a [hidden line](@/reference/blocks.md#hidden-lines) and listed in [Edit Node](@/reference/tags.md#edit-node).

## Settings

With a setting off, that kind of attachment becomes a plain hidden line again: still kept in the file, but with no icon, no panel section and no command. With both off, the panel doesn't open.

{{ settings(topic="attachments") }}

{{ keys(topic="attachments") }}
