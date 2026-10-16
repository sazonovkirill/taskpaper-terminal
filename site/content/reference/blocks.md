+++
title = "Blocks and their types"
weight = 20
description = "Projects, tasks and text, with sections, dividers, links and Markdown."
+++

Every line of the file is one **block**, the thing you move between, select and edit. A block has one of TaskPaper's three types, and nothing else decides it:

| Type | In the file | On screen |
|---|---|---|
| Project | a line ending in `:`, such as `Launch:` | a bold heading in the theme's primary color, with its `:` |
| Task | a line starting with `- ` | a `-` marker and the text; a done task shows `✔`, dimmed and struck through |
| Text | any other line, blank lines included | slightly dimmed text. The app calls a text block a **Note** in menus |

A line's type is read the way TaskPaper reads it. A line starting with `- ` is a task even if it ends in `:`. Tags never change the type. Blocks are indented two columns per level, whatever their type.

## Sections

A **section** is a text block tagged `@section`. It is drawn as a bold italic header in the theme's headers color, and the blocks indented under it are its children. `S` makes the selected text blocks sections, or plain text again when every one already is a section. Tasks and projects can't be sections: `S` leaves them as they are, and the status line says how many it skipped ("Only text can be a section: 1 skipped"). An `@section` tag on a task or project is kept but does nothing.

You can also add a section with the [creation commands](@/reference/modes.md#creating-blocks) in Go to Command (**New Section Below**, …), from the command menu while editing, and with `Ctrl+T` while editing.

## Dividers

A text block whose text is exactly `---` is a **divider**. It is drawn as a faint horizontal rule across the pane. `---Title---` (or `--- Title ---`) is a **titled divider**, with its title in the middle of the rule. Only dashes, such as `------`, stay plain text. A divider is still a text block: you select, move and edit it like any other, and while you edit it shows its raw text.

## Text formatting and links

Inside a block's text, tpt renders a little Markdown. In navigate mode the markers are hidden. While you edit, they show as typed.

| You type | You see |
|---|---|
| `**bold**` | **bold**, in the block's own color |
| `` `code` `` | code, dimmed |
| `# Heading` … `###### Heading`, at the start of a text block | a heading: level 1 bold and underlined, levels 2 to 6 bold |
| `[title](https://…)` | *title* as a link |
| `<https://…>` or a bare `https://…` | the address as a link |

Links are drawn in the link color and underlined in every block but a divider. A block may also carry an `@url(…)` tag, as other TaskPaper apps write it. The tag is hidden, and the block's whole title is drawn as a link to it. `gx` opens the focused block's link in your system browser (`open` on macOS, `xdg-open` on Linux): its `@url` address first, else the first link in its text that has a scheme such as `https:`.

Italic, lists, quotes, tables and fenced code are plain text in blocks. They do render in [notes](@/reference/attachments.md).

## Tags are hidden

Every tag, `@due(…)`, `@done`, `@context(phone)` alike, is hidden from the block's text. Tags tpt understands show their effect instead: a date chip, a star, a dimmed done block. Tags it doesn't understand are kept in the file untouched. The [Edit Node dialog](@/reference/tags.md#edit-node) is the one place where you see and type tags. While you edit text, you can't type a tag: an `@` at the start of a word is refused with the hint "Tags are set in Edit Node (e)". An `@` inside a word, as in `me@example.com`, is fine.

## Changing a block's type

`my` opens the type picker, titled "Node type:", with Project, Task, Note, Make Section and Remove Section. The choice applies to every selected block, else the focused one, as one undo step. Changing the type rewrites only the line's syntax: it adds or removes the task's `- ` or the project's `:`, and drops `@section`. Every other tag stays, `@url` included. A divider stays a divider.

While you edit, you change the type by typing: `- ` at the start makes a task, a `:` at the end makes a project, and deleting them makes text again. `Ctrl+T` and the command menu change it too. See [Navigate and edit mode](@/reference/modes.md#changing-the-type-while-editing).

{{ keys(topic="blocks") }}

## Hidden lines

A line tagged `@hidden` and everything indented under it is a **special node**: data the app keeps but never shows as blocks. tpt uses special nodes for [notes and comments](@/reference/attachments.md). Special nodes written by other tools are preserved byte for byte and listed under Hidden in a block's [Edit Node dialog](@/reference/tags.md#edit-node). They move, copy and delete with the block that owns them.
