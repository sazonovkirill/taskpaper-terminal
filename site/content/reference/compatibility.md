+++
title = "Compatibility with other TaskPaper apps"
weight = 160
description = "How tpt reads and writes the TaskPaper format, what it keeps untouched, and what its own tags look like in other apps."
+++

tpt reads and writes plain TaskPaper files, the format of [TaskPaper](https://www.taskpaper.com/) and many other apps and editor plugins. It follows the TaskPaper 3 parser's rules, so a file means the same thing in tpt as there, and you can switch between apps, or sync the file, freely.

## What tpt keeps exactly

tpt never rewrites your file to its own taste:

- **Unchanged lines are written back byte for byte.** Opening and saving a file you didn't edit gives the same bytes, even with mixed line endings or stray spaces.
- **Indentation:** tabs stay tabs, and a space-indented file stays space-indented, at its own width. New files use tabs.
- **Line endings, a byte-order mark and the final newline** stay as the file had them. New files get `\n`, no BOM and a final newline.
- **Tags tpt doesn't use** stay on their lines, hidden, and the [Edit Node](@/reference/tags.md#edit-node) dialog lists them, marked "unsupported".
- **Lines marked `@hidden`**, with everything under them, are kept as they are and never shown. See [Hidden lines](@/reference/blocks.md#hidden-lines).

tpt only opens UTF-8 text. It refuses anything else rather than risk writing back a damaged copy.

## How lines are read

| A line | Is |
|---|---|
| starting with `- ` | a task, also `- Ask: why?` |
| ending in `:`, before any tags | a project |
| anything else | a note, which tpt calls a text block, also lines starting with `* ` or `+ ` |
| empty | an empty text block, at the depth of the line after it |

- Indentation makes the outline: a deeper line is a child of the line above. A line indented several levels deeper than its parent keeps its extra tabs.
- In a space-indented file, one level is the smallest run of leading spaces in the file, as in TaskPaper. One stray line with fewer spaces skews every depth; tpt reproduces that, and warns in the status line when only one line has the smallest run.
- A tag is `@name` or `@name(value)`, after a space or at the start, and followed by a space or the end of the line. `me@example.com` holds no tag. Within a value, `\(` and `\)` are parentheses. Only the first of two tags with one name counts. `@id`, `@text` and `@type` are reserved by TaskPaper and read as plain text.

## Tags tpt uses

| Tag | In tpt | In other apps |
|---|---|---|
| `@done(2026-10-11)` | Completed on that day. Any `@done` counts, with or without a date. | The same, in TaskPaper |
| `@start(DATE)`, `@due(DATE)` | Start and due dates, as `YYYY-MM-DD` | Ordinary tags with dates, which TaskPaper can search |
| `@red` … `@violet` | A row color | Ordinary tags |
| `@star`, `@flag`, `@bookmark` | Starred, flagged, bookmarked | Ordinary tags |
| `@section` | Makes a text block a section | An ordinary tag |
| `@url(ADDRESS)` | Draws the block's title as a link to the address | As there |
| `@hidden` | A hidden line, such as a note or comments | An ordinary tag on a line that shows like any other |

Other apps show tpt's [attachments](@/reference/attachments.md) as plain lines: a `NOTE @hidden` or `COMMENTS @hidden` line with the note's lines or the comments under it. Their content is written so it never reads as a task or a project there.

tpt's view settings, the pane layout and the order of the Today view live outside the file, so the file holds only your outline.

## What tpt treats differently

- **No archive.** An `Archive:` project is an ordinary project, and there is no archive command. Completed blocks stay where they are, and the [Logbook](@/reference/views.md#logbook) lists the recent ones.
- **Tags are never typed.** tpt hides every tag from the text and sets them through commands. In another app, you see and type them as text.
- **No searches yet.** TaskPaper's saved searches (`@search(…)`) are kept, hidden, and listed in Edit Node as unsupported.
- **`@today`** means nothing to tpt: it is kept like any tag it doesn't read. Star a block, or give it a date, to put it in Today.
- **The outline stays well formed.** A block under an empty line would make that line read back differently, so tpt fills it with `…` and says so. Spaces typed at the start of a line, and empty lines at the end of a file indented under a project, are tidied the same way, each with a status message.

## Editing in another app at the same time

tpt watches the file. When another app or a sync tool changes it, tpt reloads it, keeping your place, if you have nothing unsaved; otherwise it asks whose version to keep and saves the other as a conflict copy. A save never overwrites a change it hasn't seen. See [When the file changes on disk](@/reference/files.md#when-the-file-changes-on-disk).
