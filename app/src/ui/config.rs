//! The settings file `config.toml`: where it is and the settings it sets, each left at its
//! default where the file does not set it or sets it in a way the app cannot use, which is
//! reported rather than refused, and writing a setting changed in the app back to it (FR-SET-01,
//! FR-SET-04, [keybindings.md §5](../../../docs/specs/keybindings.md#5-configuration)).

use std::{
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
    time::Duration,
};

use thiserror::Error;
use toml_edit::{Document, DocumentMut, Item, Table, TableLike, Value};

use super::{
    effects::{self, BackupSettings, atomic},
    keymap::{BindingError, Keymap},
    theme::{BuiltIn, IconSetting},
    widgets::ColorTagDisplay,
};
use crate::app::Mode;

/// The settings file: `$XDG_CONFIG_HOME/taskpaper-terminal/config.toml`, or under `~/.config`
/// when that is unset or not absolute; also on macOS (FR-SET-01). `None` when neither is known.
#[must_use]
pub fn file(xdg_config_home: Option<OsString>, home: Option<PathBuf>) -> Option<PathBuf> {
    effects::app_folder(xdg_config_home, home, ".config", "config.toml")
}

/// Something in the settings file that the app ignored (FR-SET-04).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Problem {
    /// The file exists but cannot be read as text, so every setting keeps its default.
    #[error("cannot read it: {reason}")]
    Unreadable {
        /// What the system reported.
        reason: String,
    },
    /// The file is not TOML, so every setting keeps its default.
    #[error("line {line}: {message}")]
    Syntax {
        /// The line, counted from 1.
        line: usize,
        /// What the parser reported.
        message: String,
    },
    /// The file sets something the app has no setting for.
    #[error("unknown setting `{key}`")]
    UnknownSetting {
        /// The setting's dotted key.
        key: String,
    },
    /// A setting's value is not one it can take.
    #[error("`{key}` must be {expected}")]
    InvalidValue {
        /// The setting's dotted key.
        key: String,
        /// What it can take.
        expected: &'static str,
    },
    /// A key binding of `[keymap.<context>]` cannot be applied.
    #[error("[keymap.{context}] {error}")]
    Binding {
        /// The table's context: `navigate` or `edit`.
        context: String,
        /// Why.
        error: BindingError,
    },
}

/// The least `autosave_delay_ms` may be (file-io.md §2).
pub(super) const MIN_AUTOSAVE_DELAY: Duration = Duration::from_millis(100);

/// The settings the frontend runs with, and what it ignored of the file.
#[derive(Debug, Default)]
pub struct Config {
    /// The key bindings: the defaults with the file's `[keymap.*]` tables applied.
    pub(super) keymap: Keymap,
    /// How long after the last change a save starts, when the file says (`autosave_delay_ms`).
    pub(super) autosave_delay: Option<Duration>,
    /// Which icons to draw, when the file says (`icons`, ui.md §7).
    pub(super) icons: Option<IconSetting>,
    /// The theme, when the file names one (`theme`, FR-UI-06).
    pub(super) theme: Option<BuiltIn>,
    /// How color tags show, when the file says (`color_tags`, ui.md §2).
    pub(super) color_tags: Option<ColorTagDisplay>,
    /// Whether text wraps, when the file says (`word_wrap`, FR-UI-05).
    pub(super) wrap: Option<bool>,
    /// Whether the sidebar lists the `INBOX` project apart, when the file says
    /// (`special_inbox`, D-75).
    pub(super) special_inbox: Option<bool>,
    /// Whether the sidebar lists Today, when the file says (`sidebar_today`, D-77).
    pub(super) sidebar_today: Option<bool>,
    backups: BackupSettings,
    problems: Vec<Problem>,
    /// The file the settings come from, where settings changed in the app are written.
    file: Option<PathBuf>,
}

impl Config {
    /// What the app ignored of the file, setting by setting.
    #[must_use]
    pub fn problems(&self) -> &[Problem] {
        &self.problems
    }

    /// The file the settings come from, if its place is known.
    #[must_use]
    pub fn file(&self) -> Option<&Path> {
        self.file.as_deref()
    }

    /// The backups the file asks for (`[backups]`).
    #[must_use]
    pub fn backups(&self) -> &BackupSettings {
        &self.backups
    }

    /// Takes `autosave_delay_ms` from `item`.
    fn read_autosave_delay(&mut self, item: &Item) {
        match item.as_integer().and_then(|ms| u64::try_from(ms).ok()) {
            Some(ms) if Duration::from_millis(ms) >= MIN_AUTOSAVE_DELAY => {
                self.autosave_delay = Some(Duration::from_millis(ms));
            }
            _ => self.problems.push(Problem::InvalidValue {
                key: "autosave_delay_ms".to_owned(),
                expected: "a whole number of milliseconds, at least 100",
            }),
        }
    }

    /// Takes `icons` from `item`: `"nerd"` or `"none"`.
    fn read_icons(&mut self, item: &Item) {
        match item.as_str() {
            Some("nerd") => self.icons = Some(IconSetting::Nerd),
            Some("none") => self.icons = Some(IconSetting::Off),
            _ => self.problems.push(Problem::InvalidValue {
                key: "icons".to_owned(),
                expected: "\"nerd\" or \"none\"",
            }),
        }
    }

    /// Takes `color_tags` from `item`: `"icon"` or `"line highlight"`.
    fn read_color_tags(&mut self, item: &Item) {
        match item.as_str() {
            Some("icon") => self.color_tags = Some(ColorTagDisplay::Icon),
            Some("line highlight") => self.color_tags = Some(ColorTagDisplay::Highlight),
            _ => self.problems.push(Problem::InvalidValue {
                key: "color_tags".to_owned(),
                expected: "\"icon\" or \"line highlight\"",
            }),
        }
    }

    /// Takes `word_wrap` from `item`: `true` or `false`.
    fn read_wrap(&mut self, item: &Item) {
        match item.as_bool() {
            Some(wrap) => self.wrap = Some(wrap),
            None => self.problems.push(Problem::InvalidValue {
                key: "word_wrap".to_owned(),
                expected: "true or false",
            }),
        }
    }

    /// Takes `special_inbox` from `item`: `true` or `false`.
    fn read_special_inbox(&mut self, item: &Item) {
        match item.as_bool() {
            Some(on) => self.special_inbox = Some(on),
            None => self.problems.push(Problem::InvalidValue {
                key: "special_inbox".to_owned(),
                expected: "true or false",
            }),
        }
    }

    /// Takes `sidebar_today` from `item`: `true` or `false`.
    fn read_sidebar_today(&mut self, item: &Item) {
        match item.as_bool() {
            Some(on) => self.sidebar_today = Some(on),
            None => self.problems.push(Problem::InvalidValue {
                key: "sidebar_today".to_owned(),
                expected: "true or false",
            }),
        }
    }

    /// Takes `theme` from `item`: the name of a built-in theme.
    fn read_theme(&mut self, item: &Item) {
        match item.as_str().and_then(BuiltIn::named) {
            Some(theme) => self.theme = Some(theme),
            None => self.problems.push(Problem::InvalidValue {
                key: "theme".to_owned(),
                expected: "a built-in theme's name, such as \"nord\"",
            }),
        }
    }

    /// Takes the settings of the `[backups]` table `item`, a folder starting with `~` under
    /// `home`.
    fn read_backups(&mut self, item: &Item, home: Option<&Path>) {
        let Some(settings) = table(item, "backups", &mut self.problems) else {
            return;
        };
        for (key, item) in settings.iter() {
            let invalid = |expected| Problem::InvalidValue {
                key: format!("backups.{key}"),
                expected,
            };
            match key {
                "enabled" => match item.as_bool() {
                    Some(enabled) => self.backups.enabled = enabled,
                    None => self.problems.push(invalid("true or false")),
                },
                "dir" => match item.as_str() {
                    Some(dir) if !dir.is_empty() => {
                        self.backups.folder = Some(effects::expand_path(dir, home));
                    }
                    _ => self.problems.push(invalid("a folder")),
                },
                "keep" => match item
                    .as_integer()
                    .and_then(|keep| usize::try_from(keep).ok())
                {
                    Some(keep) if keep >= 1 => self.backups.keep = keep,
                    _ => self.problems.push(invalid("a whole number, at least 1")),
                },
                _ => self.problems.push(Problem::UnknownSetting {
                    key: format!("backups.{key}"),
                }),
            }
        }
    }

    /// The defaults with only `problem`.
    fn failed(problem: Problem) -> Self {
        Self {
            problems: vec![problem],
            ..Self::default()
        }
    }

    /// Takes the settings of the `[keymap]` table `item`.
    fn read_keymap(&mut self, item: &Item) {
        let Some(contexts) = table(item, "keymap", &mut self.problems) else {
            return;
        };
        for (context, item) in contexts.iter() {
            let mode = match context {
                "navigate" => Mode::Navigate,
                "edit" => Mode::Edit,
                _ => {
                    self.problems.push(Problem::UnknownSetting {
                        key: format!("keymap.{context}"),
                    });
                    continue;
                }
            };
            let key = format!("keymap.{context}");
            let Some(bindings) = table(item, &key, &mut self.problems) else {
                continue;
            };
            let mut named = Vec::new();
            for (keys, value) in bindings.iter() {
                match value.as_str() {
                    Some(command) => named.push((keys, command)),
                    None => self.problems.push(Problem::InvalidValue {
                        key: format!("{key}.\"{keys}\""),
                        expected: "a command name or \"none\"",
                    }),
                }
            }
            let errors = self.keymap.bind(mode, &named);
            self.problems
                .extend(errors.into_iter().map(|error| Problem::Binding {
                    context: context.to_owned(),
                    error,
                }));
        }
    }
}

/// `item` as a table, or `None` with a problem saying the setting `key` must be one.
fn table<'a>(item: &'a Item, key: &str, problems: &mut Vec<Problem>) -> Option<&'a dyn TableLike> {
    let table = item.as_table_like();
    if table.is_none() {
        problems.push(Problem::InvalidValue {
            key: key.to_owned(),
            expected: "a table",
        });
    }
    table
}

/// The settings of the file at `path`: the defaults where it does not exist, and otherwise
/// what it sets that the app can use, with the problems of the rest. A folder it names starting
/// with `~` is under `home`.
#[must_use]
pub fn load(path: &Path, home: Option<&Path>) -> Config {
    let config = match fs::read_to_string(path) {
        Ok(text) => read(&text, home),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Config::default(),
        Err(error) => Config::failed(Problem::Unreadable {
            reason: error.to_string(),
        }),
    };
    Config {
        file: Some(path.to_owned()),
        ..config
    }
}

/// The settings `text`, a settings file, sets, with the problems of what it sets wrongly; a
/// folder starting with `~` is under `home`.
fn read(text: &str, home: Option<&Path>) -> Config {
    let document = match Document::parse(text) {
        Ok(document) => document,
        Err(error) => {
            let at = error.span().map_or(0, |span| span.start);
            let line = text.get(..at).unwrap_or(text).matches('\n').count() + 1;
            return Config::failed(Problem::Syntax {
                line,
                message: error.message().to_owned(),
            });
        }
    };
    let mut config = Config::default();
    for (key, item) in document.as_table() {
        match key {
            "keymap" => config.read_keymap(item),
            "autosave_delay_ms" => config.read_autosave_delay(item),
            "backups" => config.read_backups(item, home),
            "color_tags" => config.read_color_tags(item),
            "icons" => config.read_icons(item),
            "theme" => config.read_theme(item),
            "word_wrap" => config.read_wrap(item),
            "special_inbox" => config.read_special_inbox(item),
            "sidebar_today" => config.read_sidebar_today(item),
            _ => config.problems.push(Problem::UnknownSetting {
                key: key.to_owned(),
            }),
        }
    }
    config
}

/// Why a setting changed in the app could not be written to the settings file (FR-SET-04).
#[derive(Debug, Error)]
pub enum WriteError {
    /// Neither `$XDG_CONFIG_HOME` nor a home folder says where the file goes.
    #[error("no place for it: neither $XDG_CONFIG_HOME nor a home folder is set")]
    NoFile,
    /// The file is not TOML, so writing it would lose what it holds.
    #[error("not valid TOML: {message}")]
    NotToml {
        /// What the parser reported.
        message: String,
    },
    /// The file has something other than a table where the setting's key needs one, or a table
    /// where it needs a value.
    #[error("`{key}` is in the way")]
    InTheWay {
        /// The dotted key of what is in the way.
        key: String,
    },
    /// Reading or writing the file failed.
    #[error(transparent)]
    Io(#[from] io::Error),
}

impl WriteError {
    /// The settings file's problem that the setting `key` was not written for this reason, as
    /// the status line lists it after `config.toml:` (FR-SET-04).
    #[must_use]
    pub fn problem(&self, key: &str) -> String {
        format!("`{key}` not saved: {self}")
    }
}

/// Sets the setting `key`, dotted for a key in a table such as `backups.keep`, to `value` in the
/// settings file at `path`, keeping its comments, order and other settings, and creating the
/// file and its folder when they do not exist (FR-SET-01, FR-SET-04). A value already there
/// keeps its comment. The file is replaced at once, never seen half written.
///
/// # Errors
///
/// [`WriteError`] when the file is not TOML, holds something else where `key` goes, or cannot
/// be read or written; it is then left as it was.
pub fn set(path: &Path, key: &str, value: impl Into<Value>) -> Result<(), WriteError> {
    let mut document = parse(path)?;
    let in_the_way = |end: usize| WriteError::InTheWay {
        key: key.get(..end).unwrap_or(key).to_owned(),
    };
    let (tables, name) = key.rsplit_once('.').unwrap_or(("", key));
    let mut table: &mut dyn TableLike = document.as_table_mut();
    let mut end = 0;
    for part in tables.split('.').filter(|part| !part.is_empty()) {
        end += part.len() + usize::from(end > 0);
        table = table
            .entry(part)
            .or_insert_with(|| Item::Table(Table::new()))
            .as_table_like_mut()
            .ok_or_else(|| in_the_way(end))?;
    }
    let mut value = value.into();
    match table.get_mut(name) {
        Some(Item::Value(old)) => {
            *value.decor_mut() = old.decor().clone();
            *old = value;
        }
        Some(Item::None) | None => {
            table.insert(name, Item::Value(value));
        }
        Some(_) => return Err(in_the_way(key.len())),
    }
    path.parent().map(fs::create_dir_all).transpose()?;
    write(path, &document)
}

/// Removes the setting `key`, dotted as for [`set`], from the settings file at `path`, so that
/// it takes its default again, keeping everything else; a file that does not set it is left as
/// it is (FR-SET-04).
///
/// # Errors
///
/// [`WriteError`] when the file is not TOML or cannot be read or written; it is then left as it
/// was.
pub fn remove(path: &Path, key: &str) -> Result<(), WriteError> {
    let mut document = parse(path)?;
    let (tables, name) = key.rsplit_once('.').unwrap_or(("", key));
    let mut table: Option<&mut dyn TableLike> = Some(document.as_table_mut());
    for part in tables.split('.').filter(|part| !part.is_empty()) {
        table = table
            .and_then(|table| table.get_mut(part))
            .and_then(Item::as_table_like_mut);
    }
    match table.and_then(|table| table.remove(name)) {
        Some(_) => write(path, &document),
        None => Ok(()),
    }
}

/// The settings file at `path` to edit, empty when it does not exist.
fn parse(path: &Path) -> Result<DocumentMut, WriteError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error.into()),
    };
    text.parse()
        .map_err(|error: toml_edit::TomlError| WriteError::NotToml {
            message: error.message().to_owned(),
        })
}

/// Replaces the settings file at `path` with `document` at once, never seen half written.
fn write(path: &Path, document: &DocumentMut) -> Result<(), WriteError> {
    atomic::prepare(path, document.to_string().as_bytes())?.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::commands::Command;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    // FR-SET-01
    #[test]
    fn the_file_is_under_the_xdg_config_home_or_dot_config() {
        let home = || Some(PathBuf::from("/home/ann"));
        assert_eq!(
            file(Some("/xdg".into()), home()),
            Some(PathBuf::from("/xdg/taskpaper-terminal/config.toml"))
        );
        assert_eq!(
            file(None, home()),
            Some(PathBuf::from(
                "/home/ann/.config/taskpaper-terminal/config.toml"
            ))
        );
        assert_eq!(
            file(Some("relative".into()), home()),
            Some(PathBuf::from(
                "/home/ann/.config/taskpaper-terminal/config.toml"
            )),
            "a relative $XDG_CONFIG_HOME is ignored"
        );
        assert_eq!(file(None, None), None);
    }

    // FR-SET-01
    #[test]
    fn a_missing_file_gives_the_defaults_without_problems() -> TestResult {
        let dir = tempfile::tempdir()?;
        let config = load(&dir.path().join("config.toml"), None);
        assert_eq!(config.problems(), []);
        assert_eq!(
            config.keymap.labels(Command::Quit, Mode::Navigate),
            "Ctrl+Q"
        );
        Ok(())
    }

    // FR-SET-01
    #[test]
    fn a_file_is_read_for_its_settings() -> TestResult {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("config.toml");
        fs::write(&path, "[keymap.navigate]\nx = \"none\"\n")?;
        let config = load(&path, None);
        assert_eq!(config.problems(), []);
        assert_eq!(
            config.keymap.labels(Command::ToggleDone, Mode::Navigate),
            "Ctrl+Enter"
        );
        Ok(())
    }

    // FR-SET-04
    #[test]
    fn a_file_that_cannot_be_read_gives_the_defaults_and_says_why() -> TestResult {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("config.toml");
        fs::write(&path, [0xff, 0xfe])?;
        assert!(matches!(
            load(&path, None).problems(),
            [Problem::Unreadable { .. }]
        ));
        assert!(
            matches!(
                load(dir.path(), None).problems(),
                [Problem::Unreadable { .. }]
            ),
            "a folder"
        );
        Ok(())
    }

    // FR-SET-04
    #[test]
    fn a_file_that_is_not_toml_gives_the_defaults_and_names_the_line() {
        let config = read("[keymap.navigate]\nx = \"none\"\ny = \n", None);
        assert!(matches!(
            config.problems(),
            [Problem::Syntax { line: 3, .. }]
        ));
        assert_eq!(
            config.keymap.labels(Command::ToggleDone, Mode::Navigate),
            "x, Ctrl+Enter",
            "nothing of the file applies"
        );
    }

    // FR-SET-04
    #[test]
    fn unknown_settings_and_values_of_the_wrong_kind_are_reported_and_the_rest_applies() {
        let config = read(
            "font = \"mono\"\n\
             [keymap.navigate]\n\
             x = 3\n\
             \"ctrl+d\" = \"toggle_done\"\n\
             [keymap.sidebar]\n\
             j = \"none\"\n",
            None,
        );
        assert_eq!(
            config.problems(),
            [
                Problem::UnknownSetting { key: "font".into() },
                Problem::InvalidValue {
                    key: "keymap.navigate.\"x\"".into(),
                    expected: "a command name or \"none\"",
                },
                Problem::UnknownSetting {
                    key: "keymap.sidebar".into()
                },
            ]
        );
        assert_eq!(
            config.keymap.labels(Command::ToggleDone, Mode::Navigate),
            "Ctrl+D, x, Ctrl+Enter"
        );
        let config = read("keymap = 1\n", None);
        assert_eq!(
            config.problems(),
            [Problem::InvalidValue {
                key: "keymap".into(),
                expected: "a table"
            }]
        );
        let config = read("keymap = { edit = \"none\" }\n", None);
        assert_eq!(
            config.problems(),
            [Problem::InvalidValue {
                key: "keymap.edit".into(),
                expected: "a table"
            }]
        );
    }

    // keybindings.md §5
    #[test]
    fn the_keymap_tables_rebind_navigate_and_edit_mode_and_report_what_they_cannot() {
        let config = read(
            "[keymap.navigate]\n\
             \"alt+shift+up\" = \"move_up\"\n\
             x = \"none\"\n\
             \"g g g\" = \"Go Home\"\n\
             \"hyper+x\" = \"move_up\"\n\
             [keymap.edit]\n\
             \"ctrl+g\" = \"inline_code\"\n\
             \"ctrl+e\" = \"zoom_in\"\n",
            None,
        );
        assert_eq!(
            config.problems(),
            [
                Problem::Binding {
                    context: "navigate".into(),
                    error: BindingError::NotAKey {
                        keys: "hyper+x".into()
                    },
                },
                Problem::Binding {
                    context: "edit".into(),
                    error: BindingError::UnknownCommand {
                        keys: "ctrl+e".into(),
                        command: "zoom_in".into(),
                    },
                },
            ]
        );
        let keymap = &config.keymap;
        assert_eq!(
            keymap.labels(Command::MoveUp, Mode::Navigate),
            "Alt+Shift+↑, Alt+k, Alt+↑"
        );
        assert_eq!(
            keymap.labels(Command::ToggleDone, Mode::Navigate),
            "Ctrl+Enter"
        );
        assert_eq!(
            keymap.labels(Command::GoHome, Mode::Navigate),
            "ggg, Ctrl+', gh"
        );
        assert_eq!(
            keymap.labels(
                Command::ToggleMarker(crate::document::inline::Marker::Code),
                Mode::Edit
            ),
            "Ctrl+G, Alt+`, Ctrl+E"
        );
    }

    // FR-UI-10, ui.md §7
    #[test]
    fn icons_are_nerd_or_none_and_anything_else_is_reported() {
        assert_eq!(read("", None).icons, None);
        assert_eq!(
            read("icons = \"nerd\"\n", None).icons,
            Some(IconSetting::Nerd)
        );
        let off = read("icons = \"none\"\n", None);
        assert_eq!(
            (off.icons, off.problems()),
            (Some(IconSetting::Off), &[][..])
        );
        for text in ["icons = \"off\"\n", "icons = false\n"] {
            let config = read(text, None);
            assert_eq!(config.icons, None, "{text}");
            assert_eq!(
                config.problems(),
                [Problem::InvalidValue {
                    key: "icons".to_owned(),
                    expected: "\"nerd\" or \"none\"",
                }],
                "{text}"
            );
        }
    }

    // D-74, ui.md §2
    #[test]
    fn color_tags_are_icon_or_line_highlight_and_anything_else_is_reported() {
        assert_eq!(read("", None).color_tags, None);
        assert_eq!(
            read("color_tags = \"icon\"\n", None).color_tags,
            Some(ColorTagDisplay::Icon)
        );
        let highlight = read("color_tags = \"line highlight\"\n", None);
        assert_eq!(
            (highlight.color_tags, highlight.problems()),
            (Some(ColorTagDisplay::Highlight), &[][..])
        );
        for text in ["color_tags = \"highlight\"\n", "color_tags = true\n"] {
            let config = read(text, None);
            assert_eq!(config.color_tags, None, "{text}");
            assert_eq!(
                config.problems(),
                [Problem::InvalidValue {
                    key: "color_tags".to_owned(),
                    expected: "\"icon\" or \"line highlight\"",
                }],
                "{text}"
            );
        }
    }

    // D-75, FR-SET-04
    #[test]
    fn special_inbox_is_true_or_false_and_anything_else_is_reported() {
        assert_eq!(read("", None).special_inbox, None);
        assert_eq!(
            read("special_inbox = false\n", None).special_inbox,
            Some(false)
        );
        let config = read("special_inbox = 1\n", None);
        assert_eq!(config.special_inbox, None);
        assert_eq!(
            config.problems(),
            [Problem::InvalidValue {
                key: "special_inbox".to_owned(),
                expected: "true or false",
            }]
        );
    }

    // D-77, FR-SET-04
    #[test]
    fn sidebar_today_is_true_or_false_and_anything_else_is_reported() {
        assert_eq!(read("", None).sidebar_today, None);
        assert_eq!(
            read("sidebar_today = false\n", None).sidebar_today,
            Some(false)
        );
        let config = read("sidebar_today = \"no\"\n", None);
        assert_eq!(config.sidebar_today, None);
        assert_eq!(
            config.problems(),
            [Problem::InvalidValue {
                key: "sidebar_today".to_owned(),
                expected: "true or false",
            }]
        );
    }

    // FR-UI-05, FR-SET-04
    #[test]
    fn word_wrap_is_true_or_false_and_anything_else_is_reported() {
        assert_eq!(read("", None).wrap, None);
        assert_eq!(read("word_wrap = false\n", None).wrap, Some(false));
        let config = read("word_wrap = \"off\"\n", None);
        assert_eq!(config.wrap, None);
        assert_eq!(
            config.problems(),
            [Problem::InvalidValue {
                key: "word_wrap".to_owned(),
                expected: "true or false",
            }]
        );
    }

    // FR-SET-01, file-io.md §2
    #[test]
    fn the_autosave_delay_and_backups_are_read_with_their_limits() {
        let config = read(
            "autosave_delay_ms = 250\n\
             [backups]\n\
             enabled = false\n\
             dir = \"~/copies\"\n\
             keep = 3\n",
            Some(Path::new("/home/ann")),
        );
        assert_eq!(config.problems(), []);
        assert_eq!(config.autosave_delay, Some(Duration::from_millis(250)));
        assert_eq!(
            config.backups(),
            &BackupSettings {
                enabled: false,
                folder: Some(PathBuf::from("/home/ann/copies")),
                keep: 3,
            }
        );
        let defaults = read("", None);
        assert_eq!(defaults.autosave_delay, None);
        assert_eq!(defaults.backups(), &BackupSettings::default());
        assert_eq!(
            read("autosave_delay_ms = 100\n", None).autosave_delay,
            Some(Duration::from_millis(100)),
            "the least delay"
        );
    }

    // FR-SET-04, file-io.md §2
    #[test]
    fn autosave_and_backup_values_out_of_range_are_reported_and_keep_their_defaults() {
        let invalid = |key: &str, expected| Problem::InvalidValue {
            key: key.to_owned(),
            expected,
        };
        let delay = "a whole number of milliseconds, at least 100";
        for text in [
            "autosave_delay_ms = 99",
            "autosave_delay_ms = -5",
            "autosave_delay_ms = \"1s\"",
        ] {
            let config = read(text, None);
            assert_eq!(
                config.problems(),
                [invalid("autosave_delay_ms", delay)],
                "{text}"
            );
            assert_eq!(config.autosave_delay, None);
        }
        let config = read(
            "[backups]\n\
             enabled = \"no\"\n\
             dir = \"\"\n\
             keep = 0\n\
             folder = \"x\"\n",
            None,
        );
        assert_eq!(
            config.problems(),
            [
                invalid("backups.enabled", "true or false"),
                invalid("backups.dir", "a folder"),
                invalid("backups.keep", "a whole number, at least 1"),
                Problem::UnknownSetting {
                    key: "backups.folder".to_owned()
                },
            ]
        );
        assert_eq!(config.backups(), &BackupSettings::default());
        assert_eq!(
            read("[backups]\ndir = 3\nkeep = -1\n", None).problems(),
            [
                invalid("backups.dir", "a folder"),
                invalid("backups.keep", "a whole number, at least 1"),
            ]
        );
        assert_eq!(
            read("backups = true\n", None).problems(),
            [invalid("backups", "a table")]
        );
    }

    // FR-SET-04
    #[test]
    fn problems_read_as_the_status_line_says_them() {
        for (problem, text) in [
            (
                Problem::Unreadable {
                    reason: "denied".into(),
                },
                "cannot read it: denied",
            ),
            (
                Problem::Syntax {
                    line: 2,
                    message: "invalid".into(),
                },
                "line 2: invalid",
            ),
            (
                Problem::UnknownSetting {
                    key: "theme".into(),
                },
                "unknown setting `theme`",
            ),
            (
                Problem::InvalidValue {
                    key: "keymap".into(),
                    expected: "a table",
                },
                "`keymap` must be a table",
            ),
            (
                Problem::Binding {
                    context: "edit".into(),
                    error: BindingError::BoundTwice { keys: "G".into() },
                },
                "[keymap.edit] \"G\" is bound twice",
            ),
        ] {
            assert_eq!(problem.to_string(), text);
        }
    }

    // FR-UI-06, FR-SET-04
    #[test]
    fn the_theme_is_a_built_in_themes_name() -> TestResult {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("config.toml");
        fs::write(&path, "theme = \"nord\"\n")?;
        let config = load(&path, None);
        assert_eq!(config.problems(), []);
        assert_eq!(config.theme, BuiltIn::named("nord"));
        assert_eq!(config.file(), Some(path.as_path()));
        assert_eq!(read("", None).theme, None);
        assert_eq!(Config::default().file(), None);
        // Terminal Light was removed (D-63).
        for text in [
            "theme = \"Nord\"",
            "theme = 3",
            "theme = \"terminal-light\"",
        ] {
            let config = read(text, None);
            assert_eq!(config.theme, None, "{text}");
            assert_eq!(
                config.problems(),
                [Problem::InvalidValue {
                    key: "theme".into(),
                    expected: "a built-in theme's name, such as \"nord\"",
                }],
                "{text}"
            );
        }
        Ok(())
    }

    // FR-SET-01, FR-SET-04
    #[test]
    fn setting_a_value_creates_the_file_and_its_folder() -> TestResult {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("taskpaper-terminal/config.toml");
        set(&path, "theme", "nord")?;
        assert_eq!(fs::read_to_string(&path)?, "theme = \"nord\"\n");
        set(&path, "backups.keep", 3)?;
        assert_eq!(
            fs::read_to_string(&path)?,
            "theme = \"nord\"\n\n[backups]\nkeep = 3\n"
        );
        assert_eq!(load(&path, None).problems(), [], "it reads back");
        Ok(())
    }

    // FR-SET-04
    #[test]
    fn setting_a_value_keeps_comments_order_and_other_settings() -> TestResult {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("config.toml");
        let text = "# My settings\n\
             theme = \"dracula\"  # dark at night\n\
             autosave_delay_ms = 500\n\
             \n\
             [backups]\n\
             # how many\n\
             keep = 5\n\
             \n\
             [keymap.navigate]\n\
             x = \"none\"\n";
        fs::write(&path, text)?;
        set(&path, "theme", "nord")?;
        set(&path, "backups.enabled", false)?;
        set(&path, "backups.keep", 9)?;
        assert_eq!(
            fs::read_to_string(&path)?,
            "# My settings\n\
             theme = \"nord\"  # dark at night\n\
             autosave_delay_ms = 500\n\
             \n\
             [backups]\n\
             # how many\n\
             keep = 9\n\
             enabled = false\n\
             \n\
             [keymap.navigate]\n\
             x = \"none\"\n"
        );
        Ok(())
    }

    // FR-SET-04
    #[test]
    fn setting_a_value_through_a_link_replaces_the_file_it_points_to() -> TestResult {
        let dir = tempfile::tempdir()?;
        let target = dir.path().join("dotfiles.toml");
        fs::write(&target, "theme = \"dracula\"\n")?;
        let path = dir.path().join("config.toml");
        std::os::unix::fs::symlink(&target, &path)?;
        set(&path, "theme", "nord")?;
        assert!(fs::symlink_metadata(&path)?.file_type().is_symlink());
        assert_eq!(fs::read_to_string(&target)?, "theme = \"nord\"\n");
        Ok(())
    }

    // FR-SET-04, ui.md §8
    #[test]
    fn removing_a_setting_keeps_everything_else() -> TestResult {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("config.toml");
        remove(&path, "backups.dir")?;
        assert!(!path.exists(), "no file is made to remove nothing");
        let text = "# Mine\n\
             theme = \"nord\"\n\
             \n\
             [backups]\n\
             dir = \"~/copies\" # synced\n\
             keep = 5\n";
        fs::write(&path, text)?;
        remove(&path, "backups.dir")?;
        assert_eq!(
            fs::read_to_string(&path)?,
            "# Mine\ntheme = \"nord\"\n\n[backups]\nkeep = 5\n"
        );
        remove(&path, "theme")?;
        let left = "\n[backups]\nkeep = 5\n";
        assert_eq!(
            fs::read_to_string(&path)?,
            left,
            "the comment above it goes with it"
        );
        for key in [
            "backups.dir",
            "icons",
            "keymap.navigate.x",
            "backups.keep.x",
        ] {
            remove(&path, key)?;
            assert_eq!(fs::read_to_string(&path)?, left, "{key}");
        }
        fs::write(&path, "theme = \n")?;
        assert!(matches!(
            remove(&path, "theme"),
            Err(WriteError::NotToml { .. })
        ));
        assert!(matches!(
            remove(dir.path(), "theme"),
            Err(WriteError::Io(_))
        ));
        Ok(())
    }

    // FR-SET-04
    #[test]
    fn a_file_that_cannot_take_the_value_is_left_as_it_was() -> TestResult {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("config.toml");
        for (text, key, error) in [
            ("theme = \n", "theme", "not valid TOML"),
            ("backups = 3\n", "backups.keep", "`backups` is in the way"),
            ("[a.b]\nc = 1\n", "a.b.c.d", "`a.b.c` is in the way"),
            ("[theme]\nx = 1\n", "theme", "`theme` is in the way"),
        ] {
            fs::write(&path, text)?;
            let written = set(&path, key, "nord");
            let message = written.as_ref().map_err(ToString::to_string).err();
            assert!(
                message.as_deref().is_some_and(|m| m.starts_with(error)),
                "{text:?}: {message:?}"
            );
            assert_eq!(fs::read_to_string(&path)?, text, "unchanged");
        }
        assert!(matches!(
            set(&path, "theme", "nord"),
            Err(WriteError::InTheWay { .. })
        ));
        assert!(
            matches!(set(dir.path(), "theme", "nord"), Err(WriteError::Io(_))),
            "a folder cannot be read as the file"
        );
        fs::write(&path, b"theme = \"\xff\"\n")?;
        assert!(
            matches!(set(&path, "theme", "nord"), Err(WriteError::Io(_))),
            "a file that is not UTF-8 cannot be read"
        );
        assert_eq!(fs::read(&path)?, b"theme = \"\xff\"\n", "unchanged");
        assert_eq!(
            WriteError::NoFile.problem("theme"),
            "`theme` not saved: no place for it: neither $XDG_CONFIG_HOME nor a home folder is set"
        );
        Ok(())
    }
}
