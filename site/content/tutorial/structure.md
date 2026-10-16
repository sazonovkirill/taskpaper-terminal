+++
title = "Shape the outline"
weight = 3
description = "Indent, move, fold and add projects."
+++

## Indent and outdent

Under "Home", "Buy a new washer" is really part of "Fix the dripping kitchen tap".

Move to "Buy a new washer" and press <kbd>Tab</kbd>. It becomes a subtask of the tap. <kbd>Shift+Tab</kbd> moves a line back out.

A line always takes everything inside it along.

## Move lines up and down

In "Packing", move to "Comfortable shoes" and press <kbd>Alt+k</kbd> twice. The shoes move to the top of the list. <kbd>Alt+j</kbd> moves a line down.

On a Mac, <kbd>Alt</kbd> is Option, and your terminal must send it: turn on "Use Option as Meta key" in Terminal, or set Option to "Esc+" in iTerm2. Move Up and Move Down are also in the command palette (last chapter).

## Fold

Move to "Trip to Lisbon" and press <kbd>h</kbd>. The project folds up into one line, ending in `▸ 9`: nine open tasks are hidden inside. Press <kbd>l</kbd> to unfold it.

On a folded line, or one with nothing inside, <kbd>h</kbd> goes to the parent instead. <kbd>'</kbd> just toggles the fold.

Press <kbd>_</kbd> to fold everything and <kbd>+</kbd> to unfold it all. A file always opens unfolded.

{{ shot(src="tutorial/structure", alt="The sample outline with Trip to Lisbon folded into one line ending in a fold marker and the count 9, and Buy a new washer indented under Fix the dripping kitchen tap", caption="Trip to Lisbon folded, hiding nine open tasks, and the washer indented under the tap.") }}

## Add a project

Press <kbd>G</kbd> to go to the last line, then <kbd>#</kbd> to go to its parent, "Reading list". Now press <kbd>o</kbd> and then <kbd>p</kbd>. That adds a new **project** below the reading list. Type "Ideas" and press <kbd>Esc</kbd>. The colon is already there.

Now press <kbd>i</kbd> and then <kbd>t</kbd> to add a **task** inside the project. Type "Learn to juggle" and press <kbd>Esc</kbd>.

The keys that add lines pick a place: <kbd>o</kbd> below, <kbd>O</kbd> above, <kbd>i</kbd> as the last child and <kbd>I</kbd> as the first. Follow one with <kbd>p</kbd> for a project, <kbd>t</kbd> for a task, <kbd>n</kbd> for a text line or <kbd>s</kbd> for a section, or just start typing and tpt picks the type that fits. Typing changes the type too: end a line with a colon to make a project, start it with "- " to make a task.

## Delete

To delete a line and everything in it, press <kbd>d</kbd> <kbd>d</kbd>. Try it on "Learn to juggle", then press <kbd>z</kbd> to bring it back.

Copy, paste, selection and the rest are in the [Reference](@/reference/_index.md).
