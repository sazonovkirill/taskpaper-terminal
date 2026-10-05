# Spec: Key bindings

Covers FR-KEY, FR-MODE-01 and N-PORT-03. Navigate mode is vi-inspired with arrow-key equivalents (FR-KEY-02); edit mode uses standard text keys (FR-MODE-03, [editing.md §2](editing.md#2-edit-mode)).

## 1. Terminal constraints

- **No Cmd/Super** (FR-KEY-03). Bindings use `Ctrl`, `Shift` and `Alt` only.
- **Legacy key reporting** cannot distinguish `Ctrl+Enter`, `Shift+Enter`, `Ctrl+Shift+<letter>`, `Ctrl+I` vs `Tab`, `Ctrl+M` vs `Enter`, or `Ctrl+[` vs `Esc`. The app asks for the kitty keyboard protocol on start (crossterm `PushKeyboardEnhancementFlags` with `DISAMBIGUATE_ESCAPE_CODES`) and gives it back on exit; terminals without it ignore the request and keep legacy reporting.
- **Two variants per binding** (D-42). Every command bound to a key has a classic variant, which legacy reporting delivers, and a kitty variant, which the kitty protocol delivers; both stay bound. A key both deliver the same way (letters, digits, punctuation, arrows, `Enter`, `Esc`, `Tab`, `Shift+Tab`, function keys, `Ctrl+<letter>` other than `Ctrl+H`, `Ctrl+I`, `Ctrl+M` and `Ctrl+[`, and their `Alt` and `Shift` forms with named keys) is both variants at once. A key only the kitty protocol reports is a kitty variant and needs a classic variant beside it. The classic variant is not an `Alt` key alone, since `Alt` needs a setting on macOS (below). Under the kitty protocol a letter with `Ctrl` or `Alt` and `Shift` arrives as the lowercase letter plus `Shift`; the keymap matches it as the uppercase letter, as other reportings send it. The kitty-only keys and their classic variants:

  | Kitty variant | Command | Classic variant |
  |---|---|---|
  | `Ctrl+Enter` | Toggle done | `x` |
  | `Ctrl+'` | Go home | `gh` (legacy sends a plain `'`, which toggles the fold) |
  | `Ctrl+Shift+Z` | Redo | `Ctrl+Y`; `Z` in navigate mode (legacy sends `Ctrl+Z`, which undoes) |
  | `Ctrl+\` | Open focused block in a new pane | `Ctrl+W v` (legacy sends what crossterm reports as `Ctrl+4`) |
  | Menu key | Context menu | `\`, `Shift+F10` |
- **`Alt` on macOS** requires "Use Option as Meta" (Terminal.app) or "Option sends Esc+" (iTerm2); Ghostty, kitty and WezTerm have equivalents. The `?` panel says so when no `Alt` key has been seen. Every `Alt` binding also has a non-`Alt` route (a vi-style key, or the command palette).
- **`Alt+[`** is ambiguous with the CSI prefix in legacy mode and is not used.
- Raw mode disables `ISIG` and `IXON`, so `Ctrl+Z`, `Ctrl+S`, `Ctrl+Q` and `Ctrl+C` arrive as keys.
- `Ctrl+H` is `Backspace` in legacy mode and is not used.
- Multi-key sequences (`gg`, `dd`, `ms`, `Ctrl+W h`) time out after 1 s; the pending prefix is shown in the status line. A key that continues no sequence cancels the pending one and does nothing else.

## 2. Navigate mode

The owner's bindings of 2026-10-04 (D-26) are the base of this section. Where they took a key that an earlier binding used, that binding moved as proposed in [OQ-29](../open-questions.md#oq-29--where-do-the-bindings-displaced-by-d-26-go); the table notes each move.

### Moving (FR-NAV-01)

| Command | Keys |
|---|---|
| Next / previous visible block | `j` / `k`, `↓` / `↑` |
| Collapse, or go to parent if already collapsed or a leaf | `h`, `←` |
| Expand, or go to first child if already expanded | `l`, `→` |
| Next / previous sibling | `Ctrl+↓` / `Ctrl+↑` (`}` / `{` now shift the due date) |
| Parent | `gp` |
| Last child | `gl` |
| First / last block | `gg` / `G`, `Home` / `End` |
| Page down / up | `Ctrl+D` / `Ctrl+U`, `PgDn` / `PgUp` |

### Folding (FR-NAV-02)

Fold state lives in memory only and is not saved (D-26); a file opens fully expanded ([navigation.md §3](navigation.md#3-folding-fr-nav-02)).

| Command | Keys |
|---|---|
| Toggle fold | `'`, `Space` |
| Toggle fold of the whole subtree | `"` |
| Expand / collapse all blocks by one level | `=` / `-` |
| Expand / collapse all | `+` / `_` |

### Zoom and history (FR-NAV-03, FR-NAV-04)

| Command | Keys |
|---|---|
| Zoom into focused block | `.`, `Alt+l`, `Alt+→`, `gz` |
| Zoom out | `>` (`Shift+.`), `Alt+h`, `Alt+←`, `gu` |
| Go home | `Ctrl+'` (kitty keyboard protocol only, §1), `gh` |
| Back / forward | `H` / `L`, `Alt+Shift+←` / `Alt+Shift+→` |

### Editing and creating (FR-MODE)

| Command | Keys |
|---|---|
| Edit focused block (caret at end) | `Enter` (`a` now adds a task) |
| New block below / above | `o` / `O` |
| New last child / first child, expanding the block | `i` / `I` (`I` was `Alt+o`; D-41) |
| New project at the top of the list / below the focused block | `P` / `p` |
| New section at the top of the list, after its tasks / below the focused block | `S` / `s` |
| New task at the top of the list / below the focused block | `A` / `a` |

The six typed creation commands are in [editing.md §3](editing.md#3-creating-blocks-fr-mode-04).

### Selection (FR-OPS-01)

| Command | Keys |
|---|---|
| Extend selection down / up | `J` / `K`, `Shift+↓` / `Shift+↑` |
| Toggle focused block in selection | `v` |
| Select all blocks in the pane | `Ctrl+A` |
| Clear selection | `Esc` |

### Structure (FR-OPS)

| Command | Keys |
|---|---|
| Indent / outdent | `Tab` / `Shift+Tab` (`>` now zooms out) |
| Move up / down | `Alt+k` / `Alt+j`, `Alt+↑` / `Alt+↓` |
| Group | `gG` |
| Move to… | `gm` (was `m`) |
| Delete | `dd`, `Delete` |
| Duplicate | `D` (was `yd`) |
| Copy / cut | `Ctrl+C` / `Ctrl+X` (`y` now sets tomorrow) |
| Paste below / above | `Ctrl+V` / `Alt+V` (`p`, `P` now add projects) |
| Undo / redo | `z` / `Z`, `Ctrl+Z` / `Ctrl+Shift+Z` (fallback `Ctrl+Y`) |

### Block metadata (FR-BLK-08, FR-TAG, FR-DONE-01)

The `t` prefix of earlier drafts is gone because `t` now sets the start date. Commands that need a prompt or a picker use an `m` ("metadata") prefix instead.

| Command | Keys |
|---|---|
| Toggle done | `x`, `Ctrl+Enter` |
| Start date today / tomorrow | `t` / `y` |
| Due date today / tomorrow | `T` / `Y` |
| Start date one day earlier / later | `[` / `]` |
| Due date one day earlier / later | `{` / `}` |
| Set start / due date with a prompt | `ms` / `md` |
| Cycle color (none, red, orange, yellow, green, blue, violet) | `c` |
| Pick a color | `mc` |
| Toggle today | `mt` |
| Toggle bookmark | `mb` |
| Convert block type | `my` |
| Details (all tags) | `mi` |
| Comments panel (FR-SPC-02) | `C` (was `c`) |
| Open URL | `gx` |

### View, panes and panels

| Command | Keys |
|---|---|
| Show/hide completed in this pane (FR-DONE-02) | `Ctrl+O`, `gc` |
| Toggle word wrap (FR-UI-05) | `Alt+z`, `gw` |
| Toggle sidebar / focus sidebar (FR-UI-01) | `Ctrl+B` / `Alt+0` |
| Shortcuts panel (FR-KEY-04) | `?` |
| Go to Symbol (FR-NAV-05) | `Ctrl+P`, `gs` (was `Ctrl+T`) |
| Go to Command (FR-NAV-06) | `Ctrl+K`, `:` (was `Ctrl+P`) |
| Settings window (FR-SET-02) | `g,` |
| Context menu for the focused element (FR-KEY-06) | `\`, `Shift+F10`, the Menu key (was `M`) |
| Open focused block in a new pane (FR-UI-03) | `Ctrl+\`, `Ctrl+W v` |
| Open this pane's location in a new pane | `Ctrl+W s` |
| Focus pane N | `Alt+1` … `Alt+9` |
| Focus pane left / right | `Ctrl+W h` / `Ctrl+W l` |
| Close pane | `Ctrl+W c` |
| Narrow / widen pane | `Ctrl+W <` / `Ctrl+W >`, `Alt+Shift+h` / `Alt+Shift+l` |
| Search (Later, FR-SRCH-01); until search exists, Go to Symbol | `/` |
| Follow link / copy link / show backlinks (Later, FR-OPS-10, [links.md](links.md)) | `gd` / `gy` / `gb` |
| Count blocks and words | command palette |
| Save As (FR-DOC-06) | command palette |
| Quit | `Ctrl+Q` (`ZZ` is gone, `Z` is redo) |

## 3. Edit mode

Standard text keys as listed in [editing.md §2](editing.md#2-edit-mode), plus:

| Command | Keys |
|---|---|
| End editing | `Esc` |
| End editing and create the next block | `Enter` |
| End editing and edit the previous / next visible block that is not a divider, at the same column (D-43) | `↑` / `↓` |
| Select to the start / end of the text | `Shift+↑` / `Shift+↓` |
| Open the type menu (D-44, [editing.md §2.3](editing.md#23-type-menu)) | `/` |
| Indent / outdent block | `Tab` / `Shift+Tab` |
| Bold / inline code (FR-FMT-02) | `Ctrl+B` / `` Alt+` `` |
| Insert link (Later, FR-OPS-10) | typing `[[` (`Ctrl+K` is Go to Command) |
| Undo / redo (text, within the session; characters typed in a row undo together) | `Ctrl+Z` / `Ctrl+Shift+Z` (fallback `Ctrl+Y`) |

## 4. Other contexts

- **Type menu** ([editing.md §2.3](editing.md#23-type-menu)): `p`, `s`, `n`, `t` right after `/` choose Project, Section, Note (text) or Task at once; other characters filter; `↑`/`↓` or `Ctrl+P`/`Ctrl+N` move, `Enter` chooses, `Esc` closes and types `/` and the filter, `Backspace` on an empty filter closes. Divider has no letter: `/div` then `Enter`. The menu shows the type names only.
- **Context menu:** opens on the focused block or selection, or on the focused sidebar entry, crumb or pane header; `↑`/`↓` move, typing filters, `Enter` runs, `Esc` closes. `\`, `Shift+F10` and the Menu key open it in every navigate context.
- **Sidebar focused:** `j`/`k`/arrows move, `Enter` zooms the focused pane to the entry, `Alt+Enter` opens it in a new pane, `h`/`l` fold project entries, `Esc` returns to the pane.
- **Pickers and palettes:** type to filter, `↑`/`↓` or `Ctrl+P`/`Ctrl+N` move, `Enter` chooses, `Alt+Enter` opens the choice in a new pane where that applies, `Esc` cancels.
- **Comments panel:** keys as in [special-nodes.md §2.3](special-nodes.md#23-commands).
- **Settings window:** keys as in [ui.md §8](ui.md#8-settings-window-fr-set-02-fr-set-03).
- **Prompts** (dates, URL, Save As path): standard text keys, `Enter` accepts, `Esc` cancels.
- **External-change conflict prompt** ([file-io.md §3](file-io.md#3-external-changes)): `m` keeps mine, `t` takes theirs, `d` shows or hides the diff, `j`/`k`/arrows scroll the diff a line and `Ctrl+D`/`Ctrl+U`/`PageDown`/`PageUp` a page, `Ctrl+Q` quits. Other keys do nothing; the prompt has no `Esc`, because saving stays paused until a choice is made.

## 5. Configuration

```toml
[keymap.navigate]
"alt+shift+up" = "move_up"
"x" = "none"              # unbind

[keymap.edit]
"ctrl+e" = "inline_code"
```

- Keys are written as `mod+mod+key` in lower case (`ctrl`, `alt`, `shift`), or as a sequence separated by spaces (`"g g"`).
- Values are command names, as shown in the command palette.
- Unknown commands, malformed keys and a key bound twice in one context are reported at startup and ignored; the app still starts.

## 6. Validation

Every binding must be verified in the terminals listed in N-PORT-01, including Windows Terminal (FR-KEY-03), with and without the kitty protocol and inside tmux, before the keymap is accepted. Bindings that a common terminal or desktop environment swallows get a different key; the results are recorded in this file.
