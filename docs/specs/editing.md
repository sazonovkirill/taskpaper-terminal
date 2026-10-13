# Spec: Editing

Covers FR-MODE, FR-OPS, FR-BLK-08, FR-TAG, FR-FMT and FR-DONE. All edits are transactions over the [data model](data-model.md#4-edits) and are undoable (FR-OPS-08). Key bindings are in [keybindings.md](keybindings.md).

## 1. Modes

Each pane is in **navigate** or **edit** mode (FR-MODE-01, D-1).

- **Navigate mode** (default): keys move the focus between blocks, select blocks and run block commands.
- **Edit mode**: one block's text is edited in place (FR-MODE-02). `Enter` on the focused block enters edit mode with the caret at the end, and is the only key that does (D-41); `Esc` returns to navigate mode with the same block focused.
- Only the focused pane can be in edit mode; switching panes ends edit mode.

## 2. Edit mode

The block's editor opens where the block is drawn, keeping its indentation, marker, surrounding blocks and scroll position. A block is one line (D-43), so the editor is one line; text wider than the pane scrolls only as far as keeps the caret in view, so the caret moves through the part shown without moving the text. The one-line text fields of prompts, dialogs and panels scroll the same way.

**What is edited** is the block's visible text:
- without a task's `- ` (the type is shown by the marker); a project's `:` is part of the text, typed and deleted like any character, and editing a project starts with the caret before it, at the end of the name, so typing there adds to the name. The line is written back with the task's dash re-applied; if the result parses as a different TaskPaper type (a text line ending in `:` becomes a project, text starting with `-` and whitespace becomes a task), the line takes the new type and its marker updates live (OQ-27). Text that makes a block a task loses its `-` and whitespace to the marker as it is typed. `Backspace` with the caret at the start of a task's text, nothing selected, deletes the dash, also in an empty task, and deleting a project's `:` (`End` then `Backspace`, or `Delete` before it) leaves text; the block then takes the type its line reads as, the dash's removal as one change of the session (D-52). The tags stay: a section that gains `- ` is a task whose `@section` has no effect, and is a section again once its dash is deleted. A task whose text is emptied while it has tags keeps the placeholder `…` as its text, because `- @tag` is read back as text. A divider is text like any other and is edited as its raw `---` (D-52);
- without any tags (FR-TAG-01): tags are hidden and cannot be typed. Typing `@` at the start of a word is refused with the hint "Tags are set in Edit Node (e)", because TaskPaper has no escape and the word would become a tag on reload (OQ-25). `@` inside a word (`me@example.com`) is ordinary text. Text pasted in edit mode follows the same rule: a tag-shaped word is pasted without its leading `@`, with a status message. A pasted single line, ignoring line breaks that end it, goes in at the caret as one undo step; when it is the one block the app copied last (§4), it goes in as the text the block shows, without its type syntax and tags, so `y` then `Ctrl+V` while editing another block reuses its words; text of several lines ends editing and becomes blocks below the block as `paste_below` makes them, in place of the block when its text is empty and it has no tags or children (FR-OPS-06);
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
- `open_url` (§5) opens the link of a block: the only one in the block, or one chosen from a picker when the block has several. Until the picker of step 4.2, it opens the `@url` address, else the first link in the text whose address has a scheme, so `[notes](notes.txt)` is skipped.

### 2.3 Command menu

Typing `/` in edit mode opens a menu at the caret listing every command that applies to the edited block (D-50, amending D-44). It is titled "Commands:" and lists, in this order, five groups set apart by a line that cannot be highlighted:

| Group | Entries | Command |
|---|---|---|
| Type | Project, Task, Note | Give the block that type (FR-BLK-08); "Note" is the menu's label for a text node |
| | Make Section on a text block, or Remove Section on a section | Add or remove `@section` (D-52), as `S` does (§5) |
| Status | Complete, or Uncomplete when the block is done | `toggle_done` (§5) |
| | Toggle Star | `toggle_star` (§5) |
| | Toggle Flag | `toggle_flag` (§5) |
| | Bookmark, or Remove Bookmark when the block has one | `toggle_bookmark` (§5) |
| Dates | Set Start Date, Set Due Date | `set_start` / `set_due`: the date prompt (§5) |
| | Start Today, Start Tomorrow, Due Today, Due Tomorrow | `start_today`, `start_tomorrow`, `due_today`, `due_tomorrow` (§5) |
| | Clear Start Date, Clear Due Date, each only when the block has that date | Remove the date, as the prompt does with an empty text |
| Color | Color: Red, Color: Orange, Color: Yellow, Color: Green, Color: Blue, Color: Violet; Remove Color only when the block has a color | Give the block that color, or none (FR-TAG-05) |
| Block | Edit Node | the Edit Node dialog (§7) |
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

Every other entry runs the same command as its key in navigate mode, on the edited block. It ends the edit session, which stays one undo step, runs as a step of its own, and editing goes on in the block with the caret at the column it had. A block the command takes out of the pane, such as one completed while completed blocks are hidden or an entry that leaves a special view, stays where it was shown until editing ends, and only then leaves, the focus moving as [navigation.md §2](navigation.md#2-focus-and-motion-fr-nav-01) says (D-105). A command that opens the date prompt or the Edit Node dialog returns to editing when that closes, whether the date was set or not. After Duplicate editing goes on in the original block; Delete leaves navigate mode on the block after it.

The list is a core list of the commands on one block, with their labels; the command registry of step 4.1 is meant to absorb it.

### 2.4 Cycling the type

`Ctrl+T` in edit mode gives the edited block the next of project, section, task and note (text), in that order and back to project, keeping the text and the caret (D-50). A section is a text block with `@section` (D-52): a project becomes a text block with `@section`, a section a task without it, and any other text block, a divider among them, a project. It changes the line as the command menu does, and each press is a change of the session's undo step.

## 3. Creating blocks (FR-MODE-04)

| Command | Result |
|---|---|
| `new_below` | New block after the focused block's subtree, as its next sibling. |
| `new_above` | New block as the focused block's previous sibling. |
| `new_last_child` | New block as the focused block's last child (expanding it), after its attachments and before the other special nodes that end its children ([attachments.md §1](attachments.md#1-storage-fr-att-01), [special-nodes.md §1](special-nodes.md#1-special-nodes-fr-spc-01)). |
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

The siblings a block moves past or under are those the pane shows: while it hides completed blocks (FR-DONE-02), a done block is passed over and stays where it is, so `indent` goes under the previous shown sibling, `move_up`/`move_down` swap with the adjacent shown sibling and cross into the parent's previous or next shown sibling, and where only hidden blocks lie that way, the block does not move.

No move or indent puts a block under an empty line, which would then need the `…` placeholder (D-100, [file-format.md §3.1](file-format.md#31-canonical-outlines)): the line to go under is looked for past empty lines as past hidden ones, so `indent` goes under the previous sibling that is not empty and a block at the first or last position crosses into the parent's previous or next sibling that is not empty, or does not move. Swapping with the adjacent sibling still passes an empty line one step like any block.

| Command | Behavior |
|---|---|
| `indent` / `outdent` (FR-OPS-02) | Indent: become the last child of the previous sibling block (expanding it); no-op on the first sibling. Outdent: become the next sibling of the parent; following siblings stay where they are (Workflowy, inferred); no-op at the pane's zoom root level. |
| `move_up` / `move_down` (FR-OPS-03) | Swap with the adjacent sibling block. At the first/last position, move to the end/start of the previous/next sibling of the parent (expanding it); no-op when there is none. Never leaves the pane's zoom root. |
| `group` (FR-OPS-04) | Wrap the selection in a new parent block and open it in edit mode. The parent is a text block, placed where the first selected block was; the selected blocks become its children in document order. Creating and naming it is one undo step. |
| `move_to` (FR-OPS-05) | Fuzzy-pick a destination block (any block, same picker as Go to Symbol, but without the blocks that move and what they hold); the selection becomes its last children, after its attachments and before its other special nodes ([attachments.md §1](attachments.md#1-storage-fr-att-01)). When every block that moves is a project, `⌂ Home`, the document's top level, is listed first, and choosing it makes them the last top-level blocks (D-56). The focus stays on a block that moved, else goes to the last of them; the destination and the blocks above it in the zoom expand, and a destination outside the zoom becomes the zoom, Home going home, a step in the history. |
| `delete` (FR-OPS-06) | Remove the blocks with their subtrees; focus moves to the next visible block after the last of them, else the previous one. |
| `duplicate` | Deep copy of each block, inserted after the original's subtree; the focus moves to the copy of the last one. |
| `copy` / `cut` / `paste_below` / `paste_above` | Copy puts TaskPaper text on the clipboard: the selected blocks' lines, tags and special nodes included, tab-indented relative to the shallowest block, `\n` line endings. Paste parses the clipboard with the TaskPaper parser and inserts the blocks after (or before) the focused block at its depth, keeping relative structure, as one undo step that focuses the last top-level block pasted; on the zoom title they become its first children. A single line of plain text pastes as one text block. In edit mode, copy and cut take the selected text (§2). |
| `count` (FR-OPS-09) | Status message: number of blocks and words under the focused block, a divider's title words but not its dashes (D-76), (or the pane's zoom root); in the Today view, under the focused entry, and none when the view is empty. |

Clipboard backend: the system clipboard via `arboard` when a display server is reachable; otherwise OSC 52 for copy (works over SSH, and in tmux with `set-clipboard on`), and for paste the terminal's bracketed paste, which pastes below the focused block or into the text being edited, while `Ctrl+V` and `Alt+V` paste the text the app copied last.

## 5. Block commands

| Command | Behavior |
|---|---|
| `convert_type` (FR-BLK-08) | In edit mode, the command menu of §2.3 and `Ctrl+T` (§2.4) do this for the edited block. In navigate mode, `my` opens a picker titled "Node type:" listing Project, Task, Note (text), Make Section and Remove Section, opening with the focused block's type highlighted, Make Section for a section; typing filters it by name. The chosen entry goes to the selected blocks, else the focused block, as one undo step (FR-OPS-07); the section entries do what `make_section` and `remove_section` do. Rewrites the block's syntax as the command menu does: removes `- `/`:` and `@section`, then applies the new type's syntax (`- `, `:`). A colon ending a task's text is the project's own when it becomes a project, and the colons ending it go when it becomes a note or section that they would otherwise make a project; other tags stay, `@url` included, since a link is not a type (D-45). A divider is text, so it keeps its `---` (D-52). Choosing the block's own type changes nothing. |
| `make_section` / `remove_section` / `toggle_section` (FR-BLK-04, D-52, D-130) | Make Section adds bare `@section` to the targets that are text blocks without it, making them sections; Remove Section removes `@section` with any value from those that have it, making them plain text, as one undo step. `toggle_section`, on `S`, removes it when every target that is a text block is a section, and adds it otherwise. Make Section and Remove Section have no key. Tasks and projects are never sections: they are left as they are, `@section` included when they carry it, and the status line says how many were skipped ("Only text can be a section: 1 skipped"). |
| `toggle_done` (FR-DONE-01) | If every target is done, remove `@done` (with any value); otherwise add `@done(YYYY-MM-DD)` with today's local date to those that are not done (D-4). |
| `set_start` / `set_due` (FR-TAG-03) | Prompt for a date, in a panel at the bottom of the screen ([ui.md §1](ui.md#1-layout), D-54), with live preview of the resolved date, or of why the input is not a date, in which case `Enter` does nothing; accepts `YYYY-MM-DD` and the natural forms in [search.md §4](search.md#4-dates) (`today`, `tomorrow`, `fri`, `next week`, `+3d`). The prompt opens with the block's current date typed and selected, so typing replaces it and `Enter` keeps it. Empty input removes the tag. Stored as `YYYY-MM-DD` (D-6). |
| `start_today` / `start_tomorrow` / `due_today` / `due_tomorrow` (D-26) | Set `@start` or `@due` to today's or tomorrow's local date, without a prompt. |
| `start_earlier` / `start_later` / `due_earlier` / `due_later` (D-26) | Move `@start` or `@due` one day back or forward. A block without that date gets yesterday or tomorrow, as if the missing date were today. |
| `toggle_star` (FR-TAG-04, D-88, D-130), on `s` | Add bare `@star` to the targets without it; if every target has it, remove it. |
| `set_color` (FR-TAG-05) | Picker of the six colors and "none", in that order, each color with its number 1 to 6, opening with the block's color (or "none") highlighted; typing a number chooses that color at once, and other typing filters it by name (D-48). Removes every other color tag; the chosen one stays where it is or is added. The chosen color goes to the selected blocks, else the focused block, as one undo step (FR-OPS-07). |
| `color_1` … `color_6` / `remove_color` (D-48) | Give the targets red, orange, yellow, green, blue or violet, or remove their color, without the picker, as one undo step. |
| `cycle_color` (D-26) | Step through none, red, orange, yellow, green, blue, violet and back to none, starting from the focused block's color; every selected block gets the same next color. |
| `toggle_flag` (FR-TAG-12, D-91) | Add bare `@flag` to the targets without it; if every target has it, remove it, with any value. A flagged block shows a red flag after its text ([ui.md §2](ui.md#2-blocks)). |
| `toggle_bookmark` (D-11) | Add `@bookmark` to the targets without it; if every target has it, remove it. The status line says which; a bookmarked block shows a bookmark icon after its text (D-47). |
| `open_url` (FR-BLK-06) | Open the focused block's link in the system browser (`open` on macOS, `xdg-open` on Linux; N-PORT-01): its `@url` address first, else the first link in its text (§2.2, D-45). It acts on the focused block also while blocks are selected. |
| `edit_node` (FR-MODE-09, D-103) | The Edit Node dialog on the focused block (§7), which lists its tags and the special nodes it owns, except its recognized attachments, which the attachments panel shows ([special-nodes.md §1](special-nodes.md#1-special-nodes-fr-spc-01)). |

All block commands apply to every selected block (FR-OPS-07). Tags are written with the rules in [file-format.md §5](file-format.md#5-tags).

## 6. Quick Add (FR-MODE-07, FR-MODE-08)

Quick Add adds a task or note to a project without leaving the current view (D-96).

- **Opening.** `qq` (Quick Add in Go to Command) opens it with no destination; `qi` (Quick Add to Inbox) with the INBOX project the Navigator lists apart (D-75) and `qt` (Quick Add to Today) with the TODAY project (D-98), whose tasks the Today view lists (D-102), as the destination. When that project is missing, or its setting is off, `qi` and `qt` change nothing and the status line says "No INBOX project in the Navigator" or "No TODAY project". A read-only file refuses with its notice. Quick Add opens from any navigate context, the Today and other special views included, and does not change the panes.
- **Panel.** A panel above the status line, placed and dimming the screen as the date prompt does ([ui.md §1](ui.md#1-layout)), titled `Quick Add` and, once there is one, the destination's path (`Quick Add → Work › Launch`). Its first line holds the text, its second what `Enter` would do: the dates found (`Start Thu 8 Oct · Due Mon 12 Oct`), or what is missing ("Choose a project with /", "Type the text").
- **Type.** The text starts as a task: the dash stands before it as edit mode shows a task's. `Backspace` at the start of the text deletes the dash, adding a note instead, and `- ` typed at the start of a note's text makes it a task again, as in edit mode (OQ-27). Colons ending a note's text are dropped on adding, so it does not become a project; a task's text keeps them, as edit mode keeps them.
- **Text.** The standard text keys of prompts. Nothing breaks the line: a paste's line breaks become spaces, leaving out those that end it, and the blocks the app copied last (§4) paste as the text they show, as in edit mode (§2), their subtrees left out and their texts joined with spaces; an `@` that would start a tag is not typed, with edit mode's notice (OQ-25).
- **Destination.** `/` typed at the start of the text or after a space opens a project picker above the panel: a finder over the shown projects with their paths, as Go to Project lists them (D-65). Characters filter it, `↑`/`↓` or `Ctrl+P`/`Ctrl+N` move, `Enter` makes the highlighted project the destination and closes the picker, the text as it was; `Esc`, or a filter that matches nothing, closes it and types `/` and the filter into the text, and `Backspace` on an empty filter closes it, as the command menu does (§2.3). `/` anywhere else is typed.
- **Dates** (D-97). A run of whole words that the date grammar reads ([search.md §4](search.md#4-dates)) is a start date, or a due date when the word `due` stands right before it: `Buy milk tomorrow`, `Pay rent due next Mon`, `Plan trip next weekend`. Runs are found left to right, the longest at each word; a run starting with a bare number, such as `3 days`, does not count, nor does a start date that is only a weekday's three-letter abbreviation, so `Buy sun screen` stays text while `this sun`, `next sun` and `due sun` are dates (D-112). A start date before today does not count either, and none of its words read as a date of their own, so `Review last week sales` and `Notes from yesterday` stay text while `due yesterday` is a date (D-126). When several start or due dates are typed, the last of each counts. The dates that count are highlighted in the text as they are typed, a start date in the start chip's color and a due date, its `due` included, in the due chip's (D-46), and the panel's second line shows what they resolve to.
- **Adding.** `Enter` adds the block, when there is a destination and text besides the dates and a note's ending colons: the text without the dates that count and the spaces around them, then `@start` and `@due` with their dates (D-6), as the destination's last child, as `new_last_child` places it: after its attachments and before the other special nodes that end its children ([attachments.md §1](attachments.md#1-storage-fr-att-01), [special-nodes.md §1](special-nodes.md#1-special-nodes-fr-spc-01)), expanding nothing, as one step to undo. The panel closes and the status line says where the block went ("Added to INBOX"). A destination deleted while the panel was open counts as none. When the file is reloaded from disk ([file-io.md §3](file-io.md#3-external-changes)), the panel stays open, the destination stays on the same project, found by its path, and the project picker closes.
- **Closing.** `Esc` closes the panel, adding nothing.

## 7. Edit Node dialog (FR-MODE-09)

The Edit Node dialog shows and changes every property of one block in a modal window, as the task windows of Things 3 and 2Do do (D-103). It is the only place the app shows a block's tags and the only place any tag can be typed (FR-TAG-01): commands such as `x`, `s` or `c1` still set the tags they own.

- **Opening.** `e` (Edit Node in Go to Command and the context menu) and `mi` open it on the focused block, also while blocks are selected and in the special views; the zoom title counts as focused. Edit Node in edit mode's command menu (§2.3) opens it on the edited block and editing resumes when it closes, as after the date prompt. It replaces the details popup (D-3).
- **Window.** A box centered over the panes like the pickers, its border and title `Edit Node` in the accent color, as wide as it can be up to 64 columns and as tall as its rows, scrolling to keep the focused row in view, and on the last row to its end, so the Hidden rows show. Each row is a label in a column of its own, dimmed, and a value; a value too wide ends in `…`, keeping a trailing marker such as `unsupported` or `not a date` in view, and the Parent path keeps its end, as the status line keeps the file's (ui.md §1); the focused row is marked like the outline's focused block. The last line is a dimmed hint naming the keys of the focused row, or, while a field is open, what `Enter` would do, a reason in the warning color when it would do nothing. A sketch, the focus on Due:

  ```
  ┌ Edit Node ───────────────────────────────────────────────────┐
  │  Text     Book the venue                                     │
  │  Type     - Task                                             │
  │  Done     Not done                                           │
  │  Parent   Work › Launch                                      │
  │  Start    ↦ Thu 8 Oct 2026 · Today                           │
  │▌ Due      ⇥ Mon 12 Oct 2026 · Mon                            │
  │  Starred  ★ Starred                                          │
  │  Flagged  Not flagged                                        │
  │  Tags     @context(phone)                                    │
  │           @green                                             │
  │           + Add tag                                          │
  │  Hidden   @hidden @log · 3 lines                             │
  │                                                              │
  │ Type or ←/→ a date · Del clears · Enter save · Esc cancel    │
  └──────────────────────────────────────────────────────────────┘
  ```

- **Rows**, in this order:

  | Row | Shows | Changed by |
  |---|---|---|
  | Text | The block's text as edit mode edits it (§2), without a project's `:`, which the type gives | Typing: the standard text keys of prompts. An `@` that would start a tag is not typed, with the notice "Tags are set in the Tags rows below" (OQ-25), and nothing breaks the line: a paste's line breaks become spaces, leaving out those that end it, and the blocks the app copied last paste as Quick Add takes them (§6) |
  | Type | Project, Task or Note, after its icon | `Space` or `→` the next of task, note and project, `←` the previous; `p`, `t` or `n` that type. A divider and a section are notes, a section being a note with `@section` among its tags |
  | Done | `Not done`, dimmed, or the done icon, `Done` and the value of `@done` | `Space` or `x`: adds `@done` with today's date (D-4) or removes it with any value |
  | Parent | The path to the block's parent, `›` between its steps, or the home icon and `Home` for a top-level block | `Space`, or a character, which starts its filter, opens a finder over the blocks it can move under, as Move to lists them, `Home` first when the type is Project (D-56); choosing one makes it the parent. A block given another type while `Home` is chosen goes back to its own parent. The block then moves with its subtree, as its last child, as `move_to` places it (§4) |
  | Start, Due | The date as `Thu 8 Oct 2026` and named relative to today as the date chips do, with the chip's icon and color, `None`, dimmed, without one, or the tag's value as written, marked "not a date", when it is no date | `Space` opens a field holding the date as `YYYY-MM-DD`, selected, and a character opens it empty and types the character; it takes what the date prompt takes, with its preview (§5), `Enter` setting the date, or removing it when the field is empty, and `Esc` leaving the field. `←`/`→` move the date a day back or forward, as from today when there is none (D-26); `Backspace` or `Delete` removes it |
  | Starred, Flagged | The star or flag icon and `Starred` or `Flagged` in its chip's color when set, `Not starred` or `Not flagged`, dimmed, when not | `Space`; `s` and `!` as in navigate mode, and `*` on Starred, its key before D-130 |
  | Tags | Every other tag, one per row, as written but for escapes, `@name` or `@name(value)`, in the line's order; a tag the app does not read marked "unsupported" (FR-TAG-02). The first occurrence of a name counts, as TaskPaper reads it, and is the one listed | `Space` opens a field holding the tag without its `@`; a character opens it empty and types the character. `Enter` writes it: a name, or a name and a value in parentheses whose own parentheses pair up, a leading `@` dropped; an empty field removes the tag, and anything else that is not a tag keeps the field open, saying why: a name TaskPaper keeps (`id`, `text`, `type`) or `hidden`, which only special nodes carry ([special-nodes.md §1](special-nodes.md#1-special-nodes-fr-spc-01)), is not a tag here. `Backspace` or `Delete` removes the tag |
  | + Add tag | The last row of Tags | As a tag row, its field opening empty; the new tag goes after the others and the focus stays on + Add tag for the next one. A name the block has already sets that tag's value. A name with a row of its own, such as `due`, changes that row |
  | Hidden | The special nodes the block owns, as data the app does not show: each one's tags and line count, as the details popup listed them (D-3); only when there are some | Nothing; the focus skips it |

- **Keys.** `Tab` or `↓` focus the next row, `Shift+Tab` or `↑` the previous one, stopping at the first and last. `Ctrl+T` changes the type from any row, the Text row included, as it cycles the type in edit mode (§2.4). `Enter` closes the dialog and applies every change as one step to undo; `Esc` closes it and changes nothing. While a field or the parent finder is open it takes the keys: `Enter` accepts and `Esc` leaves it, the dialog staying open. Every key is a plain key or `Ctrl` with a letter, so both key reportings send it (D-42); `Ctrl+Q` still quits.
- **Applying.** When nothing changed, nothing happens. A changed text or type rewrites the line as edit mode writes it back (§2), the type's syntax as `convert_type` gives it (§5); then each tag that was removed goes with every occurrence, a renamed tag takes its new name where its listed occurrence stands, a later duplicate the dialog does not list staying as it is (or the listed one goes when the line already has the new name), each changed tag gets its new value where it stands, and each added tag goes at the end, in the order added. Changing the type keeps every listed tag: `@section` stays on a block that is no longer a note, without effect, as `my` would not keep it. A task whose text is empty while it has tags gets the placeholder `…` as its text. The focus stays on the block; a block moved outside the pane's zoom is followed there as Move to does (§4). A read-only file shows the dialog but refuses the change with its notice.
- **Reloading.** When the file is reloaded from disk ([file-io.md §3](file-io.md#3-external-changes)) the dialog closes, changing nothing, since its draft is of the block as it was; unlike edit mode and the date prompt, which carry on (D-104).
