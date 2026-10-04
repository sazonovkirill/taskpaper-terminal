# Spec: Editing

Covers FR-MODE, FR-OPS, FR-BLK-08, FR-TAG, FR-FMT and FR-DONE. All edits are transactions over the [data model](data-model.md#4-edits) and are undoable (FR-OPS-08). Key bindings are in [keybindings.md](keybindings.md).

## 1. Modes

Each pane is in **navigate** or **edit** mode (FR-MODE-01, D-1).

- **Navigate mode** (default): keys move the focus between blocks, select blocks and run block commands.
- **Edit mode**: one block's text is edited in place (FR-MODE-02). `Enter` or `i` on the focused block enters edit mode with the caret at the end (`i`: at the start); `Esc` returns to navigate mode with the same block focused.
- Only the focused pane can be in edit mode; switching panes ends edit mode.

## 2. Edit mode

The block's editor opens where the block is drawn, keeping its indentation, marker, surrounding blocks and scroll position. It grows downward as lines are added or wrap, pushing later rows down.

**What is edited** is the block's visible text:
- without `- ` and the project `:` (the type is shown by the marker). Each line is written back with its type syntax re-applied; if the result parses as a different TaskPaper type (a note line ending in `:` becomes a project, text starting with `-` and whitespace becomes a task), that line takes the new type. A single-line block's marker updates live; in a multi-line block such a line splits the block when editing ends (OQ-27 default A);
- without any tags (FR-TAG-01): tags are hidden and cannot be typed. Typing `@` at the start of a word is refused with the hint "Tags are set with commands", because TaskPaper has no escape and the word would become a tag on reload (OQ-25). `@` inside a word (`me@example.com`) is ordinary text. Text pasted in edit mode follows the same rule: a tag-shaped word is pasted without its leading `@`, with a status message;
- with inline Markdown markers visible (`**bold**`, `` `code` ``) (FR-FMT-01).

Edits are applied to the document as the user types (one undo step per edit session) and written back as: type syntax + edited text + the block's tags appended after the text in their original order (OQ-24 default A). For a note block, each text line becomes one line item at the block's depth; added lines become new note lines and removed lines are deleted. Each line keeps its own tags by position; tags of removed lines move to the first line, so no tag is lost (FR-DOC-03).

**Text keys** are standard (FR-MODE-03): arrows, `Home`/`End`, `Ctrl+←/→` and `Alt+B/F` by word, `Backspace`/`Delete`, `Ctrl+W`/`Alt+Backspace` delete word, `Shift`+motion selects, `Ctrl+A` selects all, `Ctrl+C`/`Ctrl+X`/`Ctrl+V` clipboard. Motion is by grapheme (`unicode-segmentation`).

**Formatting** (FR-FMT-02): `Ctrl+B` wraps the selection (or the word at the caret) in `**`, or removes them; `` Alt+` `` does the same with backticks. Code blocks have no inline formatting (FR-BLK-05).

**Enter, line breaks and leaving** (OQ-23 default A):

| Key | Note block | Code block | Other blocks |
|---|---|---|---|
| `Enter` | End editing; create the next block in edit mode (§3) | Insert line break | End editing; create the next block |
| `Shift+Enter` / `Alt+Enter` | Insert line break | Insert line break | — |
| `Esc` | End editing | End editing | End editing |
| `Tab` / `Shift+Tab` | Indent / outdent the block, stay in edit mode | same | same |
| `Backspace` in an empty block | Delete the block, edit the previous visible block at its end | same | same |

Ending edit mode applies the canonical-outline repairs for the block ([file-format.md §3.1](file-format.md#31-canonical-outlines)).

## 3. Creating blocks (FR-MODE-04)

| Command | Result |
|---|---|
| `new_below` | New block after the focused block's subtree, as its next sibling. |
| `new_above` | New block as the focused block's previous sibling. |
| `new_child` | New block as the focused block's first child (expanding it). |
| `Enter` at the end of edit mode | `new_below`, except that a block with visible children gets a new first child instead (TaskPaper's indent rule, reference §9). |

The new block's type follows OQ-03 default A: a task after a task, otherwise a single-line note; a new child of a project or task is a task. One exception comes from D-2: a new note would merge into an adjacent note block when its next sibling line is a plain note line not carrying `@code` (which starts its own run), or its previous sibling line is a plain note line without children; in that position the new block is a task instead (OQ-26). It opens in edit mode with empty text.

## 4. Structural operations (FR-OPS)

All act on the selection, or on the focused block when nothing is selected. Each block moves with its subtree. Operations work on lines and blocks are re-derived afterwards, so any result that puts two note runs next to each other as siblings merges them into one block, with the status message "Notes merged" (OQ-26 default A); this applies to every row below and to `convert_type`.

| Command | Behavior |
|---|---|
| `indent` / `outdent` (FR-OPS-02) | Indent: become the last child of the previous sibling block (expanding it); no-op on the first sibling. Outdent: become the next sibling of the parent; following siblings stay where they are (Workflowy, inferred); no-op at the pane's zoom root level. |
| `move_up` / `move_down` (FR-OPS-03) | Swap with the adjacent sibling block. At the first/last position, move to the end/start of the previous/next sibling of the parent. Never leaves the pane's zoom root. |
| `group` (FR-OPS-04) | Wrap the selection in a new parent block and open it in edit mode. The parent is a note, or a task where a note would merge with an adjacent note (OQ-26). |
| `move_to` (FR-OPS-05) | Fuzzy-pick a destination block (any block, same picker as Go to Symbol); the selection becomes its last children. |
| `delete` (FR-OPS-06) | Remove the blocks with their subtrees; focus moves to the next visible block, else the previous. |
| `duplicate` | Deep copy inserted after the original. |
| `copy` / `cut` / `paste_below` / `paste_above` | Copy puts TaskPaper text on the clipboard: the selected blocks' lines, tags included, tab-indented relative to the shallowest block, `\n` line endings. Paste parses the clipboard with the TaskPaper parser and inserts the blocks after (or before) the focused block at its depth, keeping relative structure. A single line of plain text pastes as one note block. |
| `count` (FR-OPS-09) | Status message: number of blocks and words under the focused block (or the pane's zoom root). |

Clipboard backend: the system clipboard via `arboard` when a display server is reachable; otherwise OSC 52 for copy (works over SSH and tmux) and the terminal's bracketed paste for paste.

## 5. Block commands

| Command | Behavior |
|---|---|
| `convert_type` (FR-BLK-08) | Picker: project, task, note, section, code, URL, divider. Rewrites each selected block's syntax: removes `- `/`:` and the type tags `@section`, `@code`, `@url`, then applies the new type's syntax (`- `, `:`, `@section`, `@code` on the first line, `@url(…)` prompting for the address). Converting a multi-line note block to a single-line type joins its lines with spaces; converting to a divider replaces the text with `---` after confirmation if the text was non-empty. |
| `toggle_done` (FR-DONE-01) | If every target is done, remove `@done` (with any value); otherwise add `@done(YYYY-MM-DD)` with today's local date to those that are not done (D-4). |
| `set_start` / `set_due` (FR-TAG-03) | Prompt for a date with live preview of the resolved date; accepts `YYYY-MM-DD` and the natural forms in [search.md §4](search.md#4-dates) (`today`, `tomorrow`, `fri`, `next week`, `+3d`). Empty input removes the tag. Stored as `YYYY-MM-DD` (D-6). |
| `toggle_today` (FR-TAG-04) | Add or remove bare `@today`. |
| `set_color` (FR-TAG-05) | Picker of the six colors and "none"; removes any existing color tag first (D-5). |
| `toggle_bookmark` (D-11) | Add or remove `@bookmark`. |
| `open_url` (FR-BLK-06) | Open the URL block's address, or a bare URL under the caret or in the focused block (D-10), in the system browser (`open`, `xdg-open`, `start`). |
| `details` (D-3) | Popup listing the block's tags, including unsupported ones, read-only. |

All block commands apply to every selected block (FR-OPS-07). Tags are written with the rules in [file-format.md §5](file-format.md#5-tags).
