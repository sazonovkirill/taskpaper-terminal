# Spec: Data model

Covers the in-memory representation behind F-FILE, F-EDIT and F-SEARCH. Parsing rules are in [file-format.md](file-format.md).

## 1. Document

```rust
struct Document {
    items: Arena<ItemId, Item>,  // stable ids for the session
    root: ItemId,                // virtual root, never serialized
    format: FileFormat,          // indent unit, line ending, bom, trailing_newline
    path: Option<PathBuf>,
    revision: u64,               // bumped on every committed change
    saved_revision: u64,         // revision last written to disk
}

struct Item {
    parent: Option<ItemId>,
    children: Vec<ItemId>,
    text: String,                // the line body without indentation: "- Buy milk @due(2026-10-05)"
    extra_indent: u16,           // tabs beyond parent depth + 1 (indent jumps); usually 0
    original: Option<LineOrigin>,// original bytes, cleared when text or depth changes
}
```

- `ItemId` is a generational index (for example `slotmap`), so stale ids from undo stacks or views fail safely instead of pointing at a different item.
- `text` is the single source of truth. `kind` (project/task/note), `content_text` and `tags` are **derived** from it and cached; the cache is invalidated when `text` changes. This mirrors TaskPaper, where attributes and text are kept in sync, and avoids two representations drifting apart.
- Item ids are runtime-only. Nothing is written into the file to identify items (see OQ-05 for persisted view state).

## 2. Derived attributes

| Attribute | Source |
|---|---|
| `kind` | [file-format.md §4](file-format.md#4-item-type) |
| `content_text` | text without `- `, project `:` and trailing tags |
| `tags: Vec<Tag { name, value: Option<String>, span }>` | [file-format.md §5](file-format.md#5-tags); first occurrence per name |
| `depth` (tabs on disk) | `extra_indent` for top-level items; otherwise `parent.depth + 1 + extra_indent`. A file whose first line has two tabs keeps that line's `extra_indent = 2`, so editing its text never changes how it or its children are written. Structural ops reset `extra_indent` where it would break placement ([file-format.md §3.1](file-format.md#31-canonical-outlines)). |
| `is_done` | has tag `done` |

## 3. Edits

All mutations go through a small set of primitive operations so that undo, autosave, view updates and file-change detection see one stream:

```rust
enum Op {
    SetText { id, before: String, after: String },
    Insert  { id, parent, index, subtree: Snapshot },
    Remove  { id, parent, index, subtree: Snapshot },
    Move    { id, from: (ItemId, usize), to: (ItemId, usize) },
    SetExtraIndent { id, before: u16, after: u16 },
}
```

- A user command produces a `Transaction` (a `Vec<Op>` plus an opaque, view-defined restore token for the cursor/selection before and after, so `model` does not depend on `view`). Undo applies the inverse ops in reverse order; redo re-applies.
- Typing into one item coalesces into a single transaction until the cursor leaves the item, a structural command runs, or 1 s passes without input.
- Higher-level helpers (`set_tag`, `remove_tag`, `set_kind`, `toggle_done`) build `SetText` ops using the rules in [file-format.md §5](file-format.md#5-tags) and TaskPaper's type-change rules:
  - to task: prepend `- `; from task: remove the leading `- `.
  - to project: insert `:` before the trailing tag run; from project: remove that `:`.

## 4. Invariants

- The tree is acyclic; every item except the root has exactly one parent.
- `Move` rejects moving an item into its own subtree.
- `text` never contains a line break. Pasting text with line breaks becomes several items ([editing.md](editing.md)).
- A canonical document ([file-format.md §3.1](file-format.md#31-canonical-outlines)) serializes to text that parses back to an equal tree. Placement (rule 1) holds after every transaction; rules 2–4 hold for every saved document.

## 5. Views

A view is state over the document, not part of it:

```rust
struct View {
    zoom: ItemId,                 // root of what is shown (TaskPaper "hoist/focus")
    collapsed: HashSet<ItemId>,   // explicitly collapsed items
    filter: Option<Query>,        // compiled query; see search.md
    hide_done: bool,
    cursor: Cursor,               // item + grapheme offset
    selection: Option<Selection>, // anchor and head items, same parent
    history: Vec<ItemId>,         // zoom back/forward stack
}
```

The visible row list is computed from (document revision, view) and cached; see [navigation.md](navigation.md) for the rules.
