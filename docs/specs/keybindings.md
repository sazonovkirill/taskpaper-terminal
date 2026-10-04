# Spec: Key bindings

Covers FR-KEY, FR-MODE-01 and N-PORT-03. Navigate mode is vi-inspired with arrow-key equivalents (FR-KEY-02); edit mode uses standard text keys (FR-MODE-03, [editing.md §2](editing.md#2-edit-mode)).

## 1. Terminal constraints

- **No Cmd/Super** (FR-KEY-03). Bindings use `Ctrl`, `Shift` and `Alt` only.
- **Legacy key reporting** cannot distinguish `Ctrl+Enter`, `Shift+Enter`, `Ctrl+Shift+<letter>`, `Ctrl+I` vs `Tab`, `Ctrl+M` vs `Enter`, or `Ctrl+[` vs `Esc`. When the terminal supports the kitty keyboard protocol, enable it (crossterm `PushKeyboardEnhancementFlags` with `DISAMBIGUATE_ESCAPE_CODES`). Every binding that needs it has a fallback that works with legacy reporting; both stay active.
- **`Alt` on macOS** requires "Use Option as Meta" (Terminal.app) or "Option sends Esc+" (iTerm2); Ghostty, kitty and WezTerm have equivalents. The `?` panel says so when no `Alt` key has been seen. Every `Alt` binding also has a non-`Alt` route (a vi-style key, or the command palette).
- **`Alt+[`** is ambiguous with the CSI prefix in legacy mode and is not used.
- Raw mode disables `ISIG` and `IXON`, so `Ctrl+Z`, `Ctrl+S`, `Ctrl+Q` and `Ctrl+C` arrive as keys.
- `Ctrl+H` is `Backspace` in legacy mode and is not used.
- Multi-key sequences (`gg`, `dd`, `za`, `Ctrl+W h`) time out after 1 s; the pending prefix is shown in the status line.

## 2. Navigate mode

### Moving (FR-NAV-01)

| Command | Keys |
|---|---|
| Next / previous visible block | `j` / `k`, `↓` / `↑` |
| Collapse, or go to parent if already collapsed or a leaf | `h`, `←` |
| Expand, or go to first child if already expanded | `l`, `→` |
| Next / previous sibling | `}` / `{`, `Ctrl+↓` / `Ctrl+↑` |
| Parent | `gp` |
| First / last block | `gg` / `G`, `Home` / `End` |
| Page down / up | `Ctrl+D` / `Ctrl+U`, `PgDn` / `PgUp` |

### Folding (FR-NAV-02)

| Command | Keys |
|---|---|
| Toggle fold | `Space`, `za` |
| Expand / collapse subtree | `zO` / `zC` |
| Expand / collapse all | `zR` / `zM` |

### Zoom and history (FR-NAV-03, FR-NAV-04)

| Command | Keys |
|---|---|
| Zoom into focused block | `Alt+l`, `Alt+→`, `Alt+.`, `gz` |
| Zoom out | `Alt+h`, `Alt+←`, `Alt+,`, `gu` |
| Go home | `gh` |
| Back / forward | `H` / `L`, `Alt+Shift+←` / `Alt+Shift+→` |

### Editing and creating (FR-MODE)

| Command | Keys |
|---|---|
| Edit focused block (caret at end / at start) | `Enter` or `a` / `i` |
| New block below / above | `o` / `O` |
| New child | `Alt+o` |

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
| Indent / outdent | `Tab` / `Shift+Tab`, `>` / `<` |
| Move up / down | `Alt+k` / `Alt+j`, `Alt+↑` / `Alt+↓` |
| Group | `gG` |
| Move to… | `m` |
| Delete | `dd`, `Delete` |
| Duplicate | `yd` |
| Copy / cut | `yy`, `Ctrl+C` / `Ctrl+X` |
| Paste below / above | `p` / `P`, `Ctrl+V` |
| Undo / redo | `u` / `Ctrl+R`, `Ctrl+Z` / `Ctrl+Shift+Z` (fallback `Ctrl+Y`) |

### Block metadata (FR-BLK-08, FR-TAG, FR-DONE-01)

A `t` prefix groups metadata commands so they are easy to discover in the `?` panel.

| Command | Keys |
|---|---|
| Toggle done | `x`, `Ctrl+Enter` |
| Toggle today | `tt` |
| Set start / due date | `ts` / `td` |
| Set color | `tc` |
| Toggle bookmark | `tb` |
| Convert block type | `ty` |
| Details (all tags) | `ti` |
| Comments panel (FR-SPC-02) | `c` |
| Open URL | `gx` |

### View, panes and panels

| Command | Keys |
|---|---|
| Show/hide completed in this pane (FR-DONE-02) | `Ctrl+O`, `gc` |
| Toggle word wrap (FR-UI-05) | `Alt+z`, `gw` |
| Toggle sidebar / focus sidebar (FR-UI-01) | `Ctrl+B` / `Alt+0` |
| Shortcuts panel (FR-KEY-04) | `?` |
| Go to Symbol (FR-NAV-05) | `Ctrl+T`, `gs` |
| Go to Command (FR-NAV-06) | `Ctrl+P`, `:` |
| Settings window (FR-SET-02) | `g,` |
| Context menu for the focused element (FR-KEY-06) | `M`, `Shift+F10`, the Menu key |
| Open focused block in a new pane (FR-UI-03) | `Ctrl+\`, `Ctrl+W v` |
| Open this pane's location in a new pane | `Ctrl+W s` |
| Focus pane N | `Alt+1` … `Alt+9` |
| Focus pane left / right | `Ctrl+W h` / `Ctrl+W l` |
| Close pane | `Ctrl+W c` |
| Narrow / widen pane | `Ctrl+W <` / `Ctrl+W >`, `Alt+Shift+h` / `Alt+Shift+l` |
| Search (Later, FR-SRCH-01) | `/` |
| Follow link / copy link / show backlinks (Later, FR-OPS-10, [links.md](links.md)) | `gd` / `gy` / `gb` |
| Count blocks and words | command palette |
| Save As (FR-DOC-06) | command palette |
| Quit | `Ctrl+Q`, `ZZ` |

## 3. Edit mode

Standard text keys as listed in [editing.md §2](editing.md#2-edit-mode), plus:

| Command | Keys |
|---|---|
| End editing | `Esc` |
| End editing and create the next block | `Enter` (code blocks: inserts a line break) |
| Line break in a note or code block | `Shift+Enter`, `Alt+Enter` |
| Indent / outdent block | `Tab` / `Shift+Tab` |
| Bold / inline code (FR-FMT-02) | `Ctrl+B` / `` Alt+` `` |
| Insert link (Later, FR-OPS-10) | `Ctrl+K`, typing `[[` |
| Undo / redo (text, within the session) | `Ctrl+Z` / `Ctrl+Shift+Z` (fallback `Ctrl+Y`) |

## 4. Other contexts

- **Context menu:** opens on the focused block or selection, or on the focused sidebar entry, crumb or pane header; `↑`/`↓` move, typing filters, `Enter` runs, `Esc` closes. `M`, `Shift+F10` and the Menu key open it in every navigate context.
- **Sidebar focused:** `j`/`k`/arrows move, `Enter` zooms the focused pane to the entry, `Alt+Enter` opens it in a new pane, `h`/`l` fold project entries, `Esc` returns to the pane.
- **Pickers and palettes:** type to filter, `↑`/`↓` or `Ctrl+P`/`Ctrl+N` move, `Enter` chooses, `Alt+Enter` opens the choice in a new pane where that applies, `Esc` cancels.
- **Comments panel:** keys as in [special-nodes.md §2.3](special-nodes.md#23-commands).
- **Settings window:** keys as in [ui.md §8](ui.md#8-settings-window-fr-set-02-fr-set-03).
- **Prompts** (dates, URL, Save As path): standard text keys, `Enter` accepts, `Esc` cancels.

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
