# Spec: Editing

Covers FR-MODE (typed shortcuts included), FR-OPS, FR-BLK-08, FR-TAG, FR-FMT and FR-DONE. All edits are transactions over the [data model](data-model.md#4-edits) and are undoable (FR-OPS-08). Key bindings are in [keybindings.md](keybindings.md).

## 1. Modes

Each pane is in **navigate** or **edit** mode (FR-MODE-01, D-1).

- **Navigate mode** (default): keys move the focus between blocks, select blocks and run block commands.
- **Edit mode**: one block's text is edited in place (FR-MODE-02). `Enter` or `i` on the focused block enters edit mode with the caret at the end (`i`: at the start); `Esc` returns to navigate mode with the same block focused.
- Only the focused pane can be in edit mode; switching panes ends edit mode.

## 2. Edit mode

The block's editor opens where the block is drawn, keeping its indentation, marker, surrounding blocks and scroll position. It grows downward as lines are added or wrap, pushing later rows down.

**What is edited** is the block's visible text:
- without `- ` and the project `:` (the type is shown by the marker). Each line is written back with its type syntax re-applied; if the result parses as a different TaskPaper type (a note line ending in `:` becomes a project, text starting with `-` and whitespace becomes a task), that line takes the new type. A single-line block's marker updates live; in a multi-line block such a line splits the block when editing ends (OQ-27 default A), and until then the status line shows the hint "Splits into blocks when editing ends". Text that makes a block a task loses its `-` and whitespace to the marker as it is typed, while a `:` typed in a note stays text until editing ends. A task whose text is emptied while it has tags keeps the placeholder `…` as its text, because `- @tag` is read back as a note. A divider's text is its type, so a divider is not edited;
- without any tags (FR-TAG-01): tags are hidden and cannot be typed. Typing `@` at the start of a word is refused with the hint "Tags are set with commands", because TaskPaper has no escape and the word would become a tag on reload (OQ-25). `@` inside a word (`me@example.com`) is ordinary text. Text pasted in edit mode follows the same rule: a tag-shaped word is pasted without its leading `@`, with a status message;
- with inline Markdown markers visible (`**bold**`, `` `code` ``) (FR-FMT-01), and in note blocks the raw Markdown of §2.2: `#` heading markers, link syntax and fence lines (FR-FMT-03).

Edits are applied to the document as the user types (one undo step per edit session) and written back as: type syntax + edited text + the block's tags appended after the text in their original order (OQ-24 default A). For a note block, each text line becomes one line item at the block's depth; added lines become new note lines and removed lines are deleted. Each line keeps its own tags by position: the unchanged lines at either end of the text keep their lines, and the changed lines between them take the lines they replace in order, so the first line stays the block's first line. Tags of removed lines move to the first line, so no tag is lost (FR-DOC-03). The block's children, special nodes included, stay under its last line: when lines are added after it or it is removed, they move in order to the new last line (FR-BLK-03, [special-nodes.md §1](special-nodes.md#1-special-nodes-fr-spc-01)).

**Text keys** are standard (FR-MODE-03): arrows (`↑`/`↓` move between the lines of a note or code block, keeping the caret's grapheme column), `Home`/`End` to the ends of the caret's line, `Ctrl+←/→` and `Alt+B/F` by word, `Backspace`/`Delete`, `Ctrl+W`/`Alt+Backspace` delete word, `Shift`+motion selects, `Ctrl+A` selects all, `Ctrl+C`/`Ctrl+X`/`Ctrl+V` clipboard. Motion is by grapheme (`unicode-segmentation`). Characters the parser splits lines on ([file-format.md §1](file-format.md#1-input-decoding)) are never typed into a line.

**Formatting** (FR-FMT-02): `Ctrl+B` wraps the selection (or the word at the caret) in `**`, or removes them; `` Alt+` `` does the same with backticks. Code blocks have no inline formatting (FR-BLK-05).

**Enter, line breaks and leaving** (OQ-23 default A):

| Key | Note block | Code block | Other blocks |
|---|---|---|---|
| `Enter` | End editing; create the next block in edit mode (§3). On an opening fence line or inside an open fence (§2.2): insert line break | Insert line break | End editing; create the next block |
| `Shift+Enter` / `Alt+Enter` | Insert line break | Insert line break | — |
| `Esc` | End editing | End editing | End editing |
| `Tab` / `Shift+Tab` | Indent / outdent the block, stay in edit mode, on the whole merged block when it joins a note (OQ-26); a block about to split (OQ-27) splits first and the block with the caret moves | same | same |
| `Backspace` in an empty block | Delete the block, edit the previous visible block at its end; a block with children and the first row stay. A block created in this session leaves no undo step | same | same |

Ending edit mode applies the canonical-outline repairs for the block ([file-format.md §3.1](file-format.md#31-canonical-outlines)).

### 2.1 Typed shortcuts

Typing one of these prefixes followed by a space converts the block (FR-MODE-05, D-24), like Workflowy's Markdown triggers ([reference §16](../reference/workflowy.md#16-node-types-listbullet-types)):

| Typed, then space | Result |
|---|---|
| `---` | Divider. The block becomes a divider and a new block opens below it in edit mode, so typing continues: a task if the block was a task, otherwise a note, then adjusted as in §3 (OQ-26). |
| ```` ``` ```` | Code block: `@code` is added to the first line and editing continues in the code block, with any text after the caret kept as its first line. |
| `[]` or `[ ]` | Task: the prefix becomes `- `. |
| `[x]` or `[X]` | Completed task: the prefix becomes `- ` and `@done(YYYY-MM-DD)` with today's date is added (FR-DONE-01). |

Rules:

- A shortcut fires only when the prefix is the start of the text of a **single-line note or task block** and the space is typed right after it, with the caret there. The divider also requires that no text follows the caret and that the block has no children (FR-BLK-07); otherwise the space is inserted as text. `[]`, `[ ]` and `[x]` in a task, and every prefix in a project, section, URL or code block or in a later line of a note, are ordinary text.
- Shortcuts fire only on a typed space. Pasted text is never converted.
- The prefix and the space are consumed; the block's type shows through its marker, as for every other type (§2).
- The very next key after a conversion can revert it: `Backspace` or `Ctrl+Z` restores the previous type, removes any `@done` the shortcut added, and puts back the typed prefix and space as text, without firing again. The divider is the exception: it ends the edit session, and the conversion plus the new block below form one undo step that undo (`z`) reverts in navigate mode.
- A conversion is part of the edit session's undo step (§2) and follows the rules of `convert_type` (§5).
- `- ` and a trailing `:` need no shortcut: TaskPaper already reads them as a task and a project, and the marker updates live (OQ-27). `# ` stays text and starts a Markdown heading in a note (§2.2, OQ-28).

### 2.2 Markdown in notes

Note blocks support this Markdown in addition to FR-FMT-01 (FR-FMT-03, D-25). It is derived from the note's text for display only; the text is stored unchanged, so the file stays plain TaskPaper.

| Syntax | Meaning |
|---|---|
| `#` to `######`, a space, then text, at the start of a line | Heading of level 1 to 6. A line ending in `:` is a project, not a heading (OQ-27). |
| `[title](url)` | Link showing `title`. |
| `<url>` and bare `http(s)://` URLs | Link showing the URL (bare URLs as in D-10). |
| `` `code` `` | Code span (FR-FMT-01). |
| A line starting with ```` ``` ````, optionally followed by a language name, up to the next line that is exactly ```` ``` ```` | Fenced code. The lines between the fences are code: no inline formatting, headings or links. An unclosed fence runs to the end of the block. |

- In navigate mode the markers are hidden and the result is rendered ([ui.md §2](ui.md#2-blocks)). In edit mode the raw text is shown and edited (FR-MODE-02).
- Anything else, including italic, lists, quotes and tables, is ordinary text (D-14).
- `open_url` (§5) opens the link of a note: the one under the caret in edit mode, the only one in the block, or one chosen from a picker when the block has several, as for comments ([special-nodes.md §2.2](special-nodes.md#22-display)).
- Fenced code is still note text. TaskPaper reads a fenced line that starts with `- ` or ends with `:` as a task or project, which splits the note as in OQ-27, and the `@` rule of §2 applies inside fences. Code with such lines belongs in a code block (FR-BLK-05).
- Markdown in task, project and section text stays limited to FR-FMT-01 and bare URLs.

## 3. Creating blocks (FR-MODE-04)

| Command | Result |
|---|---|
| `new_below` | New block after the focused block's subtree, as its next sibling. |
| `new_above` | New block as the focused block's previous sibling. |
| `new_child` | New block as the focused block's first child (expanding it). |
| `Enter` at the end of edit mode | `new_below`, except that a block with visible children gets a new first child instead (TaskPaper's indent rule, reference §9). |

**Typed creation** (D-26) adds a block of a chosen type in one key, opened in edit mode with empty text. "The list" is the focused block's siblings; in an empty pane it is the zoom block's children ([OQ-29](../open-questions.md#oq-29--where-do-the-bindings-displaced-by-d-26-go) default).

| Command | Result |
|---|---|
| `new_project_top` / `new_project_below` | A project as the first block of the list / as `new_below` would place it. |
| `new_section_top` / `new_section_below` | A section placed before the first section of the list, so it comes after the tasks above it, or at the end of the list when it has none / as `new_below` would place it. |
| `new_task_top` / `new_task_below` | A task as the first block of the list / as `new_below` would place it. |

Typed creation never makes a note, so it never merges notes (OQ-26). A new section right after a code block without children would join the code run as a note line does (D-18, [data-model.md §3](data-model.md#3-blocks)), so it is written with a project's `:` (`Name: @section`), which ends the run and is still a section.

On the zoom title, which stands for the view's root, every command adds the title's first child; in a pane without blocks the new block is the first one.

The new block's type follows OQ-03 default A: a task next to a task (after it for `new_below`, before it for `new_above`), otherwise a single-line note; a new child of a project or task is a task. One exception comes from D-2 and D-18: if a new note line would join an adjacent run under the grouping rules in [data-model.md §3](data-model.md#3-blocks) (the note or code run before it, or the note run after it), the new block is a task instead (OQ-26). It opens in edit mode with empty text.

## 4. Structural operations (FR-OPS)

All act on the selection, or on the focused block when nothing is selected. Each block moves with its subtree. Operations work on lines and blocks are re-derived afterwards, so any result that puts two note runs next to each other as siblings merges them into one block, with the status message "Notes merged" (OQ-26 default A); this applies to every row below and to `convert_type`.

| Command | Behavior |
|---|---|
| `indent` / `outdent` (FR-OPS-02) | Indent: become the last child of the previous sibling block (expanding it); no-op on the first sibling. Outdent: become the next sibling of the parent; following siblings stay where they are (Workflowy, inferred); no-op at the pane's zoom root level. |
| `move_up` / `move_down` (FR-OPS-03) | Swap with the adjacent sibling block. At the first/last position, move to the end/start of the previous/next sibling of the parent (expanding it); no-op when there is none. Never leaves the pane's zoom root. |
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
| `start_today` / `start_tomorrow` / `due_today` / `due_tomorrow` (D-26) | Set `@start` or `@due` to today's or tomorrow's local date, without a prompt. |
| `start_earlier` / `start_later` / `due_earlier` / `due_later` (D-26) | Move `@start` or `@due` one day back or forward. A block without that date gets yesterday or tomorrow, as if the missing date were today. |
| `toggle_today` (FR-TAG-04) | Add or remove bare `@today`. |
| `set_color` (FR-TAG-05) | Picker of the six colors and "none"; removes any existing color tag first (D-5). |
| `cycle_color` (D-26) | Step through none, red, orange, yellow, green, blue, violet and back to none, starting from the focused block's color; every selected block gets the same next color. |
| `toggle_bookmark` (D-11) | Add or remove `@bookmark`. |
| `open_url` (FR-BLK-06) | Open the URL block's address, or a bare URL under the caret or in the focused block (D-10), in the system browser (`open`, `xdg-open`, `start`). |
| `details` (D-3) | Popup listing the block's tags, including unsupported ones, read-only. |

All block commands apply to every selected block (FR-OPS-07). Tags are written with the rules in [file-format.md §5](file-format.md#5-tags).
