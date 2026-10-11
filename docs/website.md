# Website

- Status: Draft
- Last updated: 2026-10-11

What the tpt product website holds and how it is built. The site lives in `site/` at the repository root (owner, 2026-10-11). It describes the app; where it and the [functional requirements](functional-requirements.md) or [specs](specs/README.md) disagree, they win and the site is a bug.

tpt is free and open source under the MIT license, and the repository goes public before the site launches (owner, 2026-10-11).

Priorities use the scale of the functional requirements. IDs are stable; reference them from tasks and commits.

## Goals

1. **Interest a newcomer** in a few minutes: the landing page shows what tpt is and why it is worth trying.
2. **Teach the value** in 10 to 15 minutes of reading: the Tutorial.
3. **Answer every question** about a feature, key or setting: the Reference.

## W-SITE · Site

| ID | Requirement | Priority |
|---|---|---|
| W-SITE-01 | A static site in `site/`, built with [Zola](https://www.getzola.org) (a single Rust binary, pinned to one version) into plain HTML, CSS and a little JavaScript that any static host serves with no server code. Pages work with JavaScript off, except for animations. | Must |
| W-SITE-02 | Pages: Home (landing), Tutorial, Reference, Pricing. Every page has the same header with links to all four and to the GitHub repository, and the same footer. | Must |
| W-SITE-03 | One command from the repository root builds the site into an ignored output folder, and one serves it locally with live reload. | Must |
| W-SITE-04 | Tutorial and Reference are written in Markdown, one file per chapter or topic, so they read well in a diff and stay easy to update. | Must |
| W-SITE-05 | The visual style matches the app: dark by default, the Terminal Dark palette as accent colors, a monospace font for keys and code. The site follows the system's light or dark preference. | Must |
| W-SITE-06 | Works on phones: readable at 360 px wide, no horizontal page scroll, screenshots scale down. | Must |
| W-SITE-07 | A check command builds the site and fails on a broken internal link, a missing image or an anchor that does not exist. | Must |
| W-SITE-08 | Accessible: semantic headings, alt text on every screenshot, visible keyboard focus, motion reduced when the system asks for it. | Must |
| W-SITE-09 | Fast: no fonts or scripts from third-party hosts; images sized and lazily loaded below the fold. | Should |
| W-SITE-10 | Published to a public host on every push to `main` that changes `site/`. | Later |

## W-SHOT · Screenshots

| ID | Requirement | Priority |
|---|---|---|
| W-SHOT-01 | Every screenshot is taken from the real `tpt` binary by a script in `site/`: it starts the app on a demo `.taskpaper` file in a throwaway home, presses the scene's keys in tmux, captures the screen with its colors and renders a PNG. No screenshot is edited by hand. | Must |
| W-SHOT-02 | Scenes are a list of named key sequences in one file; adding a screenshot means adding a scene. Dates in the demo file are relative to the day the script runs, so date chips always read "today", "tomorrow" and so on. | Must |
| W-SHOT-03 | One command regenerates every screenshot after a UI change. The PNGs are committed, so building the site needs no `tpt` binary, tmux or browser. | Must |
| W-SHOT-04 | Screenshots show truecolor Terminal Dark with Nerd Font icons, at 2x for sharp text, inside a terminal window frame (frameless on request). The font is downloaded at a pinned version and checksum, not committed. | Must |
| W-SHOT-05 | Short animated clips (GIF or WebP) of a key sequence, for features that are about motion, such as Quick Add, zoom and the landing flashes. | Should |
| W-SHOT-06 | Screenshots in other built-in themes, for the Reference page on themes. | Should |

## W-HOME · Landing page

| ID | Requirement | Priority |
|---|---|---|
| W-HOME-01 | Opens with the product name, a one-line pitch, the install command `cargo install --git https://github.com/sazonovkirill/taskpaper-terminal` (owner, 2026-10-11; there are no releases yet) and a large hero screenshot of a realistic outline. | Must |
| W-HOME-02 | One long page that presents the app progressively as the reader scrolls, in the manner of Apple's product pages: a sequence of full-width sections, each one idea, one headline, a sentence or two and one screenshot or clip. | Must |
| W-HOME-03 | The sections cover, in order: plain-text TaskPaper files you own; an infinite outline you zoom into; keyboard-first editing (navigate and edit modes); dates, Today and the other special views; notes and comments; split panes; themes and animation; works anywhere a terminal does (SSH, tmux). | Must |
| W-HOME-04 | Sections reveal as they scroll into view (fade and slide, screenshots that zoom or pan). Without JavaScript, or with reduced motion, everything is shown statically. | Must |
| W-HOME-05 | Ends with a call to action: install, read the Tutorial, star on GitHub. | Must |

## W-PRICE · Pricing page

| ID | Requirement | Priority |
|---|---|---|
| W-PRICE-01 | A joke pricing page laid out like a real SaaS pricing table, with three plans side by side: Basic, Pro and Pro Max (owner, 2026-10-11). | Must |
| W-PRICE-02 | Each plan appears to add features on top of the previous one ("Everything in Basic, plus…"). Every feature listed is one the app really has, and all three plans have all of them. | Must |
| W-PRICE-03 | The page visibly pushes the reader toward the middle plan, as real pricing pages do: Pro is raised, larger, highlighted, badged "Most popular" and has the boldest button. | Must |
| W-PRICE-04 | Every plan costs $0 monthly ($0 annually), with a monthly/annual toggle that changes nothing. | Must |
| W-PRICE-05 | It still answers the real question plainly: tpt is free, with no account, telemetry or subscription, and open source under the MIT license. | Must |

## W-TUT · Tutorial

| ID | Requirement | Priority |
|---|---|---|
| W-TUT-01 | Readable start to finish in 10 to 15 minutes: about 2,500 to 3,500 words, plus screenshots. | Must |
| W-TUT-02 | Explains why plain-text TaskPaper files are useful: readable and editable anywhere, diff and sync with Git or any file sync, no lock-in, other TaskPaper apps open the same file. | Must |
| W-TUT-03 | Explains why an infinite outliner is useful: one tree for everything, any branch can become the whole screen, structure grows as thinking does. | Must |
| W-TUT-04 | Goes from simple to advanced, each chapter building on the last: open a file, move and edit, structure (indent, move, fold), zoom and breadcrumbs, tasks and done, dates and the Today view, the Navigator and finders, notes and comments, panes, settings and themes. | Must |
| W-TUT-05 | Teaches by doing: each chapter gives the keys to press in a sample file the reader can download, and shows what they will see. It does not try to cover every feature; each chapter links to the Reference for the rest. | Must |
| W-TUT-06 | Ends with where to go next: the Reference, the `?` panel and the command palette. | Must |

## W-REF · Reference

| ID | Requirement | Priority |
|---|---|---|
| W-REF-01 | Covers every user-visible feature, every key binding and every setting of the app. A feature, key or setting the app has and the Reference does not is a bug. | Must |
| W-REF-02 | Organized by topic, following the functional requirement groups: files and saving, blocks and their types, navigate and edit mode, navigation and zoom, selection and structure, tags, dates and done, special views, Navigator and finders, attachments, Quick Add, panes, appearance and themes, animation and weather, settings, command line, compatibility with other TaskPaper apps. | Must |
| W-REF-03 | The key and settings tables are generated from the app's source (the command registry and the settings list), as the printable cheat sheet's build script already does outside the repository (in the project's shared files, `cheatsheet/build.py`), so they cannot drift from the app. Each key lists its classic and kitty-protocol variants (D-42). | Must |
| W-REF-04 | A check fails when the source has a command or setting the Reference does not mention. | Must |
| W-REF-05 | Every page has a table of contents and stable anchors, so a key or setting can be linked to directly. | Must |
| W-REF-06 | A search box over the Reference, built at site build time, with no server. | Should |
| W-REF-07 | The printable cheat sheet is part of the Reference, its build script moved into `site/`. | Should |

## Out of scope

- A blog, newsletter, analytics, comments or accounts.
- Versioned documentation for older releases.
- Translations.

## Milestones

Built one milestone at a time, as the app was ([implementation plan](implementation-plan.md#milestones-and-acceptance)): each milestone is one Todoist task naming the previous one as a dependency, its steps run in parallel where they can, and when they are on `main` the Codex milestone review runs (unless the owner skips it) and work stops for the owner's review.

| # | Milestone | Covers | After |
|---|---|---|---|
| W1 | **Foundations.** The `site/` skeleton, build, preview and check commands, the shared layout, header, footer and style in light and dark, the screenshot script moved into `site/` from its prototype in the project's shared files (`site/screenshots/tools/`) with its demo file and scenes, and the Pricing page as the first real page. | W-SITE-01–09, W-SHOT-01–04, W-PRICE | — |
| W2 | **Landing page.** The scrolling story with its sections, hero shot and scroll animations, and the screenshots and clips each section needs. | W-HOME, W-SHOT-05 | W1 |
| W3 | **Tutorial.** The chapters, their sample file and screenshots. | W-TUT | W2 |
| W4 | **Reference.** Every topic page, the generated key and settings tables with their coverage check, the cheat sheet and theme screenshots. | W-REF, W-SHOT-06 | W3 |
| W5 | **Publish.** A `LICENSE` file (MIT), the repository made public by the owner, hosting, the deploy on push, the domain, page titles and social preview images. | W-SITE-10 | W4 |
