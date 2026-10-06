# Spec: Editing

Covers FR-MODE, FR-OPS, FR-BLK-08, FR-TAG, FR-FMT and FR-DONE. All edits are transactions over the [data model](data-model.md#4-edits) and are undoable (FR-OPS-08). Key bindings are in [keybindings.md](keybindings.md).

## 1. Modes

Each pane is in **navigate** or **edit** mode (FR-MODE-01, D-1).

- **Navigate mode** (default): keys move the focus between blocks, select blocks and run block commands.
- **Edit mode**: one block's text is edited in place (FR-MODE-02). `Enter` on the focused block enters edit mode with the caret at the end, and is the only key that does (D-41); `Esc` returns to navigate mode with the same block focused.
- Only the focused pane can be in edit mode; switching panes ends edit mode.

## 2. Edit mode

The block's editor opens where the block is drawn, keeping its indentation, marker, surrounding blocks and scroll position. A block is one line (D-43), so the editor is one line; text wider than the pane scrolls to keep the caret in view.

**What is edited** is the block's visible text:
- without a task's `- ` (the type is shown by the marker); a project's `:` is part of the text, typed and deleted like any character, and editing a project starts with the caret before it, at the end of the name, so typing there adds to the name. The line is written back with the task's dash re-applied; if the result parses as a different TaskPaper type (a text line ending in `:` becomes a project, text starting with `-` and whitespace becomes a task), the line takes the new type and its marker updates live (OQ-27). Text that makes a block a task loses its `-` and whitespace to the marker as it is typed. `Backspace` with the caret at the start of a task's text, nothing selected, deletes the dash, also in an empty task, and deleting a project's `:` (`End` then `Backspace`, or `Delete` before it) leaves text; the block then takes the type its line reads as, the dash's removal as one change of the session (D-52). The tags stay: a section that gains `- ` is a task whose `@section` has no effect, and is a section again once its dash is deleted. A task whose text is emptied while it has tags keeps the placeholder `…` as its text, because `- @tag` is read back as text. A divider is text like any other and is edited as its raw `---` (D-52);
- without any tags (FR-TAG-01): tags are hidden and cannot be typed. Typing `@` at the start of a word is refused with the hint "Tags are set with commands", because TaskPaper has no escape and the word would become a tag on reload (OQ-25). `@` inside a word (`me@example.com`) is ordinary text. Text pasted in edit mode follows the same rule: a tag-shaped word is pasted without its leading `@`, with a status message. A pasted single line, ignoring line breaks that end it, goes in at the caret as one undo step; text of several lines ends editing and becomes blocks below the block as `paste_below` makes them, in place of the block when its text is empty and it has no tags or children (FR-OPS-06);
- with inline Markdown markers visible (`**bold**`, `` `code` ``) (FR-FMT-01) and the raw link syntax of §2.2, and in text blocks the `#` heading markers (FR-FMT-03, D-45).

Edits are applied to the document as the user types (one undo step per edit session) and written back as: type syntax + edited text + the line's tags appended after the text in their original order (OQ-24 default A). A line whose text and type are as they were is written as it was, so its mid-line tags stay where they are.

**Text keys** are standard (FR-MODE-03): `←`/`→`, `Home`/`End` to the ends of the text, `Ctrl+←/→` and `Alt+B/F` by word, `Backspace`/`Delete`, `Ctrl+W`/`Alt+Backspace` delete word, `Shift`+motion selects (`Shift+↑`/`Shift+↓` select to the start and end of the text), `Ctrl+A` selects all, `Ctrl+C`/`Ctrl+X`/`Ctrl+V` clipboard. Motion is by grapheme (`unicode-segmentation`). Characters the parser splits lines on ([file-format.md §1](file-format.md#1-input-decoding)) are never typed into a line, and no key breaks a line (D-43).

**Formatting** (FR-FMT-02): `Ctrl+B` wraps the selection (or the word at the caret, or nothing at the caret) in `**`, or removes them when the selection or caret is inside bold text or between the markers; `` Alt+` `` and `Ctrl+E` do the same with backticks.

**Enter, arrows and leaving** (OQ-23, D-43):

| Key | Any block |
|---|---|
| `Enter` | End editing; create the next block in edit mode (§3) |
| `↑` / `↓` | End editing and edit the previous / next visible block, dividers included (D-52), with the caret as many graphemes from the start as it was, or at the end of a shorter text, before a project's `:`, like moving between lines in a traditional editor. Each block's session is its own undo step. On the first / last such block nothing happens |
| `Esc` | End editing |
| `Tab` / `Shift+Tab` | Indent / outdent the block, stay in edit mode |
| `Backspace` in an empty block, or before the lone `:` of a project without a name | An empty task first loses its dash (above). Delete the block, edit the previous visible block at its end; a block with children and the first row stay. A block created in this session leaves no undo step |

Ending edit mode applies the canonical-outline repairs for the line ([file-format.md §3.1](file-format.md#31-canonical-outlines)).

### 2.1 Typed shortcuts

Dropped (D-52). Typed prefixes such as `--- ` and `[] ` no longer change a block's type, and a space is always typed as text. A divider is text typed as `---` (D-52). `- ` and a trailing `:` still make a task and a project, and deleting them makes the block text again (§2, OQ-27). `# ` starts a Markdown heading in a text block (§2.2, OQ-28).

### 2.2 Markdown in text

Every block supports these links, and text blocks, sections included (D-52), also headings, in addition to FR-FMT-01 (FR-FMT-03, D-25, D-45). They are derived from the line's text for display only; the text is stored unchanged, so the file stays plain TaskPaper.

| Syntax | Meaning |
|---|---|
| `#` to `######`, a space, then text, at the start of the line | Heading of level 1 to 6, in text blocks only. A line ending in `:` is a project, not a heading (OQ-27). |
| `[title](url)` | Link showing `title`, which may hold bold and code but no other link. The address has no spaces, and its parentheses pair up. |
| `<url>` and bare `http(s)://` URLs | Link showing the URL. A bare URL is not preceded by a letter or digit and ends before whitespace, `` ` ``, `<`, `>`, `"`, trailing punctuation or an unmatched `)`. Between `<` and `>` it starts with a scheme such as `https:` or `mailto:`. |
| `` `code` `` | Code span (FR-FMT-01). |

- In navigate mode the markers are hidden and the result is rendered ([ui.md §2](ui.md#2-blocks)). In edit mode the raw text is shown and edited (FR-MODE-02).
- Anything else, including italic, lists, quotes, tables and fenced code, is ordinary text (D-14, D-43).
- A hidden `@url(…)` tag on a line of any type is a link too: the block's whole title is drawn as one, and `open_url` opens its address before any link in the text (FR-BLK-06, D-45).
- `open_url` (§5) opens the link of a block: the only one in the block, or one chosen from a picker when the block has several, as for comments ([special-nodes.md §2.2](special-nodes.md#22-display)). Until the picker of step 4.2, it opens the `@url` address, else the first link in the text whose address has a scheme, so `[notes](notes.txt)` is skipped.

### 2.3 Command menu

Typing `/` in edit mode opens a menu at the caret listing every command that applies to the edited block (D-50, amending D-44). It is titled "Commands:" and lists, in this order, five groups set apart by a line that cannot be highlighted:

| Group | Entries | Command |
|---|---|---|
| Type | Project, Task, Note | Give the block that type (FR-BLK-08); "Note" is the menu's label for a text node |
| | Make Section on a text block, or Remove Section on a section | Add or remove `@section` (D-52), as `S` and `s` do (§5) |
| Status | Complete, or Uncomplete when the block is done | `toggle_done` (§5) |
| | Toggle Today | `toggle_today` (§5) |
| | Bookmark, or Remove Bookmark when the block has one | `toggle_bookmark` (§5) |
| Dates | Set Start Date, Set Due Date | `set_start` / `set_due`: the date prompt (§5) |
| | Start Today, Start Tomorrow, Due Today, Due Tomorrow | `start_today`, `start_tomorrow`, `due_today`, `due_tomorrow` (§5) |
| | Clear Start Date, Clear Due Date, each only when the block has that date | Remove the date, as the prompt does with an empty text |
| Color | Color: Red, Color: Orange, Color: Yellow, Color: Green, Color: Blue, Color: Violet; Remove Color only when the block has a color | Give the block that color, or none (FR-TAG-05) |
| Block | Show Details | `details` (§5) |
| | Open Link, only when the block has a link | `open_url` (§5) |
| | Duplicate, Delete | `duplicate`, `delete` (§4) |

The `/` is not typed while the menu is open.

| Key | Effect |
|---|---|
| Characters | Filter the entries, highlighting the first match: each word typed, ignoring case, must start a word of the label, in order, so `/date` leaves Set Start Date and Set Due Date (and Clear … Date when there is one), `/due` the entries with "Due", and `/p` Project first. A line stays only between two groups that both still have matches |
| `↑`/`↓`, `Ctrl+P`/`Ctrl+N` | Move the highlight over the entries, skipping the lines between groups; a list taller than the room at the caret scrolls to keep it in view |
| `Enter` | Run the highlighted entry |
| `Esc` | Close the menu and type `/` and the filter as text |
| `Backspace` | Erase the filter's last character; on an empty filter, close the menu, typing nothing |

A filter that matches no entry closes the menu and types `/` and the filter, so a path such as `/home` stays text, and so does `/` followed by a space, as in "a / b". Other keys do nothing while the menu is open. D-44's instant letters (`/p`, `/s`, `/n`, `/t`) are gone, since filtering needs them: `/s` would otherwise make a section before `/set` is typed. `/p`, `/t` or `/n` then `Enter` still choose Project, Task or Note, which come first; `/make` and `/remove` reach the section entries.

Choosing a type rewrites the line as `convert_type` (§5) does: the old type's syntax and `@section` go, other tags, `@url` included, stay, and the new type's syntax is added: `- ` for a task, `:` before the trailing tags for a project, nothing for text. Make Section adds `@section` after the other tags and Remove Section takes it off (D-52). The caret stays where it was. The change is part of the edit session's single undo step, and within the session `Ctrl+Z` takes it back on its own.

Every other entry runs the same command as its key in navigate mode, on the edited block. It ends the edit session, which stays one undo step, runs as a step of its own, and editing goes on in the block with the caret at the column it had. A command that opens the date prompt or the details popup returns to editing when that closes, whether the date was set or not. After Duplicate editing goes on in the original block; Delete leaves navigate mode on the block after it.

The list is a core list of the commands on one block, with their labels; the command registry of step 4.1 is meant to absorb it.

### 2.4 Cycling the type

`Ctrl+T` in edit mode gives the edited block the next of project, section, task and note (text), in that order and back to project, keeping the text and the caret (D-50). A section is a text block with `@section` (D-52): a project becomes a text block with `@section`, a section a task without it, and any other text block, a divider among them, a project. It changes the line as the command menu does, and each press is a change of the session's undo step.

## 3. Creating blocks (FR-MODE-04)

| Command | Result |
|---|---|
| `new_below` | New block after the focused block's subtree, as its next sibling. |
| `new_above` | New block as the focused block's previous sibling. |
| `new_last_child` | New block as the focused block's last child (expanding it), before the special nodes that end its children ([special-nodes.md §1](special-nodes.md#1-special-nodes-fr-spc-01)). |
| `new_first_child` | New block as the focused block's first child (expanding it). |
| `Enter` at the end of edit mode | `new_below`, except that a block with visible children gets a new first child instead (TaskPaper's indent rule, reference §9). |
| `new_end` / `new_start` (D-53) | New block after the last block of the focused block's list, with its subtree / before the first block of the list. "The list" is the focused block's siblings; in an empty pane it is the zoom block's children ([OQ-29](../open-questions.md#oq-29--where-do-the-bindings-displaced-by-d-26-go) default). |

**Typed creation** (D-53) adds a block of a chosen type at any of these places: the place's key (`o`, `O`, `i`, `I`, `a`, `A`) followed by `s` for a section (a text block with `@section`, D-52), `p` for a project, `t` for a task or `n` for a note, opened in edit mode with empty text. The place's key alone, once the sequence times out or another key follows, adds a block of the type that fits, as below.

On the zoom title, which stands for the view's root, every command adds the title's first child, except `new_last_child` and `new_end`, which add its last child; in a pane without blocks the new block is the first one.

With blocks selected, a new block below or at the end of the list (`new_below`, `new_end`) goes by the last selected block, and one above or at the start of the list (`new_above`, `new_start`) by the first selected block; `new_first_child` and `new_last_child` still add to the focused block. The selection is cleared as the new block opens in edit mode.

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
| `duplicate` | Deep copy of each block, inserted after the original's subtree; the focus moves to the copy of the last one. |
| `copy` / `cut` / `paste_below` / `paste_above` | Copy puts TaskPaper text on the clipboard: the selected blocks' lines, tags and special nodes included, tab-indented relative to the shallowest block, `\n` line endings. Paste parses the clipboard with the TaskPaper parser and inserts the blocks after (or before) the focused block at its depth, keeping relative structure, as one undo step that focuses the last top-level block pasted; on the zoom title they become its first children. A single line of plain text pastes as one text block. In edit mode, copy and cut take the selected text (§2). |
| `count` (FR-OPS-09) | Status message: number of blocks and words under the focused block (or the pane's zoom root). |

Clipboard backend: the system clipboard via `arboard` when a display server is reachable; otherwise OSC 52 for copy (works over SSH, and in tmux with `set-clipboard on`), and for paste the terminal's bracketed paste, which pastes below the focused block or into the text being edited, while `Ctrl+V` and `Alt+V` paste the text the app copied last.

## 5. Block commands

| Command | Behavior |
|---|---|
| `convert_type` (FR-BLK-08) | In edit mode, the command menu of §2.3 and `Ctrl+T` (§2.4) do this for the edited block. In navigate mode, `my` opens a picker titled "Node type:" listing Project, Task, Note (text), Make Section and Remove Section, opening with the focused block's type highlighted, Make Section for a section; typing filters it by name. The chosen entry goes to the selected blocks, else the focused block, as one undo step (FR-OPS-07); the section entries do what `make_section` and `remove_section` do. Rewrites the block's syntax as the command menu does: removes `- `/`:` and `@section`, then applies the new type's syntax (`- `, `:`); other tags stay, `@url` included, since a link is not a type (D-45). A divider is text, so it keeps its `---` (D-52). Choosing the block's own type changes nothing. |
| `make_section` / `remove_section` (FR-BLK-04, D-52) | `S` adds bare `@section` to the targets that are text blocks without it, making them sections; `s` removes `@section` with any value from those that have it, making them plain text, as one undo step. Tasks and projects are never sections: they are left as they are, `@section` included when they carry it, and the status line says how many were skipped ("Only text can be a section: 1 skipped"). |
| `toggle_done` (FR-DONE-01) | If every target is done, remove `@done` (with any value); otherwise add `@done(YYYY-MM-DD)` with today's local date to those that are not done (D-4). |
| `set_start` / `set_due` (FR-TAG-03) | Prompt for a date, in a panel at the bottom of the screen ([ui.md §1](ui.md#1-layout), D-54), with live preview of the resolved date, or of why the input is not a date, in which case `Enter` does nothing; accepts `YYYY-MM-DD` and the natural forms in [search.md §4](search.md#4-dates) (`today`, `tomorrow`, `fri`, `next week`, `+3d`). The prompt opens with the block's current date typed and selected, so typing replaces it and `Enter` keeps it. Empty input removes the tag. Stored as `YYYY-MM-DD` (D-6). |
| `start_today` / `start_tomorrow` / `due_today` / `due_tomorrow` (D-26) | Set `@start` or `@due` to today's or tomorrow's local date, without a prompt. |
| `start_earlier` / `start_later` / `due_earlier` / `due_later` (D-26) | Move `@start` or `@due` one day back or forward. A block without that date gets yesterday or tomorrow, as if the missing date were today. |
| `toggle_today` (FR-TAG-04) | Add bare `@today` to the targets without it; if every target has it, remove it. |
| `set_color` (FR-TAG-05) | Picker of the six colors and "none", in that order, each color with its number 1 to 6, opening with the block's color (or "none") highlighted; typing a number chooses that color at once, and other typing filters it by name (D-48). Removes every other color tag; the chosen one stays where it is or is added. The chosen color goes to the selected blocks, else the focused block, as one undo step (FR-OPS-07). |
| `color_1` … `color_6` / `remove_color` (D-48) | Give the targets red, orange, yellow, green, blue or violet, or remove their color, without the picker, as one undo step. |
| `cycle_color` (D-26) | Step through none, red, orange, yellow, green, blue, violet and back to none, starting from the focused block's color; every selected block gets the same next color. |
| `toggle_bookmark` (D-11) | Add `@bookmark` to the targets without it; if every target has it, remove it. The status line says which; a bookmarked block shows a bookmark icon after its text (D-47). |
| `open_url` (FR-BLK-06) | Open the focused block's link in the system browser (`open` on macOS, `xdg-open` on Linux; N-PORT-01): its `@url` address first, else the first link in its text (§2.2, D-45). It acts on the focused block also while blocks are selected. |
| `details` (D-3) | Read-only popup titled "Details:" listing the focused block's tags as written, in order, each unsupported one marked "unsupported" (FR-TAG-02), then each special node the block owns as hidden data with its tags and line count ([special-nodes.md §1](special-nodes.md#1-special-nodes-fr-spc-01)); "No tags" when there is nothing to list. Any key closes it. |

All block commands apply to every selected block (FR-OPS-07). Tags are written with the rules in [file-format.md §5](file-format.md#5-tags).
