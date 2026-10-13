+++
title = "Tags, dates and done"
weight = 60
description = "Completing blocks, start and due dates, colors, stars, flags, bookmarks, and the Edit Node dialog that shows every tag."
+++

TaskPaper stores a block's metadata as tags on its line: `@done(2026-10-11)`, `@due(2026-10-14)`, `@green`. tpt never shows tags in the outline and never lets you type one into a block's text. Commands set and clear them, and their effects show instead: a done block is struck through, a date is a chip, a color tints the row. The one place to see and type any tag is the [Edit Node](#edit-node) dialog.

Typing `@` at the start of a word, while editing, does nothing but say "Tags are set in Edit Node (e)" in the status line. Tags tpt doesn't read are kept as they are, hidden like the rest, and listed in Edit Node.

| Tag | Means | Set with |
|---|---|---|
| `@done(DATE)` | Done, on that day. Any `@done`, with or without a date, counts. | `x` |
| `@start(DATE)` | Start date | `t`, `w`, `[`, `]`, `ms` |
| `@due(DATE)` | Due date | `T`, `W`, `{`, `}`, `md` |
| `@red` `@orange` `@yellow` `@green` `@blue` `@violet` | Color, at most one | `c1` … `c6`, `cc`, `mc`, `C` |
| `@star` | Starred | `s` |
| `@flag` | Flagged | `!`, `mf` |
| `@bookmark` | Listed under Bookmarks in the [Navigator](@/reference/navigator.md) | `mb` |
| `@section` | A [section](@/reference/blocks.md#sections), on text blocks | `S` |
| `@url(ADDRESS)` | The block's [link](@/reference/blocks.md#text-formatting-and-links) | Edit Node |
| `@hidden` | A [hidden line](@/reference/blocks.md#hidden-lines) the app keeps but never shows | never; reserved |

Every command below acts on the [selected blocks](@/reference/structure.md#selection), or the focused one, as one step to undo. Where a command adds or removes a tag, it adds it to every block that lacks it, and removes it only when every block has it.

## Done

`x` marks a block done, adding `@done` with today's date. On blocks that are all done already, it removes `@done`. With the kitty protocol, `Ctrl+Enter` does the same.

Any block can be done, not only tasks. A done block keeps its type, and is dimmed and struck through; a done task's `-` becomes `✔`. Everything inside a done block, at any depth, is dimmed too, but not struck through, since only the done block carries `@done`.

There is no archive. An `Archive:` project is an ordinary project.

### Showing completed blocks

`Ctrl+O` or `gc` hides the completed blocks in the focused pane, each with everything inside it, and shows them again. Each pane has its own setting, which starts on showing them and is saved with the [pane layout](@/reference/files.md#pane-layout). A new pane takes the setting of the pane it opened from.

{{ keys(topic="tags", group="View and Panels") }}

## Dates

A block can have a **start date** (`@start`) and a **due date** (`@due`). Dates are days, written `YYYY-MM-DD` in the file, with no time.

### Date chips

Dates show as chips at the right end of the block's first row, the start date before the due date. A start chip is green, a due chip red. Each names the date by how far it is from today:

| Chip | Date |
|---|---|
| `today`, `tomorrow`, `yesterday` | those days |
| `Tue` | 2 to 6 days ahead |
| `next Tue` | 7 to 13 days ahead |
| `in 15 days` | further ahead |
| `3 days ago` | further back |

A due date before today is **overdue**: its chip gets its own icon and is bold and reversed, unless the block is done or inside a done block. Chips follow the date: at midnight they change by themselves. A tag whose value is not a date shows no chip, and the block being edited shows none.

### Setting dates

| Key | Does |
|---|---|
| `t` / `w` | Start today / tomorrow |
| `T` / `W` | Due today / tomorrow |
| `[` / `]` | Start one day earlier / later |
| `{` / `}` | Due one day earlier / later |
| `ms` / `md` | Open the date prompt for the start / due date |

Moving a date a day works from today when the block has none, so `]` on an undated block starts it tomorrow. **Clear Start Date** and **Clear Due Date** have no key: run them from [Go to Command](@/reference/navigator.md#go-to-command), the context menu or the edit-mode [command menu](@/reference/modes.md#the-command-menu), or empty the date prompt.

### The date prompt

`ms` and `md` open a prompt in a panel above the status line, titled with the date it sets, and dim the rest of the screen. It opens with the block's current date typed and selected, so typing replaces it and `Enter` keeps it. Below the text it previews the date the text means, or says why it is not a date, in which case `Enter` does nothing. Empty text removes the date.

{{ context_keys(table="DATE_PROMPT") }}

The prompt takes the [text keys](@/reference/modes.md#text-keys) too.

### Date grammar

The date prompt, the Edit Node dialog's date rows and [Quick Add](@/reference/quick-add.md) all read dates the same way. Case doesn't matter, and every form is relative to today's local date:

| Type | Means |
|---|---|
| `2026-10-14` | that day |
| `10-14`, `10/14` | that day this year, or next year once it has passed, for a due date |
| `today`, `tomorrow`, `yesterday` | those days |
| `fri`, `friday`, `this fri` | the next Friday after today |
| `next fri` | the Friday 7 to 13 days ahead, the one the chip calls `next Fri` |
| `last fri` | the most recent Friday before today |
| `this week`, `next month`, `last year` | the first day of that week, month or year; weeks start on Monday |
| `weekend`, `this weekend` | the coming Saturday, or today on a Saturday |
| `next weekend`, `last weekend` | a week after or before it |
| `+3d`, `2 weeks`, `-1 month`, `1y` | that far from today: `d` days, `w` weeks, `m` months, `y` years |
| `fri +1w` | that far from a date |

Since there are no times, `m` means months, not minutes as in TaskPaper. Dates run from `0000-01-01` to `9999-12-31`.

## Colors

A block can have one color, which tints its row's background with a very dim shade from its marker to the pane's right edge. Text colors stay as they are. The colors, in order, are red, orange, yellow, green, blue and violet, numbered 1 to 6.

- `c1` … `c6` give that color, and `C` removes it.
- `cc` cycles the focused block's color: none, red, orange, yellow, green, blue, violet, and back to none. Selected blocks all get the same next color.
- `mc` opens the color picker, with each color's number. Type a number to choose at once, or type a name to filter.

Setting a color removes any other color tag. The [Colors view](@/reference/views.md#colors) lists every colored block, grouped by color.

## Star, flag and bookmark

- `s` **stars** a block, shown as a star after its text. Starred blocks have their own [view](@/reference/views.md#starred), and the [Today view](@/reference/views.md#today) lists them too, unless you turn that off.
- `!` or `mf` **flags** a block, shown as a red flag after its text. There is one flag, and it is red. Flagged blocks have their own [view](@/reference/views.md#flagged).
- `mb` **bookmarks** a block, shown as a bookmark icon after its text, and lists it under Bookmarks in the [Navigator](@/reference/navigator.md). The status line says "Bookmarked" or "Bookmark removed".

The flag, star and bookmark come in that order, each a space after the text. They are hidden while the block is edited.

{{ keys(topic="tags", group="Block Metadata") }}

## Edit Node

`e` or `mi` opens the **Edit Node** dialog on the focused block, the zoom title included, also while blocks are selected and in the special views. **Edit Node** in the edit-mode [command menu](@/reference/modes.md#the-command-menu) opens it on the block being edited, and editing resumes when it closes. It shows every property of the block, one row each, and changes any of them:

| Row | Shows | Change it with |
|---|---|---|
| Text | The block's text, as edit mode edits it, without a project's `:` | Typing. An `@` that would start a tag is not typed: tags go in the Tags rows. |
| Type | Project, Task or Note | `Space` or `→` for the next of task, note and project, `←` the previous, or `p`, `t`, `n` |
| Done | `Not done`, or `Done` and its date | `Space` or `x` |
| Parent | The path to the block's parent, or `Home` | `Space` or typing opens a finder over the blocks it can move under, as [Move to…](@/reference/structure.md#move-to) lists them |
| Start, Due | The date, such as `Thu 8 Oct 2026 · Today`, or `None` | `Space` opens a field with the date; typing opens it empty. `←`/`→` move the date a day. `Backspace` or `Delete` removes it. |
| Starred, Flagged | Whether the block has `@star` or `@flag` | `Space`, or `s` and `!` |
| Tags | Every other tag, one per row, as `@name` or `@name(value)`. Tags tpt doesn't read are marked "unsupported". | `Space` opens a field holding the tag; typing opens it empty. `Backspace` or `Delete` removes the tag. |
| + Add tag | The row after the tags | Typing a tag here adds it. A name the block already has changes that tag. |
| Hidden | The [hidden lines](@/reference/blocks.md#hidden-lines) under the block, each with its tags and line count | Nothing: the focus skips it |

Sections and dividers show as notes in the Type row; a section's `@section` is among its tags. In a tag field, a name and a value in parentheses, such as `context(phone)`, sets a value. A leading `@` is dropped. `id`, `text`, `type` and `hidden` can't be tag names here: TaskPaper reserves the first three, and `@hidden` marks hidden lines.

`Enter` closes the dialog and applies every change as one step to undo; `Esc` closes it and changes nothing. While a field or the parent finder is open, `Enter` accepts it and `Esc` leaves it, and the dialog stays open. The last line of the dialog names the keys for the focused row. If the file is [reloaded from disk](@/reference/files.md#when-the-file-changes-on-disk) while the dialog is open, it closes without changing anything.

{{ context_keys(table="NODE_EDITOR") }}

Every key in the dialog is a plain key or `Ctrl` with a letter, so it works in any terminal. `Ctrl+Q` still quits.
