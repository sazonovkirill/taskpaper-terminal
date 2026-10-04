# Spec: Key bindings

Covers F-CMD-01–05 and N-PORT-03. The edit model is modeless (OQ-06 default): typing always edits the cursor item, and structure commands use modifiers, as in Workflowy.

## 1. Terminal constraints

- **No Cmd key.** Mac Workflowy bindings with Cmd are mapped to their Windows/Linux equivalents.
- **Legacy key reporting** cannot distinguish `Ctrl+Enter`, `Shift+Enter`, `Ctrl+Shift+<letter>`, `Ctrl+I` vs `Tab` or `Ctrl+M` vs `Enter`. When the terminal supports the kitty keyboard protocol, enable it (crossterm `PushKeyboardEnhancementFlags` with `DISAMBIGUATE_ESCAPE_CODES | REPORT_ALTERNATE_KEYS`) and use the primary bindings. Every primary binding that needs it has a **fallback** that works with legacy reporting; both are active at once.
- **Alt on macOS** requires "Use Option as Meta" (Terminal.app) or "Option sends Esc+" (iTerm2); Ghostty, kitty and WezTerm have equivalent settings. The help screen says so when no Alt key has been seen.
- Raw mode disables `ISIG` and `IXON`, so `Ctrl+Z`, `Ctrl+S`, `Ctrl+Q` and `Ctrl+C` arrive as keys.
- Some desktop environments capture `Ctrl+Up/Down` and `Alt+Left/Right`; each such binding also has an alternative.

## 2. Default keymap

"Workflowy" names the Windows/Linux binding from [../reference/workflowy.md §27](../reference/workflowy.md#27-complete-default-keyboard-shortcuts).

### Editing structure

| Command | Primary | Fallback | Origin |
|---|---|---|---|
| New item / split | `Enter` | | Workflowy |
| New child note | `Shift+Enter` | `Alt+Enter` | Workflowy (note) |
| Indent / outdent | `Tab` / `Shift+Tab` | | Workflowy |
| Move up / down | `Alt+Shift+↑` / `Alt+Shift+↓` | `Ctrl+Alt+K` / `Ctrl+Alt+J` | Workflowy |
| Toggle done | `Ctrl+Enter` | `Ctrl+D` | Workflowy / TaskPaper ⌘D |
| Delete item | `Ctrl+Shift+Backspace` | `Alt+Delete` | Workflowy |
| Duplicate | `Alt+Shift+D` | | Workflowy |
| Move to… | `Alt+M` | | (Workflowy `Alt+Ctrl+M`) |
| Tag with… | `Ctrl+T` | | TaskPaper ⌘T |
| Toggle `@today` | `Alt+T` | | (TaskPaper ⌘Y) |
| Set due date | `Alt+Shift+T` | | |
| Archive @done | `Alt+Shift+A` | | TaskPaper ⇧⌘A |
| Undo / redo | `Ctrl+Z` / `Ctrl+Shift+Z` | `Ctrl+Y` for redo | Workflowy |
| Copy / cut / paste | `Ctrl+C` / `Ctrl+X` / `Ctrl+V` | bracketed paste | |
| Select items | `Shift+↑` / `Shift+↓` | | |
| Select all | `Ctrl+A` | | |

### Navigation and view

| Command | Primary | Fallback | Origin |
|---|---|---|---|
| Expand / collapse | `Ctrl+↓` / `Ctrl+↑` | `Alt+J` / `Alt+K` | Workflowy |
| Toggle fold | `Ctrl+Space` | | Workflowy |
| Expand / collapse subtree | `Ctrl+Shift+↓` / `Ctrl+Shift+↑` | `Alt+Shift+J` / `Alt+Shift+K` | |
| Zoom in / out | `Alt+.` / `Alt+,` | `Alt+→` / `Alt+←` | Workflowy |
| Go home | `Alt+Home` | | (Workflowy `Ctrl+'`) |
| Back / forward | `Alt+Shift+←` / `Alt+Shift+→` | | (`Alt+[` is ambiguous with the CSI prefix in legacy mode) |
| Previous / next sibling | `Ctrl+Alt+↑` / `Ctrl+Alt+↓` | | Workflowy |
| Parent | `Alt+U` | | |
| Show/hide completed | `Ctrl+O` | | Workflowy |
| Toggle sidebar | `Alt+L` | | Workflowy |
| Page up / down | `PgUp` / `PgDn` | | |

### Search and commands

| Command | Primary | Origin |
|---|---|---|
| Search (filter) | `Ctrl+F` | (Workflowy `Esc`, TaskPaper ⇧⌘F) |
| Clear filter / selection / cancel | `Esc` | |
| Go to anything | `Ctrl+K` | Workflowy JumpTo |
| Command palette | `Ctrl+P` | (TaskPaper ⇧⌘P, Workflowy `Alt+W`) |
| Save | `Ctrl+S` | |
| Help | `F1` | |
| Quit | `Ctrl+Q` | |
| Suspend to shell (saves first) | unbound; command palette | (`Ctrl+Z` is Undo) |

Text editing inside an item uses the usual keys: arrows, `Home`/`End`, `Ctrl+←/→` or `Alt+B/F` by word, `Backspace`/`Delete`, `Ctrl+W` / `Alt+Backspace` delete word, `Ctrl+U`/`Ctrl+K`-style kills are **not** bound by default because `Ctrl+K` is Go to anything.

## 3. Configuration

```toml
[keymap]
preset = "default"        # or "vim"

[keymap.bindings]
"alt+shift+up" = "move_up"
"ctrl+d" = "toggle_done"
"ctrl+alt+j" = "none"     # unbind
```

- Keys are written as `mod+mod+key`, lower case; modifiers `ctrl`, `alt`, `shift`, `super`.
- Values are command names (the same names shown in the command palette).
- Unknown commands or malformed keys are reported at startup and ignored; the app still starts.
- A key bound to two commands in the same context is a config error reported at startup.

## 4. Validation

Every binding must be verified in the terminals listed in N-PORT-01, with and without the kitty protocol and inside tmux, before the keymap is accepted. Bindings that a common terminal or desktop environment swallows get a different fallback; the results are recorded in this file.

## 5. Vim preset (F-CMD-05)

Sketch only; detailed when OQ-06 is decided.
- Normal mode: `j`/`k` items, `h`/`l` collapse/expand, `>>`/`<<` indent, `J`/`K` move down/up, `dd` delete, `yy`/`p` copy/paste items, `x` toggle done, `o`/`O` new item below/above, `zo`/`zc`/`za`/`zR`/`zM` folds, `Enter`/`Backspace` zoom in/out, `/` search, `u`/`Ctrl+R` undo/redo.
- Insert mode: text editing; `Esc` back to normal.
