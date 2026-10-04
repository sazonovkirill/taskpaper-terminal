# Workflowy reference

Snapshot of Workflowy's feature set as of 2026-10-04, used as the behavioral target for taskpaper-terminal. Paraphrased from the official docs; always check the live source before relying on a detail.

- Help center: https://workflowy.com/help/ (each page also available as Markdown at `https://workflowy.com/help/<slug>.md`)
- Keyboard shortcuts: https://workflowy.com/help/hotkeys
- Search: https://workflowy.com/help/search
- Product updates: https://blog.workflowy.com/

Legend: **(inferred)** = our reasoning, not stated by Workflowy. **(unverified)** = could not confirm in an official source. **[GUI/cloud]** = feature that is inherently GUI/cloud and has little or no meaning in a local terminal outliner. **[TUI-relevant]** = directly portable.

---

## 0. Core model

Source: https://workflowy.com/help/get-started , https://workflowy.com/help/navigate-around

- Workflowy is a single infinite outline: no files/folders, every item is a node in one tree. [TUI-relevant]
- Each node = one line of rich text ("name") + optional multi-line note + ordered children.
- Enter creates a sibling, Tab indents, click a bullet to zoom; everything autosaves and syncs (no save button, though `Cmd/Ctrl+S` forces "save and sync").
- Node *types* change how a node looks/behaves (bullet, todo, numbered, H1-H3, paragraph, quote, code, divider); *layouts* change how a node's children render (bullets/list, board, table, dashboard).

---

## 1. Bullets / nodes, creating and nesting

Source: https://workflowy.com/help/bullets , https://workflowy.com/help/add-edit-format

- Enter at end of a bullet creates a new sibling below. Also: "New node" button in sidebar, `+` on mobile.
- Tab indents under the previous sibling (it becomes the parent); Shift+Tab outdents. Unlimited depth.
- Moving a node always carries its whole subtree.
- Drag a bullet handle to move it; or `/move to` to search for a destination.
- Change back to plain bullet: node menu "Bullets" or `/bullets`; content and children kept.
- Slash command `prepend_child` (command palette) adds a new bullet at the top of the current page. Source: https://workflowy.com/help/command-palette
- Per-parent setting "Priority: Default to Top / Bottom" controls where items sent in by commands (move-to, Quick Add, email…) land. Source: https://workflowy.com/help/slash-command
- Setting: auto-capitalize first letter of bullets (added 2026.01). Source: https://blog.workflowy.com/update-2026-01-22/
- Enter behaviour on split of text mid-line (unverified): standard outliner behaviour would split the line; not documented.

## 2. Notes on bullets

Source: https://workflowy.com/help/add-edit-format , https://workflowy.com/help/search-your-notes

- Every node has a note field under its name; Shift+Enter moves focus into it (and back, inferred).
- Intended for supporting detail/metadata (descriptions, links, due dates).
- Notes are searched along with names ("matching nodes, including any text inside their note fields").
- `has:note` finds nodes with notes; `in:note:` restricts matching to the note location (see Search).
- Slash `Add note` / command `add_note` opens the note.
- Display: notes render under the name, typically truncated/dimmed when not focused (inferred from the app; not documented).

## 3. Expand / collapse

Source: https://workflowy.com/help/navigate-around , https://workflowy.com/help/bullets , https://workflowy.com/help/hotkeys , https://workflowy.com/help/command-palette

- Nodes with children show an arrow/triangle; click to collapse/expand. Collapse state is persistent per node (synced) (inferred - Workflowy stores collapsed state server-side; unverified in help).
- Double-click the triangle to expand/collapse the whole subtree beneath.
- Keys: Expand `Cmd+↓` / `Ctrl+↓`; Collapse `Cmd+↑` / `Ctrl+↑`; Toggle `Ctrl+Space` (both platforms).
- Commands: `expand`, `collapse`, `expand_or_collapse`, `expand_all`, `collapse_all` (all descendants under selection). [TUI-relevant]

## 4. Zoom in/out, breadcrumbs, navigation

Source: https://workflowy.com/help/navigate-around , https://workflowy.com/help/hotkeys , https://workflowy.com/help/command-palette , https://workflowy.com/help/jump-to , https://workflowy.com/help/sidebar-navigation , https://workflowy.com/help/panes

- Click a bullet (not the arrow) to zoom: that node becomes the page title, only its subtree shown.
- Breadcrumbs at the top show ancestors; click any to zoom out to it; "Home" returns to root.
- Zoom in `Cmd+.` / `Alt+.`; zoom out `Cmd+,` / `Alt+,`; Home `Cmd+'` / `Ctrl+'`.
- Previous/next sibling *page* (zoom to sibling): `Cmd+Shift+9/0` (Win `Alt+Shift+9/0`, Linux `Ctrl+Shift+9/0`).
- Focus prev/next sibling (cursor, not zoom): `Option+Cmd+↑/↓` / `Alt+Ctrl+↑/↓`.
- History: `navigate_backward` / `navigate_forward` commands (browser back/forward also work - inferred). Each pane has its own back/forward history.
- Other cursor commands: `focus_first`, `focus_last`, `focus_parent`, `focus_next_sibling`, `focus_prev_sibling`, `focus_name`.
- `move_out` moves the focused bullet out of the current zoomed page.
- **JumpTo** (`Cmd+K` / `Ctrl+K`): fuzzy-find a node by name; starred items rank first. You can assign short **shortcut codes** (e.g. `wm`) to a node (`/shortcut`) or to a saved search; typing the code in JumpTo goes there. Shortcuts also act as destinations for Email-to-Workflowy and Web Clipper. [TUI-relevant]
- **Sidebar** (`Ctrl+L` Mac / `Alt+L` Win): tree of Home + Starred; drag nodes onto a sidebar destination to move them (Shift-drag mirrors). Mostly [GUI], but a tree/side panel is doable in a TUI.
- **Panes** (2026.08, desktop only): multiple side-by-side views of the outline, each with own breadcrumbs, search, history, scroll. Alt/Option-click a bullet opens it in a new pane. Shortcuts in the table below. [TUI-relevant as split views]

## 5. Indent / outdent / move up / move down / move to

Source: https://workflowy.com/help/hotkeys , https://workflowy.com/help/slash-command , https://workflowy.com/help/command-palette , https://workflowy.com/help/plan-with-calendar

- Indent `Tab`, outdent `Shift+Tab` (work on multi-selections too).
- Move up/down among siblings: `Cmd+Shift+↑/↓` (Mac) / `Alt+Shift+↑/↓` (Win/Linux). Moving past first/last sibling hops into the adjacent parent (inferred - typical Workflowy behaviour; unverified).
- **Move to** another location: `Ctrl+Cmd+M` (Mac) / `Alt+Ctrl+M` (Win/Linux). Opens a "MOVE TO" fuzzy picker. Note: `Cmd+Shift+M` is *Copy as mirror*, not Move to. (Not `Ctrl+Shift+M`.)
- Related commands: `Move Here…` (pull another node to here), `Move to Child` (move under a child of some node), `Move to Today` / `Move to tomorrow` / `Move to next week` / `move_7_days_out` / `Move to Date` (Calendar day nodes).
- Drag-and-drop, including across browser tabs (2026.01). [GUI]

## 6. Complete, show/hide completed

Source: https://workflowy.com/help/todos , https://workflowy.com/help/hotkeys , https://workflowy.com/help/command-palette

- Complete/uncomplete: `Cmd+Enter` / `Ctrl+Enter`. Any node can be completed (not just todos); todo nodes additionally show a checkbox. Command `complete` toggles: if all selected already complete, it un-completes them.
- Completed items stay in the outline, rendered struck-through/dimmed (inferred from app UI) until hidden or deleted.
- Show/hide completed: `Cmd+O` / `Ctrl+O`, or the checkmark button in the header; per-page header action "Show completed" (panes doc). Whether this is global or per-zoom is not documented (unverified).
- Completing a parent hides its descendants with it (inferred).
- Search: `is:complete`, `-is:complete`; legacy `completed:Nd` (see Search).

## 7. Delete, trash, undo/redo

Source: https://workflowy.com/help/hotkeys , https://workflowy.com/help/command-palette , https://workflowy.com/help/add-edit-format

- Delete node (+ subtree): `Cmd+Shift+Backspace` / `Ctrl+Shift+Backspace`; slash `Delete`. Backspace on an empty bullet removes it (inferred, standard).
- Trash exists: command `show_trash` "Opens Trash" - deleted items are recoverable (retention period unverified).
- Undo `Cmd+Z` / `Ctrl+Z`; redo `Cmd+Y` or `Cmd+Shift+Z` (`Ctrl+Y` / `Ctrl+Shift+Z`). Undo covers structural ops (moves, deletes) and also file uploads (2026.07 fix).
- Duplicate: `Cmd+Shift+D` (Mac); Win/Linux listed as `Alt+Shift+D` on the hotkeys page but `Ctrl+Shift+D` on the settings page (discrepancy in official docs). Duplicate appends a `#copy` tag by default (toggle "Duplicate with #copy" in Settings). Source: https://workflowy.com/help/settings

## 8. Multi-select

Source: https://workflowy.com/help/command-palette , https://workflowy.com/help/todos , https://workflowy.com/help/plan-with-calendar , https://blog.workflowy.com/2025-7-color-and-highlight-multiple-bullets-at-once-divider-improvements/

- Multiple bullets can be selected; a "multi-select menu" applies bold/italic/color/highlight, todo conversion, move/mirror to date, etc. to all.
- Commands: `select_all` (bullets on current page), `expand_selection_up`, `expand_selection_down`, `select_matched` (select every search hit).
- Most editing commands (indent, outdent, move up/down, complete, delete, duplicate, move_to, mirror, toggle types, append/prepend tag) act on the whole selection.
- Default key bindings for extending selection are not in the hotkeys table (unverified). Commonly Shift+↑/↓ from a selected bullet, or dragging the mouse across bullets; mobile uses long-press (2026.07).

## 9. Copy / paste behaviour, import

Source: https://workflowy.com/help/add-edit-format , https://workflowy.com/help/markdown , https://blog.workflowy.com/paste-multi-level-bulleted-lists-into-workflowy/ , https://blog.workflowy.com/workflowy-now-supports-opml-import-and-export/ , https://blog.workflowy.com/2026-06-tables-and-more/

- Paste accepts plain text, Markdown, formatted HTML, and OPML; multi-line pastes become multiple bullets, with indentation / HTML `<ul>/<li>` nesting / OPML structure turned into hierarchy.
- Pasted Markdown is converted: bold, italic, headings, blockquotes, task checkboxes, numbered lists (2026.01).
- Raw OPML pasted into an item is parsed into a nested list (this is the "import" path; there is no dedicated file-import UI documented).
- Tables accept pasted Excel / Google Sheets / Markdown / TSV (2026.06).
- Copying selected bullets copies a text representation including hierarchy (inferred; format not documented). `Copy link` `Cmd+Shift+L` / `Alt+Shift+L` copies an internal URL.
- Pasting a URL auto-links it; certain URLs auto-embed (see Embeds).

## 10. Mirrors (live copies)

Source: https://workflowy.com/help/mirrors , https://workflowy.com/help/command-palette , https://workflowy.com/help/slash-command

- A mirror is the same node shown in several places; editing any instance edits all (name, note, children, completion).
- Create: bullet menu "Mirror" then paste; `Cmd+Shift+M` / `Alt+Shift+M` (copy as mirror); type `((` for the "MIRROR FROM" fuzzy picker; Shift-drag onto sidebar; commands `mirror_to`, `mirror_to_child`, `mirror_here`, `mirror_to_today`, `mirror_to_date`.
- There is an *original* and mirrors: `swap_mirror_origin` makes a mirror the original.
- "See them" / `show_mirror_breadcrumbs` lists every location; from there you can jump or delete all.
- "Detach Mirror" turns an instance into an independent node.
- `is:mirror` search operator. Exports include mirrored descendants (2026.07).
- Local TUI note (inferred): implementable as a node referenced from multiple parents (DAG); needs cycle prevention.

## 11. Tags (#tag) and mentions (@mention)

Source: https://workflowy.com/help/tags-mentions , https://workflowy.com/help/colors , https://workflowy.com/help/fractal-comments , https://workflowy.com/help/command-palette

- `#word` or `@word` anywhere in a name/note creates a tag; any number per node; autocomplete from previously used tags.
- Clicking a tag filters the outline by it (adds to search); click again to remove; multiple tags can be active.
- Tags can be colored: hover a tag, choose color; all instances update.
- `@` in a shared item can mention a collaborator, which notifies them [cloud].
- Commands: `insert_hash_tag`, `insert_at_tag`, `append_tag`, `prepend_tag` (bulk-tag selected bullets).
- Allowed tag characters / tag renaming are not documented (unverified). Tags with hyphens like `#client-a` are used in official examples, so `-` is allowed inside tags.

## 12. Dates

Source: https://workflowy.com/help/date-tags , https://workflowy.com/help/plan-with-calendar , https://workflowy.com/help/timeline , https://blog.workflowy.com/update-2026-01-22/

- Date tags are inline "pills"/chips in text.
- Insert by typing a date (`MM-DD-YYYY`, `MM/DD/YYYY`, `MM-DD-YY`, `MM/DD/YY`, `MM-DD`, `MM/DD`) - a widget suggests the date; Enter confirms.
- Natural language: `today`, `tomorrow`, `yesterday`, `this/next/last week`, `this/next/last month`, `this/next/last year`.
- `!!` or `/date` opens a date picker (calendar, frequent options, NL input); the picker supports **date ranges**.
- Remove: delete as text, or Cmd-click (Mac) / Alt-click (Win) the chip.
- Settings: custom display format (presets); week starts Sunday/Monday (affects "this week").
- Times: the search page mentions `HH`/`mm` placeholders, but no documented time syntax for tags (unverified whether times are supported). A 2026.01 fix stopped words like "morning"/"noon" auto-converting, implying some time-word parsing exists (inferred).
- Recurring dates: not documented (unverified / likely unsupported).
- Search by `day-of-week: Monday` (2026.01).

## 13. Calendar, Today, Timeline, Google Calendar

Source: https://workflowy.com/help/plan-with-calendar , https://workflowy.com/help/timeline , https://workflowy.com/help/today-digest , https://workflowy.com/help/google-calendar

- Calendar = auto-created daily note nodes (year/month/day hierarchy, inferred). Go to Today: `Ctrl+T` (Mac) / `Alt+T` (Win).
- "Found Dates": nodes elsewhere with date tags show on their calendar day (as live references).
- Move/mirror to Today/Tomorrow/Next week/date commands schedule items into day nodes.
- Timeline (2026.05): next 30 days + 12-month jump; drag nodes onto days.
- Today Digest: 8 AM email [cloud].
- Google Calendar sync (2026.04): read-only import of events, next 35 days, hourly [cloud].
- TUI note (inferred): a local "daily notes" tree plus an agenda view of date-tagged nodes is portable; email/GCal are not.

## 14. Stars / bookmarks

Source: https://workflowy.com/help/starring

- Star the current zoomed page (`Cmd+Shift+8` / `Ctrl+Shift+8`) or the current search (star in search bar).
- Starred entries appear in the sidebar; can be renamed and reordered; appear first in JumpTo.
- Starred searches re-run live ("saved searches"). [TUI-relevant]

## 15. Templates

Source: https://workflowy.com/help/templates , https://workflowy.com/help/command-palette

- Any node tagged `#template` is a template (or `make_template` command). A collapsed template shows a "New …" button to instantiate.
- `/add from template` or `/template` searches all `#template` nodes and inserts a copy (deep copy) at the cursor.
- Removing the tag makes it a normal node again. [TUI-relevant]

## 16. Node types (list/bullet types)

Source: https://workflowy.com/help/change-node-types , https://workflowy.com/help/text-block-formats , https://workflowy.com/help/numbered-lists , https://workflowy.com/help/todos , https://workflowy.com/help/headings-paragraphs , https://workflowy.com/help/code-blocks , https://workflowy.com/help/quote-blocks , https://workflowy.com/help/dividers

| Type | Markdown trigger (+space) | Slash | Hotkey Mac / Win | Search |
|---|---|---|---|---|
| Bullet (default) | - | `/bullets` | `Opt+Cmd+8` / `Alt+Ctrl+8` | - |
| To-do (checkbox) | `[ ]` or `[]` (also `- [ ]`) | `/todo` | `Opt+Cmd+9` / `Alt+Ctrl+9` | `is:todo` |
| Numbered | `1.` (any number; can start anywhere) | `/number` | `Opt+Cmd+0` / `Alt+Ctrl+0` | (unverified) |
| Heading 1/2/3 | `#`, `##`, `###` | `/h1` `/h2` `/h3` | `Opt+Cmd+1/2/3` / `Alt+Ctrl+1/2/3` | `is:h1` `is:h2` `is:h3` |
| Paragraph | - | `/paragraph` | `Opt+Cmd+4` / `Alt+Ctrl+4` | `is:paragraph` |
| Code block | ```` ``` ```` | `/code block` | `Opt+Cmd+6` / `Alt+Ctrl+6` | `is:code-block` |
| Quote block | `>` | `/quote` | `Opt+Cmd+7` / `Alt+Ctrl+7` | `is:quote` |
| Divider | `---` | `/divider` | - | `is:divider` |
| AI Node | - | `/ai node` | `Opt+Shift+Space` / `Alt+Shift+Space` | (unverified) |

Notes:
- Headings/paragraphs hide the bullet until hover but keep outline position and children.
- Numbered lists auto-renumber on add/remove/reorder; numbers persist in search, zoom, export (2026.01).
- Code block: Enter three times exits; converting several bullets into one code/quote block joins them with newlines, converting back splits lines into nodes. Quote block: Enter twice exits.
- Dividers can carry an optional title.
- Types are per node and can be mixed freely; children do not inherit the parent's type (inferred from "mix types freely").
- `Opt+Cmd+5` is unassigned in the documented table (unverified what it does).

### Layouts (how children render)

- **Board** (Kanban): first-level children = columns, second-level = cards. Create via bullet menu > Bullets > Board, layout button, or `/board`. Add cards with `+` or Enter; columns only added at right, then dragged; columns resizable; slash `Insert column left/right` inside a board. Cards/columns have the bullet on the right. [Mostly GUI; a TUI can render columns]
- **Table** (2026.06): children = rows, grandchildren = cells; a first row named "Header" supplies column headers. Tab/Shift+Tab/arrows move between cells; nested content in a cell shown with a left border but not editable inline; duplicate/delete column; import Excel/Sheets/Markdown/TSV, export HTML/Markdown/TSV.
- **Dashboard**: children as panels (only mentioned on change-node-types; details unverified).
- Source for layouts: https://workflowy.com/help/boards , https://workflowy.com/help/tables , https://workflowy.com/help/change-node-types

## 17. Inline formatting, colors, links

Source: https://workflowy.com/help/text-block-formats , https://workflowy.com/help/colors , https://workflowy.com/help/internal-external-links , https://workflowy.com/help/hotkeys

- Inline: Bold `Cmd/Ctrl+B`, Italic `Cmd/Ctrl+I`, Underline `Cmd/Ctrl+U`, Strikethrough `Cmd/Ctrl+Shift+X`, Inline code `Cmd/Ctrl+Shift+C`, Link `Cmd/Ctrl+K` (with text selected), text color and highlight color via format widget; `Cmd/Ctrl+Shift+H` re-applies the last highlight.
- Inline code: two trailing spaces exit back to normal text (2026.08).
- Formats can be applied to whole selected bullets (multi-select).
- Searchable: `text:` (color/bold/italic/underline), `highlight:` (highlight color).
- External links: pasted URLs auto-link; select text + link icon to set URL; hover arrow to edit/unlink.
- Internal links: `[[` opens a picker of existing nodes; the "New" options create a new node and link it at once. Linked-to nodes get a nested "Backlinks" pseudo-item listing referrers with breadcrumbs. `is:backlinks` search.
- Deep links `workflowy://` open in desktop/mobile app [GUI].
- Emoji style and link style are settings.

## 18. Search (full operator list)

Sources: https://workflowy.com/help/search (also https://workflowy.com/help/search.md), https://workflowy.com/help/search-your-notes , https://blog.workflowy.com/smooth-operators/ , https://blog.workflowy.com/hidden-search-operators/ , https://blog.workflowy.com/new-ways-to-search/ , https://blog.workflowy.com/product-update-august-29-2023-filter-by-creation-date-sidebar-keyboard-shortcut-mirror-and-boards-slash-commands-minor-fixes/ , https://blog.workflowy.com/update-2026-01-22/ , https://blog.workflowy.com/product-update-2026-08-panes-and-more/

### Behaviour
- `Esc` opens search (or exits it). Results filter the outline live as you type; matches in names *and notes*; matching nodes are shown with their ancestor path so structure is preserved, and results are editable in place.
- Default matching is partial and "fuzzy"; quotes force exact.
- Search is scoped to the current zoomed node.
- Clicking tags adds them to the query; filters can also be added via buttons in the search bar (an operator tab with autocomplete appears).
- Searches can be starred (saved) and given JumpTo shortcut codes.
- `select_matched` selects all hits for bulk operations.

### Boolean
| Operator | Meaning | Example |
|---|---|---|
| space | AND | `meeting #client-a` |
| `OR` (uppercase) | either | `@Steve OR @Lisa` |
| `-` prefix | NOT (works on words, tags, operators) | `-#high`, `#task -#done`, `-is:complete` |
| `"…"` | exact phrase | `"the quick brown fox"` |
| `A > B` | hierarchical: B whose ancestor matches A; chainable; spaces around `>` required | `SEO > @sam`, `#high > SEO > @sam`, `Projects > Write draft -today` |

### Property operators
| Operator | Documented values / examples |
|---|---|
| `is:` | `is:complete`, `is:todo`, `is:shared`, `is:sharedwithme`, `is:mirror`, `is:backlinks` (help) / `is:backlink` (blog), `is:embedded`, `is:h1`, `is:h2`, `is:h3`, `is:paragraph`, `is:code-block`, `is:quote`, `is:divider` |
| `has:` | `has:note` (blog also `has:notes`), `has:file`/`has:files`, `has:images`, `has:videos`, `has:tweet`/`has:tweets`, `has:children` (2026.08) |
| `text:` | text color or style (bold, italic, underline). Exact value names (e.g. `text:bold`, `text:red`) (unverified - chosen from autocomplete) |
| `highlight:` | highlight color, e.g. `highlight:yellow` (value names unverified) |
| `in:note:` | match only in a location: note or backlink |
| `changed:` | last-modified time; e.g. `changed:1d` (blog comments) |
| `last-changed:` | legacy form, units `d` and `h`: `last-changed:1d`, `last-changed:4h`, `last-changed:7d` |
| `created:` | creation time, e.g. `created:1d` (added Aug 2023) |
| `completed:` | completion time, e.g. `completed:1d`, `completed:50d` (blog comments; unverified that it still works) |
| `#tag`, `@tag` | tag match |

### Date-tag search
- Specific: `MM/DD`, `MM-DD`, `MM/DD/YY`, `MM-DD-YY`, `MM/DD/YYYY`, `MM-DD-YYYY` (or user's custom format, 2026.01/.03).
- Natural: `today`, `tomorrow`, `yesterday`, `this|next|last week`, `this|next|last month`, `this|next|last year`.
- Ranges: `MM/DD - MM/DD`, `MM/DD/YY - MM/DD/YY`, `MM/DD/YYYY - MM/DD/YYYY`.
- `date-after:` operator (e.g. `date-after:10/01` - exact argument form unverified); `date-before:` likely exists (unverified).
- `day-of-week: Monday` (2026.01).
- Combining example: `is:complete @jesse last-changed:7d`; `#task this week -is:complete` (inferred example).

## 19. Sort children

Source: https://workflowy.com/help/slash-command , https://workflowy.com/help/command-palette

- `Sort A-Z` / `Sort Z-A` slash commands (alphanumeric; numbers and dates sorted in order).
- Commands: `sort_children_asc`, `sort_children_desc`, `sort_children_by_creation_date_asc|desc`, `sort_by_match_asc|desc` (prompts for text; matching children go top/bottom).
- Sorting is a one-shot reorder, not a persistent sort view (inferred). [TUI-relevant]

## 20. Other editing commands

Source: https://workflowy.com/help/command-palette , https://workflowy.com/help/slash-command

- Slash menu `/` (needs a preceding space unless at line start), filtered by typing; Enter or Tab runs.
- Command Palette `Option+W` / `Alt+W` (Beta, Pro): every command by snake_case name (full list on the page; key ones are cited throughout this doc).
- `count`: number of bullets and words under a node. [TUI-relevant]
- `zen_mode`, `toggle_full_width_page` (focus modes).
- `customize_hotkeys`, `customize_themes`: custom keybindings and themes are stored as editable data (inferred: in nodes) - customization exists.

## 21. Export, print

Source: https://workflowy.com/help/exporting , https://workflowy.com/help/markdown , https://workflowy.com/help/share-collaborate , https://workflowy.com/help/files-images

- Per node (bullet menu > Export, `Cmd+Shift+E` / `Alt+Shift+E`) or whole account (Settings > Export all).
- Formats: formatted text (HTML-ish rich text), plain text, Markdown, OPML. Export shows text in a dialog to copy; whether a "download file" option exists is unclear (unverified).
- Tables also export HTML/Markdown/TSV.
- Print `Cmd/Ctrl+P`.
- Backups: Dropbox backup (2026.06 mention); "Download Files" zip of attachments by email link. [cloud]

## 22. Sharing, collaboration, comments [GUI/cloud]

Source: https://workflowy.com/help/share-collaborate , https://workflowy.com/help/fractal-comments , https://workflowy.com/help/hotkeys

- Share any node (and subtree) by secret link (viewers need no account) or email invite; permissions View / Edit / Full Access. Shared nodes get a blue halo.
- `@` mentions notify collaborators.
- Fractal comments: threaded, infinitely branching; drafts; unread tracking; mark read `Cmd+Shift+Enter`; prev/next unread `Opt+Ctrl+P/N`; add comment `Ctrl+M`.
- Presentation mode `Cmd/Ctrl+Shift+P`: zoomed node = title, children = slides, grandchildren = bullet points; arrow keys navigate. Source: https://workflowy.com/help/instant-presentations (a TUI could emulate this; low priority).

## 23. Files, images, embeds [GUI/cloud]

Source: https://workflowy.com/help/files-images , https://workflowy.com/help/settings

- Drag-drop or `/upload file`; images resizable, full-screen viewer; PDF inline preview; OCR on images (`ocr_image`).
- Limits: free 100 MB total; Pro unlimited with 5 GB/file; 15 uploads/min.
- Auto embeds for YouTube, YouTube Shorts, X/Tweets, Loom (toggle per type in Settings).
- TUI note (inferred): could store file paths/links only.

## 24. Offline, sync, apps [mostly cloud]

Source: https://workflowy.com/help/apps-devices , https://blog.workflowy.com/workflowy-goes-offline/ , https://blog.workflowy.com/update-2026-07-improvements-and-fixes/

- Web, desktop (macOS/Windows/Linux AppImage), iOS/iPadOS, Android; all sync to one account.
- Offline editing with later sync exists (originally announced 2013 for iOS/desktop); current help has no dedicated offline page (current scope unverified). 2026.07 improved reconnection behaviour.
- Capture: desktop global Quick Add `Option+Cmd+N` / `Win+Alt+N` to Inbox; iOS Shortcut/Siri; Android widget; Mobile Share To; Email to Workflowy; Web Clipper. Source: https://workflowy.com/help/quick-add , https://workflowy.com/help/email-to-workflowy
- A local terminal app is inherently "offline-first"; sync is out of scope (inferred).

## 25. Themes, fonts, settings

Source: https://workflowy.com/help/personalize-workflowy , https://workflowy.com/help/settings , blog updates 2025.21, 2026.01, 2026.03, 2026.06, 2026.07

- Preset light/dark themes (free since 2026.03), e.g. Chocolate, Matcha, Vintage, Wood, Steel, One Dark; fonts incl. monospace (Geist Mono, Courier New, Junicode, Bold option).
- Settings: keyboard-shortcut hint panel (`Cmd/Ctrl+/`), daily change-summary email, Labs (experimental), embeds toggles, date format, week start, image size, numbered lists toggle, auto-capitalize, Duplicate-with-#copy.
- TUI-relevant: theme/color config, date format, week start.

## 26. AI and integrations (recent) [GUI/cloud]

Source: https://workflowy.com/help/use-workflowy-with-ai , https://workflowy.com/help/ai-nodes , https://workflowy.com/help/ai-actions , https://workflowy.com/help/chat-with-your-notes , https://workflowy.com/help/mcp-server , https://workflowy.com/help/build-ai-agents-with-api

- Workflowy AI (Pro, opt-in): Chat with your notes (`Ctrl+C` Mac / `Alt+C` Win toggles chat; `/chat`), AI Nodes (inline prompt with nearby context; Accept/Reject/Regenerate), AI Actions (`/ai`: Summarize, Find tasks, Draft an outline, Fix grammar & spelling, Make shorter).
- Desktop MCP server: local Streamable-HTTP MCP from the desktop app, private connection link, configurable port 1024-49151.
- Public REST API with bearer API key (workflowy.com/api-key): create/read/list/update/move/complete/delete nodes.
- Third-party CLIs listed by Workflowy (unofficial), e.g. mholzen `workflowy-cli`. Source: https://workflowy.com/help/third-party-tools.md
- Zapier, Readwise, Kindle highlights, Web Clipper integrations (listed in /learn index).
- "Flows": no feature by that name exists in current docs (unverified/not found).

---

## 27. Complete default keyboard shortcuts

Source: https://workflowy.com/help/hotkeys (default bindings; customizable). Panes rows from https://workflowy.com/help/panes . Extras marked with their source.

### Navigation
| Action | macOS | Windows / Linux |
|---|---|---|
| Zoom in | `Cmd+.` | `Alt+.` |
| Zoom out | `Cmd+,` | `Alt+,` |
| Go to Today (Calendar) | `Ctrl+T` | `Alt+T` |
| Navigate home | `Cmd+'` | `Ctrl+'` |
| JumpTo any item | `Cmd+K` | `Ctrl+K` |
| Previous sibling page | `Cmd+Shift+9` | `Alt+Shift+9` (Linux `Ctrl+Shift+9`) |
| Next sibling page | `Cmd+Shift+0` | `Alt+Shift+0` (Linux `Ctrl+Shift+0`) |
| Focus previous sibling | `Opt+Cmd+↑` | `Alt+Ctrl+↑` |
| Focus next sibling | `Opt+Cmd+↓` | `Alt+Ctrl+↓` |
| Search / exit search | `Esc` | `Esc` |

### Editing and moving
| Action | macOS | Windows / Linux |
|---|---|---|
| New sibling | `Enter` | `Enter` |
| Add/focus note | `Shift+Enter` | `Shift+Enter` |
| Complete / uncomplete | `Cmd+Enter` | `Ctrl+Enter` |
| Duplicate | `Cmd+Shift+D` | `Alt+Shift+D` (settings page says `Ctrl+Shift+D`) |
| Delete node | `Cmd+Shift+Backspace` | `Ctrl+Shift+Backspace` |
| Copy as mirror | `Cmd+Shift+M` | `Alt+Shift+M` |
| Copy link | `Cmd+Shift+L` | `Alt+Shift+L` |
| Move to… | `Ctrl+Cmd+M` | `Alt+Ctrl+M` |
| Indent | `Tab` | `Tab` |
| Outdent | `Shift+Tab` | `Shift+Tab` |
| Move up | `Cmd+Shift+↑` | `Alt+Shift+↑` |
| Move down | `Cmd+Shift+↓` | `Alt+Shift+↓` |
| Expand | `Cmd+↓` | `Ctrl+↓` |
| Collapse | `Cmd+↑` | `Ctrl+↑` |
| Expand/collapse toggle | `Ctrl+Space` | `Ctrl+Space` |

### Formatting
| Action | macOS | Windows / Linux |
|---|---|---|
| Bold | `Cmd+B` | `Ctrl+B` |
| Italic | `Cmd+I` | `Ctrl+I` |
| Underline | `Cmd+U` | `Ctrl+U` |
| Strikethrough | `Cmd+Shift+X` | `Ctrl+Shift+X` |
| Inline code | `Cmd+Shift+C` | `Ctrl+Shift+C` |
| Apply last highlight | `Cmd+Shift+H` | `Ctrl+Shift+H` |
| Link selected text | `Cmd+K` | `Ctrl+K` |

(`Cmd/Ctrl+K` is JumpTo with no selection and Link with a text selection - context-dependent, inferred.)

### Node types
| Action | macOS | Windows / Linux |
|---|---|---|
| Heading 1 / 2 / 3 | `Opt+Cmd+1/2/3` | `Alt+Ctrl+1/2/3` |
| Paragraph | `Opt+Cmd+4` | `Alt+Ctrl+4` |
| Code block | `Opt+Cmd+6` | `Alt+Ctrl+6` |
| Quote block | `Opt+Cmd+7` | `Alt+Ctrl+7` |
| Bullet | `Opt+Cmd+8` | `Alt+Ctrl+8` |
| To-do | `Opt+Cmd+9` | `Alt+Ctrl+9` |
| Numbered list | `Opt+Cmd+0` | `Alt+Ctrl+0` |
| AI Node | `Opt+Shift+Space` | `Alt+Shift+Space` |

### Panels and visibility
| Action | macOS | Windows / Linux |
|---|---|---|
| Show/hide completed | `Cmd+O` | `Ctrl+O` |
| Star current page | `Cmd+Shift+8` | `Ctrl+Shift+8` |
| Toggle left sidebar | `Ctrl+L` | `Alt+L` |
| Keyboard shortcuts panel | `Cmd+/` | `Ctrl+/` |
| Command Palette | `Opt+W` | `Alt+W` |
| Export | `Cmd+Shift+E` | `Alt+Shift+E` |
| AI Chat | `Ctrl+C` | `Alt+C` |
| Presentation mode | `Cmd+Shift+P` | `Ctrl+Shift+P` |

### Panes (desktop)
| Action | macOS | Windows | Linux |
|---|---|---|---|
| New pane | `Ctrl+R` | `Alt+R` | `Alt+R` |
| Focus/open pane 1-8 | `Ctrl+1..8` | `Alt+1..8` | `Alt+Shift+1..8` |
| Open item in pane 1-8 | `Ctrl+Shift+1..8` | `Alt+Shift+1..8` | `Ctrl+Shift+1..8` |
| Close pane | `Ctrl+Cmd+W` | `Alt+Ctrl+W` | `Alt+Ctrl+W` |
| Open in new pane (mouse) | Option-click bullet | Alt-click bullet | Alt-click bullet |

### Collaboration
| Action | macOS | Windows / Linux |
|---|---|---|
| Add comment | `Ctrl+M` | `Ctrl+M` |
| Mark as read | `Cmd+Shift+Enter` | `Ctrl+Shift+Enter` |
| Previous unread | `Opt+Ctrl+P` | `Alt+Ctrl+P` |
| Next unread | `Opt+Ctrl+N` | `Alt+Ctrl+N` |

### System
| Action | macOS | Windows / Linux |
|---|---|---|
| Save and sync | `Cmd+S` | `Ctrl+S` |
| Undo | `Cmd+Z` | `Ctrl+Z` |
| Redo | `Cmd+Y` or `Cmd+Shift+Z` | `Ctrl+Y` or `Ctrl+Shift+Z` |
| Print | `Cmd+P` | `Ctrl+P` |

### Inline triggers (typed)
| Trigger | Effect | Source |
|---|---|---|
| `/` | slash command menu | https://workflowy.com/help/slash-command |
| `[[` | internal link picker | https://workflowy.com/help/internal-external-links |
| `((` | mirror-from picker | https://workflowy.com/help/mirrors |
| `!!` | date picker | https://workflowy.com/help/date-tags |
| `#`, `@` | tag / mention autocomplete | https://workflowy.com/help/tags-mentions |
| `#`/`##`/`###`, `>`, `[ ]`, `---`, ```` ``` ````, `1.` + space | H1-3, quote, todo, divider, code, numbered | https://workflowy.com/help/text-block-formats , https://blog.workflowy.com/update-2026-01-22/ |
| Desktop global Quick Add | `Opt+Cmd+N` / `Win+Alt+N` | https://workflowy.com/help/quick-add |
| Remove date chip | Cmd-click / Alt-click | https://workflowy.com/help/date-tags |

---

## 28. Portability summary for a local terminal outliner (inferred)

**Core, directly portable:** single tree, names + notes, Enter/Tab/Shift+Tab editing, expand/collapse (incl. expand_all/collapse_all), zoom + breadcrumbs, back/forward history, move up/down/to, complete + show/hide completed, delete + trash + undo/redo, duplicate, multi-select + bulk ops, tags with filter-by-click, live search with ancestors shown and the operator language (AND/OR/-/quotes/`>`/is:/has:/changed:/created:/dates), saved/starred searches, JumpTo fuzzy finder with shortcut codes, mirrors, internal links + backlinks, templates (`#template`), node types (todo, numbered, H1-3, paragraph, quote, code, divider), sort children, count, date tags with NL parsing and date-range search, daily notes / "move to today" agenda, OPML/Markdown/plaintext import & export, themes, split panes, slash menu / command palette.

**Adaptable with compromises:** boards and tables (render as columns/grid in TUI), inline rich text (bold/italic/underline/strike/code supported by most terminals; colors/highlights limited to palette), presentation mode, timeline.

**Inherently GUI/cloud, not meaningful locally:** sharing/permissions/secret links, @mention notifications, fractal comments and unread tracking, file/image upload, previews, OCR, media embeds, Google Calendar sync, Today Digest and daily email summaries, Email-to-Workflowy, Quick Add/Share-To/Web Clipper, Workflowy AI (chat/nodes/actions - unless re-implemented against a local LLM), desktop MCP/REST API, drag-and-drop, deep links, mobile apps, Labs, billing.
