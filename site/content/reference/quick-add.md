+++
title = "Quick Add"
weight = 100
description = "Adding a task or note to the inbox, with dates typed in plain words, without leaving the view you are in."
+++

**Quick Add** adds a task or a note to the [inbox](@/reference/navigator.md#the-inbox), the INBOX project, without moving the panes. `q` opens it from anywhere in navigate mode, the special views included.

When there is no inbox, or its setting is off, `q` changes nothing, and the status line says "No INBOX project in the Navigator". A read-only file refuses Quick Add.

{{ keys(topic="quick-add") }}

## The panel

Quick Add is a panel above the status line, titled `Quick Add → INBOX`. The first line holds the text, and the second what `Enter` would do: the dates found, such as `Start Thu 8 Oct · Due Mon 12 Oct`, or what is missing: "Type the text", or "No INBOX project in the Navigator" when the inbox is gone.

- **Task or note.** The text starts as a task, with its dash in front. `Backspace` at the start deletes the dash and makes it a note, and `- ` typed at the start makes it a task again. Colons ending a note's text are dropped, so it doesn't become a project.
- **Text.** The [text keys](@/reference/modes.md#text-keys) edit it. Nothing breaks the line: a paste's line breaks become spaces. An `@` that would start a tag is not typed.

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

`Enter` adds the block, once there is some text besides the dates. The dates that count and the spaces around them leave the text and become `@start` and `@due` tags. The block goes in as the inbox's last child, without expanding anything, as one step to undo. The panel closes, and the status line says where the block went: "Added to INBOX". `Esc` closes the panel and adds nothing.

If the file is [reloaded from disk](@/reference/files.md#when-the-file-changes-on-disk) while Quick Add is open, the panel stays and adds to the reloaded file's inbox.

The inbox's setting is with the [Navigator](@/reference/navigator.md#the-inbox).
