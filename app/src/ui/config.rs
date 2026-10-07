//! The settings file `config.toml`: where it is and the settings it sets, each left at its
//! default where the file does not set it or sets it in a way the app cannot use, which is
//! reported rather than refused (FR-SET-01, FR-SET-04,
//! [keybindings.md §5](../../../docs/specs/keybindings.md#5-configuration)).

use std::{
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
    time::Duration,
};

use thiserror::Error;
use toml_edit::{Document, Item, TableLike};

use super::{
    effects::{self, BackupSettings},
    keymap::{BindingError, Keymap},
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
const MIN_AUTOSAVE_DELAY: Duration = Duration::from_millis(100);

/// The settings the frontend runs with, and what it ignored of the file.
#[derive(Debug, Default)]
pub struct Config {
    /// The key bindings: the defaults with the file's `[keymap.*]` tables applied.
    pub(super) keymap: Keymap,
    /// How long after the last change a save starts, when the file says (`autosave_delay_ms`).
    pub(super) autosave_delay: Option<Duration>,
    backups: BackupSettings,
    problems: Vec<Problem>,
}

impl Config {
    /// What the app ignored of the file, setting by setting.
    #[must_use]
    pub fn problems(&self) -> &[Problem] {
        &self.problems
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
    match fs::read_to_string(path) {
        Ok(text) => read(&text, home),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Config::default(),
        Err(error) => Config::failed(Problem::Unreadable {
            reason: error.to_string(),
        }),
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
            _ => config.problems.push(Problem::UnknownSetting {
                key: key.to_owned(),
            }),
        }
    }
    config
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
            "theme = \"nord\"\n\
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
                Problem::UnknownSetting {
                    key: "theme".into()
                },
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
}
