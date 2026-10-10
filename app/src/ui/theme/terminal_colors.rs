//! The colors a terminal reports it draws with, which turn Terminal Dark's terminal colors into
//! the RGB colors they stand for, so that it fades like the truecolor themes (D-128).

use ratatui::style::Color;

/// The terminal's own background and text colors and those of its 16 ANSI colors it reported,
/// each as red, green and blue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalColors {
    /// The background, what [`Color::Reset`] stands for behind text.
    background: [u8; 3],
    /// The text color, what [`Color::Reset`] stands for in text.
    text: [u8; 3],
    /// ANSI colors 0 to 15, each as reported, if it was.
    ansi: [Option<[u8; 3]>; 16],
}

impl TerminalColors {
    /// The colors a terminal reported: its `background`, its `text` color and its `ansi` colors
    /// 0 to 15, each if it reported it.
    #[must_use]
    pub fn new(background: [u8; 3], text: [u8; 3], ansi: [Option<[u8; 3]>; 16]) -> Self {
        Self {
            background,
            text,
            ansi,
        }
    }

    /// The background as an RGB color.
    pub(super) fn background(&self) -> Color {
        rgb(self.background)
    }

    /// `color` as the RGB color the terminal draws it in, the text color for [`Color::Reset`].
    /// An ANSI color the terminal did not report stays as it is; the 256 colors above them are
    /// the same in every terminal.
    pub(super) fn resolve(&self, color: Color) -> Color {
        let ansi = |index: usize| self.ansi.get(index).copied().flatten().map_or(color, rgb);
        match color {
            Color::Reset => rgb(self.text),
            Color::Black => ansi(0),
            Color::Red => ansi(1),
            Color::Green => ansi(2),
            Color::Yellow => ansi(3),
            Color::Blue => ansi(4),
            Color::Magenta => ansi(5),
            Color::Cyan => ansi(6),
            Color::Gray => ansi(7),
            Color::DarkGray => ansi(8),
            Color::LightRed => ansi(9),
            Color::LightGreen => ansi(10),
            Color::LightYellow => ansi(11),
            Color::LightBlue => ansi(12),
            Color::LightMagenta => ansi(13),
            Color::LightCyan => ansi(14),
            Color::White => ansi(15),
            Color::Indexed(index @ 0..16) => ansi(usize::from(index)),
            Color::Indexed(index) => rgb(indexed(index)),
            Color::Rgb(..) => color,
        }
    }
}

/// The color `[red, green, blue]`.
fn rgb([red, green, blue]: [u8; 3]) -> Color {
    Color::Rgb(red, green, blue)
}

/// The red, green and blue of color `index` of the 256 above the 16 ANSI colors: the 6×6×6
/// color cube from 16, then 24 greys from 232, as xterm defines them.
fn indexed(index: u8) -> [u8; 3] {
    let level = |step: u8| if step == 0 { 0 } else { 55 + 40 * step };
    if let Some(grey) = index.checked_sub(232) {
        [8 + 10 * grey; 3]
    } else {
        let cube = index - 16;
        [level(cube / 36), level(cube / 6 % 6), level(cube % 6)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BLACK: [u8; 3] = [0, 0, 0];
    const WHITE: [u8; 3] = [255, 255, 255];

    fn reported(ansi: [Option<[u8; 3]>; 16]) -> TerminalColors {
        TerminalColors::new([1, 2, 3], [4, 5, 6], ansi)
    }

    #[test]
    fn the_terminals_own_colors_are_its_background_and_text() {
        let colors = reported([None; 16]);
        assert_eq!(colors.background(), Color::Rgb(1, 2, 3));
        assert_eq!(colors.resolve(Color::Reset), Color::Rgb(4, 5, 6));
    }

    #[test]
    fn each_ansi_color_is_the_one_the_terminal_reported() {
        let ansi = std::array::from_fn(|index| Some([u8::try_from(index).unwrap(), 0, 0]));
        let colors = reported(ansi);
        let named = [
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
        for (index, color) in (0..).zip(named) {
            assert_eq!(colors.resolve(color), Color::Rgb(index, 0, 0), "{color:?}");
            assert_eq!(
                colors.resolve(Color::Indexed(index)),
                Color::Rgb(index, 0, 0)
            );
        }
    }

    #[test]
    fn an_ansi_color_the_terminal_did_not_report_stays_as_it_is() {
        let mut ansi = [Some(WHITE); 16];
        ansi[1] = None;
        let colors = reported(ansi);
        assert_eq!(colors.resolve(Color::Red), Color::Red);
        assert_eq!(colors.resolve(Color::Indexed(1)), Color::Indexed(1));
        assert_eq!(colors.resolve(Color::Green), Color::Rgb(255, 255, 255));
    }

    #[test]
    fn the_256_colors_are_xterms_cube_and_greys() {
        let colors = reported([Some(BLACK); 16]);
        let cases = [
            (16, [0, 0, 0]),
            (17, [0, 0, 95]),
            (19, [0, 0, 175]),
            (52, [95, 0, 0]),
            (58, [95, 95, 0]),
            (100, [135, 135, 0]),
            (231, [255, 255, 255]),
            (232, [8, 8, 8]),
            (248, [168, 168, 168]),
            (255, [238, 238, 238]),
        ];
        for (index, [red, green, blue]) in cases {
            assert_eq!(
                colors.resolve(Color::Indexed(index)),
                Color::Rgb(red, green, blue),
                "{index}"
            );
        }
    }

    #[test]
    fn an_rgb_color_stays_as_it_is() {
        let colors = reported([None; 16]);
        assert_eq!(colors.resolve(Color::Rgb(9, 8, 7)), Color::Rgb(9, 8, 7));
    }
}
