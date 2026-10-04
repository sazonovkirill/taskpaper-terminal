# Open questions

- Status: Living
- Last updated: 2026-10-04

Product choices not settled by [functional-requirements.md](functional-requirements.md). Each lists the options, the **proposed default** the specs currently assume, and what it affects. When the owner decides one, record the answer under **Decision**, update the affected specs, and keep the entry for history.

## Open

### OQ-03 · What type does a new block get? — open

FR-MODE-04 says new blocks open in edit mode but not what type they are.

- **A. Continue the current type:** after a task → task (`- `); after anything else → a single-line note. A new child is a task if the parent is a project or task, else a note.
- **B. Always a note**; tasks are made with the type converter (FR-BLK-08).
- **C. Always a task.**

**Proposed default:** A (TaskPaper's own Return behavior, reference §9).
**Affects:** [specs/editing.md](specs/editing.md).

### OQ-05 · Where are pane layout and fold state stored? — open

FR-UI-04 restores the pane layout on launch, and D-13 makes fold state shared across panes, but neither says where that state lives. Items have no stable IDs in the file.

- **A. Sidecar state file** in `$XDG_STATE_HOME/taskpaper-terminal/` keyed by the file's canonical path. Blocks are identified by their path of texts from the root and re-matched on load; unmatched entries are dropped.
- **B. Extended attributes**, like TaskPaper (lost by many sync tools and editors).
- **C. In-file tags** such as `@collapsed` (adds text the user didn't write).

**Proposed default:** A, persisting panes (zoom, history, show-completed) and fold state.
**Affects:** [specs/file-io.md](specs/file-io.md), [specs/navigation.md](specs/navigation.md).

### OQ-14 · Write fidelity — open

TaskPaper 3 rewrites files with tabs, `\n` and UTF-8 regardless of the original.

- **A. Normalize like TaskPaper.**
- **B. Preserve detected style**: indent unit (tabs or N spaces), line endings, BOM and trailing newline. Unchanged lines stay byte-identical.

**Proposed default:** B. It keeps the app safe on files also edited elsewhere and keeps diffs clean.
**Affects:** [specs/file-format.md](specs/file-format.md).

### OQ-16 · External changes while open — open

- **A. Reload automatically if there are no unsaved local changes; otherwise prompt** (keep mine / take theirs / show diff).
- **B. Always prompt.**
- **C. Three-way merge** by line.

**Proposed default:** A.
**Affects:** [specs/file-io.md](specs/file-io.md).

### OQ-17 · Supported platforms — open

FR-KEY-03 requires bindings that work on macOS, Linux and Windows terminals.

**Proposed default:** macOS and Linux are fully supported; Windows Terminal is supported for input and rendering, with file watching and clipboard best effort.

### OQ-18 · Config file — open

FR-UI-05 and FR-UI-06 need settings (word wrap, theme) to persist.

**Proposed default:** TOML at `$XDG_CONFIG_HOME/taskpaper-terminal/config.toml` (platform equivalent via the `directories` crate) holding theme, word wrap and key overrides; choices made in the app are written back to it.

### OQ-21 · Binary name — open

**Proposed default:** crate `taskpaper-terminal`, binary `tpt`.

### OQ-22 · Outline shapes TaskPaper text cannot represent — open

TaskPaper re-attaches blank lines by indentation ([reference/taskpaper.md §2.4](reference/taskpaper.md#24-blank--whitespace-only-lines)) and treats leading whitespace as indentation. So some shapes an outliner can produce do not survive a save and reload: a block whose last line is empty and that has children, an empty line that is the last line of the file below top level, text that starts with whitespace (which also means code blocks cannot keep leading indentation on their lines), and an over-indented line moved where its indentation makes it a child of the line above.

- **A. Repair them in the editor.** Indentation is normalized silently on every edit (it changes no text). The other shapes are allowed while the user edits that block and repaired visibly and undoably when editing ends.
- **B. Allow them and accept the change on reload**, with a warning.

**Proposed default:** A, as specified in [specs/file-format.md §3.1](specs/file-format.md#31-canonical-outlines).

### OQ-23 · Line breaks inside a note block — open

D-2 makes consecutive note lines one block, and FR-BLK-05 says Enter inserts a newline in code blocks. For other blocks Enter is not specified.

- **A.** In a note block, `Shift+Enter` (fallback `Alt+Enter`) inserts a line break and `Enter` ends editing and creates the next block. In a code block, `Enter` inserts a line break and `Esc` ends editing.
- **B.** In note and code blocks `Enter` inserts a line break; only `Esc` ends editing.

**Proposed default:** A.
**Affects:** [specs/editing.md](specs/editing.md), [specs/keybindings.md](specs/keybindings.md).

### OQ-24 · Where tags go when a block's text is edited — open

Tags are hidden while editing (FR-TAG-01). Tags in the middle of a line have no visible position to keep.

- **A.** On commit, write the new text and append the block's tags after it, in their original order, so mid-line tags move to the end of the line.
- **B.** Track each tag's character offset and reinsert it at the nearest word boundary.

**Proposed default:** A. Content is preserved (FR-DOC-03); only the position of mid-line tags changes.
**Affects:** [specs/editing.md](specs/editing.md).

### OQ-25 · Typing a word that looks like a tag — open

Tags are never typed (FR-TAG-01), but TaskPaper has no way to escape `@`: a typed `@word` at a word start is a tag when the file is read back.

- **A. Refuse `@` at a word start** in edit mode, with a hint pointing to the tag commands. `@` inside a word (emails) is allowed.
- **B. Accept it and let it become a hidden tag** on commit, with a status message.
- **C. Substitute a look-alike** such as the fullwidth `＠` (U+FF20), which TaskPaper does not treat as a tag.

**Proposed default:** A.
**Affects:** [specs/editing.md](specs/editing.md).

### OQ-26 · Adjacent note blocks — open

D-2 makes consecutive sibling note lines one block, and TaskPaper text has no way to mark a boundary between two of them. So two note blocks can never be adjacent siblings: creating a note next to a note, deleting the block that separated two notes, or converting a block between notes into a note merges them.

- **A. Accept merging as the consequence of D-2, and avoid it where the app chooses the type:** a new block (new below/above, `group`'s parent) that would be a note next to a note becomes a task instead. Conversions and deletions that make notes adjacent merge them, with a status message.
- **B. Insert a separator** (an empty line or a divider) between them, changing the file.
- **C. Revisit D-2** so that blank lines separate note blocks.

**Proposed default:** A.
**Affects:** [specs/editing.md](specs/editing.md), [specs/data-model.md](specs/data-model.md).

### OQ-27 · Typed text that looks like task or project syntax — open

The type is hidden while editing, but TaskPaper derives it from the text: a note ending in `:` is a project, a line starting with `- ` is a task. There is no escape.

- **A. Let the text decide:** the block takes the type its text parses as, and the marker updates live. In a code block, a line that would parse as a task or project ends the code run; edit mode warns before that happens.
- **B. Refuse the edit** (a trailing `:` on a note, a leading `- ` on a non-task) with a hint pointing to the type converter.

**Proposed default:** A. It matches TaskPaper and loses nothing; B would forbid ordinary text like `Questions:` in a note.
**Affects:** [specs/editing.md](specs/editing.md).

## Decided

| # | Question | Decision |
|---|---|---|
| OQ-01 | How notes map to TaskPaper | Consecutive sibling note lines form one block (D-2, FR-BLK-03). |
| OQ-02 | One file or many | One file per instance (FR-DOC-01). |
| OQ-04 | Does completing add a date | Always `@done(YYYY-MM-DD)` (D-4). |
| OQ-06 | Key bindings and edit model | Navigate and edit modes, vi-inspired navigation, standard text keys in edit mode (D-1, FR-KEY-02). |
| OQ-07 | Search syntax | Workflowy-style syntax (FR-SRCH-02); search is Later (D-14). |
| OQ-08 | `#tag` support | No. Tags are hidden metadata set by commands (FR-TAG-01). |
| OQ-09 | Created/changed timestamps | Won't (D-14). |
| OQ-10 | Trash | Won't (D-14). |
| OQ-11 | Starred pages | Bookmarks via a hidden `@bookmark` tag (D-11). |
| OQ-12 | Mirrors and internal links | Mirrors Won't; internal links Later (D-14). |
| OQ-13 | Inline formatting | `**bold**` and `` `code` `` (FR-FMT-01). |
| OQ-15 | Save model | Autosave (D-15, FR-DOC-02). |
| OQ-19 | Mouse support | Must, secondary to the keyboard (D-16). |
| OQ-20 | Archive @done | No archive command (FR-DOC-04). |
