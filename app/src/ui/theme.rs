//! How the UI looks: the icons it draws ([ui.md §7](../../../docs/specs/ui.md#7-icons-fr-ui-09-fr-ui-10))
//! and its color themes ([ui.md §3](../../../docs/specs/ui.md#3-themes-fr-ui-0608)).

mod colors;
mod icons;
mod terminal_colors;

pub use colors::{BuiltIn, ColorSupport, Theme};
pub use icons::{Icon, IconSet, IconSetting, utf8_locale};
pub use terminal_colors::TerminalColors;
