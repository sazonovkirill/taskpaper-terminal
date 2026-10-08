# Spec: Special nodes

Covers FR-SPC (D-23). A **special node** is a line the app keeps in the `.taskpaper` file to store a feature's data but never shows as a block. Attachments, a block's note and comments, are the features that write them ([attachments.md](attachments.md), D-94); the app also hides and preserves those other apps or hand edits leave (D-66). Other TaskPaper apps see special nodes as ordinary lines, so the data survives any editor.

## 1. Special nodes (FR-SPC-01)

A special node is any line carrying the `@hidden` tag, together with its whole subtree. Inside a special node `@hidden` has no further meaning: only the outermost `@hidden` line starts a node. Its other tags name the feature that owns it, for example `@log`. The line's text is a label for people reading the file in another app; the app identifies the node by its tags only, except attachments, which it recognizes by their text `NOTE` or `COMMENTS` ([attachments.md §1](attachments.md#1-storage-fr-att-01)).

```
Project 1:
	- Task A
	- Task B
		LOG @hidden @log
			[2026-10-04 12:09] Started
```

Rules:

- **Never a block.** A special node and every line under it are skipped when blocks are computed ([data-model.md §3](data-model.md#3-blocks)): they are not navigable, selectable or editable as blocks. A line with a special node as a child still counts as having children, as in TaskPaper's own outline. A block whose only children are special nodes is a leaf: no fold marker and no `▸ n` count.
- **Excluded from views.** Special nodes do not appear in panes, the Navigator, Go to Symbol, Go to Command targets, the Move to picker, the Today view, counts (FR-OPS-09) or search (FR-SRCH, Later). Their tags never count as the owning block's metadata: an `@done` or `@due` inside a special node changes nothing.
- **Owner.** A special node belongs to its parent line's block, or to the document when it is top-level. The app creates them under any block type except a divider, which has no children (FR-BLK-07); a special node another app put under a divider is preserved and listed in its Edit Node dialog but not otherwise shown. The app has no UI for document-level special nodes; they are only preserved.
- **Placement.** The app writes special nodes as the last children of their owner, except attachments, which go before its first task or project child ([attachments.md §1](attachments.md#1-storage-fr-att-01)). Every operation that inserts a child "at the end" (new child, indent, move, paste, Move to) inserts it after the owner's attachments and before its other trailing special nodes, so those stay last and attachments stay before task and project children. A special node another app placed elsewhere among the children is still recognized and left where it is.
- **Edits keep the node.** A block is one line (D-43), so editing its text or type never moves or deletes its special nodes; only deleting the block takes them with it.
- **Travels with its owner.** Special nodes are part of the owner's subtree, so move, indent, outdent, delete, cut, copy, paste and duplicate carry them along, and undo restores them. Copy writes them into the TaskPaper text, so pasting into another instance keeps them.
- **Unknown features.** A special node that is not an attachment and whose feature tag the app does not know (or that has none) is preserved byte-for-byte (FR-DOC-03) and listed in the owner's Edit Node dialog (`e`, D-3, D-103) as hidden data with its tags and line count.
- **Lossless.** Lines inside a special node are written from their original bytes until the feature that owns the node changes them ([file-format.md §6](file-format.md#6-serialization)).

`@hidden` is reserved for this mechanism. The type converter (FR-BLK-08) and the other metadata commands never set or clear it.

## 2. Later features

Further features may store their data as special nodes with their own feature tag, for example an activity log, file attachments or links, or recurrence rules. Notes and comments are already specified as attachments ([attachments.md](attachments.md), D-94), recognized by their line text rather than a feature tag. Each needs its own requirement; until then such nodes are unknown features (§1).

## 3. Tests

- Parsing: special nodes and their subtrees produce no blocks; a block whose only children are special nodes is a leaf; tags inside them do not change block metadata; round-trip is byte-for-byte.
- Placement: new child, indent, move and paste insert after the owner's attachments and before its other trailing special nodes; a node placed elsewhere by another app stays put.
- Edits: editing a block's text or type leaves its special nodes where they are.
