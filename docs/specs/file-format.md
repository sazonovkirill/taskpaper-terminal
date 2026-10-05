# Spec: TaskPaper parsing and serialization

Covers F-FILE-02, F-FILE-03, F-FILE-08. Behavior follows the current TaskPaper reference parser; the evidence and edge cases are in [../reference/taskpaper.md](../reference/taskpaper.md) (§1–§3, §11).

## 1. Input decoding

1. Read the file as bytes. If it starts with a UTF-8 BOM, strip it and remember `bom = true`.
2. Decode as UTF-8. On failure, refuse to open (F-FILE-07); never write a lossy decode back.
3. Detect the dominant line ending among `\r\n`, `\n`, `\r` and remember it. Split lines on `\r\n`, `\r`, `\n`, `\f`, U+2029 and U+2B7F. U+2028 stays inside a line.
4. If the text ends with a line break, remember `trailing_newline = true`. (TaskPaper models this as a final empty item; we keep it as a file property instead so that it never shows as a stray blank row.)

## 2. Indentation

TaskPaper's canonical indent is one tab per level. Space-indented files are normalized on read.

1. For multi-line input, `spaces_per_level` = the smallest run of leading spaces on any line (runs ending at a tab or at the first non-whitespace character), as TaskPaper does.
2. Replace the run of `spaces_per_level`-space groups at the very start of the line with one tab per group. Then, if the remaining leading whitespace still contains a space, truncate it at that first space, dropping everything after it including any tabs (`"  B"` → one tab; `"\t   \tC"` with 2 spaces per level → one tab).
3. `depth = number of leading tabs`; `body = rest of line`.
4. Remember the detected indent unit (`Tab` or `Spaces(n)`) for writing (§6).

Single-line input (a one-line file or a single-line paste) is not normalized, matching TaskPaper: its leading whitespace stays in the text, so `  - Milk` is a note.

In multi-line input, whatever leading whitespace is not consumed as indentation is dropped, so item text never starts with whitespace. A one-line document whose text starts with whitespace is canonical while it has one line; once it gains a second line, rule 4 of §3.1 applies.

> Known TaskPaper quirk: one stray line indented with fewer spaces than the rest skews every depth. We reproduce it for compatibility, and log a warning in the status line when the minimum run occurs on only one line.

## 3. Hierarchy

- A line deeper than the previous line is that line's child; equal depth is a sibling; shallower pops back to the nearest ancestor with a smaller depth.
- **Indent jumps** (0 → 3 tabs) are legal. The child keeps its absolute depth so it re-serializes with the same number of tabs. The model stores `extra_indent` on such items (see [data-model.md](data-model.md)).
- **Blank lines** (empty or whitespace-only) are items of type note with empty text. Their depth is `max(own depth, depth of the next non-blank line)`. Blank lines at the end of the file are top-level. Blank lines never end a project.

### 3.1 Canonical outlines

An outline is **canonical** when writing it and parsing the text back yields an equal tree. Given the rules in §2 and §3, that holds exactly when all of these are true (OQ-22):

1. **Placement:** for every item, the nearest preceding item in document order with a smaller absolute depth is its parent (or there is none, for a top-level item). This is what the hierarchy rule reconstructs.
2. **No empty parents:** no item with empty text has children (the blank-line rule would lift it to its first child's depth).
3. **No trailing indented blanks:** no empty item at depth > 0 is followed only by empty items up to the end of the file (trailing blank lines are read as top-level).
4. **No leading whitespace:** in a document of more than one line, no item text starts with whitespace (it would be read as indentation).

The editor keeps documents canonical (OQ-22 default A):

- **Placement is maintained on every transaction.** After each structural op, walk the document in order and set `extra_indent = 0` on any item violating rule 1 (a reset can only affect later items, so one forward pass suffices). That changes only its indentation on disk, never text or structure: a top-level item then has depth 0, and a child then has `parent.depth + 1`, for which its parent is always the nearest shallower preceding item. For example, swapping top-level siblings parsed from `\t\tA\nB` resets A's `extra_indent`, and the file becomes `B\nA`.
- **Rules 2–4 are repaired outside the block being edited.** Creating a block (FR-MODE-04), `group` (FR-OPS-04) and clearing a block's text can create these shapes transiently, because the block is in edit mode and the user is about to type. Repairs are one undoable transaction with a status message: leading whitespace is trimmed (rule 4), an empty line with children gets the placeholder text `…` (rule 2), and trailing indented empty lines are moved to top level (rule 3). When edit mode ends on a block, rules 2 and 4 are repaired for its lines. Rule 3 depends on the lines that follow, so it is checked only when saving. Autosave waits while a violation involves the block in edit mode: it has an empty line with children, a line starting with whitespace, or an empty line in the trailing run (typing may make the violation disappear, as when creating a new last block under the last project). Every other save trigger in [file-io.md §2](file-io.md#2-saving) (Save As, quit, `SIGTERM`/`SIGHUP`) ends edit mode first, which repairs the block, then saves.

## 4. Item type

Computed from the body with the trailing tag run removed (`content`):

1. **Task** if `content` matches `^-\s` (dash + any whitespace). Checked first, so `- foo:` is a task.
2. **Project** if `content` ends with `:`. Trailing whitespace is not trimmed unless a trailing tag run absorbed it, so `Inbox:␠␠` is a note but `Inbox: @a␠␠` is a project.
3. **Note** otherwise.

`* ` and `+ ` lines are notes (the current app dropped them as task markers).

Display text: the body without the `- ` prefix, without the project `:` and without every tag, as [data-model.md §2](data-model.md#2-derived-line-attributes) defines it (FR-TAG-01). TaskPaper's own `content_text` drops only the trailing tag run.

## 5. Tags

Grammar (character classes are the XML name classes without `:`):

```
tag       = (line-start | whitespace) "@" name [ "(" value ")" ]   followed by whitespace or line-end
name      = { name-char }            ; Unicode letters, digits, "_", "-", ".", U+00B7, combining marks
value     = { "\(" | "\)" | any char except "(" ")" }
trailing  = tag { tag } { whitespace } line-end
```

Rules:
- `foo@bar.com`, `Proj:@x` and `@due(2026)x` contain no tags.
- Empty names (lone `@`) are not tags.
- `@id`, `@text`, `@type` are reserved and are plain text.
- Duplicate names on one line: the first wins; later ones are text.
- Only `\(` and `\)` are unescaped in values; other backslashes are literal. `@a()` has value `""`, the same as `@a`.
- Comma-separated values (`@a(1, 2)`) are one string; queries split them with the `[l]` modifier.
- Tag name lookup is case-sensitive.

The model keeps the line text as the single source of truth and derives tags by parsing it (see [data-model.md](data-model.md)). Tag edits are text edits:
- **Set**: replace the existing occurrence's value in place; otherwise append ` @name(value)` (no leading space if the body already ends in whitespace). An empty value writes `@name`. `(` and `)` in values are escaped.
- **Remove**: delete the occurrence plus its leading whitespace.

## 6. Serialization

Each item in document order is written as indent + body; lines are joined with the line ending, and a final line ending is added only when `trailing_newline` is set or the last line is written empty, which would otherwise not be read back.

- **Indent**: `depth` units of the remembered indent unit (OQ-14 default "preserve"). New files use tabs.
- **Line ending**: the remembered one; `\n` for new files.
- **BOM** and **trailing newline**: as remembered; new files get no BOM and a trailing newline.
- Items whose text and absolute depth did not change are written from their original line bytes, so an unedited file round-trips byte-for-byte even if it mixed line endings or had leftover spaces (F-FILE-03). "Did not change" means that the original line, read again where the item now is, gives the same text and depth: a blank line also needs the line after it to give it the same depth (§3). An item keeps the line break that followed its original line; new lines, and a former last line that had none, get the remembered line ending.
- In a space-indented file the original lines' depths were read with the file's smallest run of leading spaces (§2). If the lines to be written would have a different smallest run, for example because the only line with the shortest run was deleted, every line is re-indented with the remembered unit instead.

If OQ-14 is decided as "normalize", indent becomes tabs and line endings `\n`; nothing else changes.

## 7. Tests

- Golden corpus under `app/tests/fixtures/`: TaskPaper's sample files, the guide's examples, and every edge case in reference §11, each with expected item tree JSON (`insta` snapshots).
- Property test (`proptest`): for any generated outline satisfying §3.1 (generators must produce indent jumps and empty items), `parse(serialize(doc)) == doc`; for any byte string that decodes as UTF-8, `serialize(parse(s)) == s` when unedited.
