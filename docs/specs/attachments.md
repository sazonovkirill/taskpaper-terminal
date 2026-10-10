# Spec: Attachments

Covers FR-ATT (D-94). An **attachment** is extra information attached to a block and never shown in the outline itself: a **note** (Markdown text of any length) or **comments** (dated one-line entries). A block with an attachment shows its icon at the right of its row, and the attachment is read and edited in the attachments panel. Each kind is stored as a special node ([special-nodes.md](special-nodes.md)) and has its own setting.

## 1. Storage (FR-ATT-01)

Attachments are child lines of the block they belong to, so other TaskPaper apps show them as ordinary text:

```
PROJECT A:
	Regular text
	NOTE @hidden
		# Any markdown text
		multiline text
		```cpp
		Code blocks are fine
		```
	COMMENTS @hidden
		[2026-10-08 00:40] Comment 3
		[2026-10-07 18:02] Comment 2
		[2026-10-06 09:15] Comment 1
	- Task 1
		Regular text
		NOTE @hidden
			Note for task 1
		COMMENTS @hidden
			[2026-10-08 00:31] Comment 1
		- Subtask 1
		- Subtask 2
```

- **Recognition.** A note is a special node whose line text, tags aside and trimmed, is exactly `NOTE`; comments one whose text is exactly `COMMENTS`; both case-sensitive, both carrying `@hidden`. Other tags on the line are kept and ignored, so D-23's `COMMENTS @hidden @comments` is recognized too. The content is the node's subtree: one line per note line or comment, one level deeper than the `NOTE` or `COMMENTS` line.
- **Owner.** Any project, task or text block, sections included, can have one note and one comments node; a divider has none, as it has no children (FR-BLK-07). If a block has several of one kind, the first is the attachment and the others are unknown special nodes, preserved and listed by the Edit Node dialog.
- **Placement.** The app writes `NOTE`, then `COMMENTS`, right after the block's leading run of text children (a new `COMMENTS` goes right after an existing `NOTE`, and a new `NOTE` right before existing `COMMENTS`, wherever they are) and before its first task or project child; with no such child, after its text children and before any trailing special nodes. This replaces FR-SPC-01's "last child" for these two kinds only, and attachments never count as trailing special nodes: a child inserted "at the end" of the block (new child, indent, move, paste, Move to) goes after its attachments, so indenting `- Task 2` under a `- Task 1` whose only child is its `NOTE` puts the task after the note. An attachment found elsewhere among the block's direct children is still recognized and left where it is; inserting, indenting or moving other children never moves an attachment.
- **Lifetime.** The app creates the node with the first note text or comment, and deletes it when the note is emptied or the last comment deleted. Attachments travel with their block like every special node: move, indent, outdent, delete, cut, copy, paste, duplicate and undo carry them.
- **Lossless.** A file is never rewritten on load. Attachment lines the app did not change are written from their original bytes (FR-DOC-03).

### 1.1 Guarding content lines

Other TaskPaper apps parse the lines inside an attachment like any other, and this app's own parser does too ([file-format.md](file-format.md)). A content line must therefore never read as a task or a project (the owner's rule: no line starts with `-` or ends with `:`), must keep its leading whitespace, which the parser would take as indentation and drop, and must stay inside its node. The note editor never refuses text for this; the app guards such lines on write with an invisible U+200B ZERO WIDTH SPACE and removes the guard on read, so the user always sees exactly what they typed:

- A line that would start with whitespace, with `-` (a Markdown list item, `--flag` in a code block) or with U+200B itself gets U+200B as its first character. Its leading whitespace is then part of its text, so code indentation survives, and a U+200B the user typed there survives the read below.
- A line whose content, the text before any trailing tags (file-format.md §4–5), ends in `:`, or in `:` followed by any number of U+200B the user typed (`Steps:`, Python's `def f():`, `Meeting: @at(10:30)`) gets U+200B right after that colon, whether or not it also got the guard above, since `- Steps:` reads as a project once guarded at the start.
- A note's trailing blank lines are dropped by the editor: an editor that strips their whitespace would leave them to take the depth of the next line and fall out of the note (file-format.md §3). Blank lines inside a note stay, as they take the depth of the next note line.
- On read, the app removes one U+200B at the start of a content line and, when the line's content before its trailing tags ends in `:` followed by one or more U+200B, one of those; these are the two places the write puts them, and nothing else is removed.

Markdown backslash escapes (`\-`, `\:`) were rejected: they show literally inside fenced code, and cannot keep leading whitespace. The same guards apply to comment lines; a stamped comment starts with `[`, so only the colon guard can apply to it.

## 2. Display (FR-ATT-02)

- A block with a note shows a note icon, and one with comments a comments icon followed by the number of comments, both dimmed in the headers color and aligned to the right of the row, before its date chips; the flag, star and bookmark stay right after the text ([ui.md §7](ui.md#7-icons-fr-ui-09-fr-ui-10), D-109, D-110). The implementing thread picks the Nerd Font v3 glyphs, each with a Unicode fallback and an ASCII form in the icon table.
- The icons are drawn wherever the block's row is: panes, zoomed title rows and special views. They are hidden while the block is edited, and dimmed like the rest of a done block or one inside a done block (D-40).
- Attachment content never appears as rows, in counts, the Navigator, the finders, Move to or the special views (FR-SPC-01).

## 3. Attachments panel (FR-ATT-03)

- A modal panel opened over the right side of the screen, above the panes, from the top of the screen to the status line, for the focused block. It shows a Note section and below it a Comments section, each under a heading with its icon, aligned to the right in the headers color and present only while its setting is on (§6); its title names the block (D-109, D-110). The keys behind it are inactive while it is open, and `Esc` from its navigate state closes it.
- Opening commands: Open Note (`n`) and Open Comments (`N`), each opening the panel with the note or the newest comment selected, in Go to Command, the edit-mode `/` menu, the context menu and the `?` panel, available whether or not the block has an attachment yet. `Esc` closes the panel; opened from the `/` menu, it returns to editing when it closes (D-50, D-106).
- One item is selected, on the focused block's band: the note, heading and text together, or one comment. The motion keys scroll through the selected item while it goes past the panel's edge that way and otherwise select the next or previous item: `j`/`k` or the arrows a line, `Ctrl+D`/`Ctrl+U` or `Page Down`/`Page Up` a page, and `g`/`Home` and `G`/`End` go to the top with the first item and the bottom with the last. A newly selected item scrolls into view, from its start when it is taller than the panel (D-109).
- A reload from disk keeps the panel open on its block, with the note or comment being typed, and closes it only when the block is gone; saving the note then writes it over any outside change to that note. The selected comment and one being reworded are found again as the same line among the reloaded comments; a rewording of a comment that is gone or was changed outside ends, so it never lands on another comment (D-104, D-110).
- Every change the panel makes is one undo step in the document, undone with the document's undo once the panel is closed, and saved like any edit. In a read-only file the edit keys and Add Comment refuse with its notice instead of opening the editor or input, so nothing typed is lost ([file-io.md §1](file-io.md#1-opening)).

## 4. Notes (FR-ATT-04)

- The Note section shows the note rendered as Markdown: headings, emphasis, inline code, links, lists, block quotes and fenced code blocks, without syntax highlighting, in the theme's colors and wrapped to the panel's width, or a dimmed "No note". The motion keys scroll through it as §3 says (D-107, D-109).
- An edit key, `e` or `Enter`, on the selected note turns it into a multi-line plain-text editor over the raw Markdown: `Enter` breaks the line, the arrows move between lines, the standard text keys work as in edit mode, `Backspace` at the start of a line and `Delete` at its end join it to its neighbor, `Ctrl+V` pastes (the blocks the app copied last paste as the text each shows, one line each, without their tags, children or attachments), and `Esc` saves and returns to the rendered view. The whole session is one undo step. An empty note deletes the `NOTE` node. Closing the panel or quitting saves the note too (D-107).

## 5. Comments (FR-ATT-05)

- Every comment is one line starting with its local 24-hour timestamp, `[YYYY-MM-DD HH:MM] text`. The Comments section lists them newest first, or a dimmed "No comments", as the file stores them, each with its timestamp dimmed and its text rendering inline Markdown (bold, inline code, links) as a text block does.
- Add Comment (`a`), whatever is selected, opens a one-line input at the top of the list; `Enter` adds the comment there stamped with the current local time and selects it, and `Esc` cancels. A comment cannot hold a line break: a paste's line breaks become spaces, leaving out those that end it, and the blocks the app copied last paste as Quick Add takes them ([editing.md §6](editing.md#6-quick-add-fr-mode-07-fr-mode-08)). Text of only spaces adds nothing (D-108).
- The selected comment can be edited (`e` or `Enter`), in the same input in its place, keeping its timestamp, and deleted (`d` or `Delete`); each is one undo step. Deleting the last one deletes the `COMMENTS` node.
- A line under `COMMENTS` without a valid timestamp, typed by hand or by another app, is shown as a comment without a date and kept as is; the app never reorders existing comments.

## 6. Settings (FR-ATT-06)

- **Use Notes Attachments** (`attachments_notes`) and **Use Comments Attachments** (`attachments_comments`), booleans, both on by default, in the settings window's Appearance group and in `config.toml` ([ui.md §8](ui.md#8-settings-window-fr-set-02-fr-set-03)).
- With a setting off, that kind of node is an unknown special node again: hidden and preserved, with no icon, no panel section and no command; the Edit Node dialog lists it like any hidden data. With both off the panel cannot open.

## 7. Tests

- Parsing and writing: recognition by exact text and `@hidden`; placement after leading text children and before the first task or project; nodes elsewhere left in place; round-trip byte-for-byte; every guard of §1.1 written and removed, including indented code, `- ` list items, `def f():` and trailing tags after a colon; trailing blank lines dropped.
- Behavior: creating, editing and emptying a note; adding, editing and deleting comments, with timestamps from an injected clock; undo of each; attachments carried by move, copy, paste and delete; settings off hide icons and sections and keep the nodes.
- UI: dimmed icons at the right before the date chips, with the comment count, hidden while editing; the panel's sections, selection and scrolling, rendered Markdown and editor, via TestBackend.
