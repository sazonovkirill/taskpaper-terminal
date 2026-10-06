# Implementation plan

- Status: Draft
- Last updated: 2026-10-06

The order in which v1 is built. The app starts almost trivial and gains one feature per step; after every step it starts, shows the file and does everything earlier steps added, with no half-finished feature reachable by the user. Each step is sized to be one Todoist task for one agent and lists what it covers, what it needs first and what the owner can try when it lands. What each feature does is in the [functional requirements](functional-requirements.md) and the [specs](specs/README.md); this file only orders the work.

## Milestones and acceptance

Development goes one milestone at a time (owner, 2026-10-04). Each milestone is one Todoist task labeled `milestone`, with its steps listed in the description; agents work its steps, in parallel wherever the dependencies allow ([working in parallel](quality/agent-workflow.md#working-in-parallel)), committing each through the review gate and recording progress in the task's comments. When every step is on `main`, the owner runs the Codex [milestone review](quality/agent-workflow.md#milestone-review) on their Mac and its verified findings are fixed; then the task moves to Done and work stops: the owner runs the app, reviews it and gives feedback. Closing the task accepts the milestone. Each milestone task names the previous one as a dependency, so it is not runnable until that one is closed ([agent workflow](quality/agent-workflow.md#backlog-and-task-selection)). Feedback that changes later steps is recorded here before the next milestone starts.

## Rules for every step

- A step ships its core logic, its key bindings, its entries in the `?` panel, palette and context menu (once those exist), its tests and any doc updates together, through the [agent workflow](quality/agent-workflow.md).
- A key does nothing until the step that implements its command lands. No step adds a key that only reports "not implemented".
- A step that adds a user-visible feature also adds content to the demo document ([file-io.md §1](specs/file-io.md#1-opening)) that shows it, so the owner can try every step by running `tpt` without a path. The demo's test lists the block types and tags it must contain; a step adds to that list.
- Dependencies are added in the step that first uses them ([architecture.md §3](specs/architecture.md#3-dependencies)).
- **After** names the steps that must be on `main` first. Steps whose dependencies are all on `main` can run in parallel. Steps marked **hub** change `app::update`, `ui::keymap` and the main widgets heavily; run at most one hub step at a time and pair it with non-hub steps.
- Splitting a step that turns out too large is the implementer's call; merging steps or reordering them is the owner's.

## M1 · Read-only outline viewer

The app opens a real file and shows it as an outline. Nothing is written to disk yet, so it is safe to point at real files.

| # | Step | Covers | After | Try it |
|---|---|---|---|---|
| 1.1 | **Open and show a file.** `tpt <path>` reads it, refuses non-UTF-8 and unreadable files on stderr, parses indentation, hierarchy and item types, and draws one row per line with 2-column indents, bold projects and `☐` tasks. Without a path it shows the demo document from memory, still without date placeholders (the `/tmp` file arrives in 2.10, dates in 3.2). `Ctrl+Q` quits. Sets up the three layers, `update(state, message) -> effects` and the `TestBackend` test harness. | F-FILE-01 (read), F-FILE-02, F-FILE-07, FR-DOC-01, FR-DOC-03 (nothing dropped on read); file-format §1–4 without tags | — | `cargo run` shows the demo outline |
| 1.2 | **Focus and scrolling. hub** A focused row; `j`/`k`, arrows, `gg`/`G`, `Home`/`End`, page up/down; scrolling keeps focus visible. Status line with mode and path. Multi-key sequences with timeout and the pending prefix shown. | FR-NAV-01 (part), keybindings §1 | 1.1 | Move through a long file |
| 1.3 | **Tags hidden.** Tag grammar, derived tags and display text; tags vanish from rows; `@done` tasks show `☑`, dimmed and struck through. | FR-TAG-01 (display), FR-TAG-02, FR-DONE-01 (display), file-format §5 | 1.1 | Tagged lines look clean |
| 1.4 | **Blocks.** The block layer: note runs as one multi-line block, special nodes skipped, divider, section, URL and code blocks drawn as in ui §2. Focus moves by block. Since D-43 every line is one block and there are no code blocks. | FR-BLK-01–07, FR-SPC-01 (skipping), data-model §3 | 1.2, 1.3 | Notes, sections and dividers look right |
| 1.5 | **Folding. hub** In-memory shared folds; `h`/`l`, `'`, `Space`, `"`, `=`/`-`, `+`/`_`; fold markers and `▸ n`; focus repair when the focused block is hidden. Parent and sibling motion. | FR-NAV-01, FR-NAV-02, navigation §2–3 | 1.4 | Collapse a project |
| 1.6 | **Zoom and breadcrumbs. hub** `.`, `>`, `Alt+l`/`Alt+h`, `gz`/`gu`, `gh` and `Ctrl+'` (which arrives only under the kitty protocol, D-42), the title row, the breadcrumb header, back and forward with `H`/`L`. | FR-NAV-03, FR-NAV-04, navigation §4 | 1.5 | Zoom into a project and back |

## M2 · Editing that saves

From here the app changes files, so file safety comes first: the serializer, atomic saves and backups land before the first command that edits.

| # | Step | Covers | After | Try it |
|---|---|---|---|---|
| 2.1 | **Serializer.** Byte-identical round trip of unedited files; preserved indent unit, line endings, BOM and trailing newline; golden corpus and property tests. No visible change. | F-FILE-03, F-FILE-08, N-REL-02, file-format §6–7 | 1.1 | (tests only) |
| 2.2 | **Transactions and undo engine.** `Op` and `Transaction` with apply and invert in the Document layer, the undo history in the App layer, placement repair after structural ops. No visible change. | FR-OPS-08 (engine), data-model §4–5, file-format §3.1 rule 1 | 1.4 | (tests only) |
| 2.3 | **Atomic autosave and backups.** Save effect on a worker thread; atomic write keeping permissions and symlinks; 1 s debounce driven by clock ticks; save on quit and on `SIGTERM`/`SIGHUP`; `●` and the error badge; daily backups with retention; read-only files open read-only. | FR-DOC-02, FR-DOC-07, F-FILE-04, F-FILE-05, F-FILE-09, N-UX-01, file-io §2 | 2.1 | A dated copy appears in `~/.local/share/taskpaper-terminal/backups/` |
| 2.4 | **First edit: complete tasks. hub** `x` toggles `@done(date)` on the focused block, saved, and undone and redone with `z`/`Z`, `Ctrl+Z`/`Ctrl+Y`. Today's date comes from the injected clock. | FR-DONE-01, FR-OPS-08, N-UX-02 | 2.2, 2.3, 2.10 | Check off a task, see the file change, undo it |
| 2.5 | **Edit mode for one-line blocks. hub** `Enter`/`i`, `Esc`, standard text keys by grapheme, tags kept and appended (OQ-24), the `@` refusal (OQ-25), live type change (OQ-27), one undo step per session, rules 2 and 4 repaired on leaving. | FR-MODE-01–03, editing §1–2 | 2.4 | Rename a task in place |
| 2.6 | **Creating and deleting blocks. hub** `o`/`O`, `Alt+o`, `Enter` creating the next block, `Backspace` in an empty block, `dd`/`Delete`; new block types per OQ-03 and OQ-26; autosave waits on transient shapes. | FR-MODE-04, FR-OPS-06 (delete), editing §3–4 | 2.5 | Write a new list from scratch |
| 2.7 | **Typed creation.** `P`/`p`, `S`/`s`, `A`/`a` (`S`/`s` make sections since D-52; two-key sequences such as `os` since D-53). | FR-MODE-06, editing §3 | 2.6 | Add a section with `S` |
| 2.8 | ~~**Multi-line notes and code.**~~ Reverted by M2 owner feedback (D-43): every block is one line, there are no code blocks, and `↑`/`↓` in edit mode move to the block above or below. | — | 2.6 | Write three text lines with `Enter` |
| 2.9 | **Indent, outdent and move.** `Tab`/`Shift+Tab` in both modes, `Alt+j`/`Alt+k`, crossing into the neighboring parent. | FR-OPS-02, FR-OPS-03 | 2.6 | Reorder a list |
| 2.10 | **Demo file in `/tmp`.** `tpt` without a path writes the demo document with resolved dates to a new temporary file and opens it, marked in the status line. Save As follows in 4.3. | FR-DOC-05, D-27, file-io §1 | 2.3 | Start `tpt` with no path |
| 2.11 | **External changes.** File watching, silent reload, the conflict prompt with conflict copies, the disk check before every save, deleted files. | F-FILE-06, OQ-16, file-io §3 | 2.3 | Edit the file in another editor while the app is open |
| 2.12 | **Crash recovery.** A panic hook restores the terminal and writes a recovery file, offered on the next start. | N-REL-01, file-io §4 | 2.3 | (tests) |

**Milestone result:** a usable outliner with safe saving, backups and undo.

**Owner feedback on M2 (2026-10-05):** back to a model close to TaskPaper, one line per block with six types: project, section, task, divider, URL and text (D-43); code blocks, fenced code and multi-line text move to Later. In edit mode, `/` opens a type menu that makes the block a project, section, text node ("Note"), task or divider (D-44, [editing.md §2.3](specs/editing.md#23-command-menu); since D-50 the menu offers every command of the block).

## M3 · Metadata and formatting

| # | Step | Covers | After | Try it |
|---|---|---|---|---|
| 3.1 | **Date parser.** `document::dates` with every form in search §4, relative to an injected date. No visible change. | search §4 | 1.1 | (tests only) |
| 3.2 | **Quick dates. hub** `t`/`y`/`T`/`Y`, `[`/`]`/`{`/`}`, date chips, overdue `@due` highlighted, chips updated at midnight; the demo gains dates through resolved placeholders. | FR-TAG-03 (display), FR-TAG-06 | 2.4 | Make a task due tomorrow with `Y` |
| 3.3 | **Date prompt.** `ms`/`md` open a prompt with a live preview; empty input clears the date. | FR-TAG-03 | 3.1, 3.2 | Type "next fri" |
| 3.4 | **Today, colors and bookmarks.** `mt` and its marker chip; `c` cycling and the `mc` picker, text tint; `mb`. | FR-TAG-04 (tag), FR-TAG-05, FR-TAG-06, D-11 | 3.2 | Color a task red |
| 3.5 | **Selection. hub** `J`/`K`, `Shift`+arrows, `v`, `Ctrl+A`, `Esc`; every command so far applies to the selection; `gG` groups it. | FR-OPS-01, FR-OPS-04, FR-OPS-07 | 2.9, 3.4 | Complete five tasks at once |
| 3.6 | **Clipboard and duplicate.** `Ctrl+C`/`Ctrl+X`/`Ctrl+V`/`Alt+V` with hierarchy, `D`; `arboard` with the OSC 52 and bracketed-paste fallbacks; paste in edit mode with the `@` rule. | FR-OPS-06 | 3.5 | Copy a project into another file |
| 3.7 | **Type converter and details.** The `my` picker for all six types (five since D-45) and the `mi` popup with unsupported tags and unknown special nodes. The edit-mode type menu of D-44 (the command menu since D-50) already converts the edited block between five types; the picker can extend it with URL (dropped by D-45) and selections. | FR-BLK-08, D-3 | 3.4 | Turn a text block into a section |
| 3.8 | **Typed shortcuts.** Dropped by D-52 after delivery; `Backspace` and `Delete` take a task's dash and a project's `:` off instead. | FR-MODE-05, editing §2 | 3.7 | `Backspace` at the start of a task's text |
| 3.9 | **Inline formatting and URLs.** `**bold**` and `` `code` `` rendered, bare URLs underlined, URL blocks (dropped by D-45), `gx` opening the browser; `Ctrl+B` and `` Alt+` `` (with `Ctrl+E` as its non-`Alt` route) in edit mode (Should). Until the picker of 4.2, `gx` on a block with several URLs opens the first. | FR-FMT-01, FR-FMT-02, FR-BLK-06, D-10 | 2.5 | Bold text renders bold |
| 3.10 | **Markdown in text.** Headings and links rendered in navigate mode; `gx` also opens a Markdown link, the first with a URL. | FR-FMT-03, editing §2.2 | 3.9 | A text block with a `## heading` |

**Owner feedback on M3 (2026-10-05):** links need no block type (D-45). Five types remain: project, section, task, text and divider; URL leaves the `my` picker. A line carrying `@url(…)` keeps whatever type its line has, its tag hidden and preserved, and its title is drawn as a link. Every link, bare URLs, `<url>` and `[title](url)`, is drawn in the link color and underlined in every block but a divider, so Markdown links extend from text blocks to tasks, projects and sections; headings stay text-only. `gx` opens the focused block's link, `@url` first, also on a selection; it reaches the command palettes with the command registry of step 4.1. The demo shows a link in a task and in a text block and an `@url` line. Try it: `gx` on "The Pragmatic Programmer" in the demo's reading list.

**Owner feedback on M3 (2026-10-06):** date chips name the date relative to today, such as `tomorrow`, `next Tue` or `in 15 days`, after a calendar icon, start chips green and due chips red (D-46); this brings in the icon table of step 6.3 early, with only the icons drawn so far and their fallbacks unused until the `icons` setting. The `@today` star and a bookmark icon move right after the block's text, the star first (D-47). Try it: the demo's dates read `today`, `tomorrow`, a weekday, `next` and a weekday, `in 20 days` and `2 days ago`; `mb` and `mt` on any block show their icons after its text. Colors tint the row's background instead of the text: a very dim shade of the tag's color from the block's marker to the pane's right edge, a step brighter on the focused row, the gutter and indentation untouched (D-48). `c1` to `c6` set red to violet, `C` removes the color, cycling moves from `c` to `cc`, and the `mc` picker numbers its colors, a number choosing at once. The demo's weekend errands say so and hold a yellow block. Markdown formatting keeps the block's own color: bold is bold, inline code dimmed, a level-1 heading bold and underlined and the others bold, while links keep the link color (D-49). Sections are italic (D-49). The focus marker is a wider `▌`, in the primary color in navigate mode and the accent while editing (D-51). Try it: `c4` on a task, then `C`, and press `Enter` to see the marker take the accent color.

**Owner feedback on M3 (2026-10-06), slash menu:** in edit mode `/` offers every command of the block, not only types: dates, today, bookmark, colors, complete, details, link, duplicate and delete, filtered by the starts of the labels' words as typed, and `Ctrl+T` cycles the block's type (D-50, [editing.md §2.3–2.4](specs/editing.md#23-command-menu)). The demo's errands list points to it. Try it: `Enter` on a task, then `/due` and `Enter`.

**Owner feedback on M3 (2026-10-06), sections and dividers as text:** only TaskPaper's three block types remain, project, task and text (D-52). A section is a text block carrying `@section` and a divider a text block reading `---`; both look as before, and a divider is edited as its raw `---`. `S` makes the selected text blocks, else the focused one, sections and `s` plain text again, skipping tasks and projects with a notice; `@section` on a task or project is kept without effect and listed by `mi`. `my` and the command menu offer Make Section and Remove Section instead of the Section and Divider types, and `Ctrl+T` still cycles project, section, task and note. `S`/`s` no longer add sections. The typed shortcuts of step 3.8 are dropped: a divider is typed as `---`, and while editing `Backspace` at the start of a task's text or `Delete` at the end of a project's makes it text again. Try it: `S` on "Ask friends for more recommendations." in the demo's reading list, then `s`.

**Owner feedback on M3 (2026-10-06), date panel and slash menu:** the `ms`/`md` date prompt is a full-width panel at the bottom of the screen above the status line instead of a centered box, its border and title in the accent color, the rest of the screen dimmed while it is open; the outline scrolls only if the panel would cover the focused block (D-54). The slash menu is padded, each entry has an icon, and its border and title are in the accent color too. Try it: `md` on "Return Piranesi to the library" in the demo, then `Enter` and `/` on it.

**Owner feedback on M3 (2026-10-06), keys:** creating a block takes a place key and a type letter (D-53): `o`/`O` below/above, `i`/`I` last/first child, `a`/`A` end/start of the list, then `s`, `p`, `t` or `n` for a section, project, task or note; a place key alone, after the sequence timeout or before any other key, adds a block of the type that fits. `y` copies and `p`/`P` paste below/above, next to `Ctrl+C`/`Ctrl+V`/`Alt+V`; tomorrow moves from `y`/`Y` to `w`/`W`. Try it: `ip` on "Trip to Lisbon" in the demo, type a name, `Esc`, then `y` and `p`.

## M4 · Finding things

| # | Step | Covers | After | Try it |
|---|---|---|---|---|
| 4.1 | **Command registry. hub** Every command gets a name, a context and its bindings in one table that the keymap reads; the palette, the `?` panel and the context menu read it later. It absorbs the edit-mode command menu's list of node commands (`app::node_commands`, D-50). Refactor only, no visible change. | architecture §1, FR-KEY-06 (basis) | 2.6 | (no visible change) |
| 4.2 | **Fuzzy picker and Go to Symbol.** One picker overlay on `nucleo-matcher`; `Ctrl+P`, `gs` and, until search exists, `/`; ranking as in navigation §8. | FR-NAV-05 | 4.1 | Jump to any block by name |
| 4.3 | **Go to Command, Count and Save As.** `Ctrl+K` and `:` list every command with its binding; Count and Save As (with its path prompt) become reachable there. | FR-NAV-06, FR-OPS-09, FR-DOC-06 | 4.2, 2.10 | Save the `/tmp` file to your notes folder |
| 4.4 | **Move to.** `gm` with the same picker. | FR-OPS-05 | 4.2, 3.5 | Move a task to another project |
| 4.5 | **Show completed and word wrap.** `Ctrl+O`/`gc` per pane, `Alt+z`/`gw`. | FR-DONE-02, FR-UI-05 | 1.6 | Hide done tasks |
| 4.6 | **Sidebar.** Bookmarks, the Today entry and the project tree; `Ctrl+B`, `Alt+0` and the sidebar keys; hidden below 80 columns. | FR-UI-01, navigation §6 | 3.4 | Jump to a bookmark |
| 4.7 | **Today view.** A pane location listing today's blocks with their breadcrumbs, editable in place. | FR-TAG-04 (view), D-7, navigation §7 | 4.6, 3.2 | Open Today |
| 4.8 | **Shortcuts panel.** `?` opens the secondary sidebar on the right with the bindings for the current mode, from the registry. | FR-KEY-04, ui §1 and §4, D-28 | 4.1 | Press `?` |
| 4.9 | **Context menu.** `\`, `Shift+F10` and the Menu key; grouped entries from the registry. Can reuse the command menu's popup and its placement at the caret (`Popup::commands`, `layout::anchored`, `layout::room`). | FR-KEY-06, ui §5 | 4.3 | Press `\` on a task |

## M5 · Panes

| # | Step | Covers | After | Try it |
|---|---|---|---|---|
| 5.1 | **Split panes. hub** Open in a new pane (`Ctrl+\`, `Ctrl+W v`, `Ctrl+W s`, `Alt+Enter` in pickers and the sidebar), focus by number and by neighbor, close, narrow and widen, horizontal scrolling; per-pane state, shared folds, edits visible in every pane. | FR-UI-02, FR-UI-03, navigation §5 | 4.6 | Two projects side by side |
| 5.2 | **Layout restored.** `views.json` with text paths: panes, zooms, histories, widths, show-completed and sidebar visibility, but not folds. | FR-UI-04, file-io §5 | 5.1 | Quit and reopen; the panes come back |

## M6 · Settings and appearance

| # | Step | Covers | After | Try it |
|---|---|---|---|---|
| 6.1 | **Config file.** `config.toml` from the XDG path, defaults when missing, unknown keys and bad values reported once, `[keymap.*]` overrides. | FR-SET-01, FR-SET-04 (read), keybindings §5 | 4.1 | Rebind a key in the file |
| 6.2 | **Themes.** The theme struct, the two ANSI themes and the twelve truecolor ones, `COLORTERM` detection, `NO_COLOR`; the theme picker with live preview, saved with `toml_edit`. | FR-UI-06, FR-UI-08, N-UX-03, ui §3 | 4.3, 6.1 | Switch to Nord |
| 6.3 | **Icons.** The Nerd Font icon table with one fallback each, the `icons` setting, ASCII on non-UTF-8 locales. | FR-UI-09, FR-UI-10, ui §7 | 6.1 | Turn icons off |
| 6.4 | **Settings window.** `g,` and the palette; groups, live preview, reset, immediate writes that keep comments. | FR-SET-02, FR-SET-03, FR-SET-04, ui §8 | 6.2, 6.3 | Change the autosave delay |
| 6.5 | **Mouse.** Clicks, fold markers, crumbs, the sidebar, pane headers, Alt+click, right-click menus, the wheel. | FR-KEY-05, ui §6 | 4.9, 5.1 | Click around |

## M7 · Polish and release readiness

| # | Step | Covers | After | Try it |
|---|---|---|---|---|
| 7.1 | **Core animations.** The tachyonfx effect manager, synchronized output, the landing flash, change highlight, refusal flash and spinner; the `animations` setting with the SSH default. | FR-UI-11, UI-ANIM-01–03 and 08, ui §9 | 6.4 | Press `G` and see where focus landed |
| 7.2 | **Remaining animations** (Should). | UI-ANIM-04–07 and 09 | 7.1 | Complete a task |
| 7.3 | **Performance.** A 100k-line fixture; open under 300 ms, keystrokes under 16 ms, idle CPU near zero; fix what fails. | N-PERF-01–03 | 5.2 | (benchmarks) |
| 7.4 | **Terminal support.** Kitty variants and classic fallbacks for the keys added since D-42 checked, the `Alt` hint in `?`, wide characters, small terminals. | N-PORT-01–04, FR-KEY-03, keybindings §1 | 6.5 | Run in tmux at 80×24 |
| 7.5 | **Keymap validation** (owner, by hand). Every binding in the terminals of N-PORT-01, with and without the kitty protocol and inside tmux; swallowed keys get new bindings. | keybindings §6 | 7.4 | — |

## M8 · Comments

The owner moved Comments to the end on 2026-10-06 to think more about it; no Polish step needs it.

| # | Step | Covers | After | Try it |
|---|---|---|---|---|
| 8.1 | **Special-node placement.** Operations that insert at the end keep special nodes last; copy, move and delete carry them. Until this step, hand-written special nodes are preserved but may stop being the last child. | FR-SPC-01, special-nodes §1 | 3.6 | (tests with hand-made files) |
| 8.2 | **Comments.** Reading and writing the comment format, the count badge (clickable), the comments panel and its key (`C` went to removing a color, D-48) with add, edit, delete and copy, single undo steps. | FR-SPC-02, special-nodes §2 | 8.1, 4.8, 4.9, 6.5 | Comment on a task |

Cross-cutting Musts (FR-KEY-01, FR-KEY-02, FR-UI-07, FR-DOC-04) are checked in every step's review rather than owned by one step.

**v1 is complete** when M1–M8 are on `main` and `validate.sh trace` lists no untested Must requirement.

## Later

Search with live filtering and its query language, bookmarked searches, then internal links ([search.md](specs/search.md), [links.md](specs/links.md)). They are planned once v1 is accepted.

Code blocks, fenced code in text and text blocks of several lines were dropped from v1 by D-43 and may return after it (FR-BLK-05, FR-FMT-03).

## What can run in parallel

Parallel work stays inside the current milestone; a step of a later milestone waits for the earlier milestone's acceptance even when its **After** steps are on `main`. Within each milestone:

- **M1:** 1.1 first; then 1.2 and 1.3 together; then 1.4, 1.5 and 1.6 in order.
- **M2:** 2.1 and 2.2 together; then 2.3; then 2.10, 2.11 and 2.12 together; then 2.4, 2.5 and 2.6 in order; then 2.7 and 2.9 together.
- **M3:** 3.1, 3.2 and 3.9 together; then 3.3, 3.4 and 3.10; then 3.5 and 3.7; then 3.6 and 3.8.
- **M4:** 4.1 and 4.5 together; then 4.2 and 4.6; then 4.3, 4.4, 4.7 and 4.8; then 4.9.
- **M5:** in order.
- **M6:** 6.1 and 6.5 together; then 6.2 and 6.3; then 6.4.
- **M7:** 7.1, 7.3 and 7.4 together; then 7.2 and 7.5.
- **M8:** in order.
