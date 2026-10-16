+++
title = "Animation and weather"
weight = 130
description = "The short flashes and fades that show what changed, how to reduce or turn them off, and the weather that falls while tpt sits idle."
+++

## Animation

tpt animates only to show where the focus landed, what a command changed, or that it refused something. An animation never makes you wait: every key acts at once, and the effect is drawn over the result. Moving one row, scrolling, typing, opening menus and the `?` panel, and starting and quitting are never animated.

| When | Effect |
|---|---|
| The focus jumps: `gg`, `G`, a page, a finder, Move to, back or forward, zoom, another pane, undo or redo | The block it lands on flashes in the accent color |
| A command changes more than the focused block: paste, duplicate, Move to, a selection, undo, redo, a reload | The changed rows flash |
| A command is refused, such as typing `@` or editing a read-only file | The focused row flashes in the error color |
| A block is completed or uncompleted | Its text eases to its new style |
| A pane shows another place, or opens | Its rows fade in from the top |
| A block is expanded | The rows it shows fade in |
| A status message times out | It fades out |
| Saving takes longer than 0.3 s | A spinner in the status line |
| You preview another theme | A crossfade to it |

Fades need RGB colors on both sides: a truecolor theme, or Terminal Dark on a truecolor terminal that reported its colors. In 16 colors, flashes show for a moment and fades are skipped. With `NO_COLOR`, flashes and fades are skipped too.

### Full, reduced or off

The `animations` setting has three values:

- **full** runs every effect above.
- **reduced** keeps the three flashes, as a single step, with a static spinner, and drops every fade.
- **off** draws no effects, and the spinner is a static `…`.

It is `full` by default, or `reduced` over SSH (when `SSH_CONNECTION` or `SSH_TTY` is set), since each frame crosses the network. The setting is in the settings window, and **Animations: Full**, **Animations: Reduced** and **Animations: Off** in Go to Command switch it and save it. There is no key for them.

## Weather

When tpt sits idle, the season's weather starts falling over the whole screen: rain in spring (March to May), leaves in autumn (September to November) and snow in winter (December to February). Summer has none. Leaves and snow pile up slowly along the bottom, never more than a third of the screen high.

- It starts after `weather_idle_minutes` without input, 30 minutes by default, whatever window or prompt is open. Saving, reloading and other things the app does by itself don't count as input.
- The next key, click, paste or resize ends it at once, and does nothing else: the key you pressed to stop it is not applied. Everything is as it was.
- It plays only while `weather` is on and `animations` is `full`, so not over SSH unless you set `animations = "full"`.
- **Start Weather Animation**, in Go to Command, starts it right away, whatever the settings say. In summer it says "No weather in summer".

Each falling flake covers one cell and the app shows through again once it moves on. The glyphs follow the icon setting, and plain ASCII is used outside a UTF-8 locale.

{{ keys(topic="animation") }}

## Settings

{{ settings(topic="animation") }}
