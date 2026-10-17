+++
title = "Quick Add"
weight = 100
description = "Adding a task or note to the inbox or another project, with dates typed in plain words, without leaving the view you are in."
+++

**Quick Add** adds a task or a note to a project, the [inbox](@/reference/navigator.md#the-inbox) unless you choose another, without moving the panes. `q` opens it from anywhere in navigate mode, the special views included.

When there is no inbox, `q` opens Quick Add with the project picker already open. When the file has no projects at all, `q` changes nothing, and the status line says "No projects". A read-only file refuses Quick Add.

{{ keys(topic="quick-add") }}

## The panel

Quick Add is a panel above the status line, titled `Quick Add →` and the path of the project it adds to. The first line holds the text, and the second what `Enter` would do: the dates found, such as `Start Thu 8 Oct · Due Mon 12 Oct`, or what is missing: "Type the text", or "Choose a project with Tab" when there is no project to add to.

- **Task or note.** The text starts as a task, with its dash in front. `Backspace` at the start deletes the dash and makes it a note, and `- ` typed at the start makes it a task again. Colons ending a note's text are dropped, so it doesn't become a project.
- **Text.** The [text keys](@/reference/modes.md#text-keys) edit it. Nothing breaks the line: a paste's line breaks become spaces. An `@` that would start a tag is not typed.
- **Project.** `Tab` opens a picker of the projects just above the panel, listed as [Go to Project](@/reference/navigator.md#go-to-project) lists them and filtered as you type, with the current one highlighted. `Enter` makes the highlighted project the one to add to, and `Esc` closes the picker and keeps the project you had. `/` is typed as text.

{{ context_keys(table="QUICK_ADD") }}

## Dates in the text

Words in the text that read as a date, by the [date grammar](@/reference/tags.md#date-grammar), become the block's start date. After the word `due`, they become its due date.

On Sunday 11 October 2026:

| You type | Adds |
|---|---|
| `Buy milk tomorrow` | `- Buy milk @start(2026-10-12)` |
| `Pay rent due next Mon` | `- Pay rent @due(2026-10-19)` |
| `Plan trip next weekend` | `- Plan trip @start(2026-10-24)` |
| `Buy sun screen` | `- Buy sun screen` |
| `Notes from yesterday` | `- Notes from yesterday` |

The dates that count are highlighted as you type, a start date in the start chip's green and a due date, `due` included, in the due chip's red. Some words stay text so that ordinary sentences survive:

- A run starting with a bare number, such as `3 days`, is not a date.
- A start date that is only a three-letter weekday doesn't count, so `sun screen` stays text. `this sun`, `next sun` and `due sun` are dates.
- A start date before today doesn't count, so `last week` and `yesterday` stay text. `due yesterday` is a date.
- With several start or due dates, the last of each counts.

## Adding

`Enter` adds the block, once there is some text besides the dates. The dates that count and the spaces around them leave the text and become `@start` and `@due` tags. The block goes in as the project's last child, without expanding anything, as one step to undo. The panel closes, and the status line says where the block went, such as "Added to Errands". `Esc` closes the panel and adds nothing.

If the file is [reloaded from disk](@/reference/files.md#when-the-file-changes-on-disk) while Quick Add is open, the panel stays and adds to the same project in the reloaded file, and the picker closes. If that project was deleted, choose another with `Tab`.

To choose which project is the inbox, use **Select Inbox Project**, with the [Navigator](@/reference/navigator.md#the-inbox).
