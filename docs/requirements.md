# Requirements

- Status: Draft
- Last updated: 2026-10-04

[functional-requirements.md](functional-requirements.md) (Accepted) defines what the app does and overrides this file. This file adds what that one leaves open: how files are handled safely, and the non-functional requirements. The "how" lives in [specs/](specs/README.md); undecided choices are in [open-questions.md](open-questions.md).

Priorities use the same scale as the functional requirements: **Must** (v1), **Should** (soon after), **Later**, **Won't**. IDs are stable; reference them from specs, tasks and commits.

## Principles

1. **The file is the source of truth.** No database and no hidden content: everything in the outline is in the `.taskpaper` text (FR-DOC-02).
2. **Never lose data.** Crashes, external edits and bad input must not corrupt or drop the user's text.
3. **TaskPaper compatible.** Files written by the app open correctly in TaskPaper 3 and vice versa. Parsing follows the reference parser ([reference/taskpaper.md](reference/taskpaper.md)).
4. **Terminal-native.** Works over SSH, in tmux and in 80×24.

## F-FILE · File handling

| ID | Requirement | Priority |
|---|---|---|
| F-FILE-01 | Open the `.taskpaper` file given on the command line; create it if missing. Without a path, create a new file in `/tmp` (FR-DOC-05). | Must |
| F-FILE-02 | Parse files exactly as TaskPaper 3 does: item types, tags, indentation, blank lines ([specs/file-format.md](specs/file-format.md)). | Must |
| F-FILE-03 | Write files that TaskPaper 3 parses to the same outline. Round-tripping an unedited file produces identical bytes. | Must |
| F-FILE-04 | Saves are atomic (temp file + rename); file permissions are preserved. | Must |
| F-FILE-05 | Save automatically after changes and on exit (FR-DOC-02). | Must |
| F-FILE-06 | Detect external modification of the open file and reload or prompt without losing local edits (OQ-16). | Must |
| F-FILE-07 | Refuse to open non-UTF-8 files with a clear error rather than corrupting them. | Must |
| F-FILE-08 | Preserve the file's indent style, line endings, BOM and trailing newline (OQ-14). | Should |
| F-FILE-09 | Keep rolling backups of previous versions. | Should |

## Non-functional requirements

| ID | Requirement | Priority |
|---|---|---|
| N-PERF-01 | Open and render a 10 MB / 100k-line file in under 300 ms on a modern laptop. | Must |
| N-PERF-02 | Keystroke to rendered frame under 16 ms for typical edits on that file (FR-UI-07). | Must |
| N-PERF-03 | Idle CPU near zero: render only on input, resize, file change or timer. | Must |
| N-REL-01 | No panic may lose data: on panic, restore the terminal and write unsaved content to a recovery file. | Must |
| N-REL-02 | Parser and serializer are covered by round-trip property tests and a corpus of TaskPaper samples. | Must |
| N-PORT-01 | Runs on macOS and Linux in common terminals (iTerm2, Terminal.app, Ghostty, kitty, WezTerm, Alacritty, GNOME Terminal) and inside tmux; key bindings also work in Windows Terminal (FR-KEY-03) (OQ-17). | Must |
| N-PORT-02 | Works in 80×24 and degrades gracefully below it. | Must |
| N-PORT-03 | Uses the kitty keyboard protocol when available and falls back to legacy key reporting otherwise. | Should |
| N-PORT-04 | Correct display width for wide characters, emoji and combining marks; graphemes are never split. | Must |
| N-UX-01 | Never blocks the UI on disk I/O longer than a frame. | Must |
| N-UX-02 | Every destructive action is undoable (FR-OPS-08). | Must |
| N-UX-03 | Honor `NO_COLOR`; every built-in theme is usable in a 16-color terminal. | Should |
| N-DIST-01 | Single binary, installable with `cargo install`. | Should |
