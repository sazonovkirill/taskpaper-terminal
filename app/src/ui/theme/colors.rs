//! The color themes: the colors each part of the screen is drawn in, the built-in themes and
//! which of them a terminal can show ([ui.md §3](../../../../docs/specs/ui.md#3-themes-fr-ui-0608),
//! FR-UI-06, FR-UI-08, N-UX-03). Hides the palettes and how the roles derive from them.

use std::ffi::OsStr;

use ratatui::style::Color;

use crate::document::TagColor;

/// The colors of a theme, one per part of the screen (ui.md §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    /// The screen behind everything.
    pub background: Color,
    /// Text in no color of its own.
    pub text: Color,
    /// Plain text blocks, slightly dimmed, less than done blocks (D-37, D-63).
    pub dim_text: Color,
    /// Projects, keys in menus and the bookmark icon (Textual's `$primary`).
    pub primary: Color,
    /// The focus marker while editing and the borders of finders and prompts (Textual's
    /// `$accent`, D-51, D-54).
    pub accent: Color,
    /// The marker of selected blocks, which is dimmed (D-51).
    pub selection: Color,
    /// The focus marker in navigate mode (D-30, D-51).
    pub focus: Color,
    /// The band behind the focused block's lines (D-39).
    pub current_line: Color,
    /// Sections and group headings (Textual's `$secondary`, D-49).
    pub headers: Color,
    /// Rule lines: under the breadcrumbs, between panes and dividers (D-33).
    pub rule: Color,
    /// Links (D-45).
    pub link: Color,
    /// Start date chips (D-46).
    pub start: Color,
    /// Due date chips (D-46).
    pub due: Color,
    /// Overdue date chips, which are also bold and reversed (D-46).
    pub overdue: Color,
    /// Errors, such as a failed save.
    pub error: Color,
    /// The six color tags, red to violet (FR-TAG-05).
    tags: [Color; 6],
    /// The row tints of the six color tags, plain and on the focused row (D-48).
    tints: [(Color, Color); 6],
}

impl Theme {
    /// The color of the color tag `color`.
    #[must_use]
    pub fn tag(&self, color: TagColor) -> Color {
        pick(self.tags, color)
    }

    /// The background a color tag gives its block's row: a very dim shade of its color, a step
    /// brighter on the `focused` row (D-48).
    #[must_use]
    pub fn tint(&self, color: TagColor, focused: bool) -> Color {
        let (plain, bright) = pick(self.tints, color);
        if focused { bright } else { plain }
    }

    /// Warnings, such as a failed backup, and the `@star` star: the yellow tag's color.
    #[must_use]
    pub fn warning(&self) -> Color {
        self.tag(TagColor::Yellow)
    }

    /// No colors at all, for `NO_COLOR`: everything is drawn in the terminal's own colors, with
    /// only modifiers telling parts apart.
    const PLAIN: Self = Self {
        background: Color::Reset,
        text: Color::Reset,
        dim_text: Color::Reset,
        primary: Color::Reset,
        accent: Color::Reset,
        selection: Color::Reset,
        focus: Color::Reset,
        current_line: Color::Reset,
        headers: Color::Reset,
        rule: Color::Reset,
        link: Color::Reset,
        start: Color::Reset,
        due: Color::Reset,
        overdue: Color::Reset,
        error: Color::Reset,
        tags: [Color::Reset; 6],
        tints: [(Color::Reset, Color::Reset); 6],
    };

    /// Terminal Dark: the terminal's 16 ANSI colors, so it follows the user's palette, and grey
    /// 248 of the 256 for notes, as 16 colors have no grey between the default foreground,
    /// which many terminals draw as ANSI gray or white, and dark gray, which looks as dim as a
    /// done block (D-37, D-63). Its row tints are dark shades of the tags' colors from the 256
    /// colors' color cube, as 16 colors have no dim shades and text is unreadable on the bright
    /// ones; the focused row's is the cube's next step brighter, two steps for blue, whose first
    /// step barely shows (D-48, D-119).
    const fn terminal() -> Self {
        let (red, orange, yellow, green, blue, violet) = (
            Color::Red,
            Color::LightRed,
            Color::Yellow,
            Color::Green,
            Color::Blue,
            Color::Magenta,
        );
        let tags = [red, orange, yellow, green, blue, violet];
        let tints = [
            (Color::Indexed(52), Color::Indexed(88)),
            (Color::Indexed(94), Color::Indexed(130)),
            (Color::Indexed(58), Color::Indexed(100)),
            (Color::Indexed(22), Color::Indexed(28)),
            (Color::Indexed(17), Color::Indexed(19)),
            (Color::Indexed(53), Color::Indexed(90)),
        ];
        let primary = Color::LightBlue;
        Self {
            background: Color::Reset,
            text: Color::Reset,
            dim_text: Color::Indexed(248),
            primary,
            accent: Color::Yellow,
            selection: primary,
            focus: primary,
            current_line: Color::Black,
            headers: Color::LightMagenta,
            rule: Color::DarkGray,
            link: Color::Cyan,
            start: Color::Green,
            due: Color::Red,
            overdue: Color::Red,
            error: Color::Red,
            tags,
            tints,
        }
    }
}

/// What `values`, one per tag from red to violet, holds for `color`.
fn pick<T>(values: [T; 6], color: TagColor) -> T {
    let [red, orange, yellow, green, blue, violet] = values;
    match color {
        TagColor::Red => red,
        TagColor::Orange => orange,
        TagColor::Yellow => yellow,
        TagColor::Green => green,
        TagColor::Blue => blue,
        TagColor::Violet => violet,
    }
}

/// How much of a tag's color its row tint mixes into the background, in percent, plain and on
/// the focused row (D-48).
const TINT: (u32, u32) = (15, 25);

/// How much of the background plain text blocks mix into the text color, in percent (D-37).
const DIMMED: u32 = 25;

impl Theme {
    /// The theme of a truecolor palette, `base` its background, surface, text, muted and accent
    /// colors and `tags` its red to violet, each as `0xRRGGBB`: its accent as the primary color
    /// (projects and focus), orange as the accent, as Textual's default accent is orange, violet
    /// for headers, blue for links, the muted color for rules and its surface for the current
    /// line ([reference/themes.md](../../../../docs/reference/themes.md#names-and-roles)).
    fn from_palette(base: [u32; 5], tags: [u32; 6]) -> Self {
        let rgb = Color::from_u32;
        let [background, surface, text, muted, accent] = base;
        let [red, orange, _, green, blue, violet] = tags;
        let tint = |tag: u32| {
            (
                rgb(mix(background, tag, TINT.0)),
                rgb(mix(background, tag, TINT.1)),
            )
        };
        Self {
            background: rgb(background),
            text: rgb(text),
            dim_text: rgb(mix(text, background, DIMMED)),
            primary: rgb(accent),
            accent: rgb(orange),
            selection: rgb(accent),
            focus: rgb(accent),
            current_line: rgb(surface),
            headers: rgb(violet),
            rule: rgb(muted),
            link: rgb(blue),
            start: rgb(green),
            due: rgb(red),
            overdue: rgb(red),
            error: rgb(red),
            tags: tags.map(rgb),
            tints: tags.map(tint),
        }
    }
}

/// `from` with `percent` percent of `to` mixed in, channel by channel.
fn mix(from: u32, to: u32, percent: u32) -> u32 {
    [16, 8, 0].into_iter().fold(0, |mixed, shift| {
        let (a, b) = ((from >> shift) & 0xff, (to >> shift) & 0xff);
        let channel = (a * (100 - percent) + b * percent + 50) / 100;
        mixed | (channel << shift)
    })
}

/// What colors the terminal and the user allow.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ColorSupport {
    /// `NO_COLOR` is set: no colors, only modifiers (N-UX-03).
    None,
    /// The 16 ANSI colors, all a terminal is taken to have unless `COLORTERM` says more.
    #[default]
    Ansi,
    /// 24-bit color, which `COLORTERM` says the terminal has.
    TrueColor,
}

impl ColorSupport {
    /// What the environment allows: none when `no_color`, the value of `NO_COLOR`, is set and
    /// not empty (no-color.org), truecolor when `colorterm`, the value of `COLORTERM`, is
    /// `truecolor` or `24bit`, else the 16 ANSI colors.
    #[must_use]
    pub fn detect(no_color: Option<&OsStr>, colorterm: Option<&OsStr>) -> Self {
        if no_color.is_some_and(|value| !value.is_empty()) {
            Self::None
        } else if colorterm.is_some_and(|value| value == "truecolor" || value == "24bit") {
            Self::TrueColor
        } else {
            Self::Ansi
        }
    }
}

/// How a built-in theme gets its colors.
#[derive(Debug, Clone, Copy)]
enum Source {
    /// The terminal's 16 ANSI colors.
    Terminal,
    /// A truecolor palette: its background, surface, text, muted and accent colors, then its red, orange, yellow, green, blue and violet, each as `0xRRGGBB`,
    /// as [reference/themes.md](../../../../docs/reference/themes.md) lists them.
    Palette([u32; 5], [u32; 6]),
}

/// One of the built-in themes, available without any settings file (FR-UI-08).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltIn(usize);

#[expect(
    clippy::unreadable_literal,
    reason = "colors are written as the palettes write them, `#rrggbb`"
)]
/// Each built-in theme's name in `config.toml`, its name on screen and its colors: the
/// terminal theme, then the twelve truecolor ones of
/// [reference/themes.md](../../../../docs/reference/themes.md) in its order.
const THEMES: [(&str, &str, Source); 13] = [
    ("terminal-dark", "Terminal Dark", Source::Terminal),
    (
        "catppuccin-mocha",
        "Catppuccin Mocha",
        Source::Palette(
            [0x1e1e2e, 0x313244, 0xcdd6f4, 0x6c7086, 0xb4befe],
            [0xf38ba8, 0xfab387, 0xf9e2af, 0xa6e3a1, 0x89b4fa, 0xcba6f7],
        ),
    ),
    (
        "catppuccin-latte",
        "Catppuccin Latte",
        Source::Palette(
            [0xeff1f5, 0xccd0da, 0x4c4f69, 0x9ca0b0, 0x7287fd],
            [0xd20f39, 0xfe640b, 0xdf8e1d, 0x40a02b, 0x1e66f5, 0x8839ef],
        ),
    ),
    (
        "tokyo-night",
        "Tokyo Night",
        Source::Palette(
            [0x1a1b26, 0x292e42, 0xc0caf5, 0x565f89, 0x7dcfff],
            [0xf7768e, 0xff9e64, 0xe0af68, 0x9ece6a, 0x7aa2f7, 0xbb9af7],
        ),
    ),
    (
        "dracula",
        "Dracula",
        Source::Palette(
            [0x282a36, 0x44475a, 0xf8f8f2, 0x6272a4, 0xff79c6],
            [0xff5555, 0xffb86c, 0xf1fa8c, 0x50fa7b, 0x8be9fd, 0xbd93f9],
        ),
    ),
    (
        "nord",
        "Nord",
        Source::Palette(
            [0x2e3440, 0x434c5e, 0xd8dee9, 0x4c566a, 0x88c0d0],
            [0xbf616a, 0xd08770, 0xebcb8b, 0xa3be8c, 0x81a1c1, 0xb48ead],
        ),
    ),
    (
        "gruvbox-dark",
        "Gruvbox Dark",
        Source::Palette(
            [0x282828, 0x3c3836, 0xebdbb2, 0x928374, 0x8ec07c],
            [0xfb4934, 0xfe8019, 0xfabd2f, 0xb8bb26, 0x83a598, 0xd3869b],
        ),
    ),
    (
        "rose-pine",
        "Rosé Pine",
        Source::Palette(
            [0x191724, 0x26233a, 0xe0def4, 0x6e6a86, 0xc4a7e7],
            [0xeb6f92, 0xebbcba, 0xf6c177, 0x9ccfd8, 0x31748f, 0xc4a7e7],
        ),
    ),
    (
        "rose-pine-dawn",
        "Rosé Pine Dawn",
        Source::Palette(
            [0xfaf4ed, 0xf2e9e1, 0x575279, 0x9893a5, 0x907aa9],
            [0xb4637a, 0xd7827e, 0xea9d34, 0x56949f, 0x286983, 0x907aa9],
        ),
    ),
    (
        "kanagawa-wave",
        "Kanagawa Wave",
        Source::Palette(
            [0x1f1f28, 0x2a2a37, 0xdcd7ba, 0x727169, 0x7fb4ca],
            [0xff5d62, 0xffa066, 0xe6c384, 0x98bb6c, 0x7e9cd8, 0x957fb8],
        ),
    ),
    (
        "everforest-dark",
        "Everforest Dark",
        Source::Palette(
            [0x2d353b, 0x3d484d, 0xd3c6aa, 0x859289, 0x83c092],
            [0xe67e80, 0xe69875, 0xdbbc7f, 0xa7c080, 0x7fbbb3, 0xd699b6],
        ),
    ),
    (
        "one-dark",
        "One Dark",
        Source::Palette(
            [0x282c34, 0x3e4451, 0xabb2bf, 0x5c6370, 0x56b6c2],
            [0xe06c75, 0xd19a66, 0xe5c07b, 0x98c379, 0x61afef, 0xc678dd],
        ),
    ),
    (
        "solarized-light",
        "Solarized Light",
        Source::Palette(
            [0xfdf6e3, 0xeee8d5, 0x657b83, 0x93a1a1, 0x2aa198],
            [0xdc322f, 0xcb4b16, 0xb58900, 0x859900, 0x268bd2, 0x6c71c4],
        ),
    ),
];

impl BuiltIn {
    /// The theme used until one is chosen: Terminal Dark (D-63).
    pub const DEFAULT: Self = Self(0);

    /// Every built-in theme, in the order the theme picker lists them.
    pub fn all() -> impl Iterator<Item = Self> {
        (0..THEMES.len()).map(Self)
    }

    /// The theme at `position` in [`BuiltIn::all`], if there is one.
    #[must_use]
    pub fn nth(position: usize) -> Option<Self> {
        (position < THEMES.len()).then_some(Self(position))
    }

    /// The theme's position in [`BuiltIn::all`].
    #[must_use]
    pub fn position(self) -> usize {
        self.0
    }

    /// The theme `config.toml` names `id`, such as `nord`.
    #[must_use]
    pub fn named(id: &str) -> Option<Self> {
        Self::all().find(|theme| theme.id() == id)
    }

    /// The theme's name in `config.toml`, such as `catppuccin-mocha`.
    #[must_use]
    pub fn id(self) -> &'static str {
        self.entry().0
    }

    /// The theme's name on screen, such as `Catppuccin Mocha`.
    #[must_use]
    pub fn name(self) -> &'static str {
        self.entry().1
    }

    /// The colors the theme is drawn in with `support`: none without colors, and Terminal Dark
    /// when it is a truecolor theme and the terminal lacks truecolor, so that every theme is
    /// usable in 16 colors (N-UX-03, D-63).
    #[must_use]
    pub fn colors(self, support: ColorSupport) -> Theme {
        match (self.entry().2, support) {
            (_, ColorSupport::None) => Theme::PLAIN,
            (Source::Terminal, _) | (Source::Palette(..), ColorSupport::Ansi) => Theme::terminal(),
            (Source::Palette(base, tags), ColorSupport::TrueColor) => {
                Theme::from_palette(base, tags)
            }
        }
    }

    /// The theme's row of [`THEMES`].
    fn entry(self) -> (&'static str, &'static str, Source) {
        THEMES
            .get(self.0)
            .copied()
            .expect("a built-in theme is made only from a position in THEMES")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(id: &str) -> BuiltIn {
        BuiltIn::named(id).unwrap()
    }

    /// Every color of `theme`, each with what it is for.
    fn roles(theme: &Theme) -> Vec<(String, Color)> {
        let mut roles = vec![
            ("background".to_owned(), theme.background),
            ("text".to_owned(), theme.text),
            ("dim_text".to_owned(), theme.dim_text),
            ("primary".to_owned(), theme.primary),
            ("accent".to_owned(), theme.accent),
            ("selection".to_owned(), theme.selection),
            ("focus".to_owned(), theme.focus),
            ("current_line".to_owned(), theme.current_line),
            ("headers".to_owned(), theme.headers),
            ("rule".to_owned(), theme.rule),
            ("link".to_owned(), theme.link),
            ("start".to_owned(), theme.start),
            ("due".to_owned(), theme.due),
            ("overdue".to_owned(), theme.overdue),
            ("error".to_owned(), theme.error),
            ("warning".to_owned(), theme.warning()),
        ];
        for color in TagColor::ALL {
            roles.push((format!("{color:?}"), theme.tag(color)));
            roles.push((format!("{color:?} tint"), theme.tint(color, false)));
            roles.push((format!("{color:?} focused tint"), theme.tint(color, true)));
        }
        roles
    }

    // N-UX-03
    #[test]
    fn no_color_leaves_out_every_color() {
        assert_eq!(
            ColorSupport::detect(Some("1".as_ref()), Some("truecolor".as_ref())),
            ColorSupport::None
        );
        assert_eq!(
            ColorSupport::detect(Some("".as_ref()), None),
            ColorSupport::Ansi,
            "an empty NO_COLOR does not count"
        );
        for theme in BuiltIn::all() {
            for (role, color) in roles(&theme.colors(ColorSupport::None)) {
                assert_eq!(color, Color::Reset, "{} {role}", theme.id());
            }
        }
    }

    // ui.md §3
    #[test]
    fn colorterm_says_whether_the_terminal_has_truecolor() {
        for value in ["truecolor", "24bit"] {
            assert_eq!(
                ColorSupport::detect(None, Some(value.as_ref())),
                ColorSupport::TrueColor
            );
        }
        for value in [Some("256color"), Some(""), Some("TRUECOLOR "), None] {
            assert_eq!(
                ColorSupport::detect(None, value.map(OsStr::new)),
                ColorSupport::Ansi,
                "{value:?}"
            );
        }
        assert_eq!(ColorSupport::default(), ColorSupport::Ansi);
    }

    // FR-UI-08
    #[test]
    fn the_twelve_themes_and_the_terminal_one_are_built_in() {
        let names: Vec<&str> = BuiltIn::all().map(BuiltIn::name).collect();
        assert_eq!(
            names,
            [
                "Terminal Dark",
                "Catppuccin Mocha",
                "Catppuccin Latte",
                "Tokyo Night",
                "Dracula",
                "Nord",
                "Gruvbox Dark",
                "Rosé Pine",
                "Rosé Pine Dawn",
                "Kanagawa Wave",
                "Everforest Dark",
                "One Dark",
                "Solarized Light",
            ]
        );
        for (position, theme) in BuiltIn::all().enumerate() {
            assert_eq!(theme.position(), position);
            assert_eq!(BuiltIn::nth(position), Some(theme));
            assert_eq!(BuiltIn::named(theme.id()), Some(theme));
            let id = theme
                .name()
                .to_lowercase()
                .replace(' ', "-")
                .replace('é', "e");
            assert_eq!(theme.id(), id);
        }
        assert_eq!(BuiltIn::nth(13), None);
        assert_eq!(BuiltIn::named("Nord"), None, "names in the file are ids");
        assert_eq!(BuiltIn::named("terminal-light"), None, "removed (D-63)");
        assert_eq!(BuiltIn::DEFAULT, named("terminal-dark"));
    }

    // N-UX-03, D-63
    #[test]
    fn terminal_dark_uses_only_the_16_ansi_colors_but_for_notes_and_tints() {
        let ansi = [
            Color::Reset,
            Color::Black,
            Color::Red,
            Color::Green,
            Color::Yellow,
            Color::Blue,
            Color::Magenta,
            Color::Cyan,
            Color::Gray,
            Color::DarkGray,
            Color::LightRed,
            Color::LightGreen,
            Color::LightYellow,
            Color::LightBlue,
            Color::LightMagenta,
            Color::LightCyan,
            Color::White,
        ];
        let dark = named("terminal-dark").colors(ColorSupport::Ansi);
        for (role, color) in roles(&dark) {
            if role == "dim_text" || role.ends_with("tint") {
                continue;
            }
            assert!(ansi.contains(&color), "{role}: {color:?}");
        }
        assert_eq!(dark.dim_text, Color::Indexed(248), "grey 248 (D-37)");
        assert_eq!(dark, named("terminal-dark").colors(ColorSupport::TrueColor));
        assert_eq!(
            (dark.background, dark.text, dark.primary, dark.current_line),
            (Color::Reset, Color::Reset, Color::LightBlue, Color::Black)
        );
    }

    // FR-TAG-05, D-48, D-119
    #[test]
    fn terminal_dark_tints_rows_in_dark_shades_of_the_256_colors() {
        let dark = named("terminal-dark").colors(ColorSupport::Ansi);
        let tints = TagColor::ALL.map(|color| (dark.tint(color, false), dark.tint(color, true)));
        assert_eq!(
            tints,
            [(52, 88), (94, 130), (58, 100), (22, 28), (17, 19), (53, 90)]
                .map(|(plain, focused)| (Color::Indexed(plain), Color::Indexed(focused)))
        );
    }

    // N-UX-03, D-63
    #[test]
    fn a_truecolor_theme_without_truecolor_is_terminal_dark() {
        let dark = named("terminal-dark").colors(ColorSupport::Ansi);
        for theme in BuiltIn::all().skip(1) {
            assert_eq!(theme.colors(ColorSupport::Ansi), dark, "{}", theme.id());
        }
    }

    // D-37, D-63
    #[test]
    fn notes_are_dimmed_in_every_theme_with_colors() {
        for theme in BuiltIn::all() {
            for support in [ColorSupport::Ansi, ColorSupport::TrueColor] {
                let colors = theme.colors(support);
                let notes = colors.dim_text;
                let id = theme.id();
                assert_ne!(notes, colors.text, "{id} {support:?}");
                assert!(
                    ![Color::Reset, Color::Gray, Color::White].contains(&notes),
                    "{id} {support:?}: terminals draw {notes:?} as their default foreground"
                );
                assert!(
                    ![Color::DarkGray, Color::Black].contains(&notes),
                    "{id} {support:?}: {notes:?} is as dim as a done block"
                );
            }
        }
    }

    // FR-UI-08, reference/themes.md
    #[test]
    #[expect(
        clippy::unreadable_literal,
        reason = "colors as the palettes write them"
    )]
    fn a_truecolor_theme_takes_its_roles_from_its_palette() {
        let nord = named("nord").colors(ColorSupport::TrueColor);
        let rgb = Color::from_u32;
        assert_eq!(nord.background, rgb(0x2e3440));
        assert_eq!(nord.text, rgb(0xd8dee9));
        assert_eq!(nord.current_line, rgb(0x434c5e));
        assert_eq!(nord.rule, rgb(0x4c566a));
        assert_eq!(
            [nord.primary, nord.focus, nord.selection],
            [rgb(0x88c0d0); 3]
        );
        assert_eq!(nord.accent, rgb(0xd08770), "orange");
        assert_eq!(nord.headers, rgb(0xb48ead), "violet");
        assert_eq!(nord.link, rgb(0x81a1c1), "blue");
        assert_eq!(nord.start, rgb(0xa3be8c), "green");
        assert_eq!([nord.due, nord.overdue, nord.error], [rgb(0xbf616a); 3]);
        assert_eq!(nord.warning(), rgb(0xebcb8b));
        assert_eq!(nord.tag(TagColor::Violet), rgb(0xb48ead));
        // A quarter of the way from the text to the background, rounded.
        assert_eq!(nord.dim_text, rgb(0xaeb4bf));
        // 15% and 25% of the way from the background to the tag's color.
        assert_eq!(nord.tint(TagColor::Red, false), rgb(0x443b46));
        assert_eq!(nord.tint(TagColor::Red, true), rgb(0x523f4b));
        let latte = named("catppuccin-latte").colors(ColorSupport::TrueColor);
        assert_eq!(latte.background, rgb(0xeff1f5));
        assert_eq!(latte.tag(TagColor::Blue), rgb(0x1e66f5));
    }

    #[test]
    fn every_truecolor_theme_tells_its_parts_apart() {
        for theme in BuiltIn::all().skip(1) {
            let colors = theme.colors(ColorSupport::TrueColor);
            let id = theme.id();
            assert_ne!(colors.text, colors.background, "{id}");
            assert_ne!(colors.current_line, colors.background, "{id}");
            for color in TagColor::ALL {
                let (plain, focused) = (colors.tint(color, false), colors.tint(color, true));
                assert_ne!(plain, colors.background, "{id} {color:?}");
                assert_ne!(plain, focused, "{id} {color:?}");
            }
        }
    }
}
