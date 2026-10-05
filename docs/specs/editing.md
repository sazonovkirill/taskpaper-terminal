# Spec: Editing

Covers FR-MODE (typed shortcuts included), FR-OPS, FR-BLK-08, FR-TAG, FR-FMT and FR-DONE. All edits are transactions over the [data model](data-model.md#4-edits) and are undoable (FR-OPS-08). Key bindings are in [keybindings.md](keybindings.md).

## 1. Modes

Each pane is in **navigate** or **edit** mode (FR-MODE-01, D-1).

- **Navigate mode** (default): keys move the focus between blocks, select blocks and run block commands.
- **Edit mode**: one block's text is edited in place (FR-MODE-02). `Enter` on the focused block enters edit mode with the caret at the end, and is the only key that does (D-41); `Esc` returns to navigate mode with the same block focused.
- Only the focused pane can be in edit mode; switching panes ends edit mode.

## 2. Edit mode

The block's editor opens where the block is drawn, keeping its indentation, marker, surrounding blocks and scroll position. A block is one line (D-43), so the editor is one line; text wider than the pane scrolls to keep the caret in view.

**What is edited** is the block's visible text:
- without `- ` and the project `:` (the type is shown by the marker). The line is written back with its type syntax re-applied; if the result parses as a different TaskPaper type (a text line ending in `:` becomes a project, text starting with `-` and whitespace becomes a task), the line takes the new type and its marker updates live (OQ-27). Text that makes a block a task loses its `-` and whitespace to the marker as it is typed, while a `:` typed in text stays text. A task whose text is emptied while it has tags keeps the placeholder `…` as its text, because `- @tag` is read back as text. A divider's text is its type, so a divider is not edited;
- without any tags (FR-TAG-01): tags are hidden and cannot be typed. Typing `@` at the start of a word is refused with the hint "Tags are set with commands", because TaskPaper has no escape and the word would become a tag on reload (OQ-25). `@` inside a word (`me@example.com`) is ordinary text. Text pasted in edit mode follows the same rule: a tag-shaped word is pasted without its leading `@`, with a status message;
- with inline Markdown markers visible (`**bold**`, `` `code` ``) (FR-FMT-01), and in text blocks the raw Markdown of §2.2: `#` heading markers and link syntax (FR-FMT-03).

Edits are applied to the document as the user types (one undo step per edit session) and written back as: type syntax + edited text + the line's tags appended after the text in their original order (OQ-24 default A). A line whose text and type are as they were is written as it was, so its mid-line tags stay where they are.

**Text keys** are standard (FR-MODE-03): `←`/`→`, `Home`/`End` to the ends of the text, `Ctrl+←/→` and `Alt+B/F` by word, `Backspace`/`Delete`, `Ctrl+W`/`Alt+Backspace` delete word, `Shift`+motion selects (`Shift+↑`/`Shift+↓` select to the start and end of the text), `Ctrl+A` selects all, `Ctrl+C`/`Ctrl+X`/`Ctrl+V` clipboard. Motion is by grapheme (`unicode-segmentation`). Characters the parser splits lines on ([file-format.md §1](file-format.md#1-input-decoding)) are never typed into a line, and no key breaks a line (D-43).

**Formatting** (FR-FMT-02): `Ctrl+B` wraps the selection (or the word at the caret, or nothing at the caret) in `**`, or removes them when the selection or caret is inside bold text or between the markers; `` Alt+` `` and `Ctrl+E` do the same with backticks.

**Enter, arrows and leaving** (OQ-23, D-43):

| Key | Any block |
|---|---|
| `Enter` | End editing; create the next block in edit mode (§3) |
| `↑` / `↓` | End editing and edit the previous / next visible block, skipping dividers, with the caret as many graphemes from the start as it was, or at the end of a shorter text, like moving between lines in a traditional editor. Each block's session is its own undo step. On the first / last such block nothing happens |
| `Esc` | End editing |
| `Tab` / `Shift+Tab` | Indent / outdent the block, stay in edit mode |
| `Backspace` in an empty block | Delete the block, edit the previous visible block at its end; a block with children and the first row stay. A block created in this session leaves no undo step |

Ending edit mode applies the canonical-outline repairs for the line ([file-format.md §3.1](file-format.md#31-canonical-outlines)).

### 2.1 Typed shortcuts

Typing one of these prefixes followed by a space converts the block (FR-MODE-05, D-24), like Workflowy's Markdown triggers ([reference §16](../reference/workflowy.md#16-node-types-listbullet-types)):

| Typed, then space | Result |
|---|---|
| `---` | Divider. The block becomes a divider and a new block opens below it in edit mode, so typing continues: a task if the block was a task, otherwise text, as in §3. |
| `[]` or `[ ]` | Task: the prefix becomes `- `. |
| `[x]` or `[X]` | Completed task: the prefix becomes `- ` and `@done(YYYY-MM-DD)` with today's date is added (FR-DONE-01). |

Rules:

- A shortcut fires only when the prefix is the start of the text of a **text or task block** and the space is typed right after it, with the caret there. The divider also requires that no text follows the caret and that the block has no children (FR-BLK-07); otherwise the space is inserted as text. `[]`, `[ ]` and `[x]` in a task, and every prefix in a project, section or URL block, are ordinary text.
- Shortcuts fire only on a typed space. Pasted text is never converted.
- The prefix and the space are consumed; the block's type shows through its marker, as for every other type (§2).
- The very next key after a conversion can revert it: `Backspace` or `Ctrl+Z` restores the previous type, removes any `@done` the shortcut added, and puts back the typed prefix and space as text, without firing again. The divider is the exception: it ends the edit session, and the conversion plus the new block below form one undo step that undo (`z`) reverts in navigate mode.
- A conversion is part of the edit session's undo step (§2) and follows the rules of `convert_type` (§5).
- `- ` and a trailing `:` need no shortcut: TaskPaper already reads them as a task and a project, and the marker updates live (OQ-27). `# ` stays text and starts a Markdown heading in a text block (§2.2, OQ-28).

The code-block shortcut ```` ``` ```` was dropped with code blocks (D-43).

### 2.2 Markdown in text

Text blocks support this Markdown in addition to FR-FMT-01 (FR-FMT-03, D-25). It is derived from the line's text for display only; the text is stored unchanged, so the file stays plain TaskPaper.

| Syntax | Meaning |
|---|---|
| `#` to `######`, a space, then text, at the start of the line | Heading of level 1 to 6. A line ending in `:` is a project, not a heading (OQ-27). |
| `[title](url)` | Link showing `title`, which may hold bold and code but no other link. The address has no spaces, and its parentheses pair up. |
| `<url>` and bare `http(s)://` URLs | Link showing the URL (bare URLs as in D-10). Between `<` and `>` it starts with a scheme such as `https:` or `mailto:`. |
| `` `code` `` | Code span (FR-FMT-01). |

- In navigate mode the markers are hidden and the result is rendered ([ui.md §2](ui.md#2-blocks)). In edit mode the raw text is shown and edited (FR-MODE-02).
- Anything else, including italic, lists, quotes, tables and fenced code, is ordinary text (D-14, D-43).
- `open_url` (§5) opens the link of a text block: the one under the caret in edit mode, the only one in the block, or one chosen from a picker when the block has several, as for comments ([special-nodes.md §2.2](special-nodes.md#22-display)). Until the picker of step 4.2, it opens the first link whose address has a scheme, so `[notes](notes.txt)` is skipped.
- Markdown in task, project and section text stays limited to FR-FMT-01 and bare URLs.

### 2.3 Type menu

Typing `/` in edit mode opens a menu at the caret that gives the block another type (D-44, FR-BLK-08). It reads:

```
Node type:
Project
Section
Note
Task
Divider
```

"Note" is the menu's label for a text node. The `/` is not typed while the menu is open.

| Key | Effect |
|---|---|
| `p`, `s`, `n`, `t` right after `/` | Choose Project, Section, Note (text) or Task at once |
| Other characters | Filter the entries by name, ignoring case, highlighting the first (`/div` leaves Divider) |
| `↑`/`↓`, `Ctrl+P`/`Ctrl+N` | Move the highlight |
| `Enter` | Choose the highlighted entry |
| `Esc` | Close the menu and type `/` and the filter as text |
| `Backspace` | Erase the filter's last character; on an empty filter, close the menu, typing nothing |

A filter that matches no entry closes the menu and types `/` and the filter, so a path such as `/home` stays text. Other keys do nothing while the menu is open.

Choosing a type rewrites the line as `convert_type` (§5) does: the old type's syntax and the type tags `@section` and `@url` go, other tags stay, and the new type's syntax is added: `- ` for a task, `:` before the trailing tags for a project, `@section` for a section, nothing for text. The caret stays where it was. A divider replaces the text with `---`, keeping the other tags, without confirmation, and ends editing, because a divider's text is its type (§2). The change is part of the edit session's single undo step, and within the session `Ctrl+Z` takes it back on its own.

URL is not in the menu: a URL block needs an address, which typing cannot give (OQ-25); it is made by `convert_type`.

## 3. Creating blocks (FR-MODE-04)

| Command | Result |
|---|---|
| `new_below` | New block after the focused block's subtree, as its next sibling. |
| `new_above` | New block as the focused block's previous sibling. |
| `new_last_child` | New block as the focused block's last child (expanding it), before the special nodes that end its children ([special-nodes.md §1](special-nodes.md#1-special-nodes-fr-spc-01)). |
| `new_first_child` | New block as the focused block's first child (expanding it). |
| `Enter` at the end of edit mode | `new_below`, except that a block with visible children gets a new first child instead (TaskPaper's indent rule, reference §9). |

**Typed creation** (D-26) adds a block of a chosen type in one key, opened in edit mode with empty text. "The list" is the focused block's siblings; in an empty pane it is the zoom block's children ([OQ-29](../open-questions.md#oq-29--where-do-the-bindings-displaced-by-d-26-go) default).

| Command | Result |
|---|---|
| `new_project_top` / `new_project_below` | A project as the first block of the list / as `new_below` would place it. |
| `new_section_top` / `new_section_below` | A section placed before the first section of the list, so it comes after the tasks above it, or at the end of the list when it has none / as `new_below` would place it. |
| `new_task_top` / `new_task_below` | A task as the first block of the list / as `new_below` would place it. |

On the zoom title, which stands for the view's root, every command adds the title's first child, except `new_last_child`, which adds its last child; in a pane without blocks the new block is the first one.

With blocks selected, a new block below (`new_below`, `new_*_below`) goes after the last selected block's subtree, and one above or at the top of the list (`new_above`, `new_*_top`) is placed by the first selected block; `new_first_child` and `new_last_child` still add to the focused block. The selection is cleared as the new block opens in edit mode.

The new block's type follows OQ-03 default A: a task next to a task (after it for `new_below`, before it for `new_above`), otherwise text; a new child of a project or task is a task. It opens in edit mode with empty text.

## 4. Structural operations (FR-OPS)

All act on the selection, or on the focused block when nothing is selected. Each block moves with its subtree, so a selected block inside another selected block moves with it rather than on its own, and the zoom title never moves. A block is one line, so no operation merges or splits blocks (D-43). Each command is one undo step however many blocks it changes.

Selected blocks move one after another in document order, last first for `outdent` and `move_down`, so they keep their order. A block that cannot move stays, and so does a selected block whose move would pass it: with the first of a list and the block after it selected, `indent` moves neither.

| Command | Behavior |
|---|---|
| `indent` / `outdent` (FR-OPS-02) | Indent: become the last child of the previous sibling block (expanding it); no-op on the first sibling. Outdent: become the next sibling of the parent; following siblings stay where they are (Workflowy, inferred); no-op at the pane's zoom root level. |
| `move_up` / `move_down` (FR-OPS-03) | Swap with the adjacent sibling block. At the first/last position, move to the end/start of the previous/next sibling of the parent (expanding it); no-op when there is none. Never leaves the pane's zoom root. |
| `group` (FR-OPS-04) | Wrap the selection in a new parent block and open it in edit mode. The parent is a text block, placed where the first selected block was; the selected blocks become its children in document order. Creating and naming it is one undo step. |
| `move_to` (FR-OPS-05) | Fuzzy-pick a destination block (any block, same picker as Go to Symbol); the selection becomes its last children. |
| `delete` (FR-OPS-06) | Remove the blocks with their subtrees; focus moves to the next visible block after the last of them, else the previous one. |
| `duplicate` | Deep copy inserted after the original. |
| `copy` / `cut` / `paste_below` / `paste_above` | Copy puts TaskPaper text on the clipboard: the selected blocks' lines, tags included, tab-indented relative to the shallowest block, `\n` line endings. Paste parses the clipboard with the TaskPaper parser and inserts the blocks after (or before) the focused block at its depth, keeping relative structure. A single line of plain text pastes as one text block. |
| `count` (FR-OPS-09) | Status message: number of blocks and words under the focused block (or the pane's zoom root). |

Clipboard backend: the system clipboard via `arboard` when a display server is reachable; otherwise OSC 52 for copy (works over SSH and tmux) and the terminal's bracketed paste for paste.

## 5. Block commands

| Command | Behavior |
|---|---|
| `convert_type` (FR-BLK-08) | In edit mode, the type menu of §2.3 does this for the edited block. Picker: project, task, text, section, URL, divider. Rewrites each selected block's syntax: removes `- `/`:` and the type tags `@section`, `@url`, then applies the new type's syntax (`- `, `:`, `@section`, `@url(…)` prompting for the address); converting to a divider replaces the text with `---` after confirmation if the text was non-empty. |
| `toggle_done` (FR-DONE-01) | If every target is done, remove `@done` (with any value); otherwise add `@done(YYYY-MM-DD)` with today's local date to those that are not done (D-4). |
| `set_start` / `set_due` (FR-TAG-03) | Prompt for a date with live preview of the resolved date, or of why the input is not a date, in which case `Enter` does nothing; accepts `YYYY-MM-DD` and the natural forms in [search.md §4](search.md#4-dates) (`today`, `tomorrow`, `fri`, `next week`, `+3d`). The prompt opens with the block's current date typed and selected, so typing replaces it and `Enter` keeps it. Empty input removes the tag. Stored as `YYYY-MM-DD` (D-6). |
| `start_today` / `start_tomorrow` / `due_today` / `due_tomorrow` (D-26) | Set `@start` or `@due` to today's or tomorrow's local date, without a prompt. |
| `start_earlier` / `start_later` / `due_earlier` / `due_later` (D-26) | Move `@start` or `@due` one day back or forward. A block without that date gets yesterday or tomorrow, as if the missing date were today. |
| `toggle_today` (FR-TAG-04) | Add bare `@today` to the targets without it; if every target has it, remove it. |
| `set_color` (FR-TAG-05) | Picker of the six colors and "none", in that order, opening with the block's color (or "none") highlighted; typing filters it by name. Removes every other color tag; the chosen one stays where it is or is added (D-5). |
| `cycle_color` (D-26) | Step through none, red, orange, yellow, green, blue, violet and back to none, starting from the focused block's color; every selected block gets the same next color. |
| `toggle_bookmark` (D-11) | Add `@bookmark` to the targets without it; if every target has it, remove it. The status line says which, since bookmarks show only in the sidebar. |
| `open_url` (FR-BLK-06) | Open the URL block's address, or a bare URL under the caret or in the focused block (D-10), in the system browser (`open`, `xdg-open`, `start`). |
| `details` (D-3) | Read-only popup titled "Details:" listing the focused block's tags as written, in order, each unsupported one marked "unsupported" (FR-TAG-02), then each special node the block owns as hidden data with its tags and line count ([special-nodes.md §1](special-nodes.md#1-special-nodes-fr-spc-01)); "No tags" when there is nothing to list. Any key closes it. |

All block commands apply to every selected block (FR-OPS-07). Tags are written with the rules in [file-format.md §5](file-format.md#5-tags).
