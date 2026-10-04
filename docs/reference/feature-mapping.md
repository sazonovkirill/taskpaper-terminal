# Workflowy → TaskPaper feature mapping

How each Workflowy capability ([workflowy.md](workflowy.md)) can be expressed on top of the TaskPaper format ([taskpaper.md](taskpaper.md)). This table drives the scope in [../requirements.md](../requirements.md). Rows marked **OQ** depend on an open product decision in [../open-questions.md](../open-questions.md).

Legend for **Fit**:
- **Native**: TaskPaper already has the concept; no format extension needed.
- **Convention**: expressible with ordinary TaskPaper syntax (usually a tag) that other TaskPaper apps will show as plain text or tags.
- **App state**: needs state stored outside the `.taskpaper` file.
- **None**: no reasonable representation; out of scope or deferred.

| Workflowy | TaskPaper equivalent | Fit | Notes |
|---|---|---|---|
| Single infinite tree | One `.taskpaper` file = one tree; lines are nodes, tab depth is hierarchy | Native | Multiple files as one tree is **OQ-02**. |
| Bullet (plain node) | Note item (any line that is not a task or project) | Native | Which type a new line gets is **OQ-03**. |
| To-do node | Task item (`- text`) | Native | |
| Heading H1–H3 | Project item (`Text:`) | Native (approx.) | Projects are the only heading-like type; no levels. |
| Note under a bullet | Child note lines of an item | Native (approx.) | TaskPaper notes are separate items, not a field. Model choice is **OQ-01**. |
| Complete / uncomplete | `@done` or `@done(YYYY-MM-DD)` tag | Native | Whether to add the date is **OQ-04**. |
| Show/hide completed | Filter `not @done except @done//*` | Native | Same as TaskPaper's shipped "Not Done" search. |
| Expand / collapse | Folding | App state | TaskPaper keeps it in macOS xattrs, not in the file. **OQ-05**. |
| Zoom in / out, breadcrumbs | Focus / hoist | App state | Zoom target persistence is **OQ-05**. |
| Indent, outdent, move up/down | Move lines/branches | Native | |
| Move to… | Move to Project… | Native | Workflowy allows any node as target; TaskPaper only projects. |
| Delete | Delete items | Native | Trash is **OQ-10**. |
| Undo / redo | Undo / redo | App state | Session-only history. |
| Duplicate | Duplicate | Native | Workflowy appends `#copy`; TaskPaper does not. |
| Multi-select + bulk ops | Line/branch selection | Native | |
| Tags `#tag` | Tags `@tag`, `@tag(value)` | Native | `#` is not a TaskPaper tag sigil. **OQ-08**. |
| Mentions `@person` | `@person` tag | Native | No notifications. |
| Click tag to filter | Filter `@tag` / `@tag = "v"` | Native | |
| Search (live filter, ancestors kept) | Item-path filter | Native | Syntax choice is **OQ-07**. |
| `is:complete`, `has:note`, `A > B` | `@done`, items with child notes, `A//B` | Native | See the operator table in [../specs/search.md](../specs/search.md). |
| `changed:`, `created:`, `completed:` | Only `@done(date)` is in the file | None / App state | Created/modified times are not in the format. **OQ-09**. |
| Date tags, natural-language dates | `@due(YYYY-MM-DD[ HH:mm])`, `@start(...)`, TaskPaper date grammar | Native | |
| Calendar / Today | `@today` tag; "Today" search `@today union @due <[d] tomorrow` | Native | Daily-note tree is out of scope. |
| Stars (pages and searches) | Saved searches as `@search(query)` items | Native (searches) / App state (pages) | **OQ-11**. |
| JumpTo fuzzy finder | Go to Anything / Project / Search | Native | |
| Templates (`#template`) | `@template` tag + copy | Convention | |
| Sort children | Reorder lines | Native | One-shot reorder. |
| Archive completed | Archive @done (move to `Archive:` project) | Native | TaskPaper-only feature; included. |
| Count | Count of items/words in subtree | Native | |
| Mirrors | No equivalent | None | **OQ-12**. |
| Internal links, backlinks | No equivalent (only file paths and URLs are links) | Convention / None | **OQ-12**. |
| Inline bold/italic/colors | Plain text only | None | **OQ-13**. |
| Numbered, quote, code, divider, paragraph types | Plain note lines | None | Rendered as notes. |
| Board / table / dashboard layouts | — | None | Out of scope for v1. |
| Panes | Split views | App state | Could. |
| Command palette, slash menu | Command palette | Native (app feature) | |
| Paste multi-line / OPML / Markdown | Paste TaskPaper text; import OPML/Markdown | Native / Convention | |
| Export plain text / Markdown / OPML | Copy Displayed; exporters | Native / Convention | |
| Themes | TaskPaper LESS stylesheets | App state | Terminal app uses its own theme config. |
| Sharing, comments, files, embeds, AI, email, calendar sync, mobile | — | None | Cloud/GUI-only. |
