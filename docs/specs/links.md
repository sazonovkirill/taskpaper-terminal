# Spec: Internal links

- Status: Accepted (priority **Later**, FR-OPS-10, D-14)
- Last updated: 2026-10-04

Covers links from text to another block in the same file, and backlinks. The owner accepted this design on 2026-10-04; it is recorded now so that v1 does not preclude it, and implemented later.

## 1. Prior art

| App | Target identity | Link syntax | Display |
|---|---|---|---|
| Workflowy | Every node has a stable ID, used in its URL (`#/<id>`) | A pasted node URL | The target's current name |
| Roam | 9-character block uid | `((uid))` | The target block's current text |
| Logseq | UUID stored on the block as an `id::` property | `((uuid))` | The target block's current text |
| Obsidian | `^blockid` appended to the block's line | `[[note#^blockid\|alias]]` | The alias, else the link target |

All four link to a stable identifier rather than to a title, so renames and moves don't break links. Like Logseq and Obsidian, this design stores the identifier in the file next to the target, and creates it only when a block is first linked.

## 2. Syntax

**Target.** A linked block carries a hidden tag `@uid(ID)` on its line. The tag is hidden and preserved like every other tag (FR-TAG-01, FR-DOC-03). It is not `@id`, because TaskPaper 3 reserves `@id`, `@text` and `@type` and treats them as plain text ([file-format.md §5](file-format.md#5-tags)).

**Link.** `[[ID|Title]]` anywhere in a block's text, including text blocks:

```
- Ship the release @uid(k3x9q2ab)
- Write the changelog for [[k3x9q2ab|Ship the release]]
```

- `ID` is one or more ASCII letters, digits, `-` or `_`. `[[ID]]` without a title is also a link.
- `Title` is a cached copy of the target's title as the app displays it (tags removed, links shown as their titles, Markdown markers stripped) so the file stays readable in other apps. The app never relies on it while the target exists (§4). When the app writes it, it replaces `|`, `[` and `]` with spaces and drops the `@` that starts a word, so the cache can never add a tag to the linking line (as for pasted text, [editing.md §2](editing.md#2-edit-mode)).
- Anything else in double brackets is plain text.

## 3. Identifiers

- A block gets an ID the first time something links to it (§5). Blocks nobody links to have no `@uid`.
- Generated IDs are 8 random characters from `a-z0-9` (36⁸ ≈ 2.8 × 10¹² values), drawn again if the ID is already used in the file. IDs written by hand or by other apps are accepted as they are.
- Links resolve only within the open file (one file per instance).
- **No block receives an ID already in use.** Duplicating blocks (FR-OPS-06) drops `@uid` from every copy. A pasted block keeps its `@uid` only if no block in the document has that ID, so cut and paste keeps a block's links working while copy and paste does not create a duplicate.
- **Duplicates from other apps.** If a loaded file has the same ID on several blocks (a line copied in another editor), the first in document order keeps it and links resolve to that block. The later copies lose their `@uid` in one undoable edit, with a status message naming the count. Nothing linked to them specifically, so removing the tag loses nothing.

## 4. Display

- **Navigate mode:** a link shows the target's **current** display text, never the ID, styled like a URL title ([ui.md §2](ui.md#2-blocks)): underlined, in the theme's link color.
- **Edit mode:** a link is one atomic token showing the same text. The caret moves over it in one step, `Backspace`/`Delete` remove the whole link, and the raw `[[…]]` never appears. This is an exception to FR-FMT-01's visible markers because an ID is meaningless to read.
- **Broken link** (no block has the ID, for example after a delete): shown with its cached title, dimmed and struck through. It works again as soon as a block with that ID exists, for example when undo restores the deleted target. A broken link without a title, such as `[[todo]]` written in another app, is shown and edited as plain text, so no text disappears.
- **Backlinks:** a block that other blocks link to shows a small chip with the number of linking blocks after its text.
- Search (Later, [search.md](search.md)) matches a link by its displayed text.

## 5. Commands

| Command | Default binding | Behavior |
|---|---|---|
| `insert_link` | Edit mode: typing `[[` | Opens the Go to Symbol picker ([navigation.md §8](navigation.md#8-go-to-symbol-and-go-to-command-fr-nav-05-fr-nav-06)) and inserts `[[ID\|Title]]` for the chosen block at the caret, giving the target an ID if it has none. `Esc` cancels; after `[[`, the two brackets stay as typed text. |
| `copy_link` | `gy` | Copies `[[ID\|Title]]` for the focused block to the clipboard, giving it an ID if it has none (Workflowy's "copy internal link"). |
| `follow_link` | `gd` | Follows the link in the focused block, or picks one if the block has several. The focused pane zooms to the target's parent and focuses the target, as Go to Symbol does, and pushes the pane's history so `back` returns. Broken links report "Link target not found". |
| `open_in_new_pane` | `Alt+click` on a link | Opens the target in a new pane ([navigation.md §5](navigation.md#5-panes-fr-ui-02--fr-ui-04-d-13)). |
| `show_backlinks` | `gb` | A picker of the blocks linking to the focused block, each with its breadcrumb path; choosing one goes there like `follow_link`. |

## 6. Renames and deletes

- **Rename.** When editing a block that has an ID changes its display text, the cached titles of all links to it are updated in the same undoable edit. Undo reverts both. A cached title left stale by another app is not rewritten on load, so unedited lines still round-trip byte for byte (F-FILE-03). It is refreshed the next time the target is renamed or the linking block is edited.
- **Delete.** Deleting a block (or its ancestor) that has backlinks is not blocked. The status message says how many links are now broken, and undo restores them.
- Removing a link from a block's text does not remove the target's `@uid`.
