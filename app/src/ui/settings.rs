//! The settings the settings window lists, each with its group, name, description and the value
//! the frontend runs with, and applying and keeping a change made there (FR-SET-03,
//! [ui.md §8](../../../docs/specs/ui.md#8-settings-window-fr-set-02-fr-set-03)).

use std::time::Duration;

use super::{
    Tui, config, effects,
    theme::{BuiltIn, Icon, IconSetting},
    widgets,
};
use crate::app::{Configuration, Message, Setting, SettingKind, SettingValue, SpecialView, saving};

/// A setting the window lists, in the window's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Listed {
    Theme,
    Icons,
    Wrap,
    SpecialInbox,
    /// Whether the Navigator lists this special view (D-77, D-80).
    NavigatorView(SpecialView),
    /// Whether the Today view lists starred blocks (D-88).
    TodayStarred,
    /// Whether `qt` adds to the `TODAY` project (D-98).
    SpecialToday,
    AutosaveDelay,
    Backups,
    BackupFolder,
    BackupsKept,
}

/// The settings the window lists, in order. Animations join when they reach the settings file
/// (ui.md §9.3).
const LISTED: [Listed; 17] = [
    Listed::Theme,
    Listed::Icons,
    Listed::Wrap,
    Listed::SpecialInbox,
    Listed::NavigatorView(SpecialView::Today),
    Listed::SpecialToday,
    Listed::TodayStarred,
    Listed::NavigatorView(SpecialView::Upcoming),
    Listed::NavigatorView(SpecialView::Overdue),
    Listed::NavigatorView(SpecialView::Colors),
    Listed::NavigatorView(SpecialView::Logbook),
    Listed::NavigatorView(SpecialView::Starred),
    Listed::NavigatorView(SpecialView::Flagged),
    Listed::AutosaveDelay,
    Listed::Backups,
    Listed::BackupFolder,
    Listed::BackupsKept,
];

/// The `icons` setting's choices, as the settings file names them (ui.md §7).
const ICONS: [(IconSetting, &str); 2] = [(IconSetting::Nerd, "nerd"), (IconSetting::Off, "none")];

impl Listed {
    /// The setting's key in the settings file.
    fn key(self) -> &'static str {
        match self {
            Self::Theme => "theme",
            Self::Icons => "icons",
            Self::Wrap => "word_wrap",
            Self::SpecialInbox => "special_inbox",
            Self::NavigatorView(view) => config::navigator_key(view),
            Self::TodayStarred => "today_starred",
            Self::SpecialToday => "special_today",
            Self::AutosaveDelay => "autosave_delay_ms",
            Self::Backups => "backups.enabled",
            Self::BackupFolder => "backups.dir",
            Self::BackupsKept => "backups.keep",
        }
    }

    /// The icon the window draws before the setting (ui.md §7).
    fn icon(self) -> Icon {
        match self {
            Self::Theme => Icon::Color,
            Self::Icons => Icon::Glyphs,
            Self::Wrap => Icon::Wrap,
            Self::SpecialInbox => Icon::Inbox,
            Self::NavigatorView(view) => widgets::view_icon(view),
            Self::TodayStarred => Icon::Star,
            Self::SpecialToday => widgets::view_icon(SpecialView::Today),
            Self::AutosaveDelay => Icon::Autosave,
            Self::Backups | Self::BackupsKept => Icon::Backup,
            Self::BackupFolder => Icon::Folder,
        }
    }
}

/// The icon of the setting at `at` in the settings the window lists.
pub(super) fn icon(at: usize) -> Icon {
    LISTED.get(at).map_or(Icon::Command, |listed| listed.icon())
}

/// The name and description of the setting that says whether the Navigator lists `view` (D-77,
/// D-80).
fn navigator_setting(view: SpecialView) -> (&'static str, &'static str) {
    match view {
        SpecialView::Today => (
            "Show Today in Navigator",
            "List the Today view in the Navigator",
        ),
        SpecialView::Upcoming => (
            "Show Upcoming in Navigator",
            "List the Upcoming view in the Navigator",
        ),
        SpecialView::Overdue => (
            "Show Overdue in Navigator",
            "List the Overdue view in the Navigator",
        ),
        SpecialView::Colors => (
            "Show Colors in Navigator",
            "List the Colors view in the Navigator",
        ),
        SpecialView::Logbook => (
            "Show Logbook in Navigator",
            "List the Logbook view in the Navigator",
        ),
        SpecialView::Starred => (
            "Show Starred in Navigator",
            "List the Starred view in the Navigator",
        ),
        SpecialView::Flagged => (
            "Show Flagged in Navigator",
            "List the Flagged view in the Navigator",
        ),
    }
}

/// A count of milliseconds or copies as the window shows it, at most `u32::MAX`.
fn count(value: impl TryInto<u32>) -> u32 {
    value.try_into().unwrap_or(u32::MAX)
}

/// A setting whose `choices` are named as the settings file names them.
fn choices<T>(choices: &[(T, &str)]) -> SettingKind {
    SettingKind::Choice(choices.iter().map(|(_, name)| (*name).to_owned()).collect())
}

/// `value` among `choices`.
fn chosen<T: Copy + PartialEq>(choices: &[(T, &str)], value: T) -> SettingValue {
    SettingValue::Choice(
        choices
            .iter()
            .position(|&(choice, _)| choice == value)
            .unwrap_or_default(),
    )
}

/// A setting's group, name, description, kind, value and default, as [`Setting`] has them.
type Listing = (
    &'static str,
    &'static str,
    &'static str,
    SettingKind,
    SettingValue,
    SettingValue,
);

/// A switch in `group` named `name` that does what `description` says, `on` now and on by
/// default.
fn switch_on_by_default(
    group: &'static str,
    name: &'static str,
    description: &'static str,
    on: bool,
) -> Listing {
    (
        group,
        name,
        description,
        SettingKind::Switch,
        SettingValue::Switch(on),
        SettingValue::Switch(true),
    )
}

impl Tui {
    /// The settings the window lists, with the values the frontend runs with.
    pub(super) fn listed_settings(&self) -> Vec<Setting> {
        LISTED.iter().map(|&listed| self.setting(listed)).collect()
    }

    /// The setting `listed` as the window lists it.
    fn setting(&self, listed: Listed) -> Setting {
        let (group, name, description, kind, value, default): Listing = match listed {
            Listed::Theme => self.theme_listing(),
            Listed::Icons => (
                "Appearance",
                "Icons",
                "Nerd Font icons; choose none if they show as boxes",
                choices(&ICONS),
                chosen(&ICONS, self.icon_setting),
                SettingValue::Choice(0),
            ),
            Listed::Wrap => (
                "Appearance",
                "Word wrap",
                "Wrap long text, or end it in … on one line",
                SettingKind::Switch,
                SettingValue::Switch(self.wrap),
                SettingValue::Switch(false),
            ),
            Listed::SpecialInbox => switch_on_by_default(
                "Appearance",
                "Special handling of INBOX project",
                "Show INBOX: above Today, not under Projects",
                self.state.special_inbox(),
            ),
            Listed::NavigatorView(view) => {
                let (name, description) = navigator_setting(view);
                switch_on_by_default(
                    "Appearance",
                    name,
                    description,
                    self.state.navigator_lists(view),
                )
            }
            Listed::SpecialToday => switch_on_by_default(
                "Appearance",
                "Special handling of TODAY project",
                "qt adds to TODAY:, as qi adds to INBOX:",
                self.state.special_today(),
            ),
            Listed::TodayStarred => switch_on_by_default(
                "Appearance",
                "Display starred items in Today view",
                "List starred blocks too, not only dated ones",
                self.state.today_starred(),
            ),
            Listed::AutosaveDelay => (
                "Files",
                "Autosave delay",
                "How long after the last change the file is saved",
                SettingKind::Number {
                    least: count(config::MIN_AUTOSAVE_DELAY.as_millis()),
                    unit: "ms",
                },
                SettingValue::Number(count(self.state.autosave_delay().as_millis())),
                SettingValue::Number(count(saving::DELAY.as_millis())),
            ),
            Listed::Backups => switch_on_by_default(
                "Files",
                "Backups",
                "Keep a copy of each file as it was each day",
                self.backups.enabled,
            ),
            Listed::BackupFolder => (
                "Files",
                "Backup folder",
                "Where the copies go; empty for the default folder",
                SettingKind::Text { unset: "default" },
                SettingValue::Text(
                    self.backups
                        .folder
                        .as_ref()
                        .map(|folder| folder.display().to_string())
                        .unwrap_or_default(),
                ),
                SettingValue::Text(String::new()),
            ),
            Listed::BackupsKept => (
                "Files",
                "Backups kept",
                "How many daily copies of each file are kept",
                SettingKind::Number { least: 1, unit: "" },
                SettingValue::Number(count(self.backups.keep)),
                SettingValue::Number(count(effects::BACKUPS_KEPT)),
            ),
        };
        Setting {
            group,
            name,
            description,
            kind,
            value,
            default,
        }
    }

    /// The theme setting, its choices every built-in theme.
    fn theme_listing(&self) -> Listing {
        (
            "Appearance",
            "Theme",
            "The colors of the whole app",
            SettingKind::Choice(
                BuiltIn::all()
                    .map(|theme| theme.name().to_owned())
                    .collect(),
            ),
            SettingValue::Choice(self.theme.position()),
            SettingValue::Choice(BuiltIn::DEFAULT.position()),
        )
    }

    /// Applies `value` to the setting at `at` in the settings the window lists and keeps it in
    /// the settings file at time `now` (FR-SET-02, FR-SET-04).
    pub(super) fn change_setting(&mut self, at: usize, value: SettingValue, now: Duration) {
        let Some(&listed) = LISTED.get(at) else {
            return;
        };
        let kept = match (listed, value) {
            (Listed::Theme, SettingValue::Choice(choice)) => {
                let Some(theme) = BuiltIn::nth(choice) else {
                    return;
                };
                self.theme = theme;
                Some(theme.id().into())
            }
            (Listed::Icons, SettingValue::Choice(choice)) => {
                let Some(&(setting, name)) = ICONS.get(choice) else {
                    return;
                };
                self.icon_setting = setting;
                self.icons = self.icons.with_setting(setting);
                Some(name.into())
            }
            (Listed::Wrap, SettingValue::Switch(on)) => {
                self.wrap = on;
                Some(on.into())
            }
            (Listed::SpecialInbox, SettingValue::Switch(on)) => {
                self.send(Message::Configure(Configuration::SpecialInbox(on)), now);
                Some(on.into())
            }
            (Listed::NavigatorView(view), SettingValue::Switch(on)) => {
                self.send(
                    Message::Configure(Configuration::NavigatorView(view, on)),
                    now,
                );
                Some(on.into())
            }
            (Listed::SpecialToday, SettingValue::Switch(on)) => {
                self.send(Message::Configure(Configuration::SpecialToday(on)), now);
                Some(on.into())
            }
            (Listed::TodayStarred, SettingValue::Switch(on)) => {
                self.send(Message::Configure(Configuration::TodayStarred(on)), now);
                Some(on.into())
            }
            (Listed::AutosaveDelay, SettingValue::Number(ms)) => {
                let delay = Duration::from_millis(ms.into());
                self.send(Message::Configure(Configuration::AutosaveDelay(delay)), now);
                Some(i64::from(ms).into())
            }
            (Listed::Backups, SettingValue::Switch(on)) => {
                self.backups.enabled = on;
                self.io.set_backups(self.backups.clone());
                Some(on.into())
            }
            (Listed::BackupFolder, SettingValue::Text(text)) => {
                self.backups.folder =
                    (!text.is_empty()).then(|| effects::expand_path(&text, self.home.as_deref()));
                self.io.set_backups(self.backups.clone());
                (!text.is_empty()).then(|| text.into())
            }
            (Listed::BackupsKept, SettingValue::Number(keep)) => {
                self.backups.keep = usize::try_from(keep).unwrap_or(usize::MAX);
                self.io.set_backups(self.backups.clone());
                Some(i64::from(keep).into())
            }
            // The window gives each setting a value of the kind listed for it.
            _ => return,
        };
        self.keep_setting(listed.key(), kept, now);
    }

    /// The theme the settings window's value picker highlights, which the frontend previews
    /// (ui.md §3).
    pub(super) fn previewed_theme(&self) -> Option<BuiltIn> {
        match self.state.settings_preview()? {
            (at, SettingValue::Choice(choice)) if LISTED.get(at) == Some(&Listed::Theme) => {
                BuiltIn::nth(choice)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{io, sync::mpsc};

    use jiff::{Zoned, civil::DateTime};

    use super::*;
    use crate::{
        app::{MenuInput, SettingsInput, State},
        document::format,
        ui::effects::{Backups, Browser, Clipboard, FileWatch, Io, Views},
    };

    fn tui() -> Tui {
        let (results, _) = mpsc::channel();
        let io = Io::start(
            results,
            Backups::new(None, DateTime::default),
            Views::new(None),
        );
        Tui::new(
            State::new(format::parse("- a\n"), None),
            io,
            FileWatch::off(),
            Browser::new("tpt-test-no-such-opener"),
            Clipboard::new(None, io::sink()),
            Zoned::now,
            None,
        )
    }

    // ui.md §8
    #[test]
    fn a_value_of_another_kind_or_beyond_the_choices_changes_nothing() {
        let mut tui = tui();
        let before = tui.listed_settings();
        for (at, value) in [
            (0, SettingValue::Switch(false)),
            (0, SettingValue::Choice(99)),
            (1, SettingValue::Choice(9)),
            (LISTED.len(), SettingValue::Switch(false)),
        ] {
            tui.change_setting(at, value, Duration::ZERO);
        }
        assert_eq!(tui.listed_settings(), before);
        assert_eq!(tui.state.notice(), None, "nothing was kept");
        assert_eq!(icon(LISTED.len()), Icon::Command);
    }

    // FR-SET-02, ui.md §8
    #[test]
    fn every_setting_takes_a_value_of_its_kind() {
        let mut tui = tui();
        let values = [
            SettingValue::Choice(1),
            SettingValue::Choice(1),
            SettingValue::Switch(false),
            SettingValue::Switch(false),
            SettingValue::Switch(false),
            SettingValue::Switch(false),
            SettingValue::Switch(false),
            SettingValue::Switch(false),
            SettingValue::Switch(false),
            SettingValue::Switch(false),
            SettingValue::Switch(false),
            SettingValue::Switch(false),
            SettingValue::Switch(false),
            SettingValue::Number(300),
            SettingValue::Switch(false),
            SettingValue::Text("/copies".into()),
            SettingValue::Number(5),
        ];
        for (at, value) in values.iter().enumerate() {
            tui.change_setting(at, value.clone(), Duration::ZERO);
        }
        let taken: Vec<_> = tui.listed_settings().into_iter().map(|s| s.value).collect();
        assert_eq!(taken, values);
        tui.change_setting(15, SettingValue::Text(String::new()), Duration::ZERO);
        assert_eq!(
            tui.listed_settings()[15].value,
            SettingValue::Text(String::new())
        );
        assert!(
            tui.state.notice().is_some(),
            "with no settings file, nothing is kept"
        );
    }

    // FR-UI-06, ui.md §3, §8
    #[test]
    fn only_the_theme_picker_previews_a_theme() {
        let mut tui = tui();
        let settings = tui.listed_settings();
        for input in [
            SettingsInput::Open(settings),
            SettingsInput::Next,
            SettingsInput::Change,
            SettingsInput::Pick(MenuInput::Next),
        ] {
            tui.state.update(Message::Settings(input));
        }
        assert!(
            tui.state.settings_preview().is_some(),
            "the icons picker is open"
        );
        assert_eq!(tui.previewed_theme(), None);
    }

    // FR-UI-06, ui.md §3, §8
    #[test]
    fn the_theme_picker_previews_the_highlighted_theme() {
        let mut tui = tui();
        let settings = tui.listed_settings();
        for input in [
            SettingsInput::Open(settings),
            SettingsInput::Change,
            SettingsInput::Pick(MenuInput::Next),
        ] {
            tui.state.update(Message::Settings(input));
        }
        assert_eq!(
            tui.previewed_theme(),
            BuiltIn::nth(BuiltIn::DEFAULT.position() + 1)
        );
    }
}
