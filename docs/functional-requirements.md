# Functional Requirements

- Status: Accepted
- Last updated: 2026-10-04

What taskpaper-terminal does, as stated by the owner and mapped against Workflowy's feature set as of 2026-10-04. The owner's answers to the clarifying questions are recorded under [Decisions](#decisions) and referenced as **(D-n)**.

Priorities: **Must** is required for v1, **Should** is expected soon after, **Later** is planned after v1 and must not be precluded by the design, **Won't** is out of scope.

## Product summary

taskpaper-terminal is a keyboard-driven, Workflowy-style infinite outliner for the terminal that edits one `.taskpaper` file. It is a **block editor**: moving between and selecting blocks is separate from editing the text of one block, as in Things 3. Tags are metadata the app interprets and hides, not text the user reads.

## Terms

- **Line**: one line of the `.taskpaper` file. TaskPaper classifies it as a project, task or note.
- **Block**: the unit the user navigates, selects and edits. Usually one line; a note block can span several lines ([FR-BLK-03](#fr-blk--blocks)).
- **Navigate mode**: keys act on blocks. **Edit mode**: keys edit the text of one block in place.
- **Pane**: one column of the split view, showing its own zoomed part of the same document.

## FR-DOC · Document and file

| ID | Requirement | Priority |
|---|---|---|
| FR-DOC-01 | One app instance edits exactly one `.taskpaper` file. | Must |
| FR-DOC-02 | The file on disk is the source of truth; every change is saved automatically (D-15). | Must |
| FR-DOC-03 | Text the app does not interpret, including unsupported tags, is preserved on save. | Must |
| FR-DOC-04 | `Archive:` projects get no special treatment; there is no archive command. They are ordinary projects. | Must |
| FR-DOC-05 | Running without a path creates a new file in `/tmp` (D-15). | Must |
| FR-DOC-06 | A Save As command writes the document to a chosen path and continues editing that file; this is how a `/tmp` document is kept. | Must |

## FR-BLK · Blocks

Each block type, how it is stored and how it looks. Types are mutually exclusive per block. A divider, section or URL line is always its own block and ends a note run; a line carrying `@code` starts a new run. Inside a code block, a `---`, `@section` or `@url` line stays code instead of ending the block (D-18).

| ID | Type | Stored as | Shown as | Priority |
|---|---|---|---|---|
| FR-BLK-01 | Project | `Name:` | Bold heading with a fold marker. | Must |
| FR-BLK-02 | Task | `- Text` | Checkbox and text; done tasks dimmed and struck through. | Must |
| FR-BLK-03 | Note | One or more consecutive sibling plain note lines, blank lines included (D-2) | One multi-line text block. The block's children are the children of its last line. | Must |
| FR-BLK-04 | Section | `Name @section` | Things 3 style header with a rule; groups its indented children (D-8). | Must |
| FR-BLK-05 | Code | A note run whose first line carries `@code` (D-9) | Monospace, no inline formatting; Enter inserts a newline while editing. | Must |
| FR-BLK-06 | URL | `Title @url(https://…)` (D-10) | Title styled as a link; a command opens it in the system browser. | Must |
| FR-BLK-07 | Divider | Note line `---` | Horizontal rule across the pane; selectable and movable like any block, no children. | Must |

**FR-BLK-08 (Must):** a command converts the selected blocks between project, task, note, section, code, URL and divider. Because tags are never typed ([FR-TAG-01](#fr-tag--tags-as-metadata)), this is how the tag-based types are created.

## FR-TAG · Tags as metadata

| ID | Requirement | Priority |
|---|---|---|
| FR-TAG-01 | Tags are hidden from the displayed and edited text. The app sets and clears them through commands, never by typing `@`. All tags, supported or not, are hidden; unsupported ones are listed in an item details popup (D-3). | Must |
| FR-TAG-02 | Supported tags: `@start(DATE)`, `@due(DATE)`, `@today`, the colors `@red @orange @yellow @green @blue @violet`, plus the block-type tags `@section @code @url` and the internal `@done(YYYY-MM-DD)` (D-4) and `@bookmark` (D-11). | Must |
| FR-TAG-03 | Dates are stored as `YYYY-MM-DD` and entered through a prompt that accepts natural language such as "tomorrow" or "fri" (D-6). They show as a compact chip; overdue `@due` is highlighted. | Must |
| FR-TAG-04 | `@today` is toggled by a command and shown as a marker. A built-in Today view lists `@today` items and items due or started on or before today (D-7). | Must |
| FR-TAG-05 | A color tag tints the block's text; a block has at most one color, and setting one replaces the other (D-5). | Must |

## FR-FMT · Inline formatting

| ID | Requirement | Priority |
|---|---|---|
| FR-FMT-01 | Task and note text supports Markdown `**bold**` and `` `code` ``. Markers are hidden in navigate mode and visible in edit mode. | Must |
| FR-FMT-02 | Commands toggle bold and inline code on the selected text in edit mode. | Should |

## FR-MODE · Block editing

| ID | Requirement | Priority |
|---|---|---|
| FR-MODE-01 | Navigate mode moves between and selects blocks; edit mode edits one block's text. Enter or `i` enters edit mode, Esc returns (D-1). | Must |
| FR-MODE-02 | Editing happens in place: the editor opens where the block is, keeping indentation, surrounding blocks and scroll position, as in Things 3. | Must |
| FR-MODE-03 | Edit mode uses standard text-editing keys (arrows, Home/End, word motion, word delete, selection, clipboard), not vim normal mode (D-1). | Must |
| FR-MODE-04 | Creating a block (new sibling below/above, new child) opens it directly in edit mode. | Must |

## FR-KEY · Keyboard

| ID | Requirement | Priority |
|---|---|---|
| FR-KEY-01 | Every feature is reachable from the keyboard alone. | Must |
| FR-KEY-02 | Bindings are conventional and vi-inspired in navigate mode (`j/k`, `h/l`, `gg/G`, …); arrow keys work everywhere too. | Must |
| FR-KEY-03 | Modifier bindings use keys that work on macOS, Linux and Windows terminals (`Ctrl`, `Shift`, `Alt`); no binding requires `Cmd`/`Super`. | Must |
| FR-KEY-04 | `?` toggles a keyboard-shortcuts panel on the left listing the most useful bindings for the current mode, like Workflowy and Dynalist. While open it takes the sidebar's place (D-12). | Must |
| FR-KEY-05 | Mouse support: click to select a block or place the cursor, click fold markers, breadcrumbs, sidebar entries and pane headers, Alt+click a block, link or breadcrumb to open it in a new pane, wheel to scroll. Mouse support is secondary: no feature depends on it (D-16). | Must |
| FR-KEY-06 | A context menu on blocks, the selection and other UI elements (sidebar entries, breadcrumbs, pane headers) lists every command available for that element, grouped logically, with each command's key binding. It opens from a key on the focused element or by right-click, is navigable with the keyboard, and runs the chosen command. | Must |

## FR-NAV · Navigation

| ID | Requirement | Priority | Workflowy |
|---|---|---|---|
| FR-NAV-01 | Move to previous/next visible block, parent, first/last child, previous/next sibling, top/bottom. | Must | Focus commands |
| FR-NAV-02 | Expand, collapse and toggle a block; expand/collapse a whole subtree; expand/collapse all. | Must | Expand/collapse |
| FR-NAV-03 | Zoom into a block so it becomes the pane's root; zoom out; go home. Breadcrumbs show the path. | Must | Zoom, breadcrumbs |
| FR-NAV-04 | Back/forward through zoom history, per pane. | Must | Navigate back/forward |
| FR-NAV-05 | Go to Symbol: a fuzzy finder over every block, with bookmarks, projects and sections ranked first, like VS Code's Go to Symbol and Workflowy's JumpTo. | Must | JumpTo |
| FR-NAV-06 | Go to Command: a command palette that fuzzy-searches every command and shows its binding, like VS Code. | Must | Command palette |

## FR-OPS · Selection and structural operations

| ID | Requirement | Priority | Workflowy |
|---|---|---|---|
| FR-OPS-01 | Select several blocks by extending the selection (`Shift`+movement) or toggling blocks one by one. | Must | Multi-select |
| FR-OPS-02 | Indent and outdent the selection with its subtrees. | Must | Tab / Shift+Tab |
| FR-OPS-03 | Move the selection up and down among siblings, crossing into the adjacent parent at the edges. | Must | Move up/down |
| FR-OPS-04 | Group the selection under a new parent block. | Must | — (TaskPaper) |
| FR-OPS-05 | Move the selection to another block chosen in a fuzzy picker. | Must | Move to |
| FR-OPS-06 | Delete, duplicate, copy, cut and paste the selection with hierarchy, as TaskPaper text; multi-line pasted text becomes blocks. | Must | Delete, duplicate, paste |
| FR-OPS-07 | Complete/uncomplete, set color, set dates, toggle `@today` and bookmark the whole selection at once. | Must | Multi-select menu |
| FR-OPS-08 | Undo and redo every change, structural ones included. | Must | Undo/redo |
| FR-OPS-09 | Count blocks and words under a block. | Must | Count |
| FR-OPS-10 | Internal links between blocks, with backlinks (D-14). | Later | Internal links |

## FR-DONE · Completed items

| ID | Requirement | Priority | Workflowy |
|---|---|---|---|
| FR-DONE-01 | A command completes or uncompletes the selection by adding `@done(YYYY-MM-DD)` with today's date, or removing `@done` with any value (D-4). Any `@done`, with or without a value, marks a block completed (D-17). | Must | Complete |
| FR-DONE-02 | A top-level command and key binding show or hide completed blocks, per pane (D-13). | Must | Show completed |

## FR-SRCH · Search

| ID | Requirement | Priority | Workflowy |
|---|---|---|---|
| FR-SRCH-01 | Live search filters the pane as you type, keeping matches' ancestors visible; results stay editable. Search is scoped to the pane's zoom (D-14). | Later | Search |
| FR-SRCH-02 | Query syntax follows Workflowy: words, `"exact"`, `-exclude`, `OR`, `A > B`, plus operators for this app's metadata (`is:done`, `is:today`, `is:task`, `due:`, `start:`, color). | Later | Search operators |
| FR-SRCH-03 | The current search can be bookmarked and re-run from the sidebar (D-11). | Later | Starred searches |

## FR-UI · Layout and appearance

| ID | Requirement | Priority | Workflowy |
|---|---|---|---|
| FR-UI-01 | A primary sidebar on the left lists bookmarks at the top, then the built-in Today view (D-7), then all projects as a tree, like TaskPaper. Choosing an entry zooms the focused pane to it. A key toggles the sidebar. | Must | Sidebar, stars |
| FR-UI-02 | Split views: any number of panes side by side as columns, each with its own zoom, breadcrumbs, history and show-completed setting, and its own search once search exists; expand/collapse state is shared; edits appear in every pane at once (D-13). | Must | Panes |
| FR-UI-03 | Open the current block (by key, command or Alt+click), or the focused pane's location, in a new pane next to the active one; focus pane N directly by number; close a pane; resize panes. When panes do not fit, the view scrolls horizontally. | Must | Panes |
| FR-UI-04 | The pane layout is restored on the next launch (D-13). | Must | Panes |
| FR-UI-05 | A setting and key binding turn word wrap off; every block then occupies one line and overflowing text ends in `…`. | Must | — |
| FR-UI-06 | Built-in themes, switchable from the command palette with a live preview, like VS Code's theme picker. | Must | Themes |
| FR-UI-07 | The UI is clean, modern and sleek everywhere, and responds instantly. | Must | — |

## Out of scope

Owner decisions:

- Calendar and daily notes, for now.
- Tables, boards and other non-list layouts.
- `Archive:` handling, for now ([FR-DOC-04](#fr-doc--document-and-file)).
- Several files in one instance ([FR-DOC-01](#fr-doc--document-and-file)).
- Mirrors, sorting children, templates, heading/quote/numbered/paragraph block types, italic, underline, strikethrough and highlight, export and import, trash (D-14).

Cloud and GUI-only Workflowy features have no meaning here: sharing, comments, mentions, sync, uploads, embeds, AI, email and calendar integrations.

## Decisions

The owner's answers of 2026-10-04 to the clarifying questions.

| # | Decision |
|---|---|
| D-1 | Navigate and edit modes; text editing uses standard keys, not vim normal mode. |
| D-2 | Consecutive sibling note lines, blank lines included, form one block; children of the last line belong to the block. |
| D-3 | All tags are hidden; unsupported ones are preserved and shown in a details popup. |
| D-4 | Completion writes `@done(YYYY-MM-DD)`. |
| D-5 | A color tag tints the text. |
| D-6 | Dates have no time part. |
| D-7 | The sidebar has a built-in Today view. |
| D-8 | A section groups its indented children, not the following lines. |
| D-9 | `@code` on a note block's first line makes the whole block code. |
| D-10 | URL blocks are `Title @url(…)`; bare URLs in text are also openable. |
| D-11 | Bookmarks are a hidden `@bookmark` tag on any block; searches can be bookmarked once search exists. |
| D-12 | The `?` panel temporarily replaces the sidebar. |
| D-13 | Panes copy Workflowy: columns only, per-pane zoom, breadcrumbs, search, history and show-completed, shared collapse state, layout restored on launch. |
| D-14 | Zoom and breadcrumbs are the primary features. Undo/redo, duplicate, copy/paste with hierarchy and count are Must. Search comes later. Internal links are recorded for later. Sort and mirrors are dropped, as are the other unrequested Workflowy features. |
| D-15 | Running without a path creates a file in `/tmp` that can later be saved elsewhere. |
| D-16 | Mouse support is Must but secondary to the keyboard. |
| D-17 | An existing `@done` without a date counts as completed; only newly completed blocks get the date. |
| D-18 | Inside a code block, `---`, `@section` and `@url` lines stay code instead of splitting the block. |
