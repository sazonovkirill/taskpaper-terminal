# Open questions

- Status: Draft
- Last updated: 2026-10-04

Product decisions that are not settled yet. Each lists the options, the **proposed default** the specs currently assume, and what it affects. When a question is decided, record the answer under **Decision**, update the affected specs, and keep the entry for history.

Status: `open` · `decided` · `deferred`

---

## OQ-01 · How do Workflowy-style notes map to TaskPaper? — open

Workflowy gives each node one optional multi-line note field. TaskPaper has no such field: a "note" is just another line (item) and can have children of its own.

- **A. Notes are ordinary items (TaskPaper model).** Every line is a node; note lines are nodes like any other. Shift+Enter creates a child note item.
- **B. Note-run as a field.** Consecutive note-type children directly under a task/project are presented and edited as that item's note block, Workflowy-style, but stay separate lines on disk.

**Proposed default:** A for the data model (it's lossless and matches TaskPaper 3). B can be a rendering option later.
**Affects:** [specs/data-model.md](specs/data-model.md), [specs/editing.md](specs/editing.md).

## OQ-02 · One file or many? — open

- **A. One file per session.** `tpt path/to/file.taskpaper`.
- **B. Folder as a tree.** Each `.taskpaper` file in a folder becomes a top-level node; zooming into it shows the file.
- **C. A + quick file switcher** across recent files.

**Proposed default:** A for v1, with B as a later extension. The model keeps a document per file so B stays possible.
**Affects:** [specs/architecture.md](specs/architecture.md), [specs/file-io.md](specs/file-io.md).

## OQ-03 · What type does a new line get? — open

Workflowy bullets are plain; TaskPaper's Return continues the current item's syntax (a new line after a task starts with `- `).

- **A. Follow TaskPaper's autoformat**: after a task → task (`- `); after a note or a project → empty note. The new item becomes the first child if the current item has visible children, else the next sibling.
- **B. Always a plain note** (Workflowy behavior); tasks are made explicitly.
- **C. Always a task.**

**Proposed default:** A.
**Affects:** [specs/editing.md](specs/editing.md).

## OQ-04 · Does completing add a date? — open

- **A. `@done(YYYY-MM-DD)`**: enables "completed in last N days" searches.
- **B. Bare `@done`**: TaskPaper's default.

**Proposed default:** A, configurable (`done_with_date = true`).
**Affects:** [specs/editing.md](specs/editing.md), [specs/search.md](specs/search.md).

## OQ-05 · Where is view state (folds, zoom, cursor) persisted? — open

TaskPaper stores it in macOS extended attributes keyed by runtime IDs. Items have no stable IDs in the file.

- **A. Nothing persisted.** Start fully expanded at the root.
- **B. Sidecar state file** in `$XDG_STATE_HOME/taskpaper-terminal/` keyed by the file's canonical path. Items are identified by their path of line texts from the root, re-matched on load; unmatched entries are dropped.
- **C. Extended attributes** like TaskPaper (not portable; lost by many sync tools and editors).
- **D. In-file tags** such as `@collapsed` (pollutes the user's text).

**Proposed default:** B.
**Affects:** [specs/file-io.md](specs/file-io.md), [specs/navigation.md](specs/navigation.md).

## OQ-06 · Keybinding scheme and edit model — open

Workflowy is always-in-insert-mode with modifier shortcuts. Terminals can't see Cmd and often can't distinguish Ctrl+Shift combinations unless the terminal supports the kitty keyboard protocol.

- **A. Workflowy-style modeless** with terminal-safe fallbacks (Alt-based alternatives where Ctrl+Shift is not reported).
- **B. Vim-style modal** (normal mode for structure, insert mode for text).
- **C. A as default, B as an optional keymap.**

**Proposed default:** C. All bindings live in one remappable keymap.
**Affects:** [specs/keybindings.md](specs/keybindings.md).

## OQ-07 · Which search syntax? — open

TaskPaper's item-path language is powerful and is what `@search(...)` items in files contain. Workflowy's operators are simpler but differ: in TaskPaper, `socks shoes` is one phrase; in Workflowy it is an AND.

- **A. TaskPaper only.**
- **B. Workflowy-style only.**
- **C. TaskPaper language as the engine, plus a small Workflowy-style front-end** (`is:`, `has:`, `-word`, `OR`, `>`), detected by syntax and translated into an item path. Saved searches always use the TaskPaper syntax.

**Proposed default:** C, with A shipped first.
**Affects:** [specs/search.md](specs/search.md).

## OQ-08 · Support `#tag`? — open

`#tag` is plain text to TaskPaper.

- **A. No.** Only `@tag`.
- **B. Display/search `#word` as a tag** without changing the file format.

**Proposed default:** A.

## OQ-09 · Created / changed timestamps — deferred

Workflowy can search by `created:` and `changed:`. TaskPaper files carry no per-item timestamps.

- **A. Not supported.**
- **B. Opt-in tags** (`@created(…)`), written automatically.
- **C. Sidecar history** (fragile under external edits).

**Proposed default:** A.

## OQ-10 · Trash for deleted items — open

- **A. Undo only.**
- **B. Session trash view** (in memory).
- **C. Persistent trash** in a sidecar `.taskpaper` file.

**Proposed default:** A, plus a backup on save (see [specs/file-io.md](specs/file-io.md)).

## OQ-11 · Starred pages — open

Saved searches map to `@search(...)` items, but Workflowy also stars arbitrary nodes.

- **A. Saved searches only** (they can target a node, e.g. `/Work//*`).
- **B. Stars in the sidecar state file.**
- **C. A `@star` tag in the file.**

**Proposed default:** A.

## OQ-12 · Mirrors and internal links — deferred

Neither has a TaskPaper representation. One option is an `@id(x)` tag on the target with `@ref(x)` / `[[x]]` links. Every option is invisible to other TaskPaper apps.

**Proposed default:** out of scope for v1.

## OQ-13 · Inline formatting — open

- **A. None.** Plain text.
- **B. Render Markdown-style emphasis** (`**bold**`, `_italic_`, `` `code` ``) without changing the file.

**Proposed default:** A for v1; B is cheap and could come later.

## OQ-14 · Write fidelity — open

TaskPaper 3 rewrites files with tabs, `\n` and UTF-8 regardless of the original.

- **A. Normalize like TaskPaper.**
- **B. Preserve detected style**: indent unit (tabs or N spaces), line endings, BOM and trailing newline. Unchanged lines stay byte-identical.

**Proposed default:** B. It makes the app safe to use on files also edited in other editors and keeps diffs clean.
**Affects:** [specs/file-io.md](specs/file-io.md).

## OQ-15 · Save model — open

- **A. Autosave** after a short idle delay and on exit/suspend (Workflowy).
- **B. Explicit save** with a dirty indicator (`Ctrl+S`).

**Proposed default:** A, with `Ctrl+S` also forcing a save and autosave configurable.

## OQ-16 · External changes while open — open

- **A. Reload automatically if there are no unsaved local changes; otherwise prompt** (keep mine / take theirs / show diff).
- **B. Always prompt.**
- **C. Three-way merge** by line.

**Proposed default:** A. C is a possible later improvement.

## OQ-17 · Supported platforms — open

**Proposed default:** macOS and Linux are first-class. Windows (Windows Terminal) is best effort.

## OQ-18 · Config format and theming — open

**Proposed default:** TOML at `$XDG_CONFIG_HOME/taskpaper-terminal/config.toml` (platform equivalent via the `directories` crate); themes as named color sets in the same file; honor `NO_COLOR`.

## OQ-19 · Mouse support — open

**Proposed default:** optional, on by default: click to move the cursor, click a bullet to zoom, click a fold marker, scroll wheel.

## OQ-20 · Archive @done options — open

TaskPaper has two optional archive preferences, both off by default: strip extra tags, and add `@project(Outer / Inner)`.

**Proposed default:** match TaskPaper (both available, both off).

## OQ-21 · Binary name — open

**Proposed default:** crate `taskpaper-terminal`, binary `tpt`.

## OQ-22 · Outline shapes TaskPaper text cannot represent — open

TaskPaper re-attaches blank lines by indentation ([reference/taskpaper.md §2.4](reference/taskpaper.md#24-blank--whitespace-only-lines)) and treats leading whitespace as indentation. So some shapes an outliner can produce do not survive a save and reload: an empty item with children (its children re-attach elsewhere), an empty item that is the last line of the file below top level (it becomes top-level), item text that starts with whitespace (read back as indentation), and an over-indented item moved where its indentation now makes it a child of the line above.

- **A. Repair them in the editor.** Indentation is normalized silently on every edit (it changes no text). Empty parents, trailing indented blanks and leading whitespace are allowed while the user edits that item and repaired visibly and undoably when the cursor leaves it.
- **B. Allow them and accept the reload change**, with a warning.
- **C. Write a placeholder** (for example a lone `-`) for empty parents.

**Proposed default:** A, as specified in [specs/file-format.md §3.1](specs/file-format.md#31-canonical-outlines).
**Affects:** [specs/file-format.md](specs/file-format.md), [specs/editing.md](specs/editing.md), [specs/data-model.md](specs/data-model.md).
