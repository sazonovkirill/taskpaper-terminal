//! The icons the UI draws, each next to its one fallback and its ASCII form
//! ([ui.md §3, §7](../../../../docs/specs/ui.md#7-icons-fr-ui-09-fr-ui-10)). Hides which glyph
//! stands for what, so the sets cannot drift.

use std::ffi::OsStr;

/// Something the UI marks with an icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    /// A start date chip, and the command menu's start date entries.
    Start,
    /// A due date chip that is not overdue, and the command menu's due date entries.
    Due,
    /// An overdue due date chip.
    Overdue,
    /// A block with `@today`, after its text, the command menu's Toggle Today and the sidebar's
    /// Today entry.
    Today,
    /// A bookmarked block, after its text, the command menu's bookmark entries and the sidebar's
    /// bookmarks.
    Bookmark,
    /// The command menu's entry that makes a block a project, and projects in the finders.
    Project,
    /// The command menu's entry that makes a block a task, and tasks in the finders.
    Task,
    /// The command menu's entries that make a block plain text, and text in the finders.
    Note,
    /// The command menu's entry that makes a block a section, and sections in the finders.
    Section,
    /// The command menu's entry that completes or uncompletes a block.
    Complete,
    /// The command menu's color entries.
    Color,
    /// The command menu's entry that shows a block's details.
    Details,
    /// The command menu's entry that opens a block's link.
    Link,
    /// The command menu's entry that duplicates a block.
    Duplicate,
    /// The command menu's entry that deletes a block.
    Delete,
    /// A command menu entry without an icon of its own.
    Command,
    /// The document root: the first crumb and Move to's Home (D-32).
    Home,
    /// A sidebar project whose child projects are shown.
    Expanded,
    /// A sidebar project whose child projects are hidden, and the count of a collapsed block's
    /// hidden children.
    Collapsed,
    /// The marker of a task that is not done.
    TaskMarker,
    /// The marker of a done task (D-36).
    DoneTask,
    /// The status line's navigate mode.
    Navigate,
    /// The status line's edit mode.
    Edit,
    /// The status line's mark of changes not saved yet.
    Unsaved,
    /// Go to Command's Open Settings.
    Settings,
    /// The settings window's icons setting.
    Glyphs,
    /// The settings window's word wrap setting.
    Wrap,
    /// The settings window's autosave delay setting.
    Autosave,
    /// The settings window's settings of whether and how many backups are kept.
    Backup,
    /// The settings window's backup folder setting.
    Folder,
}

/// The `icons` setting (ui.md §7): Nerd Font glyphs, or none, which draws the Unicode markers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum IconSetting {
    /// `icons = "nerd"`, the default (D-20).
    #[default]
    Nerd,
    /// `icons = "none"`.
    Off,
}

/// The glyphs the UI draws for its icons.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum IconSet {
    /// Nerd Font glyphs.
    #[default]
    Nerd,
    /// The Unicode markers, for terminals without a Nerd Font.
    Unicode,
    /// ASCII, for a locale that is not UTF-8.
    Ascii,
}

impl IconSet {
    /// The set drawn once the `icons` setting is `setting`: ASCII stays, since the locale decides
    /// it whatever the setting (ui.md §7).
    #[must_use]
    pub fn with_setting(self, setting: IconSetting) -> Self {
        match (self, setting) {
            (Self::Ascii, _) => Self::Ascii,
            (_, IconSetting::Nerd) => Self::Nerd,
            (_, IconSetting::Off) => Self::Unicode,
        }
    }
}

/// Whether the locale the environment names is UTF-8: the first of `LC_ALL`, `LC_CTYPE` and
/// `LANG` that is set and not empty decides, as in POSIX; with none, the locale is `C`, which is
/// not.
#[must_use]
pub fn utf8_locale(lc_all: Option<&OsStr>, lc_ctype: Option<&OsStr>, lang: Option<&OsStr>) -> bool {
    [lc_all, lc_ctype, lang]
        .into_iter()
        .flatten()
        .find(|value| !value.is_empty())
        .is_some_and(|value| {
            let value = value.to_string_lossy().to_ascii_uppercase();
            value.contains("UTF-8") || value.contains("UTF8")
        })
}

/// An icon's Nerd Font glyph, its Unicode fallback and its ASCII form.
struct Glyphs {
    nerd: &'static str,
    unicode: &'static str,
    ascii: &'static str,
}

impl Icon {
    /// The glyph drawn for the icon in `set`.
    #[must_use]
    pub fn glyph(self, set: IconSet) -> &'static str {
        let Glyphs {
            nerd,
            unicode,
            ascii,
        } = self.glyphs();
        match set {
            IconSet::Nerd => nerd,
            IconSet::Unicode => unicode,
            IconSet::Ascii => ascii,
        }
    }

    /// The table: each Nerd Fonts v3 code point beside its fallback, every one a single cell, and
    /// its ASCII form.
    const fn glyphs(self) -> Glyphs {
        let (nerd, unicode, ascii) = match self {
            // nf-md-calendar_start
            Self::Start => ("\u{f166d}", "↦", "+"),
            // nf-md-calendar_end
            Self::Due => ("\u{f166c}", "⇥", "@"),
            // nf-md-calendar_alert
            Self::Overdue => ("\u{f0a31}", "!", "!"),
            // nf-md-star
            Self::Today => ("\u{f04ce}", "★", "*"),
            // nf-md-bookmark
            Self::Bookmark => ("\u{f00c0}", "⚑", "#"),
            // nf-md-folder_outline
            Self::Project => ("\u{f0256}", "▸", ">"),
            // nf-md-checkbox_blank_outline
            Self::Task => ("\u{f0131}", "☐", "-"),
            // nf-md-text
            Self::Note => ("\u{f09a8}", "¶", "\""),
            // nf-md-format_section
            Self::Section => ("\u{f069f}", "§", "="),
            // nf-md-check_circle_outline
            Self::Complete => ("\u{f05e1}", "✓", "x"),
            // nf-md-palette
            Self::Color => ("\u{f03d8}", "●", "o"),
            // nf-md-information_outline
            Self::Details => ("\u{f02fd}", "i", "i"),
            // nf-md-link
            Self::Link => ("\u{f0337}", "↗", "^"),
            // nf-md-content_copy
            Self::Duplicate => ("\u{f018f}", "⧉", "&"),
            // nf-md-delete_outline
            Self::Delete => ("\u{f09e7}", "✕", "X"),
            // nf-md-chevron_right
            Self::Command => ("\u{f0142}", "›", ">"),
            // nf-md-home
            Self::Home => ("\u{f02dc}", "⌂", "~"),
            // nf-md-menu_down
            Self::Expanded => ("\u{f035d}", "▾", "v"),
            // nf-md-menu_right
            Self::Collapsed => ("\u{f035f}", "▸", ">"),
            // nf-md-checkbox_blank_circle_outline
            Self::TaskMarker => ("\u{f0130}", "-", "-"),
            // nf-md-checkbox_marked_circle_outline
            Self::DoneTask => ("\u{f0134}", "✔", "[x]"),
            // nf-md-cursor_default_outline
            Self::Navigate => ("\u{f01bf}", "↕", ">"),
            // nf-md-pencil
            Self::Edit => ("\u{f03eb}", "✎", "/"),
            // nf-md-circle_medium
            Self::Unsaved => ("\u{f09de}", "●", "*"),
            // nf-md-cog_outline
            Self::Settings => ("\u{f08bb}", "⚙", "%"),
            // nf-md-shape_outline
            Self::Glyphs => ("\u{f0832}", "◇", "o"),
            // nf-md-wrap
            Self::Wrap => ("\u{f05b6}", "↩", "/"),
            // nf-md-timer_outline
            Self::Autosave => ("\u{f051b}", "◷", "t"),
            // nf-md-backup_restore
            Self::Backup => ("\u{f006f}", "↺", "b"),
            // nf-md-folder_cog_outline
            Self::Folder => ("\u{f1080}", "▭", "d"),
        };
        Glyphs {
            nerd,
            unicode,
            ascii,
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui::text::Span;

    use super::*;

    const ICONS: [Icon; 30] = [
        Icon::Start,
        Icon::Due,
        Icon::Overdue,
        Icon::Today,
        Icon::Bookmark,
        Icon::Project,
        Icon::Task,
        Icon::Note,
        Icon::Section,
        Icon::Complete,
        Icon::Color,
        Icon::Details,
        Icon::Link,
        Icon::Duplicate,
        Icon::Delete,
        Icon::Command,
        Icon::Home,
        Icon::Expanded,
        Icon::Collapsed,
        Icon::TaskMarker,
        Icon::DoneTask,
        Icon::Navigate,
        Icon::Edit,
        Icon::Unsaved,
        Icon::Settings,
        Icon::Glyphs,
        Icon::Wrap,
        Icon::Autosave,
        Icon::Backup,
        Icon::Folder,
    ];

    // FR-UI-10, ui.md §7
    #[test]
    fn every_icon_and_its_fallback_is_one_cell_wide() {
        for icon in ICONS {
            for set in [IconSet::Nerd, IconSet::Unicode] {
                assert_eq!(Span::raw(icon.glyph(set)).width(), 1, "{icon:?} {set:?}");
            }
        }
    }

    // ui.md §7
    #[test]
    fn nerd_glyphs_come_from_the_nerd_fonts_v3_material_design_range() {
        for icon in ICONS {
            let code = icon.glyph(IconSet::Nerd).chars().next().map(u32::from);
            assert!(
                code.is_some_and(|code| (0xF0001..=0xF1AF0).contains(&code)),
                "{icon:?}"
            );
        }
    }

    // A project and a collapsed block share `▸` (ui.md §2, §7), so only the Nerd glyphs are
    // all different.
    #[test]
    fn every_icon_has_its_own_nerd_glyph() {
        for (at, icon) in ICONS.iter().enumerate() {
            for other in &ICONS[at + 1..] {
                assert_ne!(icon.glyph(IconSet::Nerd), other.glyph(IconSet::Nerd));
            }
        }
    }

    // ui.md §3
    #[test]
    fn the_ascii_forms_are_ascii_and_the_markers_those_of_the_spec() {
        for icon in ICONS {
            let ascii = icon.glyph(IconSet::Ascii);
            assert!(!ascii.is_empty() && ascii.is_ascii(), "{icon:?}");
        }
        let ascii = |icon: Icon| icon.glyph(IconSet::Ascii);
        assert_eq!(ascii(Icon::Expanded), "v");
        assert_eq!(ascii(Icon::Collapsed), ">");
        assert_eq!(ascii(Icon::DoneTask), "[x]");
        assert_eq!(ascii(Icon::TaskMarker), "-");
        assert_eq!(ascii(Icon::Home), "~");
    }

    // ui.md §2, §7
    #[test]
    fn the_fallbacks_are_the_unicode_markers() {
        let unicode = |icon: Icon| icon.glyph(IconSet::Unicode);
        assert_eq!(unicode(Icon::Home), "⌂");
        assert_eq!(unicode(Icon::Expanded), "▾");
        assert_eq!(unicode(Icon::Collapsed), "▸");
        assert_eq!(unicode(Icon::TaskMarker), "-");
        assert_eq!(unicode(Icon::DoneTask), "✔");
        assert_eq!(unicode(Icon::Today), "★");
        assert_eq!(unicode(Icon::Bookmark), "⚑");
    }

    // FR-UI-10
    #[test]
    fn the_setting_picks_the_set_but_ascii_stays() {
        for set in [IconSet::Nerd, IconSet::Unicode] {
            assert_eq!(set.with_setting(IconSetting::Nerd), IconSet::Nerd);
            assert_eq!(set.with_setting(IconSetting::Off), IconSet::Unicode);
        }
        for setting in [IconSetting::Nerd, IconSetting::Off] {
            assert_eq!(IconSet::Ascii.with_setting(setting), IconSet::Ascii);
        }
        assert_eq!(
            IconSet::default(),
            IconSet::Unicode.with_setting(IconSetting::default())
        );
    }

    fn utf8([lc_all, lc_ctype, lang]: [Option<&str>; 3]) -> bool {
        utf8_locale(
            lc_all.map(OsStr::new),
            lc_ctype.map(OsStr::new),
            lang.map(OsStr::new),
        )
    }

    // FR-UI-10, ui.md §3
    #[test]
    fn the_first_locale_variable_set_decides_whether_it_is_utf_8() {
        assert!(utf8([None, None, Some("en_US.UTF-8")]));
        assert!(utf8([None, None, Some("de_DE.utf8")]));
        assert!(utf8([None, Some("C.UTF-8"), Some("C")]));
        assert!(
            utf8([Some(""), Some(""), Some("en_GB.utf-8")]),
            "empty ones are skipped"
        );
        assert!(!utf8([Some("C"), None, Some("en_US.UTF-8")]), "LC_ALL wins");
        assert!(
            !utf8([None, Some("POSIX"), Some("en_US.UTF-8")]),
            "LC_CTYPE before LANG"
        );
        assert!(!utf8([None, None, Some("en_US.ISO-8859-1")]));
        assert!(!utf8([None, None, None]), "none set is the C locale");
    }
}
