+++
title = "Navigate and edit mode"
weight = 30
description = "Keys act on blocks, or they type text: how the two modes, key sequences and the shortcuts panel work."
+++

tpt is a block editor. In **navigate mode**, the default, keys move between blocks, select them and run commands on them. In **edit mode**, keys type into one block's text, right where it sits in the outline. The status line shows the mode, and so does the focus marker in the gutter: the primary color while navigating, the accent color while editing.

## Keys in navigate mode

Navigate-mode keys are vi-inspired. `j`/`k` move, `h`/`l` fold and unfold, `gg`/`G` jump to the ends. Arrow keys work everywhere too. The full list of navigate keys is spread over the pages for each topic: [Navigation and zoom](@/reference/navigation.md), [Selection and structure](@/reference/structure.md), [Tags, dates and done](@/reference/tags.md), and so on.

### Key sequences

Many keys are two-key sequences: `gg`, `dd`, `ms`, `c1`, `Ctrl+W v`. Press the keys one after the other, within one second. While tpt waits for the second key, the first one shows at the right end of the status line. A key that continues no sequence cancels it and does nothing else. The exception is a first key that is a command of its own, such as `o`: that command runs, then the second key acts as usual.

The prefixes group keys by meaning: `g` goes somewhere, `m` sets metadata through a prompt or picker, `c` colors, `d` deletes, and `Ctrl+W` handles panes.

### Classic and kitty keys

Terminals report keys in two ways. Classic ("legacy") reporting can't tell some keys apart, for example `Ctrl+Enter` from `Enter`, or `Ctrl+I` from `Tab`. The kitty keyboard protocol can. tpt asks for the kitty protocol when it starts and hands it back when it quits. Terminals without it carry on with classic reporting.

Every command has at least one key that works with classic reporting. A few commands also have a key that only the kitty protocol can send. The key tables list those in the **Kitty protocol only** column:

| Kitty protocol only | Command | Works everywhere |
|---|---|---|
| `Ctrl+Enter` | Toggle Done | `x` |
| `Ctrl+'` | Go Home | `gh` |
| `Ctrl+M` | Move To | `gm` |
| `Ctrl+\` | Open in New Pane | `Ctrl+W v` |
| `Menu` | Context Menu | `\`, `Shift+F10`, or `Ctrl+R` in the Navigator |

### Alt on macOS

`Alt` keys need your terminal to send Option as Meta: "Use Option as Meta key" in Terminal.app, "Option sends Esc+" in iTerm2, with equivalents in Ghostty, kitty and WezTerm. The `?` panel reminds you until it has seen an `Alt` key. Every `Alt` key also has a route without `Alt`.

### Keys some terminals take

Some terminals and desktops keep certain keys for themselves. Each such key has an alternative next to it:

| Key | Taken by | Use instead |
|---|---|---|
| `Ctrl+↓` / `Ctrl+↑` | macOS Mission Control | `gj` / `gk` |
| `Ctrl+←` / `Ctrl+→` while editing | macOS Spaces | `Alt+b` / `Alt+f` |
| `Alt+Enter` in the Navigator and finders | Windows Terminal, WezTerm | `Ctrl+T` |
| `Ctrl+V` in the Navigator and finders | Windows Terminal | `Ctrl+T` |
| `Ctrl+Shift+←` / `Ctrl+Shift+→` | kitty, WezTerm | `Ctrl+W ←` / `Ctrl+W →`, `Ctrl+W h` / `Ctrl+W l` |
| `Alt` with arrows | Windows Terminal | `H` / `L`, `Alt+k` / `Alt+j` |
| `Alt+1` … `Alt+9` | GNOME Terminal and Ghostty on Linux | `g1` … `g9` |
| `Shift+F10` | GNOME Terminal | `\`, or `Ctrl+R` in the Navigator |
| `Ctrl+B` while editing | tmux | press `Ctrl+B` twice |

## Creating blocks

The creation keys add a block and open it in edit mode:

| Key | Adds a block |
|---|---|
| `o` / `O` | below / above the focused block |
| `i` / `I` | as its last / first child, expanding it |
| `a` / `A` | at the end / start of its list of siblings |

Follow any of them with `s`, `p`, `t` or `n` to add a section, project, task or note: `os`, `Ip`, `at`, `An` and so on. Alone, the key waits a second for that letter, then adds a block of the type that fits: a task next to a task or under a project or task, otherwise text. Any other key ends the wait at once and is typed into the new block, so `o` then `B` starts a block with "B".

With blocks selected, `o` and `a` go by the last selected block, `O` and `A` by the first. On a zoomed pane's title row, the keys add the zoom block's first child, except `i` and `a`, which add its last.

{{ keys(topic="modes", mode="navigate", group="Editing and Creating") }}

## Edit mode

`Enter` edits the focused block, with the caret at the end of its text. It is the only key that does. `Esc` goes back to navigate mode. Editing happens in place: the block keeps its indentation, its neighbors and the scroll position. Only the focused pane can be in edit mode, and moving to another pane ends it.

You edit the block's visible text:

- **without a task's dash.** The `-` marker shows the type. `Backspace` at the start of a task's text deletes the dash and makes the block text.
- **with a project's colon.** The `:` is part of the text, and the caret starts just before it. Deleting it makes the block text.
- **without its tags,** which stay on the line, hidden.
- **with Markdown markers visible,** such as `**`, `` ` ``, `#` and link syntax.

A block is always one line: no key breaks it. Pasting text of several lines ends editing and adds the lines as blocks below. A whole editing session is one undo step.

{{ keys(topic="modes", mode="edit") }}

| Key | While editing |
|---|---|
| `Enter` | Stop editing and add the next block, as a first child when the block has visible children |
| `↑` / `↓` | Stop editing and edit the previous / next visible block, at the same column |
| `Tab` / `Shift+Tab` | Indent / outdent the block and keep editing |
| `Backspace` in an empty block | Delete it and edit the previous block, unless it has children or is the first row |

`Ctrl+B` wraps the selection, or the word at the caret, in `**`, or removes them when the caret is inside bold text. `` Alt+` `` and `Ctrl+E` do the same with backticks. Undo and redo inside a session work on the text; characters typed in a row undo together.

### Text keys

These keys work in every text field: the block being edited, prompts, Quick Add, the Edit Node dialog and the note editor.

{{ context_keys(table="TEXT") }}

### The command menu

Typing `/` while editing opens the **command menu** at the caret, titled "Commands:". It lists every command for the block, in five groups:

- **Type:** Project, Task, Note, and Make Section on a text block, or Remove Section on a section
- **Status:** Complete or Uncomplete, Toggle Star, Toggle Flag, Bookmark or Remove Bookmark
- **Dates:** Set Start Date, Set Due Date, Start Today, Start Tomorrow, Due Today, Due Tomorrow, and Clear Start Date or Clear Due Date when the block has that date
- **Color:** Color: Red … Color: Violet, and Remove Color when it has one
- **Block:** Edit Node, Open Link when the block has a link, Open Attachments while notes or comments are on, Duplicate, Delete

Type to filter. Each word you type must start a word of the label, in order, so `/date` leaves the date entries and `/p` puts Project first. `↑`/`↓` or `Ctrl+P`/`Ctrl+N` move, `Enter` runs the highlighted entry, and `Backspace` on an empty filter closes the menu. `Esc`, or a filter that matches nothing, closes the menu and types `/` and the filter as text, so `/home` and `a / b` stay text.

A type change is part of the editing session. Any other entry runs its command, and editing resumes where the caret was, also after a date prompt or dialog it opened. A block the command takes out of view, such as one completed while completed blocks are hidden, stays put until you stop editing.

### Changing the type while editing

- Typing `- ` at the start makes a task. A `:` at the end makes a project, and so does a typed one. Deleting the dash or the colon makes the block text again.
- `Ctrl+T` cycles the block through project, section, task and note, keeping the text and caret.
- The command menu's Type group sets one directly.

Changing the type keeps every tag but `@section`.

## Copy and paste

`y` or `Ctrl+C` copies the selected blocks, else the focused one, as TaskPaper text with their children, tags and attachments. `p` or `Ctrl+V` pastes below, and `P` or `Alt+v` above, keeping the structure. Text of one line pastes as a text block. While you edit, `Ctrl+C`, `Ctrl+X` and `Ctrl+V` work on the selected text. If you paste the one block tpt copied last, you get just its words.

tpt uses the system clipboard when a display server is reachable. Otherwise, as over SSH, copying uses the terminal's OSC 52 (in tmux, enable `set-clipboard on`), and the terminal's own paste goes below the focused block or into the text you are editing.

## The shortcuts panel

`?` opens a panel on the right listing the most useful keys for where you are: navigate or edit mode, the Navigator, a picker, the date prompt, Quick Add or the Edit Node dialog. Keys you rebound show as you set them. The panel doesn't take keys, so you keep working while it is open. When its list is longer than the screen, `)` and `(` scroll it a page down and up. The panel also says whether the kitty keyboard protocol is active, and whether it has seen an `Alt` key yet.

{{ keys(topic="modes", mode="navigate", group="View and Panels") }}

## Mouse

The mouse is optional; everything has a key.

- Click a block to focus it. While editing, click in its text to place the caret.
- Click a fold marker to fold, a breadcrumb to zoom there, a Navigator entry to go there, a pane header to focus that pane.
- `Alt`+click a block or a breadcrumb to open it in a new pane.
- Right-click a block, the selection, a Navigator entry, a breadcrumb or a pane header to open its context menu.
- The wheel scrolls the pane under the pointer, 3 lines at a time, without moving the focus. Over the `?` panel it scrolls the panel.

While a dialog or prompt is open, the mouse does nothing, except that a click outside the open Navigator closes it.
