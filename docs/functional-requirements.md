# Functional Requirements

- Status: Accepted
- Last updated: 2026-10-05

What taskpaper-terminal does, as stated by the owner and mapped against Workflowy's feature set as of 2026-10-04. The owner's answers to the clarifying questions are recorded under [Decisions](#decisions) and referenced as **(D-n)**.

Priorities: **Must** is required for v1, **Should** is expected soon after, **Later** is planned after v1 and must not be precluded by the design, **Won't** is out of scope.

## Product summary

taskpaper-terminal is a keyboard-driven, Workflowy-style infinite outliner for the terminal that edits one `.taskpaper` file. It is a **block editor**: moving between and selecting blocks is separate from editing the text of one block, as in Things 3. Tags are metadata the app interprets and hides, not text the user reads.

## Terms

- **Line**: one line of the `.taskpaper` file. TaskPaper classifies it as a project, task or note.
- **Block**: the unit the user navigates, selects and edits: one line of the file (D-43).
- **Text node**: a block that is none of the other types, called a note in TaskPaper ([FR-BLK-03](#fr-blk--blocks)).
- **Navigate mode**: keys act on blocks. **Edit mode**: keys edit the text of one block in place.
- **Pane**: one column of the split view, showing its own zoomed part of the same document.

## FR-DOC · Document and file

| ID | Requirement | Priority |
|---|---|---|
| FR-DOC-01 | One app instance edits exactly one `.taskpaper` file. | Must |
| FR-DOC-02 | The file on disk is the source of truth; every change is saved automatically (D-15). | Must |
| FR-DOC-03 | Text the app does not interpret, including unsupported tags, is preserved on save. | Must |
| FR-DOC-04 | `Archive:` projects get no special treatment; there is no archive command. They are ordinary projects. | Must |
| FR-DOC-05 | Running without a path creates a new file in `/tmp` (D-15), filled with demo data: a few toy projects that show every feature the app implements, so it can be tried without preparing a file (D-27). | Must |
| FR-DOC-06 | A Save As command writes the document to a chosen path and continues editing that file; this is how a `/tmp` document is kept. | Must |
| FR-DOC-07 | Every file the app opens is backed up once a day, before the app first changes it that day, to a backups folder outside the file's own directory (`~/.local/share/taskpaper-terminal/backups/`, `$XDG_DATA_HOME` respected, also on macOS). Each file keeps its own dated copies, so files with the same name never collide; the newest 30 are kept. The folder, the number kept and whether backups run at all are settings (D-22). | Must |

## FR-BLK · Blocks

Each block type, how it is stored and how it looks. Every line of the file is one block, and types are mutually exclusive per block (D-43).

| ID | Type | Stored as | Shown as | Priority |
|---|---|---|---|---|
| FR-BLK-01 | Project | `Name:` | Bold heading with a fold marker. | Must |
| FR-BLK-02 | Task | `- Text` | Checkbox and text; done tasks dimmed and struck through. | Must |
| FR-BLK-03 | Text | Any other line, blank lines included (D-43) | Plain text, slightly dimmed. Consecutive text lines are separate blocks; a text block of several lines is Later (D-43). | Must |
| FR-BLK-04 | Section | `Name @section` | Things 3 style header with a rule; groups its indented children (D-8). | Must |
| FR-BLK-05 | Code | Dropped for v1: a line carrying `@code` is a text block, its tag hidden and kept (D-43). Code blocks may return after v1. | — | Later |
| FR-BLK-06 | URL | `Title @url(https://…)` (D-10) | Title styled as a link; a command opens it in the system browser. | Must |
| FR-BLK-07 | Divider | Text line `---` | Horizontal rule across the pane; selectable and movable like any block, no children. | Must |

**FR-BLK-08 (Must):** a command converts the selected blocks between project, task, text, section, URL and divider. Because tags are never typed ([FR-TAG-01](#fr-tag--tags-as-metadata)), this, the type menu of edit mode (D-44) and the typed shortcuts of [FR-MODE-05](#fr-mode--block-editing) are how the tag-based types are created.

## FR-TAG · Tags as metadata

| ID | Requirement | Priority |
|---|---|---|
| FR-TAG-01 | Tags are hidden from the displayed and edited text. The app sets and clears them through commands, never by typing `@`. All tags, supported or not, are hidden; unsupported ones are listed in an item details popup (D-3). | Must |
| FR-TAG-02 | Supported tags: `@start(DATE)`, `@due(DATE)`, `@today`, the colors `@red @orange @yellow @green @blue @violet`, plus the block-type tags `@section @url` and the internal `@done(YYYY-MM-DD)` (D-4) and `@bookmark` (D-11). | Must |
| FR-TAG-03 | Dates are stored as `YYYY-MM-DD` and entered through a prompt that accepts natural language such as "tomorrow" or "fri" (D-6). They show as a compact chip; overdue `@due` is highlighted. | Must |
| FR-TAG-04 | `@today` is toggled by a command and shown as a marker. A built-in Today view lists `@today` items and items due or started on or before today (D-7). | Must |
| FR-TAG-05 | A color tag tints the block's text; a block has at most one color, and setting one replaces the other (D-5). | Must |
| FR-TAG-06 | Keys set the start or due date to today or tomorrow, move either date a day back or forward, and cycle the color tag, without a prompt (D-26). | Must |

## FR-FMT · Inline formatting

| ID | Requirement | Priority |
|---|---|---|
| FR-FMT-01 | Task and text block text supports Markdown `**bold**` and `` `code` ``. Markers are hidden in navigate mode and visible in edit mode. | Must |
| FR-FMT-02 | Commands toggle bold and inline code on the selected text in edit mode. | Should |
| FR-FMT-03 | Text blocks also support Markdown headings (`# ` to `###### `) and links (`[title](url)` and `<url>`). Navigate mode renders them with the markers hidden; edit mode shows the raw Markdown ([editing.md §2.2](specs/editing.md#22-markdown-in-text)) (D-25). Fenced code, which spans lines, is Later (D-43). | Must |

## FR-MODE · Block editing

| ID | Requirement | Priority |
|---|---|---|
| FR-MODE-01 | Navigate mode moves between and selects blocks; edit mode edits one block's text. Enter enters edit mode, Esc returns (D-1, D-41). | Must |
| FR-MODE-02 | Editing happens in place: the editor opens where the block is, keeping indentation, surrounding blocks and scroll position, as in Things 3. | Must |
| FR-MODE-03 | Edit mode uses standard text-editing keys (arrows, Home/End, word motion, word delete, selection, clipboard), not vim normal mode (D-1). | Must |
| FR-MODE-04 | Creating a block (new sibling below/above, new child) opens it directly in edit mode. | Must |
| FR-MODE-05 | Typing a Markdown-style prefix followed by a space at the start of a text block or task turns it into another block type: `--- ` a divider, `[] ` or `[ ] ` a task, `[x] ` a completed task. `- ` and a trailing `:` already make a task and a project, as in TaskPaper. The very next `Backspace` or `Ctrl+Z` reverts the conversion, and undo reverts a divider ([editing.md §2.1](specs/editing.md#21-typed-shortcuts)) (D-24). | Must |
| FR-MODE-06 | Single keys create a project, section or task at the top of the current list or below the focused block, opened in edit mode (D-26). | Must |

## FR-KEY · Keyboard

| ID | Requirement | Priority |
|---|---|---|
| FR-KEY-01 | Every feature is reachable from the keyboard alone. | Must |
| FR-KEY-02 | Bindings are conventional and vi-inspired in navigate mode (`j/k`, `h/l`, `gg/G`, …); arrow keys work everywhere too. | Must |
| FR-KEY-03 | Modifier bindings use keys that work on macOS, Linux and Windows terminals (`Ctrl`, `Shift`, `Alt`); no binding requires `Cmd`/`Super`. | Must |
| FR-KEY-04 | `?` toggles a keyboard-shortcuts panel listing the most useful bindings for the current mode, like Workflowy and Dynalist. It opens in the secondary sidebar on the right, which it shares with the comments panel (D-28). | Must |
| FR-KEY-05 | Mouse support: click to select a block or place the cursor, click fold markers, breadcrumbs, sidebar entries and pane headers, Alt+click a block, link or breadcrumb to open it in a new pane, wheel to scroll. Mouse support is secondary: no feature depends on it (D-16). | Must |
| FR-KEY-06 | A context menu on blocks, the selection and other UI elements (sidebar entries, breadcrumbs, pane headers) lists every command available for that element, grouped logically, with each command's key binding. It opens from a key on the focused element or by right-click, is navigable with the keyboard, and runs the chosen command. | Must |

## FR-NAV · Navigation

| ID | Requirement | Priority | Workflowy |
|---|---|---|---|
| FR-NAV-01 | Move to previous/next visible block, parent, first/last child, previous/next sibling, top/bottom. | Must | Focus commands |
| FR-NAV-02 | Expand, collapse and toggle a block; expand/collapse a whole subtree; expand/collapse all, or all by one level, like TaskPaper. Fold state is kept in memory only (D-26). | Must | Expand/collapse |
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
| FR-OPS-10 | Internal links between blocks, with backlinks (D-14). A linked block carries a hidden `@uid(ID)` tag and links are written `[[ID\|Title]]`; the design is in [specs/links.md](specs/links.md). | Later | Internal links |

## FR-DONE · Completed items

| ID | Requirement | Priority | Workflowy |
|---|---|---|---|
| FR-DONE-01 | A command completes or uncompletes the selection by adding `@done(YYYY-MM-DD)` with today's date, or removing `@done` with any value (D-4). Any `@done`, with or without a value, marks a block completed (D-17). Any block can be completed, whatever its type; a completed block is shown dimmed and struck through, and every block inside it, at any depth, dimmed (D-40). | Must | Complete |
| FR-DONE-02 | A top-level command and key binding show or hide completed blocks, each with everything inside it, per pane (D-13, D-40). | Must | Show completed |

## FR-SPC · Special nodes and comments

| ID | Requirement | Priority | Workflowy |
|---|---|---|---|
| FR-SPC-01 | A line tagged `@hidden`, with its subtree, is a special node: a feature stores its data there, marked by a feature tag such as `@comments`. The app never shows it as a block, keeps it as the last child of the block that owns it, carries it along when that block is moved, copied or deleted, and preserves special nodes of unknown features unchanged ([spec](specs/special-nodes.md)) (D-23). | Must | — |
| FR-SPC-02 | Any block except a divider can have comments, like Todoist. Each comment is stored in the block's `@hidden @comments` node with the local date and time it was added and may span several lines. A block with comments shows a count badge; a key opens a comments panel to read, add, edit and delete them (D-23). | Must | Comments |

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
| FR-UI-08 | Twelve themes are built into the app, available without any configuration file: Catppuccin Mocha, Catppuccin Latte, Tokyo Night, Dracula, Nord, Gruvbox Dark, Rosé Pine, Rosé Pine Dawn, Kanagawa Wave, Everforest Dark, One Dark and Solarized Light. Each colors every part of the UI, including the six color tags. Palettes, origins and licenses are in [Built-in themes](reference/themes.md) (D-19). | Must | Themes |
| FR-UI-09 | Nerd Font icons mark block types, fold state, chips, sidebar entries and the status line, so the UI reads at a glance (D-20). | Must | — |
| FR-UI-10 | One setting turns every icon off, for terminals without a Nerd Font. The UI then uses plain Unicode markers, or ASCII when the locale is not UTF-8, and nothing is lost or misaligned (D-20). | Must | — |
| FR-UI-11 | Short animations show where focus landed, what a command changed and when an action was refused. They never delay input or layout, and a setting reduces or turns them off ([specs/ui.md §9](specs/ui.md#9-animation-fr-ui-11)) (D-21). | Must | — |

## FR-SET · Settings

| ID | Requirement | Priority | Workflowy |
|---|---|---|---|
| FR-SET-01 | Settings are per user and stored in one TOML file, `~/.config/taskpaper-terminal/config.toml` (`$XDG_CONFIG_HOME` respected, also on macOS), that people can read and edit by hand. The app runs with defaults when the file is missing and creates it on the first change (D-20). | Must | Settings |
| FR-SET-02 | A settings window, opened by a key and from the command palette, lists every setting grouped by topic with its current value and a one-line description. Changing a value applies it at once with a live preview, saves it immediately, and can be reset to the default (D-20). | Must | Settings |
| FR-SET-03 | The settings window covers at least the theme (FR-UI-06), icons (FR-UI-10), word wrap (FR-UI-05), autosave delay and backups (FR-DOC-07); key overrides stay in the file ([keybindings spec](specs/keybindings.md#5-configuration)). | Must | Settings |
| FR-SET-04 | Writing a setting from the app keeps the file's comments, order and unrelated keys. Unknown keys and invalid values are reported once and ignored, and the app still starts. | Must | — |

## Out of scope

Owner decisions:

- Calendar and daily notes, for now.
- Tables, boards and other non-list layouts.
- `Archive:` handling, for now ([FR-DOC-04](#fr-doc--document-and-file)).
- Several files in one instance ([FR-DOC-01](#fr-doc--document-and-file)).
- Mirrors, sorting children, templates, heading/quote/numbered/paragraph block types, italic, underline, strikethrough and highlight, export and import, trash (D-14). Headings exist only as Markdown inside a text block (D-25), not as a block type.

Cloud and GUI-only Workflowy features have no meaning here: sharing, mentions, sync, uploads, embeds, AI, email and calendar integrations.

## Decisions

The owner's answers of 2026-10-04 to the clarifying questions.

| # | Decision |
|---|---|
| D-1 | Navigate and edit modes; text editing uses standard keys, not vim normal mode. |
| D-2 | Consecutive sibling note lines, blank lines included, form one block; children of the last line belong to the block. Superseded by D-43. |
| D-3 | All tags are hidden; unsupported ones are preserved and shown in a details popup. |
| D-4 | Completion writes `@done(YYYY-MM-DD)`. |
| D-5 | A color tag tints the text. |
| D-6 | Dates have no time part. |
| D-7 | The sidebar has a built-in Today view. |
| D-8 | A section groups its indented children, not the following lines. |
| D-9 | `@code` on a note block's first line makes the whole block code. Superseded by D-43. |
| D-10 | URL blocks are `Title @url(…)`; bare URLs in text are also openable. |
| D-11 | Bookmarks are a hidden `@bookmark` tag on any block; searches can be bookmarked once search exists. |
| D-12 | The `?` panel temporarily replaces the sidebar. Superseded by D-28. |
| D-13 | Panes copy Workflowy: columns only, per-pane zoom, breadcrumbs, search, history and show-completed, shared collapse state, layout restored on launch. |
| D-14 | Zoom and breadcrumbs are the primary features. Undo/redo, duplicate, copy/paste with hierarchy and count are Must. Search comes later. Internal links are recorded for later. Sort and mirrors are dropped, as are the other unrequested Workflowy features. |
| D-15 | Running without a path creates a file in `/tmp` that can later be saved elsewhere. |
| D-16 | Mouse support is Must but secondary to the keyboard. |
| D-17 | An existing `@done` without a date counts as completed; only newly completed blocks get the date. |
| D-18 | Inside a code block, `---`, `@section` and `@url` lines stay code instead of splitting the block. Superseded by D-43. |
| D-19 | The app ships 10-12 attractive terminal color themes built in (owner, 2026-10-04). The chosen twelve are permissively licensed and include light themes. |
| D-20 | The app uses Nerd Font icons, with a setting to turn all icons off. It has a settings window; settings are per user under `~/.config` in a modern format (owner, 2026-10-04). The format is TOML, as already decided in OQ-18: it is the norm for Rust and terminal tools (Cargo, Helix, Alacritty, starship) and avoids YAML's indentation and implicit-typing pitfalls. |
| D-21 | Animation is used only where it helps orientation or feedback and never slows the user down. The owner delegated where to animate (decided in [specs/ui.md §9](specs/ui.md#9-animation-fr-ui-11)). |
| D-22 | The app makes daily backups of every file it opens (owner, 2026-10-04). They go to the XDG data directory, `~/.local/share/taskpaper-terminal/backups/`, next to the `~/.config` settings, rather than the owner's suggested `~/.taskpaper-terminal/backups/`, which a setting can still select. A copy is taken on the first open or save of each local day, before anything is written; one folder per file, named after the file plus a hash of its full path; copies are named by date; identical copies are skipped; the newest 30 are kept. Both choices of an external-change conflict first save the version they discard as a conflict copy. Details in [file-io.md](specs/file-io.md#2-saving). |
| D-23 | Some features are stored as special nodes: hidden child lines tagged `@hidden` plus a feature tag, which other TaskPaper apps see as plain lines (owner, 2026-10-04). Comments are the first: Todoist-style dated comments stored under a `COMMENTS @hidden @comments` child. The owner accepted every proposed default: a count badge and a side panel, `[YYYY-MM-DD HH:MM]` local 24-hour timestamps, multi-line comments as deeper-indented lines, bold, code and URLs in comments, `@hidden` as a general mechanism, nodes that travel with their block and stay its last child, hand-typed comment lines accepted, and comments as the only special node for now. Details in [special-nodes.md](specs/special-nodes.md). |
| D-24 | Typing `--- ` creates a divider and ```` ``` ```` plus a space a code block; similar shortcuts apply where they make input simpler (owner, 2026-10-04). The owner delegated the rest of the set. Chosen: Workflowy's `[] ` and `[ ] ` task triggers plus `[x] ` for a completed task, while `- ` and a trailing `:` keep TaskPaper's own meaning. Shortcuts fire only on a typed space, never on paste, and can be reverted at once. `# ` stays a Markdown heading in a note (OQ-28). The code-block shortcut was dropped with code blocks (D-43). Details in [editing.md §2.1](specs/editing.md#21-typed-shortcuts). |
| D-25 | Note blocks support Markdown headings, URLs, code spans and code blocks, rendered in navigate mode and shown raw while editing (owner, 2026-10-04). The owner delegated the details: headings stay note text, so the file stays plain TaskPaper and D-14's "no heading block type" holds; links are `[title](url)`, `<url>` and bare URLs; fenced code renders like a code block but stays part of the note, without syntax highlighting. Fenced code and code blocks are Later since D-43. Details in [editing.md §2.2](specs/editing.md#22-markdown-in-text). |
| D-26 | The owner's basic key bindings (2026-10-04): `P`/`p`, `S`/`s`, `A`/`a` add a project, section or task at the top of the list or below the current block; `O`/`o` insert above/below; `'` and `"` toggle a block's fold and its whole subtree's; expand and collapse by level exist, as in TaskPaper; fold state lives in memory and is not persisted; `c` cycles the color; `t`/`y` set the start date to today/tomorrow and `T`/`Y` the due date; `[`/`]` and `{`/`}` move the start and due date by a day; `/` searches; `.` and `Shift+.` zoom in and out; `\` opens the context menu; `Ctrl+'` goes home; `z`/`Shift+Z` undo and redo; `Ctrl+K` opens the command palette and `Ctrl+P` Go to Symbol. Where these displaced earlier bindings, see OQ-29; details in [keybindings.md](specs/keybindings.md#2-navigate-mode). |
| D-27 | Running without a path fills the new `/tmp` file with demo data, a few toy projects sufficient to demonstrate every feature implemented, to make testing easy (owner, 2026-10-04). The demo grows with each feature ([implementation plan](implementation-plan.md#rules-for-every-step)); details in [file-io.md §1](specs/file-io.md#1-opening). |
| D-28 | The navigation sidebar (bookmarks, Today, projects) stays on the left. A secondary sidebar on the right shows either the keyboard shortcuts or the comments of the focused block; nodes are still edited in place (owner, 2026-10-04). Supersedes D-12. |
| D-29 | Colors tell block types apart (owner, 2026-10-04, M1 review): projects are bold in the theme's accent (`$primary`) and keep their trailing `:`; sections use the headers color (`$secondary`) without a rule; notes and every other non-task, non-project block are slightly dimmed; dividers are very dim. No fold marker on expanded blocks; only a collapsed block with children shows `▸ n`. A dim horizontal rule separates the breadcrumb header from the outline, as in Workflowy. Details in [ui.md §1–2](specs/ui.md#2-blocks). |
| D-30 | The focused block is not shown inverted; a current-line marker in a gutter left of the outline shows it instead, like an editor (owner, 2026-10-04, M1 review). Details in [ui.md §2](specs/ui.md#2-blocks). |
| D-31 | On-screen indentation is exactly the block's depth in the hierarchy, 2 columns per level, the same for every block type (owner, 2026-10-05, M1 review). Any marker or icon (open task `-`, done task `✔` (Heavy Check Mark, U+2714), URL `↗`, later Nerd Font icons) goes after the indentation and never changes it. When zoomed, the title row is at depth 0 and the zoom block's children start at depth 1. A task's note is its child, so it sits one level in and lines up with the task's text. Details in [ui.md §2](specs/ui.md#2-blocks). |
| D-32 | The breadcrumbs start with a house icon instead of the word `Home` (owner, 2026-10-05, M1 review). Details in [ui.md §1](specs/ui.md#1-layout). |
| D-33 | Dividers are dimmer still than in D-29: the rule-lines color with the dim attribute, fainter than the header rule (owner, 2026-10-05, M1 review). Details in [ui.md §2](specs/ui.md#2-blocks). |
| D-34 | When zoomed, the title row shows the zoom block styled by its type, in bold (owner, 2026-10-05, M1 review). A project is colored and ends in `:`, a section is colored, a task has its open or done marker, and a note is dimmed. Details in [ui.md §1](specs/ui.md#1-layout). |
| D-35 | The breadcrumb header starts after the focus gutter, so the root crumb is in the same column as top-level blocks (owner, 2026-10-05, M1 review). The rule under it and the status line stay full width. Details in [ui.md §1](specs/ui.md#1-layout). |
| D-36 | A done task's `✔` is dimmed like its text, so the whole row recedes (owner, 2026-10-05, M1 review). Details in [ui.md §2](specs/ui.md#2-blocks). |
| D-37 | Notes are less dim than done tasks (owner, 2026-10-05, M1 review). Until themes arrive, notes use the ANSI gray, and done tasks keep the terminal's dim attribute. Details in [ui.md §2](specs/ui.md#2-blocks). |
| D-38 | A URL block has no marker; its underlined, accent-colored title is enough, and it lines up with other blocks at its depth (owner, 2026-10-05, M2 review). Supersedes the URL `↗` of D-31. Details in [ui.md §2](specs/ui.md#2-blocks). |
| D-39 | Every line of the focused block gets a slightly brighter background across the pane, in navigate and edit mode, without changing any text color or style; the gutter marker of D-30 stays (owner, 2026-10-05, M2 review). Until themes arrive the band is the 256-color grey 235, chosen for dark terminals. Details in [ui.md §2](specs/ui.md#2-blocks). |
| D-40 | Any block can be completed, not only tasks: projects, sections, notes and every other type take `@done` the same way (owner, 2026-10-05, M2 feedback). A done block is dimmed and struck through in its type's style, a task also showing `✔`; every block inside it, at any depth, is dimmed too, without being marked done or changed in the file. This holds when zoomed into or below a done block, for the title row too. Hiding completed blocks hides each done block with everything inside it. Details in [ui.md §2](specs/ui.md#2-blocks). |
| D-41 | `o`/`O` insert neighbors and `i`/`I` insert children, leaving only `Enter` for editing (owner, 2026-10-05, M2 feedback). In navigate mode `i` adds a last child and `I` a first child, each expanding the block and opening the new block in edit mode; on the zoom title they add its last and first child. `i` no longer edits with the caret at the start, and `Alt+o`, the earlier new-child key, is free. Changes D-26's key set and OQ-29's `i`. Details in [keybindings.md §2](specs/keybindings.md#2-navigate-mode) and [editing.md §3](specs/editing.md#3-creating-blocks-fr-mode-04). |
| D-42 | Every key binding has two variants: one that works in a classic terminal with legacy key reporting, and one for the kitty keyboard protocol (owner, 2026-10-05). A key both deliver the same way, such as `j`, `Enter` or `Ctrl+D`, counts as both; a key only the kitty protocol reports, such as `Ctrl+Enter` or `Shift+Enter`, always has a classic variant bound beside it, which is not an `Alt` key alone. The app asks for the kitty protocol on start. Details in [keybindings.md §1](specs/keybindings.md#1-terminal-constraints). |
| D-43 | Back to a model close to TaskPaper: every line is one block, and a block is a project, section, task, divider, URL or text node (owner, 2026-10-05, M2 feedback). "Text node" replaces "note" in the app and the docs; TaskPaper's own term stays where its format is meant. Consecutive text lines, blank lines included, are separate blocks, so nothing merges and no edit splits a block. Code is dropped for now: a line carrying `@code` is text, its tag hidden and kept, with no special styling. A URL block stays, as one line (owner: "no harm in that"). Edit mode edits one line: no key breaks a line, `Enter` ends editing and adds the next block, and, as in a traditional editor, `↑`/`↓` end the session and edit the previous or next visible block that is not a divider, at the same column where possible, each session one undo step. Supersedes D-2, D-9 and D-18 and the code parts of D-24 and D-25; code blocks, fenced code and text blocks of several lines may return after v1. Details in [data-model.md §3](specs/data-model.md#3-blocks) and [editing.md §2](specs/editing.md#2-edit-mode). |
| D-44 | In edit mode, typing `/` opens a menu at the caret that changes the block's type (owner, 2026-10-05, M2 feedback). It is titled "Node type:" and lists Project, Section, Note, Task and Divider in that order; "Note" is the owner's label for a text node. `p`, `s`, `n` and `t` typed right after `/` choose at once; any other text filters the list by name, so `/div` then `Enter` makes a divider. `↑`/`↓` or `Ctrl+P`/`Ctrl+N` move the highlight and `Enter` chooses it; `Esc`, or a filter that matches nothing, closes the menu and types `/` and the filter as text, and `Backspace` on an empty filter closes it. The change is part of the edit session's single undo step and follows `convert_type`: the old type's syntax and `@section`/`@url` go, other tags stay. A divider replaces the text with `---`, keeping the other tags, without confirmation, since undo restores it, and ends editing. URL is not in the menu: a URL block needs an address, which typing cannot give (OQ-25). Details in [editing.md §2.3](specs/editing.md#23-type-menu). |
