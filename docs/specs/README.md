# Specifications

- Status: Draft
- Last updated: 2026-10-04

Proposed defaults from [../open-questions.md](../open-questions.md) are assumed throughout and are marked with their `OQ-` number. Requirement IDs (`F-…`, `N-…`) refer to [../requirements.md](../requirements.md).

| Spec | Covers |
|---|---|
| [architecture.md](architecture.md) | Modules, dependency direction, event loop, crates, testing strategy. |
| [data-model.md](data-model.md) | Document arena, items, derived attributes, edits and undo, views. |
| [file-format.md](file-format.md) | Parsing and serializing TaskPaper text exactly like TaskPaper 3. |
| [file-io.md](file-io.md) | Opening, atomic autosave, backups, external changes, crash recovery, view-state persistence. |
| [navigation.md](navigation.md) | Visible rows, cursor, folding, zoom, go to anything, sidebar. |
| [editing.md](editing.md) | Text editing, Enter/Backspace, structure commands, selection, clipboard, done, tags, archive. |
| [search.md](search.md) | Query language, live filtering, dates, saved searches. |
| [keybindings.md](keybindings.md) | Terminal key constraints, default keymap, configuration, vim preset. |
| [ui.md](ui.md) | Screen layout, row rendering, theme, help, mouse. |
