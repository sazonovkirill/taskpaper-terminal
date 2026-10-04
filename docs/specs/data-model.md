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

A block is what the user navigates, selects and edits (FR-BLK). Blocks are computed from the line tree and cached per document revision; they are never stored.

**Grouping** (D-2, FR-BLK-03): a block is one line, except that a maximal run of consecutive sibling **plain note lines** forms one note block. A plain note line is a `note`-kind line that is not a divider and carries none of `@section`, `@url`. The run:
- includes blank lines;
- ends after a line that has children (that line's children are the block's children);
- ends before a divider, section or URL line;
- ends before a line carrying `@code`, which starts a new run.

**Type** (FR-BLK-01–07), checked in this order:

| Type | Rule |
|---|---|
| Divider | note line whose text without tags is exactly `---`, so metadata commands on a divider don't change its type |
| Section | line with `@section` |
| URL | line with `@url(...)` |
| Code | note block whose first line has `@code` |
| Task | TaskPaper task line |
| Project | TaskPaper project line |
| Note | anything else |

The type is changed with the type converter (FR-BLK-08), which rewrites the syntax and type tags of every line in the block ([editing.md §5](editing.md#5-block-commands)). Because the type is derived from the text, text typed in edit mode can also change a line's TaskPaper type (OQ-27).

**Block attributes**: `id` (its first line's `ItemId`), `lines`, `children` (the children of the last line), `type`, and metadata read from the first line's tags: `done: bool` with `done_date: Option<Date>` (a bare or non-date `@done` counts as done), `start`, `due`, `today`, `color` (first of `@red @orange @yellow @green @blue @violet`, D-5), `bookmark`, `url`, and the list of unsupported tags (D-3). For a note block, metadata tags may sit on any of its lines; the first occurrence in the block wins, and commands write metadata to the first line.

## 4. Edits

All mutations go through a small set of primitive line operations so that undo, autosave, every pane and file-change detection see one stream:

```rust
enum Op {
    SetText { id, before: String, after: String },
    Insert  { id, parent, index, subtree: Snapshot },
    Remove  { id, parent, index, subtree: Snapshot },
    Move    { id, from: (ItemId, usize), to: (ItemId, usize) },
    SetExtraIndent { id, before: u16, after: u16 },
}
```

- A user command produces a `Transaction` (a `Vec<Op>` plus an opaque, view-defined restore token for the focus/selection before and after, so `model` does not depend on `view`). Undo applies the inverse ops in reverse order; redo re-applies (FR-OPS-08).
- Block-level operations (move, indent, delete, convert) expand to line ops over all lines of the block plus its subtree.
- One edit-mode session on a block is one undo step ([editing.md §2](editing.md#2-edit-mode)). Text changes are applied to the document as the user types, so autosave and every other pane see them immediately (FR-UI-02).
- Metadata helpers (`set_tag`, `remove_tag`, `set_type`, `toggle_done`) build `SetText` ops using the tag rules in [file-format.md §5](file-format.md#5-tags) and TaskPaper's type-change rules:
  - to task: prepend `- `; from task: remove the leading `- `.
  - to project: insert `:` before the trailing tag run; from project: remove that `:`.

## 5. Invariants

- The tree is acyclic; every item except the root has exactly one parent.
- `Move` rejects moving an item into its own subtree.
- `text` never contains a line break. A multi-line note block is several items.
- A canonical document ([file-format.md §3.1](file-format.md#31-canonical-outlines)) serializes to text that parses back to an equal tree. Placement (rule 1) holds after every transaction; rules 2–4 hold for every saved document.

## 6. Panes and view state

```rust
struct Workspace {
    folds: HashSet<ItemId>,      // collapsed blocks, shared by all panes (D-13)
    panes: Vec<Pane>,            // left to right (FR-UI-02)
    focused: usize,
    sidebar: SidebarState,       // visible, focus, panel = Sidebar | Shortcuts (D-12)
    wrap: bool,                  // FR-UI-05
}

struct Pane {
    location: Location,          // Block(ItemId) zoom root (FR-NAV-03) or Today (D-7)
    history: Vec<Location>, history_pos: usize, // FR-NAV-04
    show_done: bool,             // FR-DONE-02, per pane
    focus: ItemId,               // focused block
    selection: Option<Selection>,// FR-OPS-01: a contiguous run (anchor..head) or a toggled set
    mode: Mode,                  // Navigate | Edit(EditorState)
    scroll: usize,
    width: u16,                  // FR-UI-03 resizing
    search: Option<Query>,       // Later (FR-SRCH-01)
}
```

The visible rows of a pane are computed from (document revision, folds, pane) and cached; see [navigation.md](navigation.md).
