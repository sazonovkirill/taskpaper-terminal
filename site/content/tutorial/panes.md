+++
title = "Panes"
weight = 9
description = "Look at two parts of the outline side by side."
+++

To see two places at once, split the screen into **panes**, side by side.

## Open a pane

Go to "Trip to Lisbon" and press <kbd>Ctrl+W</kbd> <kbd>v</kbd> (<kbd>Ctrl+W</kbd>, then <kbd>v</kbd>). <kbd>g</kbd> <kbd>|</kbd> does the same. A second pane opens on the right, zoomed into the trip. The left pane still shows the whole outline. Each pane's header ends in its number, `1` and `2`.

{{ shot(src="tutorial/panes", alt="Two panes side by side: the whole outline on the left, numbered 1, and Trip to Lisbon zoomed in on the right, numbered 2", caption="The whole outline on the left, the trip on the right.") }}

The new pane has the keys, and only its focus bar shows.

## Move between panes

- <kbd>g</kbd> <kbd>1</kbd> and <kbd>g</kbd> <kbd>2</kbd> focus pane 1 and pane 2. <kbd>Alt+1</kbd> and <kbd>Alt+2</kbd> do the same, where your terminal lets <kbd>Alt</kbd> through.
- <kbd>Ctrl+W</kbd> <kbd>h</kbd> and <kbd>Ctrl+W</kbd> <kbd>l</kbd> focus the pane to the left and right. In terminals with the kitty keyboard protocol, <kbd>Ctrl+,</kbd> and <kbd>Ctrl+.</kbd> do too.

Try it: press <kbd>g</kbd> <kbd>1</kbd>, go to "Look up tram pass prices" in "Before we go" and press <kbd>x</kbd>. It's checked off in both panes: they're two views of one file.

## What each pane keeps

Each pane has its own zoom, history and show-done setting. Folds are shared between panes.

<kbd>Ctrl+W</kbd> <kbd>s</kbd> opens the pane's current place, Today included, in a new pane. In the Navigator and Go to Symbol, <kbd>Ctrl+T</kbd> opens the choice in a new pane.

## Close a pane

Press <kbd>Ctrl+W</kbd> <kbd>c</kbd> to close the focused pane. The last pane can't be closed.

tpt remembers your panes: quit, open the file again, and they're back as you left them. The [Reference](@/reference/_index.md) has the keys for resizing and reordering panes.
