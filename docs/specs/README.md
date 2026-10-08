# Specifications

- Status: Accepted
- Last updated: 2026-10-04

[../functional-requirements.md](../functional-requirements.md) (Accepted) overrides these specs; requirement IDs `FR-…` and decisions `D-n` refer to it, and `F-FILE-…`/`N-…` to [../requirements.md](../requirements.md). Where the functional requirements leave a choice open, the specs assume the proposed default from [../open-questions.md](../open-questions.md), marked with its `OQ-` number.

| Spec | Covers |
|---|---|
| [architecture.md](architecture.md) | Modules, dependency direction, event loop, crates, testing strategy. |
| [data-model.md](data-model.md) | Lines and the block layer, derived attributes, edits and undo, panes and workspace state. |
| [file-format.md](file-format.md) | Parsing and serializing TaskPaper text exactly like TaskPaper 3. |
| [file-io.md](file-io.md) | Opening, `/tmp` documents, Save As, atomic autosave, backups, external changes, crash recovery, pane layout persistence. |
| [navigation.md](navigation.md) | Visible rows, focus, folding, zoom, panes, the Navigator, Today view, Go to Symbol and Go to Command. |
| [editing.md](editing.md) | Navigate and edit modes, creating blocks, structural operations, clipboard, block type, done, dates, colors, bookmarks. |
| [special-nodes.md](special-nodes.md) | Hidden `@hidden` lines that store feature data. |
| [attachments.md](attachments.md) | Notes and comments attached to blocks: storage, icons, the attachments panel, settings. |
| [links.md](links.md) | Internal links and backlinks (Later): `@uid` targets, `[[ID\|Title]]` links, display, commands. |
| [search.md](search.md) | Date parsing (v1); search syntax and bookmarked searches (Later). |
| [keybindings.md](keybindings.md) | Terminal key constraints, navigate and edit mode bindings, configuration. |
| [ui.md](ui.md) | Layout, block rendering, hidden tags, themes, shortcuts panel, context menu, mouse, animation. |
