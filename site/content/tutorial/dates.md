+++
title = "Dates and Today"
weight = 6
description = "Give tasks start and due dates, and see what's on today."
+++

A **start** date says when you want to begin something, a **due** date when it has to be done. Both show as chips at the right end of the row.

## Quick dates

Go to "Reply to Sam about Saturday" in the INBOX and press <kbd>T</kbd>. A red chip appears: due `today`. Now try the others:

- On "Buy a birthday card for Mum", press <kbd>W</kbd>: due **tomorrow**.
- On "Look up tram pass prices", press <kbd>t</kbd>: a green chip, starting **today**.
- On "Call the dentist", under "Home", press <kbd>w</kbd>: starting **tomorrow**.

Lowercase sets the start date, uppercase the due date. <kbd>]</kbd> and <kbd>[</kbd> move the start date a day later or earlier, <kbd>}</kbd> and <kbd>{</kbd> the due date. Chips name dates as you'd say them, like `tomorrow` or `Fri`, and a past due date turns into a bold overdue chip.

## Any date

For any other date, press <kbd>m</kbd> <kbd>d</kbd> for the due date or <kbd>m</kbd> <kbd>s</kbd> for the start date. Go to "Book the hotel", press <kbd>m</kbd> <kbd>d</kbd> and type `fri`. The panel at the bottom shows the date it understood. Press <kbd>Enter</kbd> to set it, or <kbd>Esc</kbd> to cancel.

The prompt reads words like `tomorrow`, `fri` or `next mon`, offsets like `+3d` or `2 weeks`, and dates like `2026-05-02`. To remove a date, erase the text and press <kbd>Enter</kbd>.

## The Today view

Now press <kbd>g</kbd> <kbd>t</kbd>. The pane switches to **Today**: a flat list of everything that starts or is due today or earlier, from anywhere in the file. You should see "Reply to Sam about Saturday" and "Look up tram pass prices", and also "Tram 28, early in the morning", because starred lines show up in Today too. Each line shows the path to where it lives.

{{ shot(src="tutorial/dates", alt="The Today view listing Reply to Sam about Saturday, due today, and Look up tram pass prices, starting today, each followed by INBOX as its location", caption="The Today view gathers what's due or starting today from the whole file.") }}

These aren't copies: <kbd>x</kbd> here checks the task off in the outline too, and <kbd>.</kbd> jumps to a line's place in the outline.

Press <kbd>g</kbd> <kbd>U</kbd> for **Upcoming**, the next seven days grouped by day: the card and the dentist under tomorrow, the hotel under its day. <kbd>H</kbd> goes back, as after a zoom.

The next chapter shows where to find the other views. The [Reference](@/reference/_index.md) lists every date you can type.
