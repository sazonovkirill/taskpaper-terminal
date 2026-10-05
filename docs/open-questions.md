# Open questions

- Status: Living
- Last updated: 2026-10-05

Product choices not settled by [functional-requirements.md](functional-requirements.md). Each lists the options, the **proposed default** the specs currently assume, and what it affects. When the owner decides one, record the answer under **Decision**, update the affected specs, and keep the entry for history.

## Open

None.

## Superseded

### OQ-30 · Where does a URL block's address come from? — superseded

FR-BLK-08 converts blocks to URL blocks (`Title @url(…)`, D-10), and editing.md §5 first said the converter prompts for the address. Tags are never typed (OQ-25), so the converter is the only way to make one. Step 3.7 had to choose; the M3 implementer chose the proposed default below, which the specs now follow.

- **A. Prompt for the address**, with the block's text as the title.
- **B. Take the first link in the text** as the address: a bare URL (D-10) or, in a text block, a Markdown link (FR-FMT-03). The rest of the text, with a `[title](url)` link's title in its place, becomes the title, or the address when nothing else is left; a block without a link keeps its type and the status line says to add one. Typing a link into a block and converting it is enough, and the converter needs no prompt.

**Proposed default:** B.
**Decision (2026-10-05):** superseded by D-45. The owner dropped the URL block type, so no conversion needs an address; a line's existing `@url` stays, and any block can hold links in its text.
**Affects:** [specs/editing.md](specs/editing.md#5-block-commands), [specs/keybindings.md](specs/keybindings.md#4-other-contexts).

## Decided with the proposed defaults

On 2026-10-04 the owner accepted the proposed default for every question below.

### OQ-28 · Does typing `# ` make a section? — decided

D-24 asks for shortcuts that turn typed prefixes into block types, and D-25 adds Markdown headings to notes. Workflowy's `# ` trigger makes a heading; this app has two header-like things, a Markdown heading inside a note and the section block (`@section`, FR-BLK-04), which groups its children under a Things 3 style rule.

- **A. Markdown heading:** `# ` stays text, so the note renders as a heading (FR-FMT-03). Sections are made with the type converter (FR-BLK-08). No tag is added.
- **B. Section shortcut:** `# ` at the start of a single-line note or task converts it to a section; Markdown headings remain available on later lines of multi-line notes.

**Proposed default:** A. It is what Markdown means by `#`, keeps one meaning for `#` everywhere in a note, and leaves the file free of a tag the user did not ask for.
**Decision (2026-10-04):** the owner chose A: `# ` makes a Markdown heading.
**Affects:** [specs/editing.md](specs/editing.md#21-typed-shortcuts).

### OQ-29 · Where do the bindings displaced by D-26 go? — decided

The owner's bindings (D-26) take keys that earlier drafts used: `c` (comments), `t` (the metadata prefix), `y` (the copy prefix), `p`/`P` (paste), `a` (edit at end), `{`/`}` (sibling motion), `>`/`<` (outdent/indent), `z` (fold prefix), `Z` (`ZZ` quit), `Ctrl+P` (Go to Command) and, in edit mode, `Ctrl+K` (insert link). D-26 also leaves open what "top" and "bottom" mean for `P`/`p`, `S`/`s`, `A`/`a`, and what `/` does before search exists.

**Proposed default:**
- Comments `C`; the metadata prefix becomes `m` (`mt` today, `ms`/`md` date prompts, `mc` color picker, `mb` bookmark, `my` convert, `mi` details), and Move to moves from `m` to `gm`.
- Copy and cut are `Ctrl+C`/`Ctrl+X` only; duplicate is `D`; paste is `Ctrl+V` below and `Alt+V` above.
- `Enter` edits with the caret at the end and `i` at the start (superseded by D-41: `Enter` alone edits, and `i`/`I` add children).
- Sibling motion keeps `Ctrl+↑`/`Ctrl+↓`; indent and outdent keep `Tab`/`Shift+Tab`.
- Folding drops the `z` prefix: `'` and `Space` toggle, `"` toggles the subtree, `=`/`-` expand and collapse by level, `+`/`_` expand and collapse all. Quit is `Ctrl+Q` only; `u` and `Ctrl+R` are no longer undo and redo.
- `Ctrl+P` is Go to Symbol (with `gs`), `Ctrl+K` Go to Command (with `:`), and inserting a link (Later) is typing `[[` only.
- The context menu moves from `M` to `\`; `Ctrl+'` needs the kitty keyboard protocol, so `gh` stays as the route that always works.
- "The list" is the focused block's siblings. Upper case adds at its top (a section goes before the first section, after the tasks); lower case adds right below the focused block, as `o` does.
- `/` opens Go to Symbol until search exists.

**Decision (2026-10-04):** the owner accepted the proposed default, left the meaning of top and bottom to Claude (kept as proposed), and confirmed that files open fully expanded.
**Affects:** [specs/keybindings.md](specs/keybindings.md#2-navigate-mode), [specs/editing.md](specs/editing.md#3-creating-blocks-fr-mode-04), [specs/special-nodes.md](specs/special-nodes.md), [specs/links.md](specs/links.md).

### OQ-03 · What type does a new block get? — decided

FR-MODE-04 says new blocks open in edit mode but not what type they are.

- **A. Continue the current type:** after a task → task (`- `); after anything else → a single-line note. A new child is a task if the parent is a project or task, else a note.
- **B. Always a note**; tasks are made with the type converter (FR-BLK-08).
- **C. Always a task.**

**Proposed default:** A (TaskPaper's own Return behavior, reference §9).
**Decision (2026-10-04):** the owner accepted the proposed default.
**Affects:** [specs/editing.md](specs/editing.md).

### OQ-05 · Where are pane layout and fold state stored? — decided

FR-UI-04 restores the pane layout on launch, and D-13 makes fold state shared across panes, but neither says where that state lives. Items have no stable IDs in the file.

- **A. Sidecar state file** in `$XDG_STATE_HOME/taskpaper-terminal/` keyed by the file's canonical path. Blocks are identified by their path of texts from the root and re-matched on load; unmatched entries are dropped.
- **B. Extended attributes**, like TaskPaper (lost by many sync tools and editors).
- **C. In-file tags** such as `@collapsed` (adds text the user didn't write).

**Proposed default:** A, persisting panes (zoom, history, show-completed) and fold state.
**Decision (2026-10-04):** the owner accepted the proposed default. Later the same day the owner decided that fold state lives in memory only (D-26), so only the panes are persisted.
**Affects:** [specs/file-io.md](specs/file-io.md), [specs/navigation.md](specs/navigation.md).

### OQ-14 · Write fidelity — decided

TaskPaper 3 rewrites files with tabs, `\n` and UTF-8 regardless of the original.

- **A. Normalize like TaskPaper.**
- **B. Preserve detected style**: indent unit (tabs or N spaces), line endings, BOM and trailing newline. Unchanged lines stay byte-identical.

**Proposed default:** B. It keeps the app safe on files also edited elsewhere and keeps diffs clean.
**Decision (2026-10-04):** the owner accepted the proposed default.
**Affects:** [specs/file-format.md](specs/file-format.md).

### OQ-16 · External changes while open — decided

- **A. Reload automatically if there are no unsaved local changes; otherwise prompt** (keep mine / take theirs / show diff).
- **B. Always prompt.**
- **C. Three-way merge** by line.

**Proposed default:** A.
**Decision (2026-10-04):** the owner accepted the proposed default.
**Affects:** [specs/file-io.md](specs/file-io.md).

### OQ-17 · Supported platforms — decided

FR-KEY-03 requires bindings that work on macOS, Linux and Windows terminals.

**Proposed default:** macOS and Linux are fully supported; Windows Terminal is supported for input and rendering, with file watching and clipboard best effort.
**Decision (2026-10-04):** the owner accepted the proposed default.

### OQ-18 · Config file — decided

FR-UI-05 and FR-UI-06 need settings (word wrap, theme) to persist.

**Proposed default:** TOML at `$XDG_CONFIG_HOME/taskpaper-terminal/config.toml` (platform equivalent via the `directories` crate) holding theme, word wrap and key overrides; choices made in the app are written back to it.
**Decision (2026-10-04):** the owner accepted the proposed default, and later asked for the file under `~/.config` on every Unix, macOS included, with a settings window that edits it (FR-SET, D-20).

### OQ-21 · Binary name — decided

**Proposed default:** crate `taskpaper-terminal`, binary `tpt`.
**Decision (2026-10-04):** the owner accepted the proposed default.

### OQ-22 · Outline shapes TaskPaper text cannot represent — decided

TaskPaper re-attaches blank lines by indentation ([reference/taskpaper.md §2.4](reference/taskpaper.md#24-blank--whitespace-only-lines)) and treats leading whitespace as indentation. So some shapes an outliner can produce do not survive a save and reload: a block whose last line is empty and that has children, an empty line that is the last line of the file below top level, text that starts with whitespace (which also means code blocks cannot keep leading indentation on their lines), and an over-indented line moved where its indentation makes it a child of the line above.

- **A. Repair them in the editor.** Indentation is normalized silently on every edit (it changes no text). The other shapes are allowed while the user edits that block and repaired visibly and undoably when editing ends.
- **B. Allow them and accept the change on reload**, with a warning.

**Proposed default:** A, as specified in [specs/file-format.md §3.1](specs/file-format.md#31-canonical-outlines).
**Decision (2026-10-04):** the owner accepted the proposed default.

### OQ-23 · Line breaks inside a note block — decided

D-2 makes consecutive note lines one block, and FR-BLK-05 says Enter inserts a newline in code blocks. For other blocks Enter is not specified.

- **A.** In a note block, `Shift+Enter` (fallback `Alt+Enter`) inserts a line break and `Enter` ends editing and creates the next block. In a code block, `Enter` inserts a line break and `Esc` ends editing.
- **B.** In note and code blocks `Enter` inserts a line break; only `Esc` ends editing.

**Proposed default:** A.
**Decision (2026-10-04):** the owner accepted the proposed default.
**Note (2026-10-05):** moot since D-43 made every block one line: no key breaks a line, and `Enter` ends editing and creates the next block in every block.
**Affects:** [specs/editing.md](specs/editing.md), [specs/keybindings.md](specs/keybindings.md).

### OQ-24 · Where tags go when a block's text is edited — decided

Tags are hidden while editing (FR-TAG-01). Tags in the middle of a line have no visible position to keep.

- **A.** On commit, write the new text and append the block's tags after it, in their original order, so mid-line tags move to the end of the line.
- **B.** Track each tag's character offset and reinsert it at the nearest word boundary.

**Proposed default:** A. Content is preserved (FR-DOC-03); only the position of mid-line tags changes.
**Decision (2026-10-04):** the owner accepted the proposed default.
**Affects:** [specs/editing.md](specs/editing.md).

### OQ-25 · Typing a word that looks like a tag — decided

Tags are never typed (FR-TAG-01), but TaskPaper has no way to escape `@`: a typed `@word` at a word start is a tag when the file is read back.

- **A. Refuse `@` at a word start** in edit mode, with a hint pointing to the tag commands. `@` inside a word (emails) is allowed. This also blocks code such as Python `@decorator` lines in code blocks.
- **B. Accept it and let it become a hidden tag** on commit, with a status message.
- **C. Substitute a look-alike** such as the fullwidth `＠` (U+FF20), which TaskPaper does not treat as a tag.

**Proposed default:** A.
**Decision (2026-10-04):** the owner accepted the proposed default.
**Affects:** [specs/editing.md](specs/editing.md).

### OQ-26 · Adjacent note blocks — decided

D-2 makes consecutive sibling note lines one block, and TaskPaper text has no way to mark a boundary between two of them. So two note blocks can never be adjacent siblings: creating a note next to a note, deleting the block that separated two notes, or converting a block between notes into a note merges them.

- **A. Accept merging as the consequence of D-2, and avoid it where the app chooses the type.** Operations work on lines and blocks are re-derived afterwards, so any operation that leaves two note runs adjacent (delete, duplicate, paste, move, indent, outdent, move to, convert) merges them, and the status line says "Notes merged"; undo separates them again. Where the app chooses the type itself (new below/above, `group`'s parent), it uses a task instead of a note when a note would merge.
- **B. Insert a separator** (an empty line or a divider) between them, changing the file.
- **C. Revisit D-2** so that blank lines separate note blocks.

**Proposed default:** A.
**Decision (2026-10-04):** the owner accepted the proposed default.
**Note (2026-10-05):** moot since D-43 made every line its own block: adjacent text blocks no longer merge, there is no "Notes merged" notice, and a new block next to text is text.
**Affects:** [specs/editing.md](specs/editing.md), [specs/data-model.md](specs/data-model.md).

### OQ-27 · Typed text that looks like task or project syntax — decided

The type is hidden while editing, but TaskPaper derives it from the text: a note ending in `:` is a project, a line starting with `- ` is a task. There is no escape.

- **A. Let the text decide:** each line takes the type its text parses as. In a single-line block the marker updates live. In a multi-line note or code block, a line that parses as a task or project splits the block at that line; the pane being edited keeps showing one edit area until editing ends, then shows the resulting blocks with focus on the one containing the caret. Edit mode shows a hint while a line would split the block.
- **B. Refuse the edit** (a trailing `:` on a note, a leading `- ` on a non-task) with a hint pointing to the type converter.

**Proposed default:** A. It matches TaskPaper and loses nothing; B would forbid ordinary text like `Questions:` in a note.
**Decision (2026-10-04):** the owner accepted the proposed default.
**Note (2026-10-05):** since D-43 every block is one line, so only the live marker update applies; there are no multi-line blocks to split and no split hint.
**Affects:** [specs/editing.md](specs/editing.md).

## Decided by the functional requirements

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
| OQ-13 | Inline formatting | `**bold**` and `` `code` `` (FR-FMT-01); notes add headings, links and fenced code (FR-FMT-03, D-25); since D-45 links work in every block but a divider. |
| OQ-15 | Save model | Autosave (D-15, FR-DOC-02). |
| OQ-19 | Mouse support | Must, secondary to the keyboard (D-16). |
| OQ-20 | Archive @done | No archive command (FR-DOC-04). |
