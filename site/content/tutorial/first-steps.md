+++
title = "Open, move and edit"
weight = 2
description = "Open the sample file, move around and type your first tasks."
+++

## Open the file

In a terminal, go to the folder that holds the sample file and run:

```sh
tpt tutorial.taskpaper
```

You see the outline. At the top is the breadcrumb bar, with a house icon for the top of the file. At the bottom is the status line: it says `NAVIGATE` and shows the file's path.

The `▌` bar on the left marks the **focused** line. Most keys act on that line.

Run `tpt` on its own to try a demo outline in a temporary file.

## Two modes

tpt has two modes, a little like vi:

- In **navigate mode** every key is a command. <kbd>j</kbd> moves down, it doesn't type a "j".
- In **edit mode** you type text into one line, as in any editor.

You start in navigate mode, and the status line always shows the mode.

## Move around

Press <kbd>j</kbd> to move down and <kbd>k</kbd> to move up, or use the arrow keys.

Press <kbd>G</kbd> to jump to the last line, then <kbd>g</kbd> <kbd>g</kbd> to jump back to the first. Keys written side by side like that are pressed one after the other, not together.

<kbd>Ctrl+D</kbd> and <kbd>Ctrl+U</kbd> move a page down and up.

## Add a task

Move to "Buy a birthday card for Mum" in the INBOX. Press <kbd>o</kbd> to open a new line below it, and type:

```
Water the plants
```

Added next to a task, the new line is a task too, with its dash already there. The status line now says `EDIT`.

Press <kbd>Enter</kbd> to finish this line and start the next one. Type "Take out the recycling", then press <kbd>Esc</kbd> to go back to navigate mode.

{{ shot(src="tutorial/first-steps", alt="tpt showing the sample outline, with a new task, Water the plants, being typed in the INBOX project", caption="Typing a new task in edit mode. The status line says EDIT.") }}

## Change a line

To edit a line that's already there, move to it and press <kbd>Enter</kbd>. The caret goes to the end. Go back to "Water the plants", press <kbd>Enter</kbd>, type " on the balcony" and press <kbd>Esc</kbd>.

## Undo, and saving

Made a mess? Press <kbd>z</kbd> in navigate mode to undo, and <kbd>Z</kbd> to redo.

You never need to save. tpt writes the file about a second after your last change, and keeps a daily backup copy too.

To quit, press <kbd>Ctrl+Q</kbd>. But stay for now: the next chapter carries on in the same file.

More on editing and files is in the [Reference](@/reference/_index.md).
