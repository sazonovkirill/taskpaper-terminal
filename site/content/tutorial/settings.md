+++
title = "Settings, themes and next steps"
weight = 10
description = "Make tpt yours, and find out where to learn the rest."
+++

## Choose a theme

Press <kbd>Ctrl+T</kbd> for the **Theme** picker: Catppuccin, Tokyo Night, Nord, Gruvbox, Solarized Light and more. Move with <kbd>↓</kbd> and <kbd>↑</kbd> and the screen previews each theme. <kbd>Enter</kbd> keeps it; <kbd>Esc</kbd> goes back.

{{ shot(src="tutorial/settings", alt="The theme picker over the sample outline, with Tokyo Night highlighted and the outline behind it drawn in Tokyo Night's colors", caption="The theme picker previews each theme on your own outline.") }}

The default, Terminal Dark, uses your terminal's colors. The others need truecolor; without it the picker says so.

## The settings window

Press <kbd>g</kbd> <kbd>,</kbd> for **Settings**. Move with <kbd>j</kbd> and <kbd>k</kbd>, and press <kbd>Enter</kbd> to change a setting. You can:

- turn word wrap on, so long lines wrap instead of ending in `…`,
- turn icons off, if your font draws them as boxes,
- choose which special views the Navigator lists,
- change how soon tpt saves after you type, and how many daily backups it keeps.

Changes apply at once and are saved to `config.toml`, a text file whose path the footer shows; you can edit it by hand, for example to change keys. <kbd>Esc</kbd> closes the window.

## Where to go next

You don't need to remember every key.

- **Press <kbd>?</kbd>** to open the shortcuts panel on the right. It lists the most useful keys for where you are. <kbd>?</kbd> again closes it.
- **Press <kbd>Ctrl+P</kbd> or <kbd>:</kbd>** to open the command palette, **Go to Command**. It lists every command with its keys, including the few that have none. Type `wrap` and press <kbd>Enter</kbd> to run one.
- **Press <kbd>\\</kbd>** for the context menu of the focused line.
- **Read the [Reference](@/reference/_index.md).** It covers every feature, key and setting, including what this tutorial skipped, such as selection, copy and paste and Quick Add.

When you're ready, start your own: `tpt ~/notes.taskpaper` creates the file when you first add something.
