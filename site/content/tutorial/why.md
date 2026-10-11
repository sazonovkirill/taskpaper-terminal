+++
title = "Why plain text and outlines"
weight = 1
description = "What a plain TaskPaper file and an infinite outline give you."
+++

## Your outline is a plain text file

tpt saves everything in a `.taskpaper` file. That is plain text, with a few simple rules. A line that ends in a colon is a project. A line that starts with a dash is a task. Anything else is a note. Tabs make the structure, and tags like `@done` or `@due(2026-05-02)` hold the details:

```
Trip to Lisbon:
	Before we go:
		- Book the hotel @due(2026-05-02)
		- Renew the travel insurance @done(2026-04-20)
```

That buys you a lot:

- **You can read and edit it anywhere.** Any text editor opens it, on any computer. Nothing is hidden in a database.
- **It syncs and diffs like code.** Put it in Git and every change is a readable diff, or keep it in Dropbox or Syncthing. tpt reloads the file when another device changes it.
- **There is no lock-in.** No account, no server, no export step. If you stop using tpt, your file stays exactly where it was.
- **Other apps open the same file.** TaskPaper on the Mac and other TaskPaper-compatible editors read it too, so you can switch tools without converting anything.

tpt is careful with that file: it writes it atomically, so a crash can't leave half a file, and keeps lines it doesn't understand as they were.

## One outline for everything

tpt is an infinite outliner, in the spirit of Workflowy. Instead of many separate lists, you keep one tree. Every line can have lines inside it, as deep as you like.

- **One tree holds it all.** Work, home, a trip, a reading list: they all live side by side, so nothing gets lost in a forgotten app or a stray file.
- **Any branch can become the whole screen.** You can zoom into a single project, or a single task, and see only that. The rest of the outline is still there, one key away.
- **Structure grows as your thinking does.** Start with a flat list. When an item turns out to have parts, indent a few lines under it. When a project gets big, split it. Moving a line moves everything inside it.

Next, you'll see how that feels in practice.
