# Spec: Editing

Covers F-EDIT and F-TAG. All edits are transactions over the [data model](data-model.md#3-edits) and are undoable.

## 1. Text editing

The cursor is always in an item's text (modeless, Workflowy-style; OQ-06). Line editing:

- Grapheme-aware motion (`unicode-segmentation`): left/right, word left/right, home/end of item, start/end of wrapped screen line.
- Delete char/word backward and forward, kill to end of item.
- Text selection within one item with Shift+motion; copy/cut/paste of selected text.
- `Home` toggles between the start of the content (after `- `) and column 0, so the task marker can be reached but isn't in the way.

## 2. Enter and Backspace

Following TaskPaper's list autoformat (reference §9), with the type of new items per OQ-03 default A:

| Situation | Result |
|---|---|
| Cursor at end of item | New item: a task (`- `) after a task, an empty note after a note or a project. It becomes the first child if the current item has visible children, otherwise the next sibling (TaskPaper's indent rule, reference §9). |
| Cursor in the middle | Split: text after the cursor moves to a new sibling below (prefixed `- ` if the current item is a task). Children stay with the first half. |
| Cursor at start of a non-empty item | New empty sibling above; cursor stays on the current item. |
| Item is just `- ` | Clear it to an empty note ("exit the list"). |
| `Shift+Enter` | New child note at the top of the current item's children (the Workflowy "add note" gesture, OQ-01 A). |
| Backspace at start of an empty item with no children | Delete the item; cursor to the end of the previous visible item. |
| Backspace at start of a non-empty item | Outdent if the item is below the zoom root's top level (TaskPaper behavior); otherwise join its text onto the previous visible item if that item is a leaf. |
| Delete at end of item | Join the next visible item's text if it is a leaf; otherwise nothing. |

Typing the project colon or a leading `- ` changes the kind immediately, since kind is derived from text.

## 3. Structure

| Command | Behavior |
|---|---|
| `indent` | Become the last child of the previous sibling (expanding it). No-op on the first sibling. |
| `outdent` | Become the next sibling of the parent. Following siblings stay where they are, as in Workflowy (inferred). In TaskPaper's line model they would become its children instead. No-op at the zoom root's top level. |
| `move_up` / `move_down` | Swap with the adjacent sibling. At the first/last position, move to the end/start of the previous/next sibling of the parent (crossing into the adjacent branch). Never leaves the zoom root. |
| `move_to` | Fuzzy-pick a destination item (any item, not only projects); the selection becomes its last child (`move_to_top` setting: first child). |
| `delete_item` | Remove items with their subtrees. |
| `duplicate` | Deep copy inserted after the original. |
| `group` | Wrap the selection in a new parent note and start editing it. |
| `sort_children asc/desc` | Stable sort of the cursor item's children by `content_text`, case-insensitive, with natural number ordering. |
| `set_kind task/project/note` | Rewrite the syntax as in [data-model.md §3](data-model.md#3-edits). |

All structural commands act on the selection when there is one.

Some commands create shapes that TaskPaper text cannot represent (an empty parent from `group`, an empty trailing item from Enter). They are allowed while the user is editing that item and repaired once the cursor leaves it, as specified in [file-format.md §3.1](file-format.md#31-canonical-outlines).

## 4. Selection (F-EDIT-09)

- `Shift+Up/Down` with no text selection extends an item selection from the cursor item. The selection is always a contiguous run of siblings; extending past the first/last sibling widens it to the parent's run.
- `select_all` selects the zoom root's children; pressing it again selects all text in the current item instead (or the reverse, if the cursor started in text).
- `Esc` clears the selection.

## 5. Clipboard (F-EDIT-10)

- Copying items puts TaskPaper text on the clipboard: tab-indented relative to the shallowest selected item, `\n` line endings.
- Pasting text with line breaks parses it with the TaskPaper parser (single-line pastes are inserted as text) and inserts the items after the cursor item at its depth, keeping relative structure.
- Backend: system clipboard via `arboard` when a display server is reachable; otherwise OSC 52 for copy (works over SSH and tmux) and the terminal's bracketed paste for paste.

## 6. Done and tags

- `toggle_done`: if every target item already has `@done`, remove it; otherwise add `@done(YYYY-MM-DD)` (local date) or bare `@done` per `done_with_date` (OQ-04).
- `tag_with`: prompt with autocomplete over tag names in the document; `name` or `name(value)`; applied to all targets.
- `remove_tag`: pick from the target's tags. `remove_all_tags` keeps nothing.
- `toggle_today`: add/remove bare `@today`.
- `set_due` / `set_start`: prompt for a TaskPaper date expression (`tomorrow 9am`, `next friday`, `+3d`, `2026-10-31`), preview the resolved date live, store `YYYY-MM-DD` or `YYYY-MM-DD HH:mm` ([search.md §4](search.md#4-dates)).
- `rename_tag` (F-TAG-06): rename a tag name in every item.

## 7. Archive @done (F-EDIT-14)

Matches TaskPaper exactly (reference §7):
1. Find the first item whose full text equals `Archive:` (case-insensitive) at any depth; if none, append `Archive:` as the last top-level item.
2. Collect `//@done except //@text = Archive://@done`, keeping only the topmost of nested done items.
3. Optionally (both off by default, OQ-20) strip every tag except `@done` from each moved root item (not its descendants), and/or add `@project(Outer / Inner)` naming ancestor projects.
4. Move them, in document order, to the top of the Archive's children. One undo step.

## 8. Templates (F-EDIT-15)

`insert_template` lists items tagged `@template`; the chosen item's subtree is deep-copied after the cursor with the `@template` tag removed from the copy's root.

## 9. Count (F-EDIT-16)

`count` shows in the status line the number of descendants and words under the cursor item (or the zoom root).
