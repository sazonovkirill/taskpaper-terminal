# Spec: Search and filtering

Covers F-SEARCH and the date handling used by F-TAG-03. The query language is TaskPaper's item-path language, implemented to match the current reference parser; full grammar, semantics and verified examples are in [../reference/taskpaper.md §4](../reference/taskpaper.md#4-query-search--item-path-language).

## 1. Interaction

- `search` opens the query bar at the bottom. The filter applies live as you type, debounced by 50 ms.
- While the query is invalid, the bar is drawn in the error color with the parser's message and position; the last valid result stays on screen (F-SEARCH-03).
- `Enter` moves focus back to the outline with the filter still active; the outline is fully editable. Edited items that stop matching stay visible until the filter is re-run (on the next query change or `refresh`), so items don't vanish while being typed in.
- `Esc` in the query bar, or `clear_filter` in the outline, removes the filter.
- `filter_by_tag` on a tag under the cursor sets the filter to `@name`, or `@name = "value"` / `@name contains[l] "value"` when clicking a value, as TaskPaper does.
- `find` (F-SEARCH-07) is a separate, non-filtering text find: highlight matches of a plain substring and jump with `n`/`N`-style next/previous.

## 2. Language (F-SEARCH-02)

Implement the complete grammar of the shipping TaskPaper app, not the older birch-outline one:

- Predicates: `@attr relation[modifier] value`, any part optional, defaulting to `@text contains[i]`.
- Bare words form one phrase (`socks shoes` = `@text contains "socks shoes"`).
- Relations: `=`, `!=`, `<`, `>`, `<=`, `>=`, `contains`, `beginswith`, `endswith`, `matches`. (`like` is a syntax error.)
- Modifiers: `[i]` (default), `[s]`, `[n]`, `[d]`, `[b]`, each optionally followed by `l` for comma lists (`[l]`, `[nl]`, `[dl]`).
- Boolean: `not` > `and` > `or`, parentheses.
- Type shortcuts: `project`, `task`, `note`.
- Paths: `/` (absolute), `//`, `///`, `..`, `.`, and all named axes (`ancestor-or-self::` … `self::`).
- Set operations: `union`, `intersect`, `except` with precedence `intersect` > `except` > `union`, right-associative.
- Slices: `[i]`, `[i:j]`, `[i:]`, negative indices. `[:j]` and `[:]` are accepted too (documented in the guide but rejected by the app); this is a deliberate superset.
- `count(path)` as a value.
- Attributes: `@text` (full line body), `@type`, `@id` (session id), any tag name.
- `matches` uses the `regex` crate. With the default `[i]` modifier it compiles case-insensitively instead of lowercasing the pattern (TaskPaper lowercases the pattern source, which breaks classes like `\D`); this is a deliberate fix.

Implementation: hand-written lexer + recursive-descent parser producing an AST with spans for error messages; evaluator over the document arena. The context item and root are the zoom root.

### Workflowy shorthand (F-SEARCH-06, OQ-07 C)

If enabled, a query is first tried as Workflowy syntax and translated to an item path:

| Workflowy | Item path |
|---|---|
| `word1 word2` | `word1 and word2` |
| `"exact phrase"` | `"exact phrase"` |
| `-word`, `-#tag`, `-is:complete` | `not …` |
| `a OR b` | `a or b` |
| `A > B` | `A//B` |
| `#tag`, `@tag` | `@tag` |
| `is:complete` / `is:todo` | `@done` / `task` |
| `is:project` | `project` |
| `has:note` | items with a child note (encoding TBD) |
| `has:children` | items with any child (encoding TBD) |
| `completed:7d` | `@done >=[d] "today -7 days"` |

The bar shows the translated item path beneath the input so users learn the native syntax. Detection rule, translation edge cases and the `has:` encodings are to be settled when OQ-07 is decided.

## 3. Results

Matching items are shown with their ancestors (context rows dimmed) and without non-matching descendants ([navigation.md §1](navigation.md#1-visible-rows)). The status line shows the match count. `select_matched` selects all matches for bulk commands (only valid when the matches are siblings; otherwise it applies bulk commands to each match individually).

## 4. Dates

Implement TaskPaper's date grammar (reference §6) for `[d]` comparisons and for `set_due`/`set_start`:

- Absolute `YYYY`, `YYYY-MM`, `YYYY-MM-DD`, with optional time `HH:mm[:ss]` or `H[:mm] am|pm`.
- Relative `now`, `today`, `yesterday`, `tomorrow`, `[this|next|last] <weekday|month|unit>`, ISO weeks (Monday start).
- Durations `[+|-]N unit`, repeatable, units `ms s m h d w o q y` and long forms; `m` = minutes, `o` = months.
- Unparseable values are null and never satisfy a `[d]` comparison.
- Relative dates are evaluated at query time against the local time zone; re-running the filter after midnight updates results.

Use `jiff` for time zones and arithmetic. Format stored dates as `YYYY-MM-DD` when the time is midnight, else `YYYY-MM-DD HH:mm`.

## 5. Saved searches

- Any item with a `@search(query)` tag is a saved search; its name is the item's `content_text`. Its type doesn't matter (F-SEARCH-05).
- `save_search` creates `Name @search(<current query>)` under a top-level `Searches:` project (created at the end if missing), escaping `(` and `)` in the query.
- Saved searches appear in Go to anything and in the sidebar.
- Built-in searches available even without `@search` items: **Not Done** `not @done except @done//*`, **Today** `@today union @due <[d] tomorrow`, **Overdue** `@due <[d] today and not @done`.

## 6. Tests

- Every query in the TaskPaper guide and reference §4.9 parses and returns the expected items on fixture documents.
- Precedence, defaults, phrase handling, slices, list modifiers and the documented deviations each have dedicated cases.
- Fuzz the parser (`cargo fuzz` or `proptest`) for panics.
