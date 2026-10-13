# Spec: Key bindings

Covers FR-KEY, FR-MODE-01 and N-PORT-03. Navigate mode is vi-inspired with arrow-key equivalents (FR-KEY-02); edit mode uses standard text keys (FR-MODE-03, [editing.md §2](editing.md#2-edit-mode)). [key-mnemonics.md](../key-mnemonics.md) explains the logic of the keys as memory aids.

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
  | Menu key | Context menu | `\`, `Shift+F10`; `Ctrl+R` in the Navigator (D-101) |
- **`Alt` on macOS** requires "Use Option as Meta" (Terminal.app) or "Option sends Esc+" (iTerm2); Ghostty, kitty and WezTerm have equivalents. The `?` panel says so when no `Alt` key has been seen. Every `Alt` binding also has a non-`Alt` route (a vi-style key, or the command palette).
- **`Alt+[`** is ambiguous with the CSI prefix in legacy mode and is not used.
- Raw mode disables `ISIG` and `IXON`, so `Ctrl+Z`, `Ctrl+S`, `Ctrl+Q` and `Ctrl+C` arrive as keys.
- `Ctrl+H` is `Backspace` in legacy mode and is not used.
- Multi-key sequences (`gg`, `dd`, `ms`, `c1`, `Ctrl+W h`) time out after 1 s; the pending prefix is shown in the status line. A key that continues no sequence cancels the pending one and does nothing else, unless the pending key is a binding of its own: then it fires first and the key acts after it (§2, D-53).

## 2. Navigate mode

The owner's bindings of 2026-10-04 (D-26) are the base of this section. Where they took a key that an earlier binding used, that binding moved as proposed in [OQ-29](../open-questions.md#oq-29--where-do-the-bindings-displaced-by-d-26-go); the table notes each move.

### Moving (FR-NAV-01)

| Command | Keys |
|---|---|
| Next / previous visible block | `j` / `k`, `↓` / `↑` |
| Collapse, or go to parent if already collapsed or a leaf | `h`, `←` |
| Expand, or go to first child if already expanded | `l`, `→` |
| Next / previous sibling | `Ctrl+↓` / `Ctrl+↑`, `gj` / `gk` (D-90, §6; `}` / `{` now shift the due date) |
| First / last sibling (D-118) | `^` / `$` |
| Parent | `#` (was `gp`, D-118) |
| Last child | command palette (was `gl`, D-118) |
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
| Go to Today: show the Today view, as choosing it in the Navigator does (D-64, D-95) | `gt`, `gT` |
| Go to Upcoming / Overdue / Colors / Logbook: show that special view, as choosing it in the Navigator does (D-80, D-95) | `gU` / `gO` / `gC` / `gL` |
| Go to Starred: show the Starred view, as choosing it in the Navigator does (D-92, D-95) | `gS` |
| Go to Flagged: show the Flagged view, as choosing it in the Navigator does (D-93, D-95) | `gF` |
| Go to Inbox: zoom to the INBOX project the Navigator lists apart, as choosing its entry does; with none, or the setting off, a status notice says so (D-78) | `gi` |
| Back / forward | `H` / `L`, `Alt+Shift+←` / `Alt+Shift+→` |

### Editing and creating (FR-MODE)

| Command | Keys |
|---|---|
| Edit focused block (caret at end) | `Enter` |
| New block below / above | `o` / `O` |
| New last child / first child, expanding the block | `i` / `I` (`I` was `Alt+o`; D-41) |
| New block at the end / start of the list (D-53) | `a` / `A` |
| The same, as a section, project, task or note (D-53) | any of the six keys above, then `s`, `p`, `t` or `n`: `os`, `Ip`, `at`, `An`, … |
| Quick Add: add a task or note to a project chosen with `/` ([editing.md §6](editing.md#6-quick-add-fr-mode-07-fr-mode-08), D-96) | `qq` |
| Quick Add to the INBOX project (D-96) / to the TODAY project (D-98) | `qi` / `qt` |

The creation keys are prefixes (D-53): `o`, `O`, `i`, `I`, `a` and `A` wait for a type letter for the sequence timeout of §1, shown in the status line like `g`, and then add a block of the type that fits ([editing.md §3](editing.md#3-creating-blocks-fr-mode-04)). Any other key ends the wait at once: the block is added and the key goes on to it, so `o` then `B` starts a new block with `B`, while `o` then `t` adds a task. `p`/`P` and `a`/`A` no longer add projects and tasks, and `S`/`s` no longer add sections (D-52). A section is a text block with `@section`, also made with `S` (below).

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
| Move up / down | `Alt+k` / `Alt+j`, `Alt+↑` / `Alt+↓`; in the Today view they reorder its entries (D-57) |
| Group | `gG` |
| Move to… | `gm` (was `m`) |
| Delete | `dd`, `Delete` |
| Duplicate | `D` (was `yd`) |
| Copy / cut | `y`, `Ctrl+C` / `Ctrl+X` (D-53) |
| Paste below / above | `p`, `Ctrl+V` / `P`, `Alt+V` (D-53); text pasted into the terminal goes below |
| Undo / redo | `z` / `Z`, `Ctrl+Z` / `Ctrl+Shift+Z` (fallback `Ctrl+Y`) |

### Block metadata (FR-BLK-08, FR-TAG, FR-DONE-01)

The `t` prefix of earlier drafts is gone because `t` now sets the start date. Commands that need a prompt or a picker use an `m` ("metadata") prefix instead.

| Command | Keys |
|---|---|
| Toggle done | `x`, `Ctrl+Enter` |
| Start date today / tomorrow | `t` / `w` (`w` was `y`, D-53) |
| Due date today / tomorrow | `T` / `W` (`W` was `Y`, D-53) |
| Start date one day earlier / later | `[` / `]` |
| Due date one day earlier / later | `{` / `}` |
| Set start / due date with a prompt | `ms` / `md` |
| Set the color red, orange, yellow, green, blue or violet (D-48) | `c1` … `c6` |
| Remove the color (D-48) | `C` |
| Cycle color (none, red, orange, yellow, green, blue, violet) | `cc` (was `c`, D-48) |
| Pick a color | `mc` |
| Toggle star (`@star`, D-88; was Toggle today, `mt`) | `s` (was `*`, D-130) |
| Toggle bookmark | `mb` |
| Toggle flag (D-91) | `mf`, `!` |
| Convert block type | `my` |
| Toggle section: make the selected text blocks sections, or plain text when every one is (D-52, D-130) | `S` (was Make Section, with `s` Remove Section; both now in Go to Command only) |
| Edit Node: every property and tag of the focused block ([editing.md §7](editing.md#7-edit-node-dialog-fr-mode-09), D-103) | `e`, `mi` |
| Open Note / Open Comments: the attachments panel on the focused block's note or comments ([attachments.md §3](attachments.md#3-attachments-panel-fr-att-03), D-106) | `n` / `N` |
| Open the focused block's link: its `@url` address, else the first link in its text (FR-BLK-06, D-45) | `gx` |

### View, panes and panels

| Command | Keys |
|---|---|
| Show/hide completed in this pane (FR-DONE-02) | `Ctrl+O`, `gc` |
| Toggle word wrap (FR-UI-05) | `Alt+z`, `gw` |
| Open Navigator: the window of places to go, on the entry of where the pane is, else the first (FR-UI-01, D-68, D-79); one control key in both key reportings (D-42) | `Ctrl+G` |
| Shortcuts panel (FR-KEY-04) | `?` |
| Scroll the shortcuts panel down / up a page, while it is open (D-55) | `)` / `(` |
| Go to Symbol (FR-NAV-05) | `Ctrl+K`, `gs` (was `Ctrl+P`, D-72) |
| Go to Project: Go to Symbol over the projects alone (D-65) | `gP` |
| Go to Command (FR-NAV-06) | `Ctrl+P`, `:` (was `Ctrl+K`, D-72) |
| Settings window (FR-SET-02); also Open Settings in the command palette | `g,` |
| Context menu for the focused element (FR-KEY-06) | `\`, `Shift+F10`, the Menu key (was `M`) |
| Open focused block in a new pane (FR-UI-03) | `Ctrl+\`, `Ctrl+W v` |
| Open this pane's location in a new pane | `Ctrl+W s` |
| Focus pane N | `Alt+1` … `Alt+9`, `g1` … `g9` (the non-`Alt` route of §1) |
| Focus the pane left / right (D-71, D-118) | `Ctrl+Shift+←` / `Ctrl+Shift+→`, `Ctrl+W ←` / `Ctrl+W →`, `Ctrl+W h` / `Ctrl+W l`, `gl` or `gp` / `gr` or `gn` |
| Move the focused pane left / right, keeping the keys (D-120) | `Ctrl+W H` / `Ctrl+W L` |
| Close pane | `Ctrl+W c` |
| Narrow / widen pane | `Ctrl+W <` / `Ctrl+W >`, `Alt+Shift+h` / `Alt+Shift+l` |
| Search (Later, FR-SRCH-01); until search exists, Go to Symbol | `/` |
| Follow link / copy link / show backlinks (Later, FR-OPS-10, [links.md](links.md)) | `gd` / `gy` / `gb` |
| Count blocks and words | command palette |
| Save As (FR-DOC-06) | command palette |
| Start Weather Animation (FR-UI-12, D-129) | command palette |
| Choose Theme (FR-UI-06, D-62) | `Ctrl+T` |
| Quit | `Ctrl+Q` (`ZZ` is gone, `Z` is redo) |

## 3. Edit mode

Standard text keys as listed in [editing.md §2](editing.md#2-edit-mode), plus:

| Command | Keys |
|---|---|
| End editing | `Esc` |
| End editing and create the next block | `Enter` |
| End editing and edit the previous / next visible block, at the same column (D-43) | `↑` / `↓` |
| Select to the start / end of the text | `Shift+↑` / `Shift+↓` |
| Open the command menu of the block's commands (D-50, [editing.md §2.3](editing.md#23-command-menu)) | `/` |
| Cycle the block's type: project, section, task, note (D-50, [editing.md §2.4](editing.md#24-cycling-the-type)) | `Ctrl+T` |
| Make a task text again / a project text again (OQ-27, [editing.md §2](editing.md#2-edit-mode)) | `Backspace` at the start of the text / delete its `:` like any character (`End`, `Backspace`) |
| Indent / outdent block | `Tab` / `Shift+Tab` |
| Bold / inline code (FR-FMT-02) | `Ctrl+B` / `` Alt+` ``, `Ctrl+E` (the non-`Alt` route of §1) |
| Insert link (Later, FR-OPS-10) | typing `[[` (`Ctrl+K` is Go to Symbol) |
| Undo / redo (text, within the session; characters typed in a row undo together) | `Ctrl+Z` / `Ctrl+Shift+Z` (fallback `Ctrl+Y`) |
| Copy / cut the selected text, paste ([editing.md §2](editing.md#2-edit-mode)) | `Ctrl+C` / `Ctrl+X`, `Ctrl+V` |

## 4. Other contexts

- **Command menu** ([editing.md §2.3](editing.md#23-command-menu)): characters filter by the starts of the labels' words, in order (`/date`, `/due`, `/color r`); `↑`/`↓` or `Ctrl+P`/`Ctrl+N` move, skipping the lines between groups, `Enter` runs the highlighted entry (the first by default), `Esc` closes and types `/` and the filter, `Backspace` on an empty filter closes. There are no instant letters (D-50): `/p` then `Enter` makes a project. The menu shows the labels only.
- **Color picker** (`mc`, [editing.md §5](editing.md#5-block-commands)): a picker as below that lists each color with its number; typing `1` to `6` chooses that color at once (D-48), and other characters filter by name.
- **Type picker** (`my`, [editing.md §5](editing.md#5-block-commands)): a picker as below; it has no letter shortcuts, so `my` then typing `make` and `Enter` makes a section (D-52).
- **Edit Node dialog** (`e`, `mi`, [editing.md §7](editing.md#7-edit-node-dialog-fr-mode-09)): `Tab`/`↓` and `Shift+Tab`/`↑` move between rows, `Ctrl+T` cycles the type, `Enter` saves and `Esc` cancels; on a row, `Space` acts, a character starts its field, `←`/`→` step its value and `Backspace`/`Delete` remove it; `p`/`t`/`n` on Type, `x` on Done, `s` on Starred and `!` on Flagged as in navigate mode, and `*` on Starred too, its navigate-mode key before D-130. An open field takes the standard text keys, `Enter` and `Esc`; the parent finder the keys of pickers. `Ctrl+Q` still quits.
- **Attachments panel** (`n`, `N`, [attachments.md §3](attachments.md#3-attachments-panel-fr-att-03), D-106, D-107, D-108, D-109): `j`/`k` or `↓`/`↑` scroll a line and `Ctrl+D`/`Ctrl+U` or `Page Down`/`Page Up` a page through the selected item, the note or a comment, while it goes past the panel's edge, and otherwise select the next or previous item; `g`/`Home` and `G`/`End` go to the top with the first item and the bottom with the last. `e` or `Enter` edits the selected item, `a` adds a comment, `d` or `Delete` deletes the selected comment, `Esc` closes, `Ctrl+Q` quits; other keys do nothing. While the note is edited, `Enter` breaks the line, `↑`/`↓` move between lines, the text keys of edit mode edit the line, `Ctrl+V` pastes and `Esc` saves. While a comment is typed, `Enter` adds or saves it, `Esc` cancels, the text keys of edit mode edit it and `Ctrl+V` pastes.
- **Context menu:** opens on the focused block or selection, or on the focused Navigator entry, crumb or pane header; `↑`/`↓` move, typing filters, `Enter` runs, `Esc` closes. `\`, `Shift+F10` and the Menu key open it in every navigate context, but `\` is typed into the Navigator's filter (D-86), where `Ctrl+R` opens it instead (D-101).
- **Navigator** (D-79, D-86): typing filters its entries and `Backspace` erases the filter, so its keys are arrows and control keys, as in the finders: `↑`/`↓` or `Ctrl+P`/`Ctrl+N` move, `Enter` zooms the focused pane to the entry and closes the window, `Alt+Enter`, `Ctrl+V` (the non-`Alt` route of §1, as in fzf and Telescope) or `Ctrl+T` (as fzf and Telescope open a tab, for terminals that take the other two, §6, D-90) opens it in a new pane, `←`/`→` fold project entries, `Ctrl+R`, `Shift+F10` or the Menu key opens the context menu (`\` is typed; D-101), `Esc` or `Ctrl+G` closes it, `Ctrl+Q` quits. It keeps the keys until it closes: other keys, those that focus a pane included, do nothing.
- **Pickers and palettes:** type to filter, `↑`/`↓` or `Ctrl+P`/`Ctrl+N` move, `Enter` chooses, `Alt+Enter`, `Ctrl+V` or `Ctrl+T` opens the choice in a new pane where that applies (Go to Symbol, Go to Project), `Esc` cancels. The theme picker previews the highlighted theme as it moves ([ui.md §3](ui.md#3-themes-fr-ui-0608)).
- **Settings window:** keys as in [ui.md §8](ui.md#8-settings-window-fr-set-02-fr-set-03): `j`/`k`/arrows move, `Tab` switches column, `Enter`/`Space` change, `h`/`l`/`←`/`→` cycle, `r` resets, `/` filters, `Esc` closes; its value picker takes the keys of pickers, and its prompt and filter the standard text keys with `Enter` and `Esc`. `Ctrl+Q` still quits. Every key is a plain key, so both key reportings send it (D-42).
- **Quick Add** ([editing.md §6](editing.md#6-quick-add-fr-mode-07-fr-mode-08)): standard text keys, `Enter` adds, `Esc` cancels, `Backspace` at the start of the text deletes the task's dash, `/` at the start of the text or after a space opens the project picker, which takes the keys of pickers above, except that `Esc`, or a filter that matches nothing, closes it and types `/` and the filter, and `Backspace` on an empty filter closes it. Every key is a plain key (D-42).
- **Prompts** (dates, Save As path): standard text keys, `Enter` accepts, `Esc` cancels. In the Save As prompt `Tab` completes the folder being typed, and when the file exists `Enter` replaces it and `Esc` goes back to the path.
- **External-change conflict prompt** ([file-io.md §3](file-io.md#3-external-changes)): `Ctrl+K` keeps mine, `Ctrl+R` takes theirs, `Ctrl+L` shows or hides the diff (D-115: the prompt can open while the user types, so no plain letter answers it), `j`/`k`/arrows scroll the diff a line and `Ctrl+D`/`Ctrl+U`/`PageDown`/`PageUp` a page, `Ctrl+Q` quits. Other keys do nothing; the prompt has no `Esc`, because saving stays paused until a choice is made.

## 5. Configuration

```toml
[keymap.navigate]
"alt+shift+up" = "move_up"
"x" = "none"              # unbind

[keymap.edit]
"ctrl+g" = "inline_code"
```

- The file is `config.toml` in `$XDG_CONFIG_HOME/taskpaper-terminal/`, or `~/.config/taskpaper-terminal/` when that is unset, also on macOS (FR-SET-01). The contexts are `navigate` and `edit`.
- Keys are written as `mod+mod+key` in lower case (`ctrl`, `alt`, `shift`), or as a sequence separated by spaces (`"g g"`). Key names are those of the tables above in lower case (`enter`, `esc`, `tab`, `space`, `up`, `pageup`, `f10`, `menu`, …); a capital letter is the same as `shift+` with the letter. Edit mode binds single keys only.
- Values are command names, as shown in the command palette, of a command of that context; case, spaces and punctuation do not matter, so `Move Up`, `move up` and `move_up` are the same name. `"none"` unbinds the key. The sidebar's old names `toggle_sidebar` and `focus_sidebar` still name Open Navigator, so a file written before D-79 keeps working, and `show_details` names Edit Node (D-103).
- A binding takes the key from the command it gave and adds it before the command's other keys, which stay bound. The `?` panel, Go to Command and the context menu show the keys as configured.
- Unknown commands, malformed keys and a key bound twice in one context (such as `"G"` and `"shift+g"`) are reported at startup and ignored, leaving that key's default; the app still starts. Problems are shown once, in the status line, as are unknown settings and invalid values elsewhere in the file (FR-SET-04).

## 6. Validation

Every binding must be verified in the terminals listed in N-PORT-01, including Windows Terminal (FR-KEY-03), with and without the kitty protocol and inside tmux, before the keymap is accepted. Bindings that a common terminal or desktop environment swallows get a different key; the results are recorded in this file.

Keys that common terminals, multiplexers and desktops take for themselves by default, and the route beside each that they leave alone (D-90). The terminals checked are macOS Terminal, iTerm2, Ghostty, kitty, WezTerm, Alacritty, GNOME Terminal and Windows Terminal, and tmux inside them; a key not listed here reaches the app in all of them. Every command keeps a route that no listed program takes and both key reportings send (D-42), except pasting, where a terminal that takes `Ctrl+V` pastes the clipboard itself as pasted text, which the app takes the same way.

| Key | Taken by | Route beside it |
|---|---|---|
| `Ctrl+↓` / `Ctrl+↑` (siblings) | macOS: Mission Control and application windows | `gj` / `gk` |
| `Ctrl+←` / `Ctrl+→` (words, edit mode) | macOS: switching Spaces | `Alt+B` / `Alt+F` |
| `Alt+Enter` (new pane from the Navigator and Go to Symbol) | Windows Terminal and WezTerm: full screen | `Ctrl+T` |
| `Ctrl+V` (new pane from the Navigator and Go to Symbol; paste) | Windows Terminal: paste | `Ctrl+T`; paste arrives as pasted text |
| `Ctrl+Shift+←` / `Ctrl+Shift+→` (pane left / right; select a word in edit mode) | kitty: tabs; WezTerm: panes | `Ctrl+W ←` / `Ctrl+W →`, `Ctrl+W h` / `Ctrl+W l`; in edit mode, `Shift+←` / `Shift+→` |
| `Ctrl+Shift+Z` (redo) | kitty: previous prompt; WezTerm: zoom pane | `Z`, `Ctrl+Y` |
| `Alt+←` / `Alt+→`, `Alt+↑` / `Alt+↓`, `Alt+Shift+←` / `Alt+Shift+→` | Windows Terminal, between its own split panes | `>` / `.`, `Alt+k` / `Alt+j`, `H` / `L` |
| `Alt+1` … `Alt+9` (focus pane N) | GNOME Terminal and Ghostty on Linux: tabs | `g1` … `g9` |
| `Shift+F10` (context menu) | GNOME Terminal: its own menu | `\`; in the Navigator, `Ctrl+R` (D-101) |
| `Ctrl+B` (bold, edit mode) | tmux: its prefix | `Ctrl+B` twice in tmux, which sends it on |
