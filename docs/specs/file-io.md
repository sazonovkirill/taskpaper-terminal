# Spec: File I/O, saving and external changes

Covers FR-DOC-01, FR-DOC-02, FR-DOC-05, FR-DOC-06, FR-DOC-07, FR-UI-04 (storage), F-FILE-01, 04–07, 09, N-REL-01 and N-UX-01.

## 1. Opening

- `tpt <path>`: open the file; if it does not exist, start an empty document and create the file on first save. Parent directories must exist.
- `tpt` with no path: create a new empty file `/tmp/taskpaper-<YYYYMMDD-HHMMSS>.taskpaper` and open it (FR-DOC-05). The status line marks it as a temporary file until it is saved elsewhere with Save As.
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

**When** (FR-DOC-02, D-15): there is no manual save.
- 1 s after the last change (configurable `autosave_delay_ms`, minimum 100).
- On quit and on `SIGTERM`/`SIGHUP`.

**Save As** (FR-DOC-06): prompt for a path (with `~` expansion and completion of directories); if the file exists, confirm overwriting. Write the document there with the atomic routine below, then make it the open file: autosave, file watching, backups and view-state persistence switch to the new path. The previous file is left as it was.

Saving runs on a background thread with a snapshot of the serialized text; the UI never waits on disk (N-UX-01). The status line shows `●` while there are unsaved changes and an error badge if the last save failed. A failed save keeps retrying on the next change and on quit; quit with unsaved changes after a failed save asks before exiting.

**Daily backups** (FR-DOC-07, F-FILE-09, D-22):
- **Where:** `$XDG_DATA_HOME/taskpaper-terminal/backups/`, or `~/.local/share/taskpaper-terminal/backups/` when it is unset, on macOS too. Backups are user data rather than disposable state, so they go under the data directory, not `$XDG_STATE_HOME`. The `backups.dir` setting overrides the folder (for example `~/.taskpaper-terminal/backups`); `~` is expanded.
- **One folder per file:** `<file name>-<hash>/`, where `<hash>` is the 64-bit FNV-1a hash of the file's canonical path as 16 lowercase hex digits. Two `todo.taskpaper` files in different directories get different folders; the hash is written by hand, so it is stable across Rust releases (std's `DefaultHasher` is not). The folder also holds `source-path.txt` with the full path, so a person can tell which file it belongs to.
- **When:** on opening the file, and before the first save of each new local day for a session left running past midnight. If the folder has no copy dated today, copy the file as it is on disk, before the app writes anything, to `<YYYY-MM-DD>.taskpaper` (local date) with the atomic-write routine above. So each day's copy is the file as it was before that day's first change by the app.
- **Skipped:** when the file does not exist on disk or is empty (a new path, or the empty `/tmp` document of §1), and when the on-disk bytes equal the newest daily copy, so a file left unchanged for weeks does not use up its retention on duplicates.
- **Retention:** keep the newest `backups.keep` daily copies per file (default 30), newest by the date in the name; after writing a daily copy, delete the oldest ones beyond that. Conflict copies (§3) are pruned the same way but counted separately, so conflicts never push out daily copies. Nothing is pruned while backups are disabled. Folders of files that no longer exist are left alone.
- **Settings** (FR-SET-01): `backups.enabled` (default `true`), `backups.dir` (default unset, meaning the path above), `backups.keep` (default 30, minimum 1). Changing the folder takes effect on the next backup; existing copies are not moved.
- **Failures** never block opening or saving: the status line shows a warning once per session with the reason, and the next day's check tries again.
- **Save As** switches backups to the new path, which gets its own folder.
- There is no restore command; a backup is a plain `.taskpaper` file that `tpt` opens directly.

## 3. External changes

Watch the file with the `notify` crate (debounced), plus a metadata check on focus regain and before every save.

When the on-disk content hash differs from the known disk state (OQ-16 default):
- **No unsaved local changes:** reload silently, keep each pane's focus and zoom on the same blocks where they can be re-matched (same path of texts from root, else nearest surviving ancestor), and show "Reloaded (changed on disk)".
- **Unsaved local changes:** pause autosave and prompt: *Keep mine* (first write the on-disk version to the file's backup folder as a conflict copy, then overwrite disk), *Take theirs* (write the local version there as a conflict copy, then reload; undo history is cleared because item ids no longer apply), *Show diff* (open a read-only line diff pane). A conflict copy is named `<YYYY-MM-DD>-<HHMMSS>-conflict.taskpaper` (local time), is written even when backups are disabled, and failing to write it cancels the choice and shows the error, so the discarded version is never lost.
- **File deleted or moved:** keep the buffer, mark it "not on disk", and recreate the file on next save.

Saves check the disk state immediately before renaming; if it changed since the last read, the save is aborted and the conflict prompt shown instead. This prevents overwriting edits made by TaskPaper 3, a sync tool or another editor.

## 4. Crash safety

- Install a panic hook that restores the terminal first, then writes the current serialized document to `$XDG_STATE_HOME/taskpaper-terminal/recovery/<name>-<timestamp>.taskpaper` and prints its path (N-REL-01).
- On start, if a recovery file newer than the target exists, offer to open it.

## 5. View-state persistence

Restores the pane layout (FR-UI-04) and fold state (OQ-05 default A).

- File: `$XDG_STATE_HOME/taskpaper-terminal/views.json` (platform equivalent via `directories`), keyed by canonical file path.
- Stored per file: the panes in order with their location, history, focused block, width and show-completed setting; the focused pane; collapsed blocks; sidebar visibility.
- Blocks are stored as **text paths**: the list of first-line `text` values from the top level down to the block, plus the index among equal-text siblings.
- On load, each path is re-matched; anything that doesn't match is dropped silently. View state never blocks opening a file.
- Written on quit and on save, with the same atomic-write routine.
