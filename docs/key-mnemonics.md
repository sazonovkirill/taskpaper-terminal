# Key mnemonics

The logic behind the default key bindings, as an aid to memorizing them. [keybindings.md](specs/keybindings.md) is the authoritative list; where they differ, it wins and this guide is a bug.

## 1. Five rules carry most of it

1. **Vim first.** If you know vim, guess the vim key. `hjkl` move, `gg`/`G` top/bottom, `^`/`$` start/end, here of the block's level (first/last sibling), with `#` up to the parent, `o`/`O` open a line below/above, `dd` cut, `y`/`p` yank and paste, `u`/`Ctrl+R` undo/redo, `>`/`<` indent/outdent, `v` and `V` select, `Ctrl+O`/`Ctrl+I` back/forward, `Ctrl+D`/`Ctrl+U` page down/up, `Ctrl+W` window (pane) commands, `:` command line, `/` search, `gx` open link.
2. **Bare letter = do it to this block, now.** No prompt, no picker.
3. **A prefix letter = a family.** The status line shows the pending prefix for 1 second.
   - `g` **go / view**: where you look, not what the file says. A capital after `g` opens a view or a list of places.
   - `m` **metadata that needs a prompt or picker** (think "more…"). A mark that toggles at once is a single key: `s`, `b`, `!`.
   - `d` **delete** (`dd`, which cuts, as in vim).
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
| `j`/`k` move focus | `J`/`K` move the block | Shift carries the block along; Shift+arrow selects (D-139) |
| `h`/`l` fold / unfold | `H`/`L` back / forward | big step sideways = history, like a browser's `Alt+←`/`Alt+→` |
| `t` start today | `T` due today | due date is the bigger deal |
| `w` start tomorrow | `W` due tomorrow | same rule as `t` |
| `z` undo | `Z` redo | Ctrl+Z without Ctrl, plus Shift for the other way (vim's `u`/`Ctrl+R` work too) |
| `s` toggle **s**tar | `S` toggle **S**ection | capital = the bigger mark, a heading (D-130) |
| `d` (`dd`) cut | `D` duplicate | "**D**ouble it" |
| `c` cycle color | `C` clear color | one exception to "capital = more": **C**lear |
| `gg` first block | `G` last block | vim |
| `,` zoom out | `<` (Shift+`,`) outdent | the `,<` key goes out or left (D-139) |
| `.` zoom in | `>` (Shift+`.`) indent | the `.>` key goes in or right, `>` as vim indents |

## 3. Creating blocks: *where*

The key says **where**: `o`/`O` **o**pen a line below/above · `i`/`I` **i**nside (last/first child) · `a`/`A` **a**ppend to the list end/start. It adds the block at once, so `o` then `Buy milk` just works. A block of a chosen type, such as New Task Below, comes from Go to Command (D-136).

## 4. The `g` family: go and view

| key | mnemonic |
|---|---|
| `gg` | **g**o top |
| `gp` / `gn` | **p**revious / **n**ext pane, left / right |
| `gj` / `gk` | next / previous sibling: a bigger `j`/`k` that skips the children (= `Ctrl+↓`/`Ctrl+↑`, which macOS takes) |
| `gz` / `gu` / `gh` | **z**oom in / **u**p a level / **h**ome |
| `gt` | **t**oday view (`t` alone dates the block today; after `g` it goes there) |
| `gT` `gU` `gO` `gC` `gL` `gS` `gF` | capital = a special view: **T**oday, **U**pcoming, **O**verdue, **C**olors, **L**ogbook, **S**tarred, **F**lagged |
| `gi` | **i**nbox, the INBOX project (`i` alone adds a last child; after `g` it goes there) |
| `gs` | **s**ymbol (= `Ctrl+K`) |
| `gP` | **P**roject: Go to Symbol over projects only, a list of places, so a capital |
| `gm` | **m**ove block to… |
| `gx` | open link (vim's `gx`) |
| `gc` / `gw` | show **c**ompleted / word **w**rap |
| `g1`…`g9` | pane N (numbers = places, like `Alt+1` tabs) |
| `g,` | settings (`,` is preferences, as `Cmd+,` on macOS) |

## 5. The `m` family: metadata with a prompt or picker, and the quick marks

| key | mnemonic |
|---|---|
| `ms` / `md` | **s**tart / **d**ue date, typed |
| `mc` | **c**olor picker |
| `my` | t**y**pe (`t` was taken; `Ctrl+T` too, as in edit mode) |
| `s` / `b` / `!` | toggle at once, no prompt: **s**tar, **b**ookmark, red flag (D-139) |

## 6. Colors, dates, done

- `c` **c**ycles the color: none, red, yellow, green, blue, in rainbow order from hottest to coolest (D-138); `C` clears it.
- `x` toggles done, as in a `[x]` checkbox.
- `t` = **t**oday, `w` = **w**ait a day. Lower = start, upper = due.
- `[`/`]` start date a day earlier/later, `{`/`}` the same for due. Left = earlier, right = later.

## 7. Modifier keys

- **Ctrl = standard app shortcuts.** `Ctrl+C/X/V` copy/cut/paste, `Ctrl+Z/Y` undo/redo, `Ctrl+A` all, `Ctrl+Q` quit, `Ctrl+G` **g**o: the Navigator, `Ctrl+K` go to a block and `Ctrl+P` command palette (D-72), `Ctrl+T` convert the **t**ype (as in edit mode; Choose Theme is in the palette), `Ctrl+O`/`Ctrl+I` back/forward (vim's jump list; `Ctrl+I` with the kitty protocol), `Ctrl+R` redo (vim), `Ctrl+↑/↓` jump between siblings (`gk`/`gj` too).
- **Alt = carry the block, or a second route.** `Alt+k/j` (`Alt+↑/↓`) move the block up/down, `Alt+Shift+h/l` narrow/widen the pane, `Alt+←/→` back/forward (as in a browser), `Alt+z` wrap (as in VS Code), `Alt+1…9` panes. Every Alt key also has a route without Alt (a `g` key, a letter, `Ctrl+W`, or the command palette), because macOS needs a terminal setting for Alt.

## 8. Panes: `Ctrl+W` then a vim letter

`v` open this block in a new pane (**v**ertical split) · `s` open this location again (**s**plit) · `h`/`l` or `←`/`→` focus left/right, `W`/`w` the same, as vim cycles windows (`Ctrl+Shift+←/→` and `gp`/`gn` too) · `H`/`L` (Shift) move the pane itself left/right, as vim moves a window · `c` or `q` **c**lose / **q**uit the pane · `<`/`>` narrower/wider.

In the Navigator, Go to Symbol and Go to Project, `Alt+Enter`, `Ctrl+V` or `Ctrl+T` opens the choice in a new pane: **v**ertical split and **t**ab, as in fzf and Telescope. `Ctrl+T` is there for Windows Terminal, which takes the other two.

## 9. Kitty-only keys and their everyday twins

Every kitty-only key has a classic twin that works in any terminal. Learn the twin first.

| kitty | classic twin |
|---|---|
| `Ctrl+Enter` done | `x` |
| `Ctrl+'` home | `gh` |
| `Ctrl+\` block in new pane | `Ctrl+W v` |
| Menu key | `\`, `Shift+F10`; in the Navigator, `Ctrl+R` |
| `Ctrl+I` forward | `L` |

## 10. Odds and ends

- `'` folds, `"` folds the subtree.
- `\` is the "right-click": context menu.
- `?` shortcuts panel; `(`/`)` page it.
- `Enter` edits; `Esc` clears the selection.
- `Tab`/`Shift+Tab` indent/outdent, in both modes; `>`/`<` too in navigate mode.
- The **Navigator** (`Ctrl+G` or `|`, a side bar) is a place finder: letters type into its filter, so its keys are arrows and `Ctrl` keys, as in fzf. `↑`/`↓` or `Ctrl+N`/`Ctrl+P` move, `←`/`→` fold, `Enter` goes, `Alt+Enter`, `Ctrl+V` or `Ctrl+T` opens in a new pane, `Backspace` erases, `Ctrl+R` (**r**ight-click, as `\` is outside it), `Shift+F10` or the Menu key opens the context menu, `Esc`, `Ctrl+G` or `|` closes, `Ctrl+Q` quits.
- **Edit mode** uses normal text-editor keys, plus `/` command menu, `Ctrl+T` cycle **t**ype, `Ctrl+B` **b**old, `Ctrl+E` inline cod**e** (or `` Alt+` ``), `↑`/`↓` edit the neighbor block.

## Where the logic breaks

- `C` clears the color while `S` toggles a section, so "capital = more" has exceptions.
- `w` for tomorrow is a fallback: `y` went to copy ([D-53](functional-requirements.md)).
- `/` opens Go to Symbol until real search exists.
