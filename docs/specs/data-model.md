# Spec: Data model

The in-memory representation behind FR-BLK, FR-TAG, FR-OPS and FR-UI-02. The document is stored as a tree of **lines** (lossless, mirrors the file); **blocks** (FR-BLK) are a layer derived from it. Parsing rules are in [file-format.md](file-format.md).

## 1. Document: a tree of lines

```rust
struct Document {
    items: Arena<ItemId, Item>,  // one item per line; stable ids for the session
    root: ItemId,                // virtual root, never serialized
    format: FileFormat,          // indent unit, line ending, bom, trailing_newline
    path: PathBuf,               // a /tmp path when started without one (FR-DOC-05)
    revision: u64,               // bumped on every committed change
    saved_revision: u64,         // revision last written to disk
}

struct Item {
    parent: Option<ItemId>,
    children: Vec<ItemId>,
    text: String,                // the line body without indentation: "- Buy milk @due(2026-10-05)"
    extra_indent: u16,           // tabs beyond the expected depth (indent jumps); usually 0
    original: Option<LineOrigin>,// original bytes, cleared when text or depth changes
}
```

- `ItemId` is a generational index (for example `slotmap`), so stale ids from undo stacks or panes fail safely instead of pointing at a different item.
- `text` is the single source of truth. Line kind (project/task/note), tags and display text are **derived** from it and cached; the cache is invalidated when `text` changes. Hidden tags (FR-TAG-01) therefore stay in the text, which is how unsupported tags are preserved (FR-DOC-03).
- Item ids are runtime-only. Nothing is written into the file to identify items (OQ-05).

## 2. Derived line attributes

| Attribute | Source |
|---|---|
| `kind` | TaskPaper line type, [file-format.md §4](file-format.md#4-item-type) |
| `tags: Vec<Tag { name, value: Option<String>, span }>` | [file-format.md §5](file-format.md#5-tags); first occurrence per name |
| `display_text` | text without `- `, the project `:` and **all** tags (FR-TAG-01); whitespace left by a removed mid-line tag collapses to one space |
| `depth` (tabs on disk) | `extra_indent` for top-level items; otherwise `parent.depth + 1 + extra_indent`. A file whose first line has two tabs keeps that line's `extra_indent = 2`, so editing its text never changes how it or its children are written. Structural ops reset `extra_indent` where it would break placement ([file-format.md §3.1](file-format.md#31-canonical-outlines)). |

## 3. Blocks

A block is what the user navigates, selects and edits (FR-BLK). Every line of the document is one block, as in TaskPaper (D-43); blocks are derived from the line tree and never stored.

**Special nodes** (FR-SPC-01): a line tagged `@hidden` and its whole subtree form no block, and their tags are not block metadata ([special-nodes.md](special-nodes.md)).

**Type** (FR-BLK-01–03, D-52): TaskPaper's three, and nothing else:

| Type | Rule |
|---|---|
| Task | TaskPaper task line |
| Project | TaskPaper project line |
| Text | anything else, blank lines included |

Two kinds of text block are drawn differently, as attributes rather than types (D-52):

- **Section** (FR-BLK-04): a text block carrying `@section`. On a task or project the tag has no effect: it is preserved, hidden and listed by the Edit Node dialog as unsupported, and the file is never rewritten to move it.
- **Divider** (FR-BLK-07): a text block whose text, tags aside and trimmed, is exactly `---`, so metadata commands on a divider don't change what it is, or `---Title---`, a titled divider whose title is the text between the dashes, trimmed, holding something other than dashes (D-76).

`@code` has no meaning for the type: a line carrying it is whatever the rules above make it, usually text, and the tag stays hidden and preserved like any unsupported tag (D-43). Nor has `@url(...)`: a line carrying it keeps its type, and the tag, hidden and preserved, makes the block's title a link to its address (FR-BLK-06, D-45).

The type is changed with the type converter (FR-BLK-08), which rewrites the line's syntax and drops `@section`, and a text block is made a section or plain text again by adding or removing `@section` ([editing.md §5](editing.md#5-block-commands)). Because the type is derived from the text, text typed in edit mode can also change a line's TaskPaper type (OQ-27).

**Block attributes**: `id` (its line's `ItemId`), `children` (the blocks of its line's children), `type`, `section` and `divider` (above), and metadata read from the line's tags: `done: bool` with `done_date: Option<Date>` (a bare or non-date `@done` counts as done), `start`, `due`, `starred` (`@star`, D-88), `color` (first of `@red @orange @yellow @green @blue @violet`, D-48), `bookmark`, `flag` (`@flag`, D-91), `url: bool` (the line carries `@url`, D-45), and the list of unsupported tags (D-3).

## 4. Edits

All mutations go through a small set of primitive line operations so that undo, autosave, every pane and file-change detection see one stream:

```rust
enum Op {
    SetText { id, before: String, after: String },
    Insert  { parent, index, subtree: Snapshot },   // the snapshot carries the ids
    Remove  { parent, index, subtree: Snapshot },
    Move    { id, from: (ItemId, usize), to: (ItemId, usize) },
    SetExtraIndent { id, before: u16, after: u16 },
}
```

- A user command produces a `Transaction` (a `Vec<Op>` plus an opaque, view-defined restore token for the focus/selection before and after, so `document::model` does not depend on `app::workspace`). The Document layer applies and inverts transactions; the undo/redo history itself lives in the App layer ([architecture.md §1](architecture.md#1-layers-and-modules)). Undo applies the inverse ops in reverse order; redo re-applies (FR-OPS-08).
- Applying a transaction is all or nothing: every op checks that the document is in its `before` state, and if one does not fit, the ops before it are undone and the error returned. Placement repairs (rule 1) are appended to the transaction as `SetExtraIndent` ops, so undo and redo restore them too.
- A removed line keeps its id: `Remove` empties its arena slot instead of freeing it, so undoing the removal puts the same ids back and later steps in the history still find their lines. A new line takes a reserved id that finds nothing until its `Insert` applies.
- Block-level operations (move, indent, delete, convert) are line ops on the block's line, which carry its subtree.
- One edit-mode session on a block is one undo step ([editing.md §2](editing.md#2-edit-mode)). Text changes are applied to the document as the user types, so autosave and every other pane see them immediately (FR-UI-02).
- Metadata helpers (`set_tag`, `remove_tag`, `set_type`, `toggle_done`) build `SetText` ops using the tag rules in [file-format.md §5](file-format.md#5-tags) and TaskPaper's type-change rules:
  - to task: prepend `- `; from task: remove the leading `- `.
  - to project: insert `:` before the trailing tag run; from project: remove that `:`.

## 5. Invariants

- The tree is acyclic; every item except the root has exactly one parent.
- `Move` rejects moving an item into its own subtree.
- `text` never contains a line break.
- A canonical document ([file-format.md §3.1](file-format.md#31-canonical-outlines)) serializes to text that parses back to an equal tree. Placement (rule 1) holds after every transaction; rules 2–4 hold for every saved document.

## 6. Panes and view state

```rust
struct Workspace {
    folds: HashSet<ItemId>,      // collapsed blocks, shared by all panes (D-13)
    panes: Vec<Pane>,            // left to right (FR-UI-02)
    focused: usize,
    sidebar: SidebarState,       // the Navigator window: focus while open, folds (FR-UI-01, D-79)
    secondary: Option<Panel>,    // right sidebar: Shortcuts (D-28, D-66)
}

struct Pane {
    location: Location,          // Block(ItemId) zoom root (FR-NAV-03) or Today (D-7)
    history: Vec<Location>, history_pos: usize, // FR-NAV-04
    show_done: bool,             // FR-DONE-02, per pane
    focus: ItemId,               // focused block
    selection: Option<Selection>,// FR-OPS-01: a contiguous run (anchor..head) or a toggled set
    mode: Mode,                  // Navigate | Edit(EditorState)
    search: Option<Query>,       // Later (FR-SRCH-01)
}
```

The visible blocks of a pane are computed from (document revision, folds, pane) and cached; see [navigation.md](navigation.md). Presentation state (scroll offsets, pane widths, word wrap, theme) belongs to the UI layer, not to this model ([architecture.md §1](architecture.md#1-layers-and-modules)).
