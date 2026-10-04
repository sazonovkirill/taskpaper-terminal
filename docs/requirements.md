# Requirements

- Status: Draft
- Last updated: 2026-10-04

What taskpaper-terminal must do. The "how" lives in [specs/](specs/). Background is in [reference/](reference/), and unsettled decisions in [open-questions.md](open-questions.md).

## Product goal

A keyboard-first terminal outliner with Workflowy's feel (one infinite tree, zoom, fold, live filtering, fast structural editing) that stores everything in plain `.taskpaper` files, staying fully compatible with TaskPaper 3 and any text editor.

## Principles

1. **The file is the source of truth.** No database, no hidden content. Anything the user sees in the outline is in the `.taskpaper` text.
2. **Never lose data.** Crashes, external edits and bad input must not corrupt or drop the user's text.
3. **TaskPaper compatible.** Files written by the app open correctly in TaskPaper 3, and vice versa. Behavior follows the reference parser ([reference/taskpaper.md](reference/taskpaper.md)).
4. **Workflowy-fast.** Every common operation is one keystroke away and feels instant.
5. **Terminal-native.** Works over SSH, in tmux, in 80×24, and without a mouse.

## Priorities

- **Must**: required for v1.0.
- **Should**: expected in v1.x.
- **Could**: nice to have.
- **Won't**: explicitly out of scope (see the end of this file).

IDs are stable; reference them from specs, issues and commits.

---

## Functional requirements

### F-FILE · Files and storage

| ID | Requirement | Priority |
|---|---|---|
| F-FILE-01 | Open a `.taskpaper` file given on the command line; create it if missing. | Must |
| F-FILE-02 | Parse files exactly as TaskPaper 3 does: item types, tags, indentation, blank lines ([specs/file-format.md](specs/file-format.md)). | Must |
| F-FILE-03 | Write files that TaskPaper 3 parses to the same outline. Round-tripping an unedited file produces identical bytes. | Must |
| F-FILE-04 | Saves are atomic (temp file + rename); file permissions are preserved. | Must |
| F-FILE-05 | Autosave after edits and on exit/suspend; explicit save command too (OQ-15). | Must |
| F-FILE-06 | Detect external modification of the open file and reload or prompt without losing local edits (OQ-16). | Must |
| F-FILE-07 | Refuse to open non-UTF-8 files with a clear error rather than corrupting them. | Must |
| F-FILE-08 | Preserve the file's indent style, line endings, BOM and trailing newline (OQ-14). | Should |
| F-FILE-09 | Keep a rolling backup of the previous version on save. | Should |
| F-FILE-10 | Persist folds, zoom and cursor per file between sessions (OQ-05). | Should |
| F-FILE-11 | Work with several files: switcher or folder-as-tree (OQ-02). | Could |
| F-FILE-12 | Read from stdin / write to stdout for scripting (`tpt --query '…' file`). | Could |

### F-VIEW · Outline display

| ID | Requirement | Priority |
|---|---|---|
| F-VIEW-01 | Render the outline as an indented tree, one item per row, with long lines soft-wrapped under their text. | Must |
| F-VIEW-02 | Distinguish projects, tasks and notes visually; show tags highlighted; show `@done` items struck through or dimmed. | Must |
| F-VIEW-03 | Show a fold marker on items with children, distinguishing collapsed from expanded. | Must |
| F-VIEW-04 | Show breadcrumbs of the zoomed item's ancestors. | Must |
| F-VIEW-05 | Status line: file name, dirty/saved state, active filter, mode, messages. | Must |
| F-VIEW-06 | Scroll to keep the cursor visible; handle 100k-line files smoothly. | Must |
| F-VIEW-07 | Correct width handling for wide characters, emoji and combining marks. | Must |
| F-VIEW-08 | Hide/show completed items toggle. | Must |
| F-VIEW-09 | Themeable colors; honor `NO_COLOR`; usable in 16-color terminals. | Should |
| F-VIEW-10 | Sidebar listing projects, saved searches and tags (TaskPaper sidebar). | Should |
| F-VIEW-11 | Split panes showing different zooms/filters of the same document. | Could |
| F-VIEW-12 | Render Markdown-style emphasis and clickable URLs/paths (OQ-13). | Could |

### F-NAV · Navigation

| ID | Requirement | Priority |
|---|---|---|
| F-NAV-01 | Move the cursor between visible items and within an item's text. | Must |
| F-NAV-02 | Expand, collapse and toggle an item; expand/collapse an entire subtree; expand/collapse all to a level. | Must |
| F-NAV-03 | Zoom into any item, so it becomes the root of the view; zoom out one level; go home. | Must |
| F-NAV-04 | Back/forward history of zoom locations. | Should |
| F-NAV-05 | Jump to parent, first/last child, previous/next sibling. | Must |
| F-NAV-06 | Fuzzy "Go to anything": items, projects and saved searches. | Must |
| F-NAV-07 | Click to place cursor, zoom, fold; wheel scroll (OQ-19). | Should |

### F-EDIT · Editing

| ID | Requirement | Priority |
|---|---|---|
| F-EDIT-01 | Edit item text inline with standard line-editing keys (word motion, home/end, delete word, etc.). | Must |
| F-EDIT-02 | Enter splits/creates a new item following TaskPaper's rules (OQ-03); Enter on an empty task turns it into an empty note. | Must |
| F-EDIT-03 | Indent/outdent items with their subtrees. | Must |
| F-EDIT-04 | Move items (with subtrees) up/down among siblings, crossing into adjacent parents at the boundary. | Must |
| F-EDIT-05 | Change item type: task / project / note. | Must |
| F-EDIT-06 | Toggle done (`@done`, optionally dated, OQ-04) on one or many items. | Must |
| F-EDIT-07 | Delete items with their subtrees. | Must |
| F-EDIT-08 | Undo/redo for every change, including structural ones, grouped per command. | Must |
| F-EDIT-09 | Select a range of sibling items and apply indent, outdent, move, delete, done, tag, copy to all. | Must |
| F-EDIT-10 | Copy/cut/paste items as TaskPaper text, keeping hierarchy; paste multi-line text as items. Use the system clipboard where available, OSC 52 otherwise. | Must |
| F-EDIT-11 | Duplicate items. | Should |
| F-EDIT-12 | Move items to another location chosen with a fuzzy picker. | Should |
| F-EDIT-13 | Sort children A→Z / Z→A (one-shot). | Should |
| F-EDIT-14 | Archive @done items into `Archive:` like TaskPaper (OQ-20). | Should |
| F-EDIT-15 | Insert from template (items tagged `@template`). | Could |
| F-EDIT-16 | Count items and words in a subtree. | Could |
| F-EDIT-17 | Group selected items under a new parent. | Could |

### F-TAG · Tags and dates

| ID | Requirement | Priority |
|---|---|---|
| F-TAG-01 | Add, edit and remove `@tag` / `@tag(value)` on items, with tag-name autocomplete from the document. | Must |
| F-TAG-02 | Filter by a tag (or tag value) from the item under the cursor. | Must |
| F-TAG-03 | Set `@due` / `@start` from natural-language input using TaskPaper's date grammar, stored as `YYYY-MM-DD[ HH:mm]`. | Should |
| F-TAG-04 | Toggle `@today`. | Should |
| F-TAG-05 | Highlight overdue and due-today items. | Should |
| F-TAG-06 | Rename a tag across the document. | Could |

### F-SEARCH · Search and filtering

| ID | Requirement | Priority |
|---|---|---|
| F-SEARCH-01 | Live filter: matching items are shown with their ancestors; the outline stays editable while filtered. | Must |
| F-SEARCH-02 | Full TaskPaper query language: predicates, relations, modifiers, axes, type shortcuts, set operations, slices ([specs/search.md](specs/search.md)). | Must |
| F-SEARCH-03 | Clear syntax errors, without losing the last good result. | Must |
| F-SEARCH-04 | Search is scoped to the zoomed item. | Must |
| F-SEARCH-05 | Saved searches read from `@search(...)` items in the document; create one from the current filter. | Should |
| F-SEARCH-06 | Workflowy-style query shorthand (`is:`, `has:`, `-word`, `OR`, `A > B`) (OQ-07). | Could |
| F-SEARCH-07 | Plain incremental text find (jump between matches without filtering). | Should |

### F-CMD · Commands and configuration

| ID | Requirement | Priority |
|---|---|---|
| F-CMD-01 | Every action is a named command; keys map to commands through a remappable keymap. | Must |
| F-CMD-02 | Command palette with fuzzy search over commands, showing their keys. | Must |
| F-CMD-03 | In-app help listing current key bindings. | Must |
| F-CMD-04 | Config file for keymap, theme and preferences (OQ-18). | Should |
| F-CMD-05 | Optional vim-style modal keymap (OQ-06). | Could |

### F-IO · Import and export

| ID | Requirement | Priority |
|---|---|---|
| F-IO-01 | Export the zoomed subtree as plain TaskPaper text, Markdown or OPML. | Could |
| F-IO-02 | Import OPML and indented Markdown lists. | Could |

---

## Non-functional requirements

| ID | Requirement | Priority |
|---|---|---|
| N-PERF-01 | Open and render a 10 MB / 100k-line file in under 300 ms on a modern laptop. | Must |
| N-PERF-02 | Keystroke to rendered frame under 16 ms for typical edits on that file; filter updates under 100 ms. | Must |
| N-PERF-03 | Idle CPU near zero: render only on input, resize, file change or timer. | Must |
| N-REL-01 | No panic may lose data: on panic, restore the terminal and write unsaved content to a recovery file. | Must |
| N-REL-02 | Parser and serializer are covered by round-trip property tests and a corpus of TaskPaper samples. | Must |
| N-REL-03 | Query engine is tested against the TaskPaper guide's examples and the edge cases in [reference/taskpaper.md](reference/taskpaper.md#11-edge-cases-and-ambiguities-checklist-for-a-reimplementation). | Must |
| N-PORT-01 | Runs on macOS and Linux in common terminals (iTerm2, Terminal.app, Ghostty, kitty, WezTerm, Alacritty, GNOME Terminal) and inside tmux (OQ-17). | Must |
| N-PORT-02 | Works in 80×24 and degrades gracefully below it. | Must |
| N-PORT-03 | Uses the kitty keyboard protocol when available and falls back to legacy key reporting otherwise. | Should |
| N-UX-01 | Never blocks the UI on disk I/O longer than a frame. | Must |
| N-UX-02 | Every destructive action is undoable. | Must |
| N-DIST-01 | Single static binary; installable with `cargo install`. | Should |

---

## Out of scope (Won't, v1)

Cloud and collaboration features have no meaning for a local file editor: sharing, permissions, comments, @mention notifications, sync, mobile apps, email capture, calendar sync, AI features, file uploads and embeds.

Also out of scope for v1, pending the linked questions: mirrors and internal links (OQ-12), boards/tables/dashboards, rich text (OQ-13), per-item created/changed timestamps (OQ-09).
