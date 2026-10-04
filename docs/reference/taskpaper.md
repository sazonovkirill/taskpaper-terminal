# TaskPaper reference

Snapshot of the TaskPaper plain-text format and the TaskPaper 3 app's behavior as of 2026-10-04. This is the compatibility target for taskpaper-terminal's parser, serializer, query engine and commands. Content is paraphrased; check the live sources before relying on a detail.

- User guide: https://guide.taskpaper.com/
- Getting started (format basics): https://guide.taskpaper.com/getting-started/
- Searches (query language): https://guide.taskpaper.com/reference/searches/
- Dates: https://guide.taskpaper.com/reference/dates/
- Product site: https://www.taskpaper.com/
- Reference parser, current: https://github.com/jessegrosjean/TaskPaper (`BirchOutline/birch-outline.js/`)
- Reference parser, 2017 standalone (stale): https://github.com/jessegrosjean/birch-outline
- Support forum: https://support.hogbaysoftware.com/

The author states there is no formal format definition and points to the model layer's code as the reference parser (https://support.hogbaysoftware.com/t/where-is-taskpapers-file-format-defined/2098). Where the guide and the shipping code disagree, this document follows the code and says so.

Anything not read directly in a source is tagged **(inferred)**.

## 0. Sources and their authority

| Tag | Source | Notes |
|---|---|---|
| [GUIDE] | https://guide.taskpaper.com/ (GitBook, JS-rendered) | Official user guide. Loose; sometimes disagrees with the code. |
| [BIRCH] | https://github.com/jessegrosjean/birch-outline (HEAD `8dc71b3`, 2017-05-08) | Open-source model layer ("same code that TaskPaper uses"), but **stale**. |
| [APP] | https://github.com/jessegrosjean/TaskPaper (HEAD `0782360`, 2026-07-06) | "Shared source" for the shipping app. It vendors a **newer** birch-outline at `BirchOutline/birch-outline.js/` plus `birch-editor.js` and the Swift app. **Use this as the ground truth.** |
| [FORUM-*] | support.hogbaysoftware.com threads (read via Discourse `.json` endpoints) | Author (Jesse Grosjean) answers. |

From the forum's file-format FAQ entry (https://support.hogbaysoftware.com/t/where-is-taskpapers-file-format-defined/2098): the author says there is no formal format definition. He points developers at the open-source model layer as the reference parser instead.

From https://support.hogbaysoftware.com/t/whats-new-in-taskpaper-3/950: TaskPaper 3 added "no new features in the underlying file format" compared with TP2. Tabs are still part of the file. The TP3 editor just hides them as indentation.

**[APP] vs [BIRCH] differences that matter for a parser** (from `diff -r` of the two `src/` trees):
1. Task marker: [APP] accepts only `- ` (`/^\-\s/`). [BIRCH] also accepted `+ ` and `* ` (`/^([\-+*])\s/`).
2. Query modifiers: [APP] grammar is `[` (s|i|n|b|d)? l? `]`, so it has list (`l`) and boolean (`b`). [BIRCH] only supports `s|i|n|d`.
3. Query relation `like`: present in the [BIRCH] grammar but never evaluated. [APP] removed it entirely, so `like` is a syntax error.
4. Paragraph breaks: [APP] normalizes `\r\n`, `\r`, `\n`, `\f`, U+2B7F and U+2029 to `\n` before parsing. U+2028 (line separator) is deliberately kept inside an item.
5. Blank-line indentation: the rule changed slightly; see §2.4.
6. Date parser: weekdays and "this/next/last week" use ISO weeks (Monday start). [BIRCH] used locale weeks and had an `isoweek` unit, which [APP] removed.
7. Tag-value click links: [APP] splits comma values. Clicking one value filters with `@tag contains[l] "v"`; a single value filters with `@tag = "v"`.

Source: [APP] `BirchOutline/birch-outline.js/src/**` vs [BIRCH] `src/**`.

---

## 1. File level

Sources: https://github.com/jessegrosjean/TaskPaper/blob/master/BirchEditor/BirchEditor.swift/BirchEditor/OutlineDocument.swift ,
https://github.com/jessegrosjean/TaskPaper/blob/master/BirchOutline/birch-outline.js/src/item-serializer.coffee ,
https://github.com/jessegrosjean/TaskPaper/blob/master/BirchOutline/birch-outline.js/src/serializations/text.coffee ,
https://support.hogbaysoftware.com/t/how-can-i-edit-my-taskpaper-files-on-windows-ios-or-android/1114

- **Extension / UTI**: `.taskpaper`. The UTI is `com.taskpaper.text`, mapped to the TaskPaper serializer. The FAQ calls ".taskpaper" a plain-text format.
- **Encoding**: the app reads and writes **UTF-8** only. Reading fails with an error "Failed to decode string as utf8". There is no BOM handling in the code (inferred: a BOM would end up as a character in the first item).
- **Line endings on read**: all of `\r\n`, `\r`, `\n`, `\f`, U+2B7F and U+2029 become `\n` (`ItemSerializer.paragraphBreakRegex`). U+2028 stays inside an item's text.
- **Line endings on write**: the app always writes `\n` (`lines.join('\n')`). Original CRLF is **not preserved** (inferred from code).
- **Trailing newline**: each item is one line, and lines are joined by `\n`. A file ending in `\n` therefore has a final **empty item**, and serializing it writes the trailing `\n` back. Round-trip is stable (inferred from the split/join logic).
- **No in-file metadata.** There is no header or front matter. Per-document editor state is stored in **macOS extended attributes**, not in the text:
  - `com.taskpaper.outline.metadata`: JSON of `outline.metadata`.
  - `com.taskpaper.editor.defaultRestorableState.<username>`: JSON `{hoistedID, focusedID, collapsedItemIDs, itemPathFilter}`. IDs are `branchContentID`s.
  - Folding, focus and filter state therefore live outside the file.
- **Item IDs** (`@id` in queries) are runtime-generated short IDs. They are not persisted in the text (inferred: the serializer writes only indentation plus the body string).
- **Saved searches and tag lists** can be embedded in the document as ordinary items with `@search(...)` tags; see §5.

---

## 2. Lines → items: exact parsing rules

Sources: https://github.com/jessegrosjean/TaskPaper/blob/master/BirchOutline/birch-outline.js/src/serializations/text.coffee ,
https://github.com/jessegrosjean/TaskPaper/blob/master/BirchOutline/birch-outline.js/src/serializations/taskpaper/index.coffee ,
https://github.com/jessegrosjean/TaskPaper/blob/master/BirchOutline/birch-outline.js/src/serializations/taskpaper/types.coffee ,
https://github.com/jessegrosjean/TaskPaper/blob/master/BirchOutline/birch-outline.js/src/serializations/taskpaper/change-delegate.coffee ,
https://github.com/jessegrosjean/TaskPaper/blob/master/BirchOutline/birch-outline.js/src/item.coffee ,
https://guide.taskpaper.com/getting-started/

### 2.1 Every line is exactly one item
The file is split on `\n` after paragraph-break normalization. Each line becomes one item, including empty lines. Hierarchy comes from indentation only.

### 2.2 Indentation: tabs are canonical, spaces are normalized
`_parseLinesAndNormalizeIndentation`:
1. **If the text has more than one line**, compute `spacesPerTab`. This is the minimum, over all lines, of each run of leading spaces. A run is counted when it ends at a tab or at the first non-whitespace character; runs before a tab count too. Lines made only of spaces also count (inferred from the loop: their space run is counted at end of line).
2. If any leading spaces were found, every line's leading `( {spacesPerTab})+` is replaced by one tab per group.
3. Next, any leading-whitespace run that still contains a space is truncated at its first space. Leftover odd spaces are dropped; tabs before them are kept.
4. Single-line input (e.g. a paste) is **not** normalized.
5. Then for each line, `indent = (number of leading \t) + 1` and `bodyString = line without the leading tabs`.
- Example: a file indented with 4 spaces (like the shipped `searches.taskpaper`, whose search items are indented by 4 spaces) parses as one level per 4 spaces.
- **Edge (inferred)**: one stray line indented by 2 spaces in a 4-space file makes `spacesPerTab = 2`, which doubles everyone else's depth. A whitespace-only line can also lower `spacesPerTab`.
- **Edge (inferred)**: step 3 uses `/^\s+/gm`, and `\s` matches `\n`. A run of spaces at the start of a line followed by a newline and then tabs could therefore be truncated across the line break. That would merge a whitespace-only line into the next one. This is a bug-level corner case.
- The serializer writes `'\t' × (depth − baseDepth)` + bodyString. **Output is always tab-indented.** Space indentation is not preserved.

### 2.3 Hierarchy from indent
`Item.buildItemHiearchy`: a line whose indent is greater than the previous line's becomes that line's child. Equal indent makes a sibling. A smaller indent pops back to the nearest ancestor whose depth is lower.
- **Indent jumps** are allowed. A line with 3 tabs under a line with 0 tabs becomes a child whose `indent` attribute is 3 relative to its parent (`siblingsIndent = siblingsDepth − parentDepth`). Its depth stays 4, so it re-serializes with 3 tabs. The jump survives a round trip (inferred). The stylesheet/Item API exposes `indent` as the "visual indent of Item relative to parent" (https://guide.taskpaper.com/reference/scripting/Item.html).
- An over-indented first line simply becomes a root with a larger indent (inferred).

### 2.4 Blank / whitespace-only lines
A line matching `/^\s*$/` is still an item (an empty note), but its indent is adjusted:
- [APP]: every run of blank lines gets `indent = max(own indent, indent of the next non-blank line)`.
- Blank lines at the end of the file get `indent = 1` (top level).
- [BIRCH] instead overwrote the indent with the next line's indent unconditionally.
- Effect: a blank line between two children of a project becomes a child of that project. A blank line before a top-level project becomes a top-level item. So **blank lines do not end a project**; only indentation does.
- In the app these empty items have type `note`, because an empty string matches neither the task nor the project regex.

### 2.5 Item type (project / task / note)
Type is computed from the **content string**: the body with the **trailing tag run removed** (§3.4). Leading tabs are already stripped, so `bodyString` is used.

```
taskRegex    = /^\-\s/      # [APP]  (BIRCH: /^([\-+*])\s/)
projectRegex = /:$/
type = task     if content matches taskRegex     (checked FIRST)
     = project  else if content matches projectRegex
     = note     otherwise
```

Verified results (regex harness reproducing the [APP] code):

| line body | type | why |
|---|---|---|
| `- Milk` | task | dash + whitespace |
| `-\tMilk` | task | `\s` includes tab |
| `- ` | task | empty task |
| `-` / `-Milk` | note | no whitespace after dash |
| `* Milk` / `+ Milk` | **note** in [APP] (task in old [BIRCH]) | |
| `- Buy: milk:` | task | task wins over project |
| `Groceries:` | project | ends with colon |
| `Groceries: @home @due(2016-01-01)` | project | trailing tags stripped first |
| `Groceries:@home` | note | `@home` is not a tag (needs leading whitespace), so content ends in `e` |
| `Groceries:   ` (trailing spaces, no tags) | **note** | regex is `:$` and trailing whitespace is not stripped unless tags are present |
| `Groceries: @home   ` | project | the trailing-tag regex absorbs trailing whitespace |
| `a @x(1) b:` | project | the tag is not trailing; content still ends with `:` |
| `Proj: @a:` | project | `@a:` is not a tag (`:` is not a name char and the lookahead fails), so content ends `:` |
| `  - Milk` (leading spaces, single-line paste) | note | no normalization for single lines (inferred) |

- **Getting Started** phrasing: projects are lines "ending with a colon", tasks start with "a dash followed by a space", and everything else is a note (https://guide.taskpaper.com/getting-started/).
- `bodyContentString` (Item API) = the body without the leading `- `, without the trailing `:` and without trailing tags. The sidebar uses it as the project and search display name.
- When an item's type changes through the API or commands ([APP] `types.coffee`):
  - to task: prepend `- `.
  - from task: remove the first 2 chars.
  - to project: insert `:` just before the trailing tag run.
  - from project: delete the char just before the trailing tags.
  - Example from the tests: `one @done` → project gives `one: @done`.
- Type depends **only on the item's own line**, not on its parent. Notes and tasks can have children, and projects can nest inside tasks (inferred from the absence of any constraint; the guide says TaskPaper doesn't enforce an organizational system).

### 2.6 Inline link detection (display only; affects nothing in the model)
Source: https://github.com/jessegrosjean/TaskPaper/blob/master/BirchOutline/birch-outline.js/src/serializations/taskpaper/smart-links.coffee , https://guide.taskpaper.com/reference/links/
- Email: `/\b[A-Z0-9._%+\-]+@[A-Z0-9.\-]+\.[A-Z]{2,8}\b/i`. This is why `foo@bar.com` is not a tag: tags need whitespace or line start before the `@`.
- File path: `(?:^|\s)(~?\.?\/(?:\\\s|[^\0 ])+)`, so absolute `/x`, `./x` and `~/x` all match. Spaces are escaped as `\ `. Relative paths resolve against the document's folder.
- URLs: a Gruber-style regex covering `scheme:...` and `www.` (a `www.` link gets `http://` prefixed).

---

## 3. Tags

Sources: https://github.com/jessegrosjean/TaskPaper/blob/master/BirchOutline/birch-outline.js/src/serializations/taskpaper/tags.coffee ,
https://github.com/jessegrosjean/birch-outline/blob/master/test/serializations/taskpaper-spec.coffee ,
https://guide.taskpaper.com/getting-started/ , https://support.hogbaysoftware.com/t/option-to-strip-tags-except-done-when-archiving-done-items/1019

### 3.1 Regex (exact)
```
NameStart = [A-Z_a-zÀ-ÖØ-öø-˿Ͱ-ͽͿ-῿
             ‌-‍⁰-↏Ⰰ-⿯、-퟿豈-﷏ﷰ-�]
NameChar  = [\-.0-9·̀-ͯ‿-⁀]
TagWord   = (?:NameStart|NameChar)*            # NOTE: '*' and no start-char requirement
TagValue  = \(((?:\\\(|\\\)|[^()])*)\)
Tag       = (^|\s+)@(TagWord)(?:TagValue)?(?=\s|$)     # global
Trailing  = (Tag)+\s*$
```
(These are the XML Name character classes minus `:`.)

### 3.2 Tag-name rules
- The `@` must be at line start or preceded by whitespace. `foo@bar` is not a tag.
- The tag must be followed by whitespace or end of line. `@due(2016)x` and `@a,b` are **not** tags at all; the regex does not match a shorter prefix either.
- Allowed name chars are letters (broad Unicode), `_`, digits, `-`, `.`, U+00B7, combining marks U+0300–U+036F and U+203F–U+2040.
  - Names may **start with a digit**, `-` or `.`: `@2`, `@1abc` and `@a.b-c` are all valid. The test suite has `@2`.
  - Not allowed: `:`, `/`, `@`, `#`, `!`, `,`, quotes, brackets and so on.
- **Empty name** (inferred from regex; not documented): a lone `@` followed by whitespace or end of line matches as a tag named `""`, giving attribute `data-`. A real implementation should probably ignore it.
- Names are case-sensitive in the attribute map (`data-Done` ≠ `data-done`) (inferred). Queries compare **values** case-insensitively by default, but the attribute **name** lookup in `@name` is exact (inferred from `getAttribute`).
- Reserved: tags that would map to `data-id`, `data-text` or `data-type` (that is `@id`, `@text`, `@type`) are **ignored** as tags (`reservedTags`). The text stays as plain text.

### 3.3 Tag values
- An optional `(...)` comes right after the name, with no space before it.
- Inside, any chars except unescaped `(` and `)`. Escape them as `\(` and `\)`. The parser unescapes **only** those two sequences; other backslashes stay literal.
- Writing a value escapes `(` and `)` with a backslash. From the tests: setting `(hello)` writes `@jesse(\(hello\))`.
- Unbalanced or unescaped parens break the tag. `@a(b @c(d)` gives no `a` tag but does give a `c` tag. `@a(b(c)d)` gives no tag.
- Empty `()` gives the value `""`, the same as no value (`@a()` ≡ `@a` in the model) (verified with the regex harness). Re-serializing after an API set writes `@a` for an empty value (`encodeTag`), but text typed by the user is kept verbatim.
- Values may contain spaces: `@b(c d)`.
- **Comma lists**: a value like `@a(1,2, 3)` is stored as one string. Queries with the `[l]` modifier, and `getAttribute(name, clazz, array=true)`, split it on `/\s*,\s*/`. The guide shows `@priority(1,2)`.
- **Dates in values** are just strings; see §6.

### 3.4 Duplicates, trailing tags, storage
- Each tag becomes item attribute `data-<name>` = value string.
- If the same tag appears twice on a line, the **first occurrence wins**; later ones are plain text in the model. The author confirms this in FORUM-1019: only the first `@today` is recognized as a tag.
- The **trailing tag run** is a sequence of tags followed by optional whitespace at end of line. It is excluded from the "content" used for type detection and display (`bodyContentString`). Tags in the middle of a line are still tags (attributes) but count as content for type detection.
- Adding a tag through the API or commands replaces the existing occurrence in place if there is one. Otherwise it appends `" @tag(value)"` to the end of the line, with a space unless the body already ends in whitespace. Removing a tag deletes the matched text including its leading whitespace.
- `@done` is just a tag. Its styling (strike-through) comes from the stylesheet.

### 3.5 `@done` format
Source: https://github.com/jessegrosjean/TaskPaper/blob/master/BirchEditor/birch-editor.js/src/plugins/taskpaper.coffee , `.../BirchEditor.swift/BirchEditor/PreferencesWindowController.swift`
- Toggle Done (⌘D): if **all** selected items already have `data-done`, the tag is removed. Otherwise `@done` is set on all of them.
- With preference `BIncludeDateWhenTaggingDone` (default **false**), the value is `moment().format('YYYY-MM-DD')`, giving `@done(2019-11-20)`. Without it, the tag is a bare `@done`.
- Clicking the leading `-` of a task toggles done (the `button://toggledone` link on the dash).
- `@due` / `@start` (Tag > Due / Start) open a date picker. The value is `DateTime.format(date, false, false)`:
  - `YYYY-MM-DD` if the time is midnight.
  - `YYYY-MM-DD HH:mm` otherwise.
  - Example: `@due(2016-03-18 19:00)`.
- `@today` toggles a bare tag (⌘Y).

---

## 4. Query (search / item path) language

Sources: https://guide.taskpaper.com/reference/searches/ (prose and examples),
https://github.com/jessegrosjean/TaskPaper/blob/master/BirchOutline/birch-outline.js/src/item-path-parser.pegjs (the grammar the app ships),
https://github.com/jessegrosjean/TaskPaper/blob/master/BirchOutline/birch-outline.js/src/item-path.coffee (evaluation).
The parse examples below were verified by running the app's compiled `item-path-parser.js` under node.

### 4.1 Grammar (EBNF summary of the [APP] PEG; `_` = optional `[ \t\n\r]*`)
```
Expr          := Union _
Union         := Except   ( _ 'union'     _ Union     )?     # lowest precedence, right-assoc
Except        := Intersect( _ 'except'    _ Except    )?
Intersect     := PathExpr ( _ 'intersect' _ Intersect )?     # highest set-op precedence
PathExpr      := '(' _ Union _ ')' Slice?                     # only if not parseable as a predicate group
               | ItemPath
ItemPath      := '/'? Step ( '/' Step )*
Step          := Axis? Type? OrPred? Slice?                   # at least Type or OrPred required
Axis          := 'ancestor-or-self::' | 'ancestor::' | 'child::' | 'descendant-or-self::'
               | 'descendant::' | 'following-sibling::' | 'following::' | 'preceding-sibling::'
               | 'preceding::' | 'parent::' | 'self::'
               | '//' (→descendant-or-self) | '/' (→descendant) | '..' (→parent) | '.' (→child)
Type          := 'project' | 'task' | 'note'  followed by _   # the serializer's itemPathTypes
OrPred        := AndPred ( _ 'or'  _ OrPred  )?
AndPred       := NotPred ( _ 'and' _ AndPred )?
NotPred       := ('not' ws+)* PredExpr                         # odd count = negate; 'not not x' = x
PredExpr      := '(' _ OrPred _ ')' | Comparison
Comparison    := '*'
               | Value? _ Relation? _ Modifier? _ Value?       # at least one Value
Relation      := 'beginswith' | 'contains' | 'endswith' | 'matches'
               | '=' | '!=' | '<=' | '>=' | '<' | '>'
Modifier      := '[' ('s'|'i'|'n'|'b'|'d')? 'l'? ']'          # e.g. [s] [n] [d] [l] [nl] [dl]
Value         := '@' AttrName (':' AttrName)*                  # attribute reference
               | 'count' '(' Expr ')'                          # number of results, as a string
               | String
String        := ( _ (Quoted | Unquoted) _ )+   → concatenated, then trimmed
Quoted        := '""' | '"' (char | \" | \\ | \/ | \b | \f | \n | \r | \t)+ '"'
Unquoted      := one of: [0-9] | [~`!#$%^&*-+=\{\}|\\;',.?-] | XMLName   — and not a reserved word
Slice         := '[' Integer (':' Integer?)? ']'              # Integer may be negative
Reserved      := union except intersect and or not beginswith contains endswith like matches = ! < >
```
XMLName here **includes `:`** as a name char, so `Archive:` is one unquoted word. `/`, `(`, `)`, `[`, `]`, `"`, `@`, `<`, `>`, `!` (alone) and whitespace act as delimiters. Strings containing them must be quoted.

### 4.2 Defaults and shortcuts
- **Omitted parts of a predicate** ([GUIDE] and parser):
  - A bare value `X` means `@text contains[i] X`.
  - A bare attribute `@tag` is an **existence test**: true if the attribute is present, even with an empty value.
  - With a relation but no left side, the left side defaults to `@text`.
  - The default relation is `contains` and the default modifier is `i`.
  - These are all equivalent ([GUIDE]): `Inbox`, `@text Inbox`, `contains Inbox`, `@text contains Inbox`.
- **Multiple words without operators form one phrase**: `socks shoes` means `@text contains "socks shoes"` (verified). Whitespace between unquoted tokens is kept and the result is trimmed.
- **Type shortcut**: a leading `project`, `task` or `note` in a step restricts the item type. `project Inbox` ≈ `@type = project and Inbox` ([GUIDE]). On its own, `task` means all tasks.
- **Path defaults**:
  - The first step of a **non-absolute** path with no explicit axis uses the **descendant** axis from the context root. So `foo` searches the whole (hoisted or focused) outline.
  - Later steps with no explicit axis default to **child**: `a/b` means b items that are children of a items anywhere.
  - A **leading `/`** makes the path absolute, so the first step is evaluated on the children of the root (`/*` = top-level items).
  - Inside a path, `/x` is child, `//x` is descendant and `///x` is descendant-or-self.
  - A leading `//x` is absolute + descendant.
- **Reserved words as values must be quoted**: `contains and` is a syntax error; `contains "and"` is fine ([GUIDE], verified).

### 4.3 Attributes
Source: `item-path.coffee` `evaluteGetAttributeFunction`
- `@text` is the full `bodyString`: the whole line minus indentation tabs, **including** `- `, `:` and the tag text.
- `@id` is the runtime item id.
- `@type` gives `project|task|note`. It falls back to `data-type` because `getAttribute('type')` is unset.
- `@foo` reads attribute `foo`, falling back to `data-foo`, so it is effectively the tag value. A missing tag is `undefined`.
- `@a:b`: extra `:` segments parse, but evaluation only uses the first name (inferred from code; the trailing segments are ignored).
- `count(<path>)` evaluates a sub-path relative to the current item and returns the count as a string. Example: `count(//@done) > 2` (combine with `[n]` for numeric compare) (inferred usage).

### 4.4 Relations and modifiers (evaluation semantics)
- Both sides are converted per the modifier before comparing. The right side is converted once and cached.
  - `i` (default): lowercase.
  - `s`: as-is.
  - `n`: `parseFloat`.
  - `d`: parsed with the TaskPaper date parser (§6) to epoch ms.
  - `b`: `'true'` → true, anything else → false.
- `=` / `!=`: strict equality.
- `<` `>` `<=` `>=`: JS comparison. Strings compare lexically unless `n` or `d` is used. A missing left value gives false.
- `contains` is substring, `beginswith` is prefix, `endswith` is suffix. A missing left value gives false.
- `matches`: JS `RegExp(right)` (no flags) tested against `left.toString()`. Note that with the default `i` modifier the right-hand regex source has **already been lowercased** (inferred from the code order; this can break patterns like `\D`). An invalid regex makes the predicate false.
- **List modifier `l`**: both sides are split on `/\s*,\s*/`, with the element modifier applied to each element.
  - `=` / `!=`: arrays must be equal element-wise.
  - `contains`: every right element must be in the left list.
  - `beginswith` / `endswith`: the right list is a prefix or suffix of the left list.
  - `<` and friends: every right element must have some left element satisfying the relation.
  - `matches`: every right regex must match some left element.
  - Example: `@job contains[l] John` ([GUIDE]).
- A predicate with an empty right value is still compared (`@done = ""` matches a bare `@done`).
- `not` applies to the following predicate only; `(… )` groups. Precedence, tightest first: `not` > `and` > `or`. Example: `x and y or z` = `(x and y) or z` (verified).

### 4.5 Axes (evaluation)
- `child`: the children.
- `descendant`: everything inside the item, excluding the item itself.
- `descendant-or-self`: the same plus the item.
- `ancestor` and `ancestor-or-self`: results are kept in document order.
- `parent`: the parent item.
- `following-sibling` and `preceding-sibling`.
- `following` / `preceding`: all items after or before the item in document order, not just siblings.
- `self`: the item itself.
- With multiple context items, each step merges per-context results in **document order** and removes duplicates.
- `..` on its own is a syntax error; it needs a predicate, e.g. `..*` (verified).

### 4.6 Set operations
- `union`, `intersect` and `except` operate on document-ordered item lists. All results are in outline order.
- Precedence: `intersect` binds tightest, then `except`, then `union`. All are right-associative. So `a union b except c` parses as `a union (b except c)` (verified). Use parentheses.

### 4.7 Slicing
Slices attach to a step (applied **per context item**) or to a parenthesized expression (applied to the whole result).
- `[i]` gives a single item at index i. Negative indices count from the end.
- `[i:j]` gives a half-open range.
- `[i:]` runs from i to the end.
- Out-of-range indices are clamped.
- **`[:j]` and `[:]` (documented in [GUIDE]) are syntax errors in the shipped grammar**, because the start integer is mandatory (verified: "Expected … but ':' found"). Use `[0:j]` instead.
- Next-actions idiom ([GUIDE]): `project *//not @done[0]` takes, for each project, the first descendant that is not done.
- Whole-result slicing: `(project *//not @done)[0]`.

### 4.8 Evaluation context in the app
Source: https://github.com/jessegrosjean/TaskPaper/blob/master/BirchEditor/birch-editor.js/src/item-buffer/index.coffee
- The filter is evaluated with the hoisted or focused item as both context and root. An absolute `/` therefore means the focused project's children.
- **Display of results**: each matching item is shown **together with all its ancestors** up to the root, but **not** its descendants unless they match too. A parent with some children hidden is marked `filtered` (vs `expanded`/`collapsed`).
- With no filter, the expanded items are shown, skipping the children of explicitly collapsed items.

### 4.9 Example queries (from [GUIDE] unless marked)
| query | meaning |
|---|---|
| `socks` / `not socks` | items whose text contains / doesn't contain "socks" |
| `@status` | has a `@status` tag |
| `@status = complete` | tag value equals "complete" (case-insensitive) |
| `not @done` | items without `@done` |
| `@done and @today` | both tags |
| `(one or two) and not three` | boolean grouping |
| `project Inbox` | projects whose text contains "Inbox" |
| `/my heading/subhead` | top-level "my heading" items → their children containing "subhead" |
| `/my heading//subhead` | "subhead" anywhere under top-level "my heading" |
| `project Inbox//* union //@today` | everything under Inbox plus all @today items |
| `(project Inbox//* union //@today) except //@done` | the same minus done items |
| `project *//not @done[0]` | first not-done item in each project |
| `@due <=[d] March 18 7pm` / `@due <=[d] today + 24h` | date comparisons (reference/dates) |
| `@today union @due <[d] tomorrow` | shipped default "Today" saved search ([APP] `TaskPaper/searches.taskpaper`) |
| `not @done except @done//*` | shipped default "Not Done" search: hides done items and everything under them ([APP]) |
| `//@done except //@text = Archive://@done` | the app's own Archive command query ([APP] plugins/taskpaper.coffee) |
| `ancestor::@type=project` | used by Archive to collect project names ([APP]) |

### 4.10 Searchbar / errors
Source: https://guide.taskpaper.com/using-taskpaper/using-the-searchbar.html
- ⇧⌘F opens the editor search, Esc closes it, and ⌃⌘P picks a saved search.
- A syntax error highlights the search field red. Internally a parse error gives an empty result list (`ItemPath.evaluate` returns `[]` when the AST is null).

---

## 5. Saved searches and tag config (`@search`)

Sources: https://guide.taskpaper.com/using-taskpaper/using-the-sidebar.html ,
https://github.com/jessegrosjean/TaskPaper/blob/master/BirchEditor/birch-editor.js/src/outline-sidebar.coffee ,
https://github.com/jessegrosjean/TaskPaper/blob/master/TaskPaper/searches.taskpaper ,
https://github.com/jessegrosjean/TaskPaper/blob/master/TaskPaper/tags.taskpaper

- A saved search is **any item** carrying a `@search(<query>)` tag, found via `//@search`. Its type does not matter.
  - Display name is the item's `bodyContentString` (text without trailing tags).
  - The query is the tag value, unescaped.
  - Guide example: `Todo @search(@today and not @done)`.
- Because the query lives in a tag value, **`(` and `)` in the query must be written `\(` `\)`**. The guide says you "must escape them" with a backslash. Example: `Mine @search(\(a or b\) and not @done)`. Unescaped inner parens break the tag regex, so the item would not be a search at all.
- Sources of searches: searches embedded in the document come first, then the shared `searches` configuration outline. That file lives in the app's support folder under `Configurations/`; open it via Window › StyleSheet › Open StyleSheet Folder. The shipped file is:
  ```
  This is a configuration outline. Use it to store searches …
  (blank)
  Searches:
      Today @search(@today union @due <[d] tomorrow)
      Not Done @search(not @done except @done//*)
  ```
  This file is indented with spaces, which shows the space normalization (§2.2).
- The `tags` config outline uses top-level **projects** `Include Tags:` and `Exclude Tags:` whose own tags list what to always show or hide in the sidebar. Shipped values:
  - `Include Tags: @due @start @today @done`
  - `Exclude Tags: @search`
  - These are found by queries `/project Include Tags` and `/project Exclude Tags`.
- Sidebar sections ([GUIDE] using-the-sidebar and [APP] outline-sidebar.coffee):
  - **Home**.
  - **Projects**: every project in the document, nested by project ancestry. Non-project items in between are skipped.
  - **Searches**.
  - **Tags**: every `data-*` name in the doc plus Include, minus Exclude, minus `type`.
    - Each tag lists its distinct values as children. Comma lists are split into separate values.
    - Clicking a tag filters `@name`. Clicking a value filters `@name contains[l] "value"`.
- Selecting a project **focuses** it. Selecting a search, tag or value sets the **item-path filter**. Double-clicking a project **hoists** it ([GUIDE]).

---

## 6. Dates

Sources: https://guide.taskpaper.com/reference/dates/ , https://guide.taskpaper.com/using-taskpaper/using-dates-and-reminders.html ,
https://github.com/jessegrosjean/TaskPaper/blob/master/BirchOutline/birch-outline.js/src/date-time-parser.pegjs ,
https://github.com/jessegrosjean/TaskPaper/blob/master/BirchOutline/birch-outline.js/src/date-time.coffee

- Storage convention: ISO 8601 inside tag values. `@due` and `@start` are conventional and open the date palette when typed ([GUIDE]).
- Grammar: `DateTimeExpression := Date? Time? Duration*`. All parts are optional; an empty string means "now". Parsing is case-insensitive and relative to local "now".
  - **Absolute date**: `YYYY`, `YYYY-MM` or `YYYY-MM-DD`, each meaning local midnight on the first unspecified day/month.
  - **Relative date**:
    - Keywords: `now`, `today`, `yesterday`, `tomorrow`.
    - `[this|next|last] <month> [day]`, e.g. `June`, `next June 3`, `last jun`.
    - `[this|next|last] <weekday>`, using ISO weeks (Monday start) in [APP]. Example: `next monday`.
    - `this|next|last <unit>`: the start of that unit; for weeks, the ISO week. Example: `next week`, `last month`.
  - **Time**:
    - `[at] H am|pm`, e.g. `6 am`, `at 7pm`.
    - `H:MM[:SS[:mmm]] [am|pm]`.
    - 24h `16:15`.
    - A time resets to the start of the day, then adds the time.
  - **Durations**: `[+|-] N unit`, repeatable. Examples: `2 days`, `+1 week`, `-6 hours`, `2 days 6 hours`, `today + 24h`.
    - Units (with short forms): ms; s/sec/second(s); m/min/mins/minute(s); h/hour(s); d/day(s); w/week(s); o/month(s); q/quarter(s); y/year(s).
    - **`m` = minutes; months use `o`**.
  - Combined forms, e.g. `tomorrow 9am`, `Nov 26 3:15 +1day`.
- **Fallback**: if the grammar fails, strict `moment(string, ISO_8601)` is tried. Otherwise the result is null, which never matches `[d]` comparisons.
- Formatting by the app (`DateTime.format`) uses the minimal form:
  - `YYYY-MM-DD`
  - `YYYY-MM-DD HH:mm`
  - `YYYY-MM-DD HH:mm:ss`
  - `YYYY-MM-DD HH:mm:ss:SSS` (note the colon before milliseconds)
- `[d]` comparisons are evaluated at query time, so `@due <[d] tomorrow` is a live relative query.
- Reminders import/export exists (File › Import Reminders, Item › Export to Reminders) but is macOS-specific ([GUIDE]).

---

## 7. Archive @done

Sources: https://guide.taskpaper.com/using-taskpaper/making-lists.html , https://github.com/jessegrosjean/TaskPaper/blob/master/BirchEditor/birch-editor.js/src/plugins/taskpaper.coffee , https://support.hogbaysoftware.com/t/how-to-build-the-add-project-tag-from-preferences/3225

Exact algorithm (Tag › Archive @done Items, ⇧⌘A):
1. `archive` = the first result of `//@text = Archive:`. This is a case-insensitive exact match on the whole line, at any depth. A line with trailing tags (`Archive: @x`) does **not** match (inferred).
2. `done` = `//@done except //@text = Archive://@done`, reduced to **common ancestors**. A done item inside another done item moves with its parent and is not processed separately. Children of a done item move along even if they are not done ([GUIDE]: moves "@done items and the items they contain").
3. If there is no archive item, append `Archive:` as the **last top-level** item.
4. For each done item (in one undo group):
   - If pref `BRemoveExtraTagsWhenArchivingDone` (default false): remove every `data-*` attribute except `data-type` and `data-done`. This applies only to the moved root item, not its descendants (forum report in FORUM-1019 matches the code).
   - If pref `BIncludeProjectWhenArchivingDone` (default false): set `@project(<names>)`, where names are the `bodyContentString` of all ancestor projects joined with `" / "`, outermost first. Example: `@project(Work / Client A)`. A `(`/`)` in a name gets escaped.
5. Insert all done items **at the top** of the Archive project's children, keeping their document order.

---

## 8. Folding, focusing (hoisting), filtering

Sources: https://guide.taskpaper.com/using-taskpaper/fold-focus-and-filter.html , https://guide.taskpaper.com/reference/scripting/OutlineEditor.html , https://github.com/jessegrosjean/TaskPaper/blob/master/BirchEditor/birch-editor.js/src/item-buffer/index.coffee , https://support.hogbaysoftware.com/t/whats-new-in-taskpaper-3/950

- **Fold**:
  - Click an item's handle (dot) to collapse or expand its children.
  - ⌘0 / ⌘9 expand or collapse the selected items, one level.
  - ⌥⌘0 / ⌥⌘9 do it completely.
  - ⇧⌘0 / ⇧⌘9 expand or collapse **all** by level; ⇧⌥⌘0 / ⇧⌥⌘9 do all completely.
  - A collapsed item moves together with its children.
  - Only explicitly collapsed items are stored, in `collapsedItemIDs` in an xattr.
- **Focus** vs **hoist** (OutlineEditor API):
  - `hoistedItem` is the root displayed in the buffer.
  - `focusedItem` is a displayed item whose subtree is shown, with leading indentation removed.
  - Ways to focus: select a project in the sidebar, ⌥-click a handle, ⌥⌘→ (Focus In), ⌥⌘← (Focus Out), ⇧⌘H (Go Home), or ⌘P (Go to Anything).
  - The TP3 announcement says a focused project is shown without "all the leading indentation".
- **Filter**: `itemPathFilter` string; display rules are in §4.8. Prefs `BMaintainHoistedItemWhenFiltering` and `BMaintainItemPathFilterWhenHoisting` (both default false) control whether filter and focus clear each other.
- Clicking a tag in the text filters by it, using the `filter://@tag` link. Clicking a tag value filters `@tag = "value"`, or `contains[l]` per element for comma lists.

---

## 9. Editor behavior

Sources: https://github.com/jessegrosjean/TaskPaper/blob/master/BirchEditor/birch-editor.js/src/outline-editor.coffee , `.../plugins/taskpaper.coffee` , https://guide.taskpaper.com/using-taskpaper/making-lists.html

- **Return** (`insertNewline`), with pref `BAutoformatListsAsYouType` (default true):
  - If the current item matches `^- (.*)`:
    - Cursor inside the `- ` prefix, or at line start with content: insert a new empty item **above**.
    - Item is just `- ` (empty task) and the cursor is at the end of the prefix: clear the body (turns the empty task into an empty note, "exits" the list).
    - Otherwise split at the cursor, and prefix the new item with `- ` if the split text doesn't already start with it.
  - Notes and projects just split with no prefix.
  - The new item's indent is `max(depth of current, depth of next displayed item, hoisted+1)`, so a new line after a project with children lands at child level (inferred from the `insertItem` formula).
- **⌥Return** inserts a newline without auto-indent or auto-format ([GUIDE]).
- **Tab / ⇧Tab** = move lines right or left (indent/outdent). Tabs are never inserted as characters.
- **Backspace** at offset 0 of an item deeper than hoisted+1 outdents instead of deleting.
- New-item commands insert template text and select the placeholder:
  - New Task: `- New Task`.
  - New Note: `New Note`.
  - New Project: `New Project:`.
- **Format as project/task/note** rewrites the line syntax (§2.5).
- **Remove Tags** (⌃⌥⌘K) removes all `data-*` attributes except type.
- Move commands:
  - ⌃⌘↑/↓/←/→ move lines; adding ⌥ moves the whole branch.
  - Move to Project… (⌘\\).
  - Group (⌥⌘G).
  - Duplicate (⇧⌘D).
  - Delete Items (⌃⇧K).
  - Select Branch (⇧⌘B).
- Keyboard map extracted from `TaskPaper/Base.lproj/MainMenu.storyboard` ([APP]); an uppercase key means Shift:

| Command | Keys |
|---|---|
| Tag with… | ⌘T |
| Done | ⌘D |
| Today | ⌘Y |
| Archive @done Items | ⇧⌘A |
| Remove Tags | ⌃⌥⌘K |
| Move to Project… | ⌘\\ |
| Move Up/Down/Left/Right | ⌃⌘ + arrows (⌃⌥⌘ = branch) |
| Expand / Collapse Items | ⌘0 / ⌘9 (⌥ = completely) |
| Expand / Collapse All By Level | ⇧⌘0 / ⇧⌘9 (⇧⌥⌘ = completely) |
| Focus In / Focus Out / Go Home | ⌥⌘→ / ⌥⌘← / ⇧⌘H |
| Begin Editor Search / Refresh | ⇧⌘F / ⌘R |
| Go to Anything / Project / Search | ⌘P / ⌥⌘P / ⌃⌘P |
| Command palette | ⇧⌘P |
| Show Sidebar | ⌥⌘S |
| Select Branch / Select Paragraph | ⇧⌘B / ⇧⌘L |
| Copy Displayed | ⌥⌘C |
| Group / Duplicate / Delete Items | ⌥⌘G / ⇧⌘D / ⌃⇧K |

---

## 10. Scripting / JS API (relevant bits)

Sources: https://guide.taskpaper.com/using-taskpaper/using-scripts.html , https://guide.taskpaper.com/customizing-taskpaper/creating-scripts.html , https://guide.taskpaper.com/reference/scripting/Item.html , https://guide.taskpaper.com/reference/scripting/OutlineEditor.html , https://github.com/jessegrosjean/birch-outline/blob/master/doc/getting-started.md

- Scripts are JS run inside TaskPaper's JSContext through the AppleScript/JXA `evaluate({script, withOptions})` command. The script is a function like `TaskPaperContextScript(editor, options)`.
- Model API:
  - `Outline`: `evaluateItemPath(path, contextItem?, options?)`, `serialize()`, `groupUndoAndChanges()`, `createItem()`.
  - `Item`: `bodyString`, `bodyContentString`, `depth`, `indent`, `getAttribute(name, clazz?, array?)`, `setAttribute`, `attributeNames`.
  - `DateTime.parse` and `DateTime.format`.
- Tags map to `data-<name>` attributes, and the type maps to `data-type`. Setting attributes rewrites the line text (§3.4) and vice versa; the change delegate keeps them in sync.
- birch-outline can be used standalone in Node: `Outline.createTaskPaperOutline(text)` then `outline.serialize()`.

---

## 11. Edge cases and ambiguities (checklist for a reimplementation)

1. **`+ ` / `* ` bullets**: tasks only in old birch-outline. In the shipping app they are **notes**. Follow the app.
2. **Trailing whitespace after a project colon** with no tags (`Inbox:  `) gives a **note**. With a trailing tag present (`Inbox: @a  `) it gives a project.
3. **`Project:@tag`** (no space) is a note, and `@tag` is not a tag.
4. **Tasks win over projects**: `- foo:` is a task.
5. **Empty-name tag**: a lone `@` before whitespace or end of line matches the tag regex as name `""` (inferred). Probably ignore it.
6. **Tag names may start with digits, `-` or `.`** (`@2`, `@-x`, `@.x`). A tag is not recognized at all if followed by a non-whitespace char, e.g. `@tag,` or `@tag.` at the end of a sentence followed by more text. Note that `.` is a name char, so `@tag.` at end of line is the tag `tag.` (inferred).
7. **Duplicate tags**: the first occurrence wins. Later occurrences are plain text for queries but still count as tags in the trailing-tag strip (inferred from `trailingTagsRegex` being independent of dedupe).
8. **Reserved tags** `@id`, `@text`, `@type` are not tags.
9. **Escapes**: only `\(` and `\)` are unescaped inside values. `@search(...)` values need escaped parens.
10. **`@a()` vs `@a`**: identical in the model (value `""`). Existence queries match both.
11. **Space indentation**: the tab width is inferred from the smallest leading-space run in the whole file. A single odd line can skew depth. Mixed tab+space prefixes are converted, then leftover spaces are dropped. Output is always tabs.
12. **Single-line text is not space-normalized** (paste behavior).
13. **Indent jumps** (0 → 3 tabs) are preserved as a relative `indent > 1` on the child.
14. **Blank lines are items** (empty notes), indented to `max(own, next non-blank)`. Trailing blank lines are top-level. They never terminate a project. Queries like `*` will match them.
15. **Trailing newline** = trailing empty item. Writing always uses `\n` and UTF-8; CRLF is not preserved.
16. **U+2028** inside text is an intra-item soft break, not a new line.
17. **Query `[:n]` / `[:]`** are documented but unparseable in the app; `[0:n]` works.
18. **Query `like`** was removed; it is a syntax error now.
19. **Set-op precedence** (`intersect` > `except` > `union`, right-assoc) is undocumented. Users should parenthesize.
20. **`matches` + default `[i]`**: the regex source is lowercased (inferred). Use `matches[s]` for case-sensitive or character-class-sensitive regexes.
21. **Unquoted strings** cannot contain `/ ( ) [ ] " @ < > !` or spaces (other than as part of a phrase). Reserved words must be quoted.
22. **Multi-word bare text** is a phrase search, not an AND.
23. **Archive detection** is the exact line `Archive:` (case-insensitive), anywhere in the outline. A new Archive is appended at the end of the document. Archived items are prepended inside it.
24. **Archive `@project` value** uses `" / "` between nested project names and includes only project-type ancestors.
25. **`@done` date** is optional (preference, default off). Format `YYYY-MM-DD`; the app never writes a time for `@done`.
26. **Dates**: `m` = minutes and `o` = months. Weeks are ISO (Monday). Unparseable dates are null and fail `[d]` comparisons.
27. **Folding/focus/filter state** is stored in xattrs, never in the file. A terminal app needs its own side-car or nothing.
28. **Email vs tag**: `a@b.com` is not a tag (no leading whitespace), but ` @b.com` would be the tag `b.com` (inferred).
29. **Sidebar Projects** shows nested projects even when separated by non-project parents.
30. **Search display name** is `bodyContentString`, so `Todo: @search(x)` would display as "Todo" (the trailing colon is stripped because the item is a project) (inferred).
