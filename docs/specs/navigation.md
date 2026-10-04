# Spec: Display and navigation

Covers F-VIEW and F-NAV. Key bindings are in [keybindings.md](keybindings.md); layout and styling in [ui.md](ui.md).

## 1. Visible rows

Given the document and a [View](data-model.md#5-views), the visible rows are produced by a depth-first walk from the zoomed item's children:

1. **No filter:** show every item; don't descend into collapsed items.
2. **Hide done** (F-VIEW-08): skip items tagged `@done` and their whole subtrees. Equivalent to TaskPaper's `not @done except @done//*`.
3. **Filter active** ([search.md](search.md)): show each matching item plus all its ancestors up to the zoom root. Descendants are shown only if they match too. An ancestor shown only for context is drawn dimmed. Collapse state is ignored while filtering (matches are always revealed), but is kept for when the filter clears.

Each row carries: item id, display depth (relative to the zoom root), whether it has hidden children (collapsed or filtered out), and whether it is a context-only ancestor.

The row list is recomputed only when the document revision or the view changes, and is computed in O(visible + matched ancestors).

## 2. Cursor

- The cursor is on one item and, when editing text, at a grapheme offset in it. Up/Down move to the previous/next visible row and keep the preferred column.
- If the cursor item becomes invisible (collapsed ancestor, filtered out, deleted), it moves to the nearest visible item: next sibling, else previous, else parent.
- Soft-wrapped items take several screen rows; Up/Down move by screen row inside a wrapped item before moving to the adjacent item.

## 3. Folding (F-NAV-02)

| Command | Effect |
|---|---|
| `expand` / `collapse` | One item, one level. Collapsing a leaf or an already collapsed item moves the cursor to its parent, so repeated presses walk up the tree. |
| `toggle_fold` | Expand if collapsed, else collapse. |
| `expand_all_under` / `collapse_all_under` | The cursor item's entire subtree. |
| `expand_to_level N` | Whole view: items at display depth < N expanded, the rest collapsed. `0` collapses everything. |

Fold state belongs to the view and is persisted per OQ-05.

## 4. Zoom (F-NAV-03, F-NAV-04)

- `zoom_in`: the cursor item becomes the view root; its children are shown at depth 0 and the item's own text is shown as the page title above them (editable). Zooming into a leaf is allowed and shows an empty page with the cursor on the title.
- `zoom_out`: zoom to the parent and put the cursor on the item you were zoomed into.
- `go_home`: zoom to the document root.
- Every zoom change pushes onto the view's history; `back` / `forward` walk it, skipping deleted items.
- The breadcrumb bar lists ancestors of the zoom root, truncated from the left with `…` when too wide.
- Filtering is scoped to the zoom root (F-SEARCH-04).

## 5. Go to anything (F-NAV-06)

A fuzzy picker (`nucleo-matcher`) over:
- every item's `content_text`, shown with its breadcrumb path; projects and saved searches rank first;
- saved searches (`@search` items, [search.md §5](search.md#5-saved-searches)).

Choosing an item zooms to its parent and places the cursor on it (the "reveal" behavior); `Alt+Enter` zooms into the item itself. Choosing a saved search applies its filter.

## 6. Structural motions (F-NAV-05)

`parent`, `first_child`, `last_child`, `prev_sibling`, `next_sibling`, `first_visible`, `last_visible`, `page_up`, `page_down`.

## 7. Sidebar (F-VIEW-10)

Optional left panel with three sections, as in TaskPaper:
- **Projects**: all projects as a tree, nested by project ancestry (non-project items in between are skipped). Enter zooms.
- **Searches**: saved searches. Enter applies the filter.
- **Tags**: all tag names, each expandable to its distinct values (comma lists split). Enter filters `@name` or `@name contains[l] "value"`.
