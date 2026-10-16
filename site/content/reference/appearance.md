+++
title = "Appearance and themes"
weight = 120
description = "The thirteen built-in themes, color depth and NO_COLOR, Nerd Font icons and their fallbacks, and word wrap."
+++

## Themes

tpt has thirteen built-in themes. **Terminal Dark**, the default, uses your terminal's own 16 colors, so it follows whatever palette you set there. The twelve others are truecolor themes, light and dark, that bring their own colors and background.

### Choosing a theme

**Choose Theme** in Go to Command opens the theme picker, with the theme in use highlighted. Each theme you highlight, by typing or with the arrows, shows at once across the whole app. `Enter` keeps it and saves it to `config.toml`; `Esc` goes back to the theme you had. The [settings window](@/reference/settings.md#the-settings-window) can change the theme too, with the same live preview. In `config.toml`, the theme's name goes in `theme`:

```toml
theme = "nord"
```

### Color depth

The truecolor themes need a terminal that says it has truecolor, through `COLORTERM=truecolor` or `COLORTERM=24bit`. Without it, a truecolor theme shows as Terminal Dark. Your choice is kept, and shows in full once the terminal has truecolor. Meanwhile the theme picker says "No truecolor here: themes show as Terminal Dark".

On a truecolor terminal, tpt asks the terminal for its colors when it starts, and draws Terminal Dark in those exact colors. It looks the same, and its fades can run.

With `NO_COLOR` set to anything but empty, tpt uses no colors at all: only bold, dim, italic, underline, strikethrough and reverse. The theme picker then says "NO_COLOR is set: themes show no colors".

### The built-in themes

{{ themes() }}

## Icons

tpt draws [Nerd Font](https://www.nerdfonts.com/) icons by default: the house in the breadcrumbs, the date chips' calendars, the star, flag and bookmark, the views' icons, the attachments and the entries of menus and finders. Your terminal's font must have the glyphs. If they show as boxes or question marks, set `icons = "none"`, and tpt draws plain Unicode symbols instead: `⌂`, `↦`, `⇥`, `★`, `⚐`, `⚑`, `☼`. The `?` panel and the settings window's Icons entry say how. The settings window labels its settings in plain text, so it stays readable either way.

When the locale is not UTF-8, by the first of `LC_ALL`, `LC_CTYPE` and `LANG` that is set, both settings draw ASCII instead: `~` for home, `[x]` for done, `>` for a fold.

## Word wrap

With word wrap **off**, the default, each block takes one row, and text too long for it ends in `…`, before its flag, star and bookmark. With it **on**, long text wraps onto more rows under its start, breaking between words. `Alt+z` or `gw` turns it on or off for the session, and the `word_wrap` setting keeps it. The block being edited always takes one line, scrolling sideways as you type.

{{ keys(topic="appearance") }}

## Settings

{{ settings(topic="appearance") }}
