# Spec: Search and dates

Covers FR-SRCH (priority **Later**, D-14) and the date handling that FR-TAG-03 and the Today view need now. The search sections describe the target so that v1 doesn't preclude it; only §4 (dates) is needed for v1.

## 1. Interaction (Later, FR-SRCH-01)

- `/` opens a query bar at the bottom of the focused pane. The pane filters live as you type, debounced by 50 ms. Search is scoped to the pane's zoom block; each pane has its own query (D-13).
- Matching blocks are shown with their ancestors (context rows dimmed) and without non-matching descendants ([navigation.md §1](navigation.md#1-visible-rows-of-a-pane)). The bar shows the match count.
- `Enter` returns to navigate mode with the filter active; results stay editable. A block edited so that it no longer matches stays visible until the query changes, so blocks don't vanish while being edited.
- `Esc` in the bar, or `clear_search` in the pane, removes the filter.
- An invalid query keeps the last valid result and shows the error position in the bar.

## 2. Query syntax (Later, FR-SRCH-02)

Workflowy-style ([reference/workflowy.md §18](../reference/workflowy.md#18-search-full-operator-list)), matched against blocks' display text (tags are hidden, so tag names never match as words):

| Syntax | Meaning |
|---|---|
| `word other` | blocks containing both words (AND), case-insensitive, anywhere in the text |
| `"exact phrase"` | the phrase |
| `-term` | negation of any term, including operators (`-is:done`) |
| `a OR b` | either (upper-case `OR`) |
| `A > B` | blocks matching B that have an ancestor matching A; chainable; spaces around `>` required |
| `is:done`, `is:today`, `is:bookmarked` | metadata flags |
| `is:task`, `is:project`, `is:note`, `is:section`, `is:code`, `is:url`, `is:divider` | block type |
| `due:EXPR`, `start:EXPR` | date comparison; `EXPR` is a date (§4) meaning "on that day", or `<EXPR`, `<=EXPR`, `>EXPR`, `>=EXPR`, or `EXPR..EXPR` (inclusive range). `due:` alone means "has a due date". |
| `color:red` … `color:violet` | color tag |

Precedence: `-` binds tightest, then AND (adjacency), then `OR`, then `>`. Parentheses are not part of Workflowy's syntax and are not supported.

## 3. Bookmarked searches (Later, FR-SRCH-03, D-11)

Saving the current query creates a block `Name @search(query) @bookmark` under a top-level `Searches:` project (created at the end of the file if missing), with `(` and `)` in the query escaped. Both tags are hidden; the sidebar lists it among bookmarks and choosing it runs the query in the focused pane. `@search` values written by TaskPaper 3 use TaskPaper's own query language ([reference/taskpaper.md §4](../reference/taskpaper.md#4-query-search--item-path-language)), which this syntax does not accept; such searches are listed as unsupported in the details popup.

## 4. Dates (Must, FR-TAG-03, D-6)

Dates are stored as `YYYY-MM-DD` with no time (D-6). The date prompt and `due:`/`start:` accept:

- Absolute `YYYY-MM-DD`, plus `MM-DD` and `MM/DD` (this year, or next year if already past for `due`).
- `today`, `tomorrow`, `yesterday`.
- Weekday names and three-letter abbreviations (`fri`, `friday`): the next such day after today; `next fri` the one after that; `last fri` the most recent past one.
- `this|next|last week|month|year`: the first day of that period; weeks start on Monday (ISO, as TaskPaper).
- Offsets `[+|-]N d|w|m|y` and long forms (`+3d`, `2 weeks`, `-1 month`), applied to today or to a preceding date (`fri +1w`). Unlike TaskPaper's grammar (where `m` is minutes), `m` means months here because there are no times.

Parsing is case-insensitive and relative to the local date, using `jiff`. The prompt previews the resolved date live and rejects unparseable input. The local date is read from the injected clock ([architecture.md §2](architecture.md#2-event-loop)), so "today", overdue highlighting and the Today view are testable and update at midnight.

## 5. Tests

- Every row in §2 and every form in §4 has unit tests, including negation of operators, `>` chains, ranges, month-end offsets, ISO week boundaries and year rollover.
- The query parser is fuzzed for panics.
