+++
title = "Settings"
weight = 140
description = "The settings window, every setting in config.toml with its default, and how to rebind keys."
+++

tpt keeps its settings in one file, `config.toml`, in `$XDG_CONFIG_HOME/taskpaper-terminal/`, or `~/.config/taskpaper-terminal/` when `XDG_CONFIG_HOME` is unset, on macOS too. You can change most settings in the settings window, which writes the file for you, or edit the file by hand. Key bindings are only in the file.

## The settings window

`g,` or **Open Settings** in Go to Command opens the **settings window**. Groups are on the left, Appearance and Files, and the group's settings on the right, each with its current value and a one-line description. A `*` marks a setting that differs from its default, and its value is bold. The window's footer shows where `config.toml` is.

- `j`/`k` or the arrows move, and `Tab` switches between the groups and the settings.
- `Enter` or `Space` toggles an on/off setting, or opens a value picker for a choice, or a prompt for a number or folder. `h`/`l` or `←`/`→` step through the choices and toggle on/off settings.
- `r` resets the setting to its default.
- `/` filters the groups and settings by name.
- `Esc` closes the window. `Ctrl+Q` still quits.

{{ context_keys(table="SETTINGS") }}

Every change applies at once and is written to `config.toml` straight away. Theme, icons, word wrap and the INBOX setting repaint the app behind the window, and the theme previews live as you move through the picker. A write that fails shows an error and keeps the value for the session.

Numbers and the backups folder are typed on the window's bottom line. A number opens selected, so typing replaces it. A number below its least, or above 4294967295, is refused with the reason, and the prompt stays open. An empty folder means the default one. The filter is typed there too:

{{ context_keys(table="SETTINGS_TEXT") }}

Both take the [text keys](@/reference/modes.md#text-keys).

{{ keys(topic="settings") }}

## Every setting

Each setting's row says what its key in `config.toml` takes and its default. `backups.*` settings go in a `[backups]` table, or as dotted keys.

### Appearance

{{ settings(group="Appearance") }}

### Files

{{ settings(group="Files") }}

A complete `config.toml` that sets everything to its default:

```toml
theme = "terminal-dark"
icons = "nerd"
word_wrap = false
special_inbox = true
special_today = true
today_starred = true
sidebar_upcoming = true
sidebar_overdue = true
sidebar_colors = true
sidebar_logbook = true
sidebar_starred = true
sidebar_flagged = true
attachments_notes = true
attachments_comments = true
animations = "full"
weather = true
weather_idle_minutes = 30
autosave_delay_ms = 1000

[backups]
enabled = true
# dir = "~/.local/share/taskpaper-terminal/backups"
keep = 30
```

## Key bindings {#setting-keymap}

Two tables in `config.toml` rebind keys: `[keymap.navigate]` for navigate mode and `[keymap.edit]` for edit mode. Each line binds a key to a command:

```toml
[keymap.navigate]
"alt+shift+up" = "move_up"
"g x" = "toggle_done"
"x" = "none"              # unbind x

[keymap.edit]
"ctrl+g" = "inline_code"
```

- **Keys** are written in lower case, modifiers first and joined by `+`: `ctrl`, `alt` and `shift`, as in `ctrl+d` or `alt+shift+h`. A capital letter is the same as `shift+` with the letter, so `"G"` and `"shift+g"` are one key. A sequence is its keys separated by spaces: `"g g"`, `"ctrl+w v"`. Edit mode binds single keys only, not sequences.
- **Key names** besides characters: `enter`, `esc`, `tab`, `space`, `backspace`, `delete`, `up`, `down`, `left`, `right`, `home`, `end`, `pageup`, `pagedown`, `menu` and `f1` to `f12`.
- **Commands** are named as Go to Command shows them. Case, spaces and punctuation don't matter: `Move Up`, `move up` and `move_up` are one name. Every key table in this Reference has a `config.toml` column with the name. `"none"` unbinds the key.
- A binding takes the key from whatever had it and puts it first among the command's keys. The command's other keys stay bound. The `?` panel, Go to Command and the context menu show your keys.
- Old names keep working: `toggle_sidebar` and `focus_sidebar` name `open_navigator`, and `show_details` names `edit_node`.

The Navigator, the finders, the dialogs and the prompts keep their own keys; only navigate and edit mode can be rebound. Each command's default keys are on the page for its topic, and all of them on the [cheat sheet](@/reference/cheatsheet.md).

## Problems in the file

tpt never refuses to start over `config.toml`. An unknown setting, a value of the wrong kind, an unknown command, a key it can't read or a key bound twice in one table is skipped, keeping its default, and reported once in the status line, starting `config.toml:`, such as:

```
config.toml: unknown setting `wrap`; [keymap.edit] "g g": edit mode binds single keys, not sequences
```
