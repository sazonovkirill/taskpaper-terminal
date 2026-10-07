# Key mnemonics

The logic behind the default key bindings, as an aid to memorizing them. [keybindings.md](specs/keybindings.md) is the authoritative list; where they differ, it wins and this guide is a bug.

## 1. Five rules carry most of it

1. **Vim first.** If you know vim, guess the vim key. `hjkl` move, `gg`/`G` top/bottom, `o`/`O` open a line below/above, `dd` delete, `y`/`p` yank and paste, `v` select, `Ctrl+D`/`Ctrl+U` page down/up, `Ctrl+W` window (pane) commands, `:` command line, `/` search, `gx` open link.
2. **Bare letter = do it to this block, now.** No prompt, no picker.
3. **A prefix letter = a family.** The status line shows the pending prefix for 1 second.
   - `g` **go / view**: where you look, not what the file says.
   - `m` **metadata that needs a prompt or picker** (think "more…").
   - `c` **color**.
   - `d` **delete** (`dd`).
   - `o O i I a A` **create** (see §3).
   - `Ctrl+W` **window**: panes, as in vim.
4. **Shift = the other end, or more of it.** Lowercase is the common case; capital goes up/before, to the bigger thing, or reverses.
5. **Brackets come in pairs, and Shift makes the pair stronger.** `[ ]` nudge start date, `{ }` (Shift) nudge the due date. `'` fold, `"` (Shift) fold the whole subtree. `=`/`-` one level, `+`/`_` (Shift) all levels.

## 2. Shift pairs

| lower | UPPER | the idea |
|---|---|---|
| `o` new below | `O` new above | capital goes up |
| `i` new last child | `I` new first child | capital goes to the top |
| `a` new at end of list | `A` new at start | capital goes to the top |
| `p` paste below | `P` paste above | capital goes up |
| `j`/`k` move focus | `J`/`K` extend selection | Shift+arrow selects, everywhere |
| `h`/`l` fold / unfold | `H`/`L` back / forward | big step sideways = history, like a browser |
| `t` start today | `T` due today | due date is the bigger deal |
| `w` start tomorrow | `W` due tomorrow | same rule as `t` |
| `z` undo | `Z` redo | Ctrl+Z / Ctrl+Shift+Z without Ctrl |
| `s` remove section | `S` make section | capital = promote to a heading |
| `d` (`dd`) delete | `D` duplicate | "**D**ouble it" |
| `cc` cycle color | `C` clear color | one exception to "capital = more": **C**lear |
| `gg` first block | `G` last block | vim |
| `.` zoom in | `>` (Shift+`.`) zoom out | same key, Shift reverses |

## 3. Creating blocks: *where* + *what*

First key says **where**, optional second key says **what**.

- Where: `o`/`O` **o**pen a line below/above · `i`/`I` **i**nside (last/first child) · `a`/`A` **a**ppend to the list end/start
- What: `s` **s**ection · `p` **p**roject · `t` **t**ask · `n` **n**ote

So `ot` = task below, `ip` = project inside, `As` = section at the top. Type anything else after the first key and it lands in the new block, so `o` then `Buy milk` just works.

## 4. The `g` family: go and view

| key | mnemonic |
|---|---|
| `gg` / `gp` / `gl` | **g**o top / **p**arent / **l**ast child |
| `gz` / `gu` / `gh` | **z**oom in / **u**p a level / **h**ome |
| `gt` | **t**oday view (`t` alone dates the block today; after `g` it goes there) |
| `gs` | **s**ymbol (= `Ctrl+P`) |
| `gm` | **m**ove block to… |
| `gG` | **G**roup the selection |
| `gx` | open link (vim's `gx`) |
| `gc` / `gw` | show **c**ompleted / word **w**rap |
| `g0` / `g1`…`g9` | sidebar / pane N (numbers = places, like `Alt+1` tabs) |
| `g,` | settings (`,` is preferences, as `Cmd+,` on macOS) |

## 5. The `m` family: metadata with a prompt or picker

| key | mnemonic |
|---|---|
| `ms` / `md` | **s**tart / **d**ue date, typed |
| `mc` | **c**olor picker |
| `mt` / `mb` | **t**oday flag / **b**ookmark |
| `my` | t**y**pe (`t` was taken) |
| `mi` | **i**nfo: all tags |

## 6. Colors, dates, done

- `c1`…`c6`: red, orange, yellow, green, blue, violet. Rainbow order (ROYGBV), so 1 is hottest and 6 coolest.
- `x` toggles done, as in a `[x]` checkbox.
- `t` = **t**oday, `w` = **w**ait a day. Lower = start, upper = due.
- `[`/`]` start date a day earlier/later, `{`/`}` the same for due. Left = earlier, right = later.

## 7. Modifier keys

- **Ctrl = standard app shortcuts.** `Ctrl+C/X/V` copy/cut/paste, `Ctrl+Z/Y` undo/redo, `Ctrl+A` all, `Ctrl+Q` quit, `Ctrl+B` sidebar and `Ctrl+P` go to symbol (VS Code), `Ctrl+K` command palette, `Ctrl+T` choose **t**heme, `Ctrl+O` show completed (**O**ld tasks), `Ctrl+↑/↓` jump between siblings.
- **Alt = carry the block, or a second route.** `Alt+k/j` (`Alt+↑/↓`) move the block up/down, `Alt+h/l` zoom out/in, `Alt+Shift+h/l` narrow/widen the pane, `Alt+Shift+←/→` back/forward, `Alt+z` wrap (as in VS Code), `Alt+0…9` sidebar and panes. Every Alt key also has a route without Alt (a `g` key, a letter, `Ctrl+W`, or the command palette), because macOS needs a terminal setting for Alt.

## 8. Panes: `Ctrl+W` then a vim letter

`v` open this block in a new pane (**v**ertical split) · `s` open this location again (**s**plit) · `h`/`l` focus left/right · `c` **c**lose · `<`/`>` narrower/wider.

## 9. Kitty-only keys and their everyday twins

Every kitty-only key has a classic twin that works in any terminal. Learn the twin first.

| kitty | classic twin |
|---|---|
| `Ctrl+Enter` done | `x` |
| `Ctrl+'` home | `gh` |
| `Ctrl+Shift+Z` redo | `Z`, `Ctrl+Y` |
| `Ctrl+\` block in new pane | `Ctrl+W v` |
| Menu key | `\`, `Shift+F10` |

## 10. Odds and ends

- `'` and `Space` fold, `"` folds the subtree.
- `\` is the "right-click": context menu.
- `?` shortcuts panel; `(`/`)` page it.
- `Enter` edits; `Esc` clears the selection.
- `Tab`/`Shift+Tab` indent/outdent, in both modes.
- **Edit mode** uses normal text-editor keys, plus `/` command menu, `Ctrl+T` cycle **t**ype, `Ctrl+B` **b**old, `Ctrl+E` inline cod**e** (or `` Alt+` ``), `↑`/`↓` edit the neighbor block.

## Where the logic breaks

- `C` clears the color while `S` adds a section, so "capital = more" has exceptions.
- `w` for tomorrow is a fallback: `y` went to copy ([D-53](functional-requirements.md)).
- `/` opens Go to Symbol until real search exists.
