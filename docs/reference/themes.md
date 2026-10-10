# Built-in Themes

- Status: Accepted
- Last updated: 2026-10-07

The twelve color themes the app ships built in ([FR-UI-08](../functional-requirements.md#fr-ui--layout-and-appearance)). Each is a popular, well-maintained terminal and editor theme under a permissive license, so its palette can be compiled into the binary with attribution. Nine are dark and three are light.

## Selection

| # | Theme | Mode | Origin | License |
|---|---|---|---|---|
| 1 | Catppuccin Mocha | Dark | [catppuccin/palette](https://github.com/catppuccin/palette) | MIT, © 2021 Catppuccin |
| 2 | Catppuccin Latte | Light | [catppuccin/palette](https://github.com/catppuccin/palette) | MIT, © 2021 Catppuccin |
| 3 | Tokyo Night | Dark | [enkia/tokyo-night-vscode-theme](https://github.com/enkia/tokyo-night-vscode-theme); hex values from the [folke/tokyonight.nvim](https://github.com/folke/tokyonight.nvim) "night" palette | MIT, © 2018 Enkia; the nvim port is Apache-2.0 |
| 4 | Dracula | Dark | [draculatheme.com](https://draculatheme.com), [dracula/alacritty](https://github.com/dracula/alacritty) | MIT, © 2018 Dracula Theme |
| 5 | Nord | Dark | [nordtheme.com](https://www.nordtheme.com), [nordtheme/vim](https://github.com/nordtheme/vim) | MIT, © 2016 Sven Greb |
| 6 | Gruvbox Dark | Dark | [morhetz/gruvbox](https://github.com/morhetz/gruvbox) | MIT/X11 (stated in the README) |
| 7 | Rosé Pine | Dark | [rosepinetheme.com](https://rosepinetheme.com), [rose-pine/alacritty](https://github.com/rose-pine/alacritty) | MIT, © Rosé Pine |
| 8 | Rosé Pine Dawn | Light | [rose-pine/alacritty](https://github.com/rose-pine/alacritty) | MIT, © Rosé Pine |
| 9 | Kanagawa Wave | Dark | [rebelot/kanagawa.nvim](https://github.com/rebelot/kanagawa.nvim) | MIT, © 2021 Tommaso Laurenzi |
| 10 | Everforest Dark | Dark | [sainnhe/everforest](https://github.com/sainnhe/everforest) (medium contrast) | MIT, © 2019 sainnhe |
| 11 | One Dark | Dark | Atom's [one-dark-syntax](https://github.com/atom/atom/tree/master/packages/one-dark-syntax) | MIT, © 2016 GitHub Inc. |
| 12 | Solarized Light | Light | [altercation/solarized](https://github.com/altercation/solarized) | MIT, © 2011 Ethan Schoonover |

Candidates left out: Monokai Pro is commercial; Synthwave '84, Ayu, Night Owl and Solarized Dark were dropped to keep the list at twelve with visibly different moods. Gruvbox Light, Everforest Light and Tokyo Night Day are natural additions if more light themes are wanted.

## Palettes

Each palette is mapped to the roles the UI needs. Values were copied from the origin's own palette files on 2026-10-04 (One Dark's are computed from the HSL values in Atom's `colors.less`).

- **Background**: the screen.
- **Surface**: the selected block, sidebar and popups.
- **Text**: normal block text.
- **Muted**: completed tasks, breadcrumbs, hints.
- **Accent**: the cursor, focused pane border and project headings.
- **Red … Violet**: the six color tags of [FR-TAG-02](../functional-requirements.md#fr-tag--tags-as-metadata), also reused for overdue dates (red) and chips.

The remaining roles in [the UI spec](../specs/ui.md#3-themes-fr-ui-0608), such as links, rule lines and errors, are drawn from these columns by the implementation.

| Theme | Background | Surface | Text | Muted | Accent | Red | Orange | Yellow | Green | Blue | Violet |
|---|---|---|---|---|---|---|---|---|---|---|---|
| Catppuccin Mocha | `#1e1e2e` | `#313244` | `#cdd6f4` | `#6c7086` | `#b4befe` | `#f38ba8` | `#fab387` | `#f9e2af` | `#a6e3a1` | `#89b4fa` | `#cba6f7` |
| Catppuccin Latte | `#eff1f5` | `#ccd0da` | `#4c4f69` | `#9ca0b0` | `#7287fd` | `#d20f39` | `#fe640b` | `#df8e1d` | `#40a02b` | `#1e66f5` | `#8839ef` |
| Tokyo Night | `#1a1b26` | `#292e42` | `#c0caf5` | `#565f89` | `#7dcfff` | `#f7768e` | `#ff9e64` | `#e0af68` | `#9ece6a` | `#7aa2f7` | `#bb9af7` |
| Dracula | `#282a36` | `#44475a` | `#f8f8f2` | `#6272a4` | `#ff79c6` | `#ff5555` | `#ffb86c` | `#f1fa8c` | `#50fa7b` | `#8be9fd` | `#bd93f9` |
| Nord | `#2e3440` | `#434c5e` | `#d8dee9` | `#4c566a` | `#88c0d0` | `#bf616a` | `#d08770` | `#ebcb8b` | `#a3be8c` | `#81a1c1` | `#b48ead` |
| Gruvbox Dark | `#282828` | `#3c3836` | `#ebdbb2` | `#928374` | `#8ec07c` | `#fb4934` | `#fe8019` | `#fabd2f` | `#b8bb26` | `#83a598` | `#d3869b` |
| Rosé Pine | `#191724` | `#26233a` | `#e0def4` | `#6e6a86` | `#c4a7e7` | `#eb6f92` | `#ebbcba` | `#f6c177` | `#9ccfd8` | `#31748f` | `#c4a7e7` |
| Rosé Pine Dawn | `#faf4ed` | `#f2e9e1` | `#575279` | `#9893a5` | `#907aa9` | `#b4637a` | `#d7827e` | `#ea9d34` | `#56949f` | `#286983` | `#907aa9` |
| Kanagawa Wave | `#1f1f28` | `#2a2a37` | `#dcd7ba` | `#727169` | `#7fb4ca` | `#ff5d62` | `#ffa066` | `#e6c384` | `#98bb6c` | `#7e9cd8` | `#957fb8` |
| Everforest Dark | `#2d353b` | `#3d484d` | `#d3c6aa` | `#859289` | `#83c092` | `#e67e80` | `#e69875` | `#dbbc7f` | `#a7c080` | `#7fbbb3` | `#d699b6` |
| One Dark | `#282c34` | `#3e4451` | `#abb2bf` | `#5c6370` | `#56b6c2` | `#e06c75` | `#d19a66` | `#e5c07b` | `#98c379` | `#61afef` | `#c678dd` |
| Solarized Light | `#fdf6e3` | `#eee8d5` | `#657b83` | `#93a1a1` | `#2aa198` | `#dc322f` | `#cb4b16` | `#b58900` | `#859900` | `#268bd2` | `#6c71c4` |

Palette names behind the less obvious cells:

- **Dracula** has no blue; Blue uses its cyan.
- **Rosé Pine** and **Dawn** have no orange or green: Orange uses *rose*, Green uses *foam*, Blue uses *pine*, and the accent is *iris*, the same as Violet.
- **Rosé Pine** and **Dawn**: Surface uses the palette's *overlay* color, which stands out more than its *surface* color.
- **Kanagawa**: Surface is *sumiInk4*, Muted *fujiGray*, Red *peachRed*, Orange *surimiOrange*, Yellow *carpYellow*, Green *springGreen*, Blue *crystalBlue*, Violet *oniViolet*, Accent *springBlue*.
- **Nord**: Muted is *nord3*, which is low-contrast for text; the implementation may lighten it for readability.
- **One Dark**: Surface is the selection color, `lighten(background, 10%)`.

## Names and roles

`config.toml` names each theme in lower case with hyphens and without accents: `terminal-dark`, `catppuccin-mocha`, `catppuccin-latte`, `tokyo-night`, `dracula`, `nord`, `gruvbox-dark`, `rose-pine`, `rose-pine-dawn`, `kanagawa-wave`, `everforest-dark`, `one-dark` and `solarized-light`. The theme picker lists them in that order.

The roles of [the UI spec](../specs/ui.md#3-themes-fr-ui-0608) come from a palette's columns as follows: text and background as named; the palette's Accent is the primary color (projects, the focus and selection markers, keys in menus); Orange is the accent (the focus marker while editing, finder and prompt borders), as Textual's default accent is orange; Violet the headers; Blue the links; Muted the rule lines; Surface the current line; Green the start chips and Red the due and overdue chips and errors; Yellow the `@today` star and warnings. Dim text (notes) is the text color mixed with 25% of the background; text on the current line and on row tints is the text color; a tag's row tint is the background mixed with 15% of the tag's color, 25% on the focused row.

Terminal Dark, the only 16-color theme (D-63), uses the terminal's ANSI colors, so it follows its palette, except for notes, row tints and the text on the band and tints (D-125): 16 colors have no grey between the default foreground, which many terminals draw as ANSI gray or white, and dark gray, which looks as dim as a done task, so notes take the 256-color grey 248 (D-37):

| Role | Terminal Dark |
|---|---|
| Text, background | the terminal's |
| Dim text | grey 248 (`#a8a8a8`) of the 256 colors |
| Primary, focus, selection | light blue |
| Accent | yellow |
| Current line | black |
| Text on the current line and row tints | grey 255 (`#eeeeee`) of the 256 colors, as the terminal's text is dark on a light palette (D-125) |
| Headers | light magenta |
| Rule lines | dark gray |
| Links | cyan |
| Red, orange, yellow, green, blue, violet | red, light red, yellow, green, blue, magenta |
| Start, due and overdue chips, errors | green, red, red, red |

Sixteen colors have no dim shades, and text is hard to read on their bright ones, so a color tag's row tint comes from the 256 colors' color cube, like the notes' grey: the darkest shade of the tag's color, and the cube's next step brighter on the focused row (two steps for blue, whose first step is hardly brighter), so focus shows on a tinted row too (D-119):

| Tag | Row tint | Focused row tint |
|---|---|---|
| Red | 52 (`#5f0000`) | 88 (`#870000`) |
| Orange | 94 (`#875f00`) | 130 (`#af5f00`) |
| Yellow | 58 (`#5f5f00`) | 100 (`#878700`) |
| Green | 22 (`#005f00`) | 28 (`#008700`) |
| Blue | 17 (`#00005f`) | 19 (`#0000af`) |
| Violet | 53 (`#5f005f`) | 90 (`#870087`) |

On a truecolor terminal that reports its colors when asked, Terminal Dark keeps these roles but draws each in the RGB color the terminal reported for it, and the 256 colors as xterm defines them (`#hex` above), so it looks the same and fades like the truecolor themes ([the UI spec](../specs/ui.md#3-themes-fr-ui-0608), D-128).

## Notes for implementation

- The hex values assume a 24-bit-color terminal. [The UI spec](../specs/ui.md#3-themes-fr-ui-0608) uses them only when `COLORTERM` indicates truecolor; behavior in 16-color terminals follows N-UX-03 in [requirements.md](../requirements.md).
- Each origin's license requires keeping its copyright and permission notice, so the shipped binary's license notices list these twelve. Tokyo Night's values come from an Apache-2.0 port, which also asks for its license text; Gruvbox states MIT in its README without a copyright line.
- The default theme is Terminal Dark (D-63). Without truecolor every theme, light ones included, is drawn as Terminal Dark.
