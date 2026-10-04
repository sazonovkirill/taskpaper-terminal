# Spec: Special nodes and comments

Covers FR-SPC (D-23). A **special node** is a line the app keeps in the `.taskpaper` file to store a feature's data but never shows as a block. Comments are the first feature built on it. Other TaskPaper apps see special nodes as ordinary lines, so the data survives any editor.

## 1. Special nodes (FR-SPC-01)

A special node is any line carrying the `@hidden` tag, together with its whole subtree. Inside a special node `@hidden` has no further meaning: only the outermost `@hidden` line starts a node. Its other tags name the feature that owns it, for example `@comments`. The line's text is a label for people reading the file in another app; the app identifies the node by its tags only.

```
Project 1:
	- Task A
	- Task B
		COMMENTS @hidden @comments
			[2026-10-04 12:09] Comment 1 **text** text text. http://some-url.com
```

Rules:

- **Never a block.** A special node and every line under it are skipped when blocks are computed ([data-model.md §3](data-model.md#3-blocks)): they are not navigable, selectable or editable as blocks and never join a note run. A line with a special node as a child still counts as having children, so it ends its note or code run, as in TaskPaper's own outline; the node therefore always sits under the last line of its block. A block whose only children are special nodes is a leaf: no fold marker and no `▸ n` count.
- **Excluded from views.** Special nodes do not appear in panes, the sidebar, Go to Symbol, Go to Command targets, the Move to picker, the Today view, counts (FR-OPS-09) or search (FR-SRCH, Later). Their tags never count as the owning block's metadata: an `@done` or `@due` inside a special node changes nothing.
- **Owner.** A special node belongs to its parent line's block, or to the document when it is top-level. The app creates them under any block type except a divider, which has no children (FR-BLK-07); a special node another app put under a divider is preserved and listed in its details popup but not otherwise shown. The app has no UI for document-level special nodes; they are only preserved.
- **Placement.** The app writes special nodes as the last children of their owner. Every operation that inserts a child "at the end" (new child, indent, move, paste, Move to) inserts it before the trailing special nodes, so they stay last. A special node another app placed elsewhere among the children is still recognized and left where it is.
- **Edits keep the node under the last line.** When editing a note or code block adds lines after its last line, removes the line holding a special node, or joins the block's lines (type conversion, [editing.md §5](editing.md#5-block-commands)), the block's special nodes move, in order, to the end of the children of the block's new last line. They are never deleted with a line, the same way tags of removed lines move rather than vanish ([editing.md §2](editing.md#2-edit-mode)).
- **Travels with its owner.** Special nodes are part of the owner's subtree, so move, indent, outdent, delete, cut, copy, paste and duplicate carry them along, and undo restores them. Copy writes them into the TaskPaper text, so pasting into another instance keeps them.
- **Unknown features.** A special node whose feature tag the app does not know (or that has none) is preserved byte-for-byte (FR-DOC-03) and listed in the owner's details popup (`ti`, D-3) as hidden data with its tags and line count.
- **Lossless.** Lines inside a special node are written from their original bytes until the feature that owns the node changes them ([file-format.md §6](file-format.md#6-serialization)).

`@hidden` is reserved for this mechanism. The type converter (FR-BLK-08) and the other metadata commands never set or clear it.

## 2. Comments (FR-SPC-02)

### 2.1 Storage

Comments on a block live in a special node tagged `@hidden @comments` under the block's last line. The app creates the node with the text `COMMENTS @hidden @comments` when the first comment is added, and deletes it when the last comment is deleted.

Each child line of the node starts one comment:

```
[YYYY-MM-DD HH:MM] first line of the comment
	continuation line
	continuation line
```

- **Timestamp:** local time when the comment was added, 24-hour, no time zone, ISO date as in `@done`. On read the app also accepts a 12-hour time with `am`/`pm` (`[2026-10-04 12:09pm]`) and a date alone (`[2026-10-04]`); it never rewrites a timestamp it did not write.
- **Multi-line comments:** the first line holds the timestamp; each further line is a child of the comment's line, one level deeper. Leading and trailing blank lines are trimmed when the app writes a comment, and leading whitespace of each line is dropped, as for every line ([file-format.md §3.1](file-format.md#31-canonical-outlines), rule 4). Blank lines inside the comment are kept.
- **Comments typed elsewhere:** any non-blank child line of the node is a comment, with its subtree as continuation lines; blank child lines are skipped. A line without a leading timestamp shows with no date.
- **Several comment nodes** under one block (possible after editing in another app) are shown together in file order; new comments go into the last one.
- Comment text is stored verbatim. Tags and TaskPaper line types inside a comment have no meaning: `@due(2026-10-05)` in a comment is text, and a comment line ending in `:` is not a project. A comment's text is the raw body of its lines without the timestamp; nothing such as a leading `- ` is stripped.

### 2.2 Display

- **Badge:** a block with comments shows a comment icon and the count as a chip (`󰅺 3`), after its date chips (FR-UI-09). Without icons it shows `✎3`, or `c3` when the locale is not UTF-8 (FR-UI-10).
- **Comments panel:** `c` in navigate mode, the context menu (FR-KEY-06) and Go to Command open a panel on the right of the focused pane, Todoist style. Its header shows the block's display text. It lists the comments oldest first, each with its timestamp in a compact local form (`Today 12:09`, `Fri 12:09`, `4 Oct 2026 12:09`) above the text, wrapped to the panel width. An input at the bottom adds a comment.
- **Formatting:** comment text renders `**bold**` and `` `code` `` (FR-FMT-01), and URLs are underlined. `gx` on a comment opens its URL, or offers a picker when it has several (D-10). Tags inside comments are shown as typed, since they mean nothing there.
- Clicking the badge opens the panel (FR-KEY-05).

### 2.3 Commands

Inside the panel, focus moves between the comment list and the input with `Tab`; `Esc` closes the panel and returns focus to the block.

| Command | Keys in the panel | Effect |
|---|---|---|
| Add | type in the input, `Enter` | Appends a comment stamped with the current local time. `Shift+Enter` or `Alt+Enter` inserts a line break. |
| Edit | `Enter` or `i` on a comment | Edits its text in place with the same keys as edit mode (FR-MODE-03); the timestamp is kept. |
| Delete | `dd` or `Delete` on a comment | Removes the comment and its continuation lines; removing the last one removes the node. |
| Move between comments | `j` / `k`, `↓` / `↑` | |
| Copy text | `yy` | Copies the comment's text without the timestamp. |

Unlike block editing, `@` can be typed freely in the input and while editing a comment, because tags inside comments are text (§2.1) and the app never reads them as metadata.

Adding, editing or deleting a comment is one undo step (FR-OPS-08), undoable from the panel or the outline. Saving the file follows [file-io.md §2](file-io.md#2-saving) as for any edit.

### 2.4 Selections

With several blocks selected, `c` opens the panel for the focused block only; comments are per block.

## 3. Later features

Further features may store their data as special nodes with their own feature tag, for example an activity log, attachments or links, or recurrence rules. Each needs its own requirement; until then such nodes are unknown features (§1).

## 4. Tests

- Parsing: special nodes and their subtrees produce no blocks; a block whose only children are special nodes is a leaf; tags inside them do not change block metadata; round-trip is byte-for-byte.
- Placement: new child, indent, move and paste insert before trailing special nodes; a node placed elsewhere by another app stays put.
- Edits: adding lines after a note block's last line, removing the line that holds a node, and joining lines on type conversion all move the node to the new last line and lose nothing.
- Comments: add, edit and delete write the format in §2.1 and are single undo steps; the node is created on the first comment and removed with the last; timestamps in every accepted form parse; untimestamped lines and several nodes display as §2.1 says.
- UI (`TestBackend`): the badge with and without icons, and the panel listing, adding and editing comments.
