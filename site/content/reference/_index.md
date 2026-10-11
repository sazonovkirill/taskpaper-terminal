+++
title = "Reference"
sort_by = "weight"
insert_anchor_links = "right"
description = "Every feature, key and setting of tpt."
template = "reference-section.html"
page_template = "reference-page.html"
+++

The Reference explains every feature, key and setting of tpt, one topic to a page. The key and settings tables come straight from the app's source: its command registry and the list in its settings window. They show exactly what the app does.

A few conventions run through every page:

- **Keys** are written the way the app writes them in its panels. `Ctrl+D` means hold Ctrl and press D. `gg` and `Ctrl+W v` are sequences: press one key after the other, within a second. A capital letter, such as `G`, means Shift with that letter.
- **Two columns of keys.** Every command has keys that work in any terminal. A few also have a key that only terminals speaking the [kitty keyboard protocol](@/reference/modes.md#classic-and-kitty-keys) can send, such as `Ctrl+Enter`. Those keys get a column of their own, next to the key that works everywhere.
- **Names.** Each command has a name in `config.toml`. Use it to [rebind keys](@/reference/settings.md#setting-keymap), and every row and setting has a link of its own.
- A <span class="ref-panel">?</span> next to a command means it also appears in the `?` shortcuts panel in the app.

In the app, press `?` to see the keys for where you are. `Ctrl+P` or `:` opens Go to Command, which lists every command with its keys. For one page with every key, print the [cheat sheet](@/reference/cheatsheet.md).
