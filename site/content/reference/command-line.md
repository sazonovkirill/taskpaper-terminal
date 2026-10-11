+++
title = "Command line"
weight = 150
description = "Starting tpt, its options, the messages it prints and the environment variables it reads."
+++

## Usage

```
usage: tpt [path]
       tpt --help | --version
```

| Command | Does |
|---|---|
| `tpt notes.taskpaper` | Opens the file. If it doesn't exist yet, tpt starts with an empty outline and creates the file on the first save. Its folder must exist. |
| `tpt` | Creates a new file in `/tmp`, named like `taskpaper-20261011-093000.taskpaper`, holding a demo outline that shows off every feature. The status line marks it as temporary until you keep it with [Save As](@/reference/files.md#save-as). |
| `tpt --help` | Prints the usage and exits. |
| `tpt --version` | Prints the version, such as `tpt 0.1.0`, and exits. |

tpt edits one file at a time. To work on two files, run two tpt.

Any other argument starting with `-` is refused, so a mistyped option never becomes a file name. To open a file whose name starts with `-`, write it as a path: `tpt ./-notes.taskpaper`. More than one path is refused too. Both print the reason and the usage, and exit with status 1:

```
$ tpt -x
Error: unknown option -x; open a file named like it as ./-x
usage: tpt [path]
...
```

A file tpt can't read, one that isn't UTF-8 text, or a folder it may not enter is reported before the app starts, and tpt exits with a non-zero status. A file you may not write opens **read-only**: the status line says `read-only`, and every edit says "cannot edit a read-only file".

## Before the app starts

If tpt crashed with changes it hadn't saved, it wrote them to a recovery file. The next time you open that file, tpt names the recovery file and asks:

```
Open the file with these changes? [y/N]
```

`y` opens the file with the recovered changes as unsaved changes, after keeping the file as it is on disk as a conflict copy. Any other answer opens the file as it is. See [Crash recovery](@/reference/files.md#crash-recovery).

## Exiting

`Ctrl+Q` quits from anywhere, after saving. tpt also saves when the terminal closes or it is told to end (`SIGTERM`, `SIGHUP`). See [Quitting](@/reference/files.md#quitting).

## Environment

| Variable | Effect |
|---|---|
| `XDG_CONFIG_HOME` | Where [`config.toml`](@/reference/settings.md) lives: `$XDG_CONFIG_HOME/taskpaper-terminal/`, else `~/.config/taskpaper-terminal/` |
| `XDG_DATA_HOME` | Where [daily backups](@/reference/files.md#daily-backups) go: `$XDG_DATA_HOME/taskpaper-terminal/backups/`, else `~/.local/share/taskpaper-terminal/backups/`, unless `backups.dir` says otherwise |
| `XDG_STATE_HOME` | Where the [pane layouts](@/reference/files.md#pane-layout) (`views.json`) and recovery files go: under `$XDG_STATE_HOME/taskpaper-terminal/`, else `~/.local/state/taskpaper-terminal/` |
| `COLORTERM` | `truecolor` or `24bit` enables the truecolor [themes](@/reference/appearance.md#color-depth) |
| `NO_COLOR` | When set and not empty, tpt draws no colors |
| `LC_ALL`, `LC_CTYPE`, `LANG` | The first one set decides whether the locale is UTF-8. Without UTF-8, tpt draws [ASCII](@/reference/appearance.md#icons) symbols. |
| `SSH_CONNECTION`, `SSH_TTY` | Either one makes [animations](@/reference/animation.md#full-reduced-or-off) default to `reduced` |

The XDG folders are used on macOS too.

## Terminals

tpt runs in any terminal on macOS and Linux, and over SSH. It asks for the [kitty keyboard protocol](@/reference/modes.md#classic-and-kitty-keys) when it starts and gives it back when it quits, uses the mouse when the terminal reports it, and draws each frame with synchronized output where the terminal supports it. For `Alt` keys on macOS, see [Alt on macOS](@/reference/modes.md#alt-on-macos). Copying over SSH needs a terminal with OSC 52; see [Copy and paste](@/reference/modes.md#copy-and-paste).
