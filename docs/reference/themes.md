# Built-in Themes

- Status: Accepted
- Last updated: 2026-11-15

The thirty-six truecolor themes the app ships built in, alongside Terminal Dark ([FR-UI-08](../functional-requirements.md#fr-ui--layout-and-appearance)). Thirty-five come from permissively licensed terminal and editor themes; Star Trek TNG is an original LCARS-inspired adaptation (D-150). Twenty-four are dark and twelve are light, and the theme pickers list them in those two sections, **Dark Themes** and **Light Themes**, with Terminal Dark first among the dark ones (D-153). A theme is light when its background is: a test holds every theme's section to its background's WCAG luminance, above 0.5 for a light one.

## Selection

The original twelve were chosen on 2026-10-04 (D-19) and Star Trek TNG added on 2026-10-30 (D-150). On 2026-11-15 (D-153) came the thirteen themes of the owner's [textual-items-plus-plus](https://github.com/sazonovkirill/textual-items-plus-plus) that tpt lacked, with the palettes that project took from each origin, and ten more well-known themes, chosen for popularity (Neovim's dotfyle rankings, VS Code installs, terminal theme catalogs), a permissive license and a look not already covered: Catppuccin Macchiato, Solarized Dark, Monokai, Ayu Mirage, Material Palenight and Poimandres among the dark ones, and Tokyo Night Day, Everforest Light, One Light and Ayu Light among the light ones, which with the five from textual-items-plus-plus grow from three to twelve. Monokai Pro is left out as commercial; the classic Monokai is not.

### Dark Themes

| # | Theme | Origin | License | Added |
|---|---|---|---|---|
| — | Terminal Dark | The terminal's own 16 ANSI colors; [below](#names-and-roles) | — | 2026-10-07 (D-63) |
| 1 | Catppuccin Mocha | [catppuccin/palette](https://github.com/catppuccin/palette) | MIT, © 2021 Catppuccin | 2026-10-04 |
| 2 | Catppuccin Macchiato | [catppuccin/palette](https://github.com/catppuccin/palette) | MIT, © 2021 Catppuccin | 2026-11-15 |
| 3 | Tokyo Night | [enkia/tokyo-night-vscode-theme](https://github.com/enkia/tokyo-night-vscode-theme); hex values from the [folke/tokyonight.nvim](https://github.com/folke/tokyonight.nvim) "night" palette | MIT, © 2018 Enkia; the nvim port is Apache-2.0 | 2026-10-04 |
| 4 | Dracula | [draculatheme.com](https://draculatheme.com), [dracula/alacritty](https://github.com/dracula/alacritty) | MIT, © 2018 Dracula Theme | 2026-10-04 |
| 5 | Nord | [nordtheme.com](https://www.nordtheme.com), [nordtheme/vim](https://github.com/nordtheme/vim) | MIT, © 2016 Sven Greb | 2026-10-04 |
| 6 | Gruvbox Dark | [morhetz/gruvbox](https://github.com/morhetz/gruvbox) | MIT/X11 (stated in the README) | 2026-10-04 |
| 7 | Rosé Pine | [rosepinetheme.com](https://rosepinetheme.com), [rose-pine/alacritty](https://github.com/rose-pine/alacritty) | MIT, © Rosé Pine | 2026-10-04 |
| 8 | Kanagawa Wave | [rebelot/kanagawa.nvim](https://github.com/rebelot/kanagawa.nvim) | MIT, © 2021 Tommaso Laurenzi | 2026-10-04 |
| 9 | Kanagawa Dragon | [rebelot/kanagawa.nvim](https://github.com/rebelot/kanagawa.nvim) | MIT, © 2021 Tommaso Laurenzi | 2026-11-15, from textual-items-plus-plus |
| 10 | Everforest Dark | [sainnhe/everforest](https://github.com/sainnhe/everforest) (medium contrast) | MIT, © 2019 sainnhe | 2026-10-04 |
| 11 | One Dark | Atom's [one-dark-syntax](https://github.com/atom/atom/tree/master/packages/one-dark-syntax) | MIT, © 2016 GitHub Inc. | 2026-10-04 |
| 12 | Solarized Dark | [altercation/solarized](https://github.com/altercation/solarized) | MIT, © 2011 Ethan Schoonover | 2026-11-15 |
| 13 | Monokai | Wimer Hazenberg's Monokai, as [sickill/vim-monokai](https://github.com/sickill/vim-monokai) gives it; orange from the original TextMate theme | MIT, © 2009-2016 Marcin Kulik | 2026-11-15 |
| 14 | GitHub Dark | [primer/github-vscode-theme](https://github.com/primer/github-vscode-theme) "Dark Default" (palette from [primer/primitives](https://github.com/primer/primitives)) | MIT, © 2020 Primer / © 2018 GitHub Inc. | 2026-11-15, from textual-items-plus-plus |
| 15 | Flexoki Dark | [kepano/flexoki](https://github.com/kepano/flexoki) | MIT, © 2023 Steph Ango | 2026-11-15, from textual-items-plus-plus |
| 16 | Melange Dark | [savq/melange-nvim](https://github.com/savq/melange-nvim) | MIT, © 2021 Sergio Alejandro Vargas | 2026-11-15, from textual-items-plus-plus |
| 17 | Moonfly | [bluz71/vim-moonfly-colors](https://github.com/bluz71/vim-moonfly-colors) | MIT, © 2017-present vim-moonfly-colors authors | 2026-11-15, from textual-items-plus-plus |
| 18 | Nightfly | [bluz71/vim-nightfly-colors](https://github.com/bluz71/vim-nightfly-colors) | MIT, © 2020-present vim-nightfly-colors authors | 2026-11-15, from textual-items-plus-plus |
| 19 | Cyberdream | [scottmckendry/cyberdream.nvim](https://github.com/scottmckendry/cyberdream.nvim) | MIT, © 2023 Scott McKendry | 2026-11-15, from textual-items-plus-plus |
| 20 | Synthwave '84 | [robb0wen/synthwave-vscode](https://github.com/robb0wen/synthwave-vscode) | MIT, © 2019 Robb Owen | 2026-11-15, from textual-items-plus-plus |
| 21 | Ayu Mirage | [ayu-theme/ayu-colors](https://github.com/ayu-theme/ayu-colors) (npm `ayu` 9.1.0) | MIT, © Konstantin Pschera | 2026-11-15 |
| 22 | Material Palenight | [kaicataldo/material.vim](https://github.com/kaicataldo/material.vim), after Mattia Astorino's Material Theme | MIT, © 2017 Kai Cataldo | 2026-11-15 |
| 23 | Poimandres | [drcmda/poimandres-theme](https://github.com/drcmda/poimandres-theme) | MIT, © 2023 drcmda | 2026-11-15 |
| 24 | Star Trek TNG | Original adaptation of the TNG LCARS colors; research below | Project MIT license; no third-party code or artwork included | 2026-10-30 |

### Light Themes

| # | Theme | Origin | License | Added |
|---|---|---|---|---|
| 1 | Catppuccin Latte | [catppuccin/palette](https://github.com/catppuccin/palette) | MIT, © 2021 Catppuccin | 2026-10-04 |
| 2 | Tokyo Night Day | [folke/tokyonight.nvim](https://github.com/folke/tokyonight.nvim) "day" palette (`extras/lua/tokyonight_day.lua`) | Apache-2.0, Folke Lemaitre | 2026-11-15 |
| 3 | Rosé Pine Dawn | [rose-pine/alacritty](https://github.com/rose-pine/alacritty) | MIT, © Rosé Pine | 2026-10-04 |
| 4 | Everforest Light | [sainnhe/everforest](https://github.com/sainnhe/everforest) (medium contrast) | MIT, © 2019 sainnhe | 2026-11-15 |
| 5 | One Light | Atom's [one-light-syntax](https://github.com/atom/one-light-syntax) | MIT, © 2016 GitHub Inc. | 2026-11-15 |
| 6 | Solarized Light | [altercation/solarized](https://github.com/altercation/solarized) | MIT, © 2011 Ethan Schoonover | 2026-10-04 |
| 7 | Gruvbox Light | [morhetz/gruvbox](https://github.com/morhetz/gruvbox) (`faded_*` accents) | MIT/X11 (stated in the README) | 2026-11-15, from textual-items-plus-plus |
| 8 | GitHub Light | [primer/github-vscode-theme](https://github.com/primer/github-vscode-theme) "Light Default" (palette from [primer/primitives](https://github.com/primer/primitives)) | MIT, © 2020 Primer / © 2018 GitHub Inc. | 2026-11-15, from textual-items-plus-plus |
| 9 | Flexoki Light | [kepano/flexoki](https://github.com/kepano/flexoki) | MIT, © 2023 Steph Ango | 2026-11-15, from textual-items-plus-plus |
| 10 | Melange Light | [savq/melange-nvim](https://github.com/savq/melange-nvim) | MIT, © 2021 Sergio Alejandro Vargas | 2026-11-15, from textual-items-plus-plus |
| 11 | Dayfox | [EdenEast/nightfox.nvim](https://github.com/EdenEast/nightfox.nvim) | MIT, © 2021 James Simpson | 2026-11-15, from textual-items-plus-plus |
| 12 | Ayu Light | [ayu-theme/ayu-colors](https://github.com/ayu-theme/ayu-colors) (npm `ayu` 9.1.0) | MIT, © Konstantin Pschera | 2026-11-15 |

## Palettes

Each palette is mapped to the roles the UI needs. The original twelve palettes were copied from the origin's own palette files on 2026-10-04 (One Dark's are computed from the HSL values in Atom's `colors.less`). Star Trek TNG was researched on 2026-10-30 as described below. The 2026-11-15 palettes were copied on that day: the thirteen from textual-items-plus-plus from that project's [themes reference](https://github.com/sazonovkirill/textual-items-plus-plus/blob/main/specs/reference/themes.md), which cites each origin's palette file (its Background, Current-line band, Text, Checklist done, Primary, Due-date chip and Tag colors map to these columns), and the other ten from the origin files named below.

The table lists the dark themes, then the light ones.

- **Background**: the screen.
- **Surface**: the selected block, sidebar and popups.
- **Text**: normal block text.
- **Muted**: completed tasks, breadcrumbs, hints.
- **Accent**: the cursor, focused pane border and project headings.
- **Red … Violet**: the palette's hues. Red, yellow, green and blue are the four color tags of [FR-TAG-02](../functional-requirements.md#fr-tag--tags-as-metadata) (D-138); all six are also reused for other roles, such as overdue dates (red), the accent (orange) and headers (violet).

The remaining roles in [the UI spec](../specs/ui.md#3-themes-fr-ui-0608), such as links, rule lines and errors, are drawn from these columns by the implementation.

| Theme | Background | Surface | Text | Muted | Accent | Red | Orange | Yellow | Green | Blue | Violet |
|---|---|---|---|---|---|---|---|---|---|---|---|
| Catppuccin Mocha | `#1e1e2e` | `#313244` | `#cdd6f4` | `#6c7086` | `#b4befe` | `#f38ba8` | `#fab387` | `#f9e2af` | `#a6e3a1` | `#89b4fa` | `#cba6f7` |
| Catppuccin Macchiato | `#24273a` | `#363a4f` | `#cad3f5` | `#6e738d` | `#b7bdf8` | `#ed8796` | `#f5a97f` | `#eed49f` | `#a6da95` | `#8aadf4` | `#c6a0f6` |
| Tokyo Night | `#1a1b26` | `#292e42` | `#c0caf5` | `#565f89` | `#7dcfff` | `#f7768e` | `#ff9e64` | `#e0af68` | `#9ece6a` | `#7aa2f7` | `#bb9af7` |
| Dracula | `#282a36` | `#44475a` | `#f8f8f2` | `#6272a4` | `#ff79c6` | `#ff5555` | `#ffb86c` | `#f1fa8c` | `#50fa7b` | `#8be9fd` | `#bd93f9` |
| Nord | `#2e3440` | `#434c5e` | `#d8dee9` | `#4c566a` | `#88c0d0` | `#bf616a` | `#d08770` | `#ebcb8b` | `#a3be8c` | `#81a1c1` | `#b48ead` |
| Gruvbox Dark | `#282828` | `#3c3836` | `#ebdbb2` | `#928374` | `#8ec07c` | `#fb4934` | `#fe8019` | `#fabd2f` | `#b8bb26` | `#83a598` | `#d3869b` |
| Rosé Pine | `#191724` | `#26233a` | `#e0def4` | `#6e6a86` | `#c4a7e7` | `#eb6f92` | `#ebbcba` | `#f6c177` | `#9ccfd8` | `#31748f` | `#c4a7e7` |
| Kanagawa Wave | `#1f1f28` | `#2a2a37` | `#dcd7ba` | `#727169` | `#7fb4ca` | `#ff5d62` | `#ffa066` | `#e6c384` | `#98bb6c` | `#7e9cd8` | `#957fb8` |
| Kanagawa Dragon | `#181616` | `#393836` | `#c5c9c5` | `#737c73` | `#8ba4b0` | `#c4746e` | `#b6927b` | `#c4b28a` | `#87a987` | `#8ba4b0` | `#8992a7` |
| Everforest Dark | `#2d353b` | `#3d484d` | `#d3c6aa` | `#859289` | `#83c092` | `#e67e80` | `#e69875` | `#dbbc7f` | `#a7c080` | `#7fbbb3` | `#d699b6` |
| One Dark | `#282c34` | `#3e4451` | `#abb2bf` | `#5c6370` | `#56b6c2` | `#e06c75` | `#d19a66` | `#e5c07b` | `#98c379` | `#61afef` | `#c678dd` |
| Solarized Dark | `#002b36` | `#073642` | `#839496` | `#586e75` | `#2aa198` | `#dc322f` | `#cb4b16` | `#b58900` | `#859900` | `#268bd2` | `#6c71c4` |
| Monokai | `#272822` | `#3c3d37` | `#f8f8f2` | `#75715e` | `#66d9ef` | `#f92672` | `#fd971f` | `#e6db74` | `#a6e22e` | `#66d9ef` | `#ae81ff` |
| GitHub Dark | `#0d1117` | `#161b22` | `#e6edf3` | `#6e7681` | `#58a6ff` | `#ff7b72` | `#db6d28` | `#d29922` | `#3fb950` | `#58a6ff` | `#a371f7` |
| Flexoki Dark | `#100f0f` | `#1c1b1a` | `#cecdc3` | `#6f6e69` | `#4385be` | `#d14d41` | `#da702c` | `#d0a215` | `#879a39` | `#4385be` | `#8b7ec8` |
| Melange Dark | `#292522` | `#34302c` | `#ece1d7` | `#867462` | `#a3a9ce` | `#d47766` | `#e49b5d` | `#ebc06d` | `#85b695` | `#a3a9ce` | `#cf9bc2` |
| Moonfly | `#080808` | `#1c1c1c` | `#c6c6c6` | `#626262` | `#80a0ff` | `#ff5d5d` | `#de935f` | `#e3c78a` | `#8cc85f` | `#80a0ff` | `#ae81ff` |
| Nightfly | `#011627` | `#092236` | `#c3ccdc` | `#7c8f8f` | `#82aaff` | `#fc514e` | `#f78c6c` | `#e3d18a` | `#a1cd5e` | `#82aaff` | `#ae81ff` |
| Cyberdream | `#16181a` | `#1e2124` | `#ffffff` | `#7b8496` | `#5ea1ff` | `#ff6e5e` | `#ffbd5e` | `#f1ff5e` | `#5eff6c` | `#5ea1ff` | `#bd5eff` |
| Synthwave '84 | `#262335` | `#34294f` | `#ffffff` | `#848bbd` | `#36f9f6` | `#fe4450` | `#f97e72` | `#fede5d` | `#72f1b8` | `#36f9f6` | `#b893ce` |
| Ayu Mirage | `#242936` | `#1a1f29` | `#cccac2` | `#6e7c8f` | `#ffcc66` | `#f28779` | `#ffa659` | `#ffcd66` | `#d5ff80` | `#73d0ff` | `#dfbfff` |
| Material Palenight | `#292d3e` | `#343b51` | `#a6accd` | `#676e95` | `#c792ea` | `#ff5370` | `#f78c6c` | `#ffcb6b` | `#c3e88d` | `#82aaff` | `#bb80b3` |
| Poimandres | `#1b1e28` | `#303340` | `#a6accd` | `#767c9d` | `#5de4c7` | `#d0679d` | `#fcc5e9` | `#fffac2` | `#5fb3a1` | `#89ddff` | `#f087bd` |
| Star Trek TNG | `#000000` | `#1a121a` | `#ffcc99` | `#664466` | `#cc99cc` | `#cc6666` | `#ffcc66` | `#ffff99` | `#99cc99` | `#99ccff` | `#9999cc` |
| Catppuccin Latte | `#eff1f5` | `#ccd0da` | `#4c4f69` | `#9ca0b0` | `#7287fd` | `#d20f39` | `#fe640b` | `#df8e1d` | `#40a02b` | `#1e66f5` | `#8839ef` |
| Tokyo Night Day | `#e1e2e7` | `#c4c8da` | `#3760bf` | `#848cb5` | `#007197` | `#f52a65` | `#b15c00` | `#8c6c3e` | `#587539` | `#2e7de9` | `#9854f1` |
| Rosé Pine Dawn | `#faf4ed` | `#f2e9e1` | `#575279` | `#9893a5` | `#907aa9` | `#b4637a` | `#d7827e` | `#ea9d34` | `#56949f` | `#286983` | `#907aa9` |
| Everforest Light | `#fdf6e3` | `#efebd4` | `#5c6a72` | `#939f91` | `#35a77c` | `#f85552` | `#f57d26` | `#dfa000` | `#8da101` | `#3a94c5` | `#df69ba` |
| One Light | `#fafafa` | `#e5e5e6` | `#383a42` | `#a0a1a7` | `#0184bc` | `#e45649` | `#986801` | `#c18401` | `#50a14f` | `#4078f2` | `#a626a4` |
| Solarized Light | `#fdf6e3` | `#eee8d5` | `#657b83` | `#93a1a1` | `#2aa198` | `#dc322f` | `#cb4b16` | `#b58900` | `#859900` | `#268bd2` | `#6c71c4` |
| Gruvbox Light | `#fbf1c7` | `#ebdbb2` | `#3c3836` | `#928374` | `#427b58` | `#9d0006` | `#af3a03` | `#b57614` | `#79740e` | `#076678` | `#8f3f71` |
| GitHub Light | `#ffffff` | `#f6f8fa` | `#1f2328` | `#6e7781` | `#0969da` | `#cf222e` | `#bc4c00` | `#9a6700` | `#1a7f37` | `#0969da` | `#8250df` |
| Flexoki Light | `#fffcf0` | `#f2f0e5` | `#100f0f` | `#878580` | `#205ea6` | `#af3029` | `#bc5215` | `#8e6b01` | `#66800b` | `#205ea6` | `#5e409d` |
| Melange Light | `#f1f1f1` | `#e9e1db` | `#54433a` | `#7d6658` | `#465aa4` | `#bf0021` | `#bc5c00` | `#a06d00` | `#3a684a` | `#465aa4` | `#904180` |
| Dayfox | `#f6f2ee` | `#e4dcd4` | `#3d2b5a` | `#837a72` | `#2848a9` | `#a5222f` | `#955f61` | `#ac5402` | `#396847` | `#2848a9` | `#6e33ce` |
| Ayu Light | `#fcfcfc` | `#f0f1f3` | `#5c6166` | `#adaeb1` | `#f29718` | `#f07171` | `#fa8532` | `#eba400` | `#86b300` | `#22a4e6` | `#a37acc` |

Palette names behind the less obvious cells:

- **Dracula** has no blue; Blue uses its cyan.
- **Rosé Pine** and **Dawn** have no orange or green: Orange uses *rose*, Green uses *foam*, Blue uses *pine*, and the accent is *iris*, the same as Violet.
- **Rosé Pine** and **Dawn**: Surface uses the palette's *overlay* color, which stands out more than its *surface* color.
- **Kanagawa**: Surface is *sumiInk4*, Muted *fujiGray*, Red *peachRed*, Orange *surimiOrange*, Yellow *carpYellow*, Green *springGreen*, Blue *crystalBlue*, Violet *oniViolet*, Accent *springBlue*.
- **Nord**: Muted is *nord3*, which is low-contrast for text; the implementation may lighten it for readability.
- **One Dark**: Surface is the selection color, `lighten(background, 10%)`.
- **One Light**: computed from the HSL values in `styles/colors.less`; Surface is the selection color, `darken(background, 8%)`, Muted `mono-3`, Accent `hue-1` (cyan), Orange `hue-6`, Yellow `hue-6-2`.
- **Catppuccin Macchiato** maps like Mocha: Surface *surface0*, Muted *overlay0*, Accent *lavender*, Orange *peach*, Violet *mauve*.
- **Solarized Dark** maps like Solarized Light with the dark base tones: Background *base03*, Surface *base02*, Text *base0*, Muted *base01*.
- **Monokai** has no separate blue: Blue and the accent use its cyan `#66d9ef`; Red is its pink `#f92672`; Surface is the line highlight `#3c3d37`.
- **Ayu Mirage** and **Ayu Light** come from the generated values of npm `ayu` 9.1.0 (`dist/generated/*.js`, built from `themes/*.yaml`): Background *editor.bg*, Text *editor.fg*, Muted *syntax.comment*, Accent *common.accent.tint*, Red *markup*, Orange *keyword*, Yellow *func*, Green *string*, Blue *entity*, Violet *constant*. Mirage's Surface is *editor.line* `#1a1f29`, darker than its background, as Ayu draws its current line; Light's is *editor.line*, `#828e9f` at 10% over the background, `#f0f1f3`.
- **Material Palenight**: Surface is *selection*, Muted *comments*, Accent *purple*, Violet *violet* `#bb80b3`.
- **Poimandres** has no orange or green of its own: Orange uses *pink2* `#fcc5e9`, Green *teal2* `#5fb3a1`, so it differs from the accent *teal1* `#5de4c7`; Surface is *background1*, Muted the comment color *blueGray2*, Violet the magenta `#f087bd`.
- **Tokyo Night Day** maps like Tokyo Night: Surface *bg_highlight*, Muted *comment*, Accent *cyan*. Its text and Solarized's are low-contrast by design; every theme's text clears 4:1 on its background and 3:1 on its surface.
- **Everforest Light** maps like Everforest Dark: Surface *bg2*, Muted *grey1*, Accent *aqua*.
- **Gruvbox Light** takes *faded_aqua* `#427b58` for the accent, as Gruvbox Dark takes *bright_aqua*.
- **Synthwave '84**'s config name drops the apostrophe: `synthwave-84`.

### Star Trek TNG research and adaptation

[StarTrek.com credits Michael Okuda](https://www.startrek.com/news/collectible-usable-okudagrams-now-a-reality) with the LCARS computer graphics. The numerical starting point here is the published [trekcolors LCARS color reference](https://leonawicz.github.io/trekcolors/reference/lcars_colors.html): lilac `#cc99cc`, golden-tanoi `#ffcc66`, tanoi `#ffcc99`, pale-canary `#ffff99`, eggplant `#664466`, blue-bell `#9999cc`, chestnut-rose `#cc6666` and anakiwa `#99ccff`. These are a digital interpretation, not a claim of exact screen-matched production colors. [TheLCARS.com's current color guide](https://www.thelcars.com/colors.php) explicitly revises its Classic colors, so those newer values were not used as the TNG palette.

The adaptation uses black behind peach text, lilac for project headings and focus, gold for editing focus and popup borders, blue-violet section headings, pale-yellow warnings and pale-blue links. The current-line surface is a dark eggplant-derived `#1a121a`, keeping the active row visible without overwhelming the text. Green `#99cc99` is an added companion hue for the app's green tags and start dates; red, yellow, green and blue retain their existing semantic roles. Row tints and dim text use the same mixing rules as every other truecolor theme. The original adaptation includes no LCARS artwork, fonts or code.

## Names and roles

`config.toml` names each theme in lower case with hyphens and without accents or apostrophes, such as `terminal-dark`, `catppuccin-mocha`, `rose-pine-dawn` and `synthwave-84`. The theme picker lists them in the order of the tables above: Terminal Dark and the dark themes under **Dark Themes**, then the light ones under **Light Themes**.

The roles of [the UI spec](../specs/ui.md#3-themes-fr-ui-0608) come from a palette's columns as follows: text and background as named; the palette's Accent is the primary color (projects, the focus and selection markers, keys in menus); Orange is the accent (the focus marker while editing, finder and prompt borders), as Textual's default accent is orange; Violet the headers; Blue the links; Muted the rule lines; Surface the current line; Green the start chips and Red the due and overdue chips and errors; Yellow the `@today` star and warnings; Red, Yellow, Green and Blue the four color tags (D-138). Dim text (notes) is the text color mixed with 25% of the background; text on the current line and on row tints is the text color; a tag's row tint is the background mixed with 15% of the tag's color, 25% on the focused row.

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
| Red, yellow, green, blue color tags | red, yellow, green, blue |
| Start, due and overdue chips, errors | green, red, red, red |

Sixteen colors have no dim shades, and text is hard to read on their bright ones, so a color tag's row tint comes from the 256 colors' color cube, like the notes' grey: the darkest shade of the tag's color, and the cube's next step brighter on the focused row (two steps for blue, whose first step is hardly brighter), so focus shows on a tinted row too (D-119):

| Tag | Row tint | Focused row tint |
|---|---|---|
| Red | 52 (`#5f0000`) | 88 (`#870000`) |
| Yellow | 58 (`#5f5f00`) | 100 (`#878700`) |
| Green | 22 (`#005f00`) | 28 (`#008700`) |
| Blue | 17 (`#00005f`) | 19 (`#0000af`) |

On a truecolor terminal that reports its colors when asked, Terminal Dark keeps these roles but draws each in the RGB color the terminal reported for it, and the 256 colors as xterm defines them (`#hex` above), so it looks the same and fades like the truecolor themes ([the UI spec](../specs/ui.md#3-themes-fr-ui-0608), D-128).

## Notes for implementation

- The hex values assume a 24-bit-color terminal. [The UI spec](../specs/ui.md#3-themes-fr-ui-0608) uses them only when `COLORTERM` indicates truecolor; behavior in 16-color terminals follows N-UX-03 in [requirements.md](../requirements.md).
- Each origin's license requires keeping its copyright and permission notice, so the shipped binary's license notices list every origin of the Selection tables. Tokyo Night's and Tokyo Night Day's values come from an Apache-2.0 port, which also asks for its license text; Gruvbox states MIT in its README without a copyright line.
- The default theme is Terminal Dark (D-63). Without truecolor every theme, light ones included, is drawn as Terminal Dark.
