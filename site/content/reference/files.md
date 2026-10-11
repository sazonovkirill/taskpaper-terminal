+++
title = "Files and saving"
weight = 10
description = "One file per window, saved automatically, backed up daily, never lost."
+++

tpt edits one `.taskpaper` file at a time, and that file on disk is the source of truth. Nothing is kept in a database. Everything in your outline is in the file as plain text that any TaskPaper app or text editor can open. How to start tpt on a file is on the [Command line](@/reference/command-line.md) page.

## Saving

There is no Save command. tpt saves every change automatically:

- a moment after the last change, 1000 ms by default ([Autosave delay](#setting-autosave-delay-ms), at least 100 ms),
- when you quit, and when the terminal closes or the system asks tpt to end (`SIGTERM`, `SIGHUP`).

Saving runs in the background, so you never wait on the disk. The status line shows `●` while there are unsaved changes, and an error if the last save failed, with the system's reason. A failed save is tried again on the next change and on quit. If you quit with unsaved changes after a failed save, the status line warns that quitting again exits without saving.

Every save is atomic. tpt writes a temporary file next to yours (`.<name>.tpt-XXXX`), gives it your file's permissions, then renames it over the original. A crash halfway never leaves a half-written file. If your file is a symlink, the link stays and the file it points to is written.

Lines tpt did not change are written back byte for byte, with their tabs or spaces, line endings, byte order mark and final newline as they were. See [Compatibility](@/reference/compatibility.md).

A file that is read-only on disk opens read-only, and every editing command says "read-only" in the status line.

## Save As

Save As writes the outline to a new path and continues there: autosave, backups, file watching and the saved pane layout all move to the new file. The old file stays as it was. Save As has no key. Run it from Go to Command (`Ctrl+P`).

In its prompt, `~` stands for your home folder and `Tab` completes the folder you are typing. If the file already exists, `Enter` replaces it and `Esc` goes back to the path. If the folder does not exist, the prompt says `Cannot save: the folder <folder> does not exist`.

Save As is how you keep the temporary demo file you get when you start tpt without a path.

{{ context_keys(table="SAVE_AS") }}

## Daily backups

Before tpt first changes a file on a given day, it copies the file as it was on disk to a backups folder:

- **Where:** `$XDG_DATA_HOME/taskpaper-terminal/backups/`, or `~/.local/share/taskpaper-terminal/backups/` when `XDG_DATA_HOME` is not set, on macOS too. The [Backup folder](#setting-backups-dir) setting changes the location.
- **One folder per file**, named after the file plus a hash of its full path (`todo.taskpaper-1a2b3c4d5e6f7a8b/`), so two files with the same name never mix. A `source-path.txt` in the folder names the file it belongs to.
- **One copy per day**, named by the date (`2026-10-11.taskpaper`). The copy is taken when tpt opens the file, and again before the first save of a new day if tpt runs past midnight.
- **Skipped** for a new or empty file, for the temporary demo file, and when the file is identical to the newest copy, so a file left alone does not use up its copies.
- **The newest 30** daily copies of each file are kept ([Backups kept](#setting-backups-keep)). Older ones are deleted after each new copy.

A failed backup never stops tpt from opening or saving. The status line warns once with the reason, and tpt tries again the next day. There is no restore command: a backup is an ordinary `.taskpaper` file, so open it with `tpt` and copy back what you need.

{{ settings(topic="files") }}

## When the file changes on disk

tpt watches your file. When another program changes it, such as a sync tool, TaskPaper or a text editor, tpt notices within a fraction of a second, and again when the terminal regains focus.

- **Nothing unsaved in tpt:** the file reloads silently and the status line says "Reloaded (changed on disk)". Each pane keeps its zoom, focus and folds on the same blocks wherever it can find them again. Undo history is cleared. Editing carries on: the block you were typing in, open prompts, finders and menus all stay, unless what they act on is gone. The Edit Node dialog closes without changes.
- **Unsaved changes in tpt:** autosave pauses and the conflict prompt opens, saying "Your changes are not saved yet." Choose one:
  - **Keep mine** (`Ctrl+K`): the version on disk is first saved as a conflict copy, then your version overwrites it. The status line says `Kept mine; theirs is in <copy>`.
  - **Take theirs** (`Ctrl+R`): your version is saved as a conflict copy, then the file reloads. The status line says `Took theirs; mine is in <copy>`. Editing carries on in the same block when it is still there.
  - **Show diff** (`Ctrl+L`): shows or hides the line-by-line difference from the file on disk to your version.
- **Not valid UTF-8 on disk:** the prompt opens even with nothing unsaved and says "The file on disk is not UTF-8, so it cannot be reloaded." Only Keep mine works.
- **Deleted or moved:** tpt keeps your outline, marks it "not on disk", and creates the file again on the next save.

The prompt can open while you are typing, so its choices use `Ctrl` keys that typing never sends. It has no `Esc`: saving stays paused until you choose. Conflict copies are named `<YYYY-MM-DD>-<HHMMSS>-conflict.taskpaper` and go into the file's backups folder, even when backups are turned off. They are kept apart from the daily copies, so conflicts never push those out. If a conflict copy cannot be written, the choice is cancelled, so the version being replaced is never lost.

Quitting while the prompt is open leaves the file on disk as it is, saves your version as a conflict copy, and prints `tpt: left theirs on disk; mine is in <copy>`.

{{ context_keys(table="PROMPT") }}

## Crash recovery

If tpt ever crashes with unsaved changes, it restores the terminal first, then writes your outline to `$XDG_STATE_HOME/taskpaper-terminal/recovery/` (`~/.local/state/…` by default) and prints the path. The next time you open the file, tpt names the recovery file and asks `Open the file with these changes? [y/N]`. With `y`, the file on disk is first saved as a conflict copy, then tpt opens with the recovered outline as unsaved changes. tpt never deletes recovery files.

## Pane layout

When you quit, tpt remembers each file's panes: their zooms, histories, widths, focused blocks and show-completed settings, and your order of the Today view. It restores them the next time you open the same file. They are stored in `$XDG_STATE_HOME/taskpaper-terminal/views.json` (`~/.local/state/…` by default), readable only by you. Folds are not remembered: a file always opens fully expanded.

## Quitting

{{ keys(topic="files") }}

`Ctrl+Q` quits from anywhere, saving first. It also works in the Navigator, every dialog, and while you edit.
