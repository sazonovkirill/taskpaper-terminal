# Spec: File I/O, saving and external changes

Covers F-FILE-01, 04–07, 09, 10, N-REL-01 and N-UX-01.

## 1. Opening

- `tpt <path>`: open the file; if it does not exist, start an empty document and create the file on first save. Parent directories must exist.
- `tpt` with no path: open the default file from config (`default_file`), or print usage.
- Read errors, non-UTF-8 content and permission errors are shown on stderr before the TUI starts, and the app exits non-zero.
- Read-only files open read-only; editing commands report "read-only" in the status line.

## 2. Saving

**Atomic write** (F-FILE-04):
1. Serialize the document ([file-format.md §6](file-format.md#6-serialization)).
2. Write to a temp file in the same directory (`.<name>.tpt-XXXX`), `fsync` it.
3. Copy the original file's permissions.
4. Rename over the target; `fsync` the directory.
5. Record the new file's metadata (mtime, size, content hash) as the "known disk state".

If the target is a symlink, write through to the resolved path so the link is preserved.

**When** (OQ-15 default, autosave):
- 1 s after the last change (configurable `autosave_delay_ms`, `0` disables autosave).
- On quit, on `SIGTERM`/`SIGHUP`, and before the `suspend` command hands the terminal back to the shell. (`Ctrl+Z` is Undo, not suspend; see [keybindings.md](keybindings.md).)
- On the explicit Save command.

Saving runs on a background thread with a snapshot of the serialized text; the UI never waits on disk (N-UX-01). The status line shows `●` while there are unsaved changes and an error badge if the last save failed. A failed save keeps retrying on the next change and on quit; quit with unsaved changes after a failed save asks before exiting.

**Backups** (F-FILE-09): before the first save in a session, and again before any save more than 10 minutes after the last backup, copy the on-disk file to `$XDG_STATE_HOME/taskpaper-terminal/backups/<name>-<timestamp>.taskpaper`, keeping the last N (default 10) per file.

## 3. External changes

Watch the file with the `notify` crate (debounced), plus a metadata check on focus regain and before every save.

When the on-disk content hash differs from the known disk state (OQ-16 default):
- **No unsaved local changes:** reload silently, keep the cursor on the same item if it can be re-matched (same path of texts from root, else nearest surviving ancestor), and show "Reloaded (changed on disk)".
- **Unsaved local changes:** pause autosave and prompt: *Keep mine* (overwrite disk), *Take theirs* (write the local version to the backups folder, then reload; undo history is cleared because item ids no longer apply), *Show diff* (open a read-only line diff pane).
- **File deleted or moved:** keep the buffer, mark it "not on disk", and recreate the file on next save.

Saves check the disk state immediately before renaming; if it changed since the last read, the save is aborted and the conflict prompt shown instead. This prevents overwriting edits made by TaskPaper 3, a sync tool or another editor.

## 4. Crash safety

- Install a panic hook that restores the terminal first, then writes the current serialized document to `$XDG_STATE_HOME/taskpaper-terminal/recovery/<name>-<timestamp>.taskpaper` and prints its path (N-REL-01).
- On start, if a recovery file newer than the target exists, offer to open it.

## 5. View-state persistence (OQ-05 default B)

- File: `$XDG_STATE_HOME/taskpaper-terminal/views.json` (platform equivalent via `directories`), keyed by canonical file path.
- Stored per file: zoom item, collapsed items, cursor item, hide-done flag, last filter.
- Items are stored as **text paths**: the list of `text` values from the top level down to the item, plus the index among equal-text siblings.
- On load, each path is re-matched; anything that doesn't match is dropped silently. View state never blocks opening a file.
- Written on quit and on save, with the same atomic-write routine.
