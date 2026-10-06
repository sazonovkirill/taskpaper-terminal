//! Save As: the prompt for a path, confirming a file it would replace, writing the document there
//! and making it the open file
//! ([file-io.md §2](../../../docs/specs/file-io.md#2-saving), FR-DOC-06). The frontend expands
//! and completes the path typed and writes the file, reporting back as [`SaveAs`] events.

use std::{ops::Range, path::PathBuf};

use super::{
    Effect, State, Version,
    text_field::{TextEdit, TextField},
    update::Disk,
};
use crate::document::format;

/// What happens to Save As: the user's keys and the frontend's reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveAs {
    /// Open the prompt with the open file's path typed and selected.
    Open,
    /// Change the path typed.
    Edit(TextEdit),
    /// Complete the folder being typed; the frontend reports [`SaveAs::Completed`].
    Complete,
    /// Write the document to the path typed, or to the file to replace once asked.
    Accept,
    /// Go back from replacing a file to the path, or close the prompt.
    Cancel,
    /// The path typed, completed by the frontend.
    Completed(String),
    /// The path typed as the frontend resolves it, `~` expanded.
    Resolved(PathBuf),
    /// The [`Disk::SaveAs`] under way wrote the file.
    Saved,
    /// The [`Disk::SaveAs`] under way failed.
    Failed {
        /// Why, for the prompt.
        reason: String,
    },
    /// The [`Disk::SaveAs`] under way was not done: the file is not the version it expected.
    Refused {
        /// The file's bytes now, or `None` when there is no file.
        contents: Option<Vec<u8>>,
    },
}

/// The open prompt as a frontend shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveAsView<'a> {
    /// The path typed.
    pub text: &'a str,
    /// The caret's byte offset into `text`.
    pub caret: usize,
    /// The selected bytes of `text`, if any.
    pub selection: Option<Range<usize>>,
    /// What the prompt is doing.
    pub status: SaveAsStatus<'a>,
}

/// What the Save As prompt is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveAsStatus<'a> {
    /// Taking the path.
    Typing,
    /// Taking the path again after writing failed, for this reason.
    Failed(&'a str),
    /// Asking whether to replace the file at the path.
    Replacing,
    /// Writing the file.
    Saving,
}

/// The open prompt: the path typed and what it is doing.
#[derive(Debug, Clone)]
pub(super) struct Prompt {
    field: TextField,
    stage: Stage,
}

#[derive(Debug, Clone)]
enum Stage {
    /// Taking the path, after a failure to write it when there is a reason.
    Typing { failure: Option<String> },
    /// Waiting for the frontend to resolve the path typed.
    Resolving,
    /// Asking whether to replace the file at `path`, which is at `expected` or gone.
    Replacing {
        path: PathBuf,
        expected: Option<Version>,
    },
    /// Writing the document at `revision` to `path`, as `version`.
    Writing {
        path: PathBuf,
        version: Version,
        revision: u64,
    },
}

impl State {
    /// The Save As prompt, while it is open.
    #[must_use]
    pub fn save_as(&self) -> Option<SaveAsView<'_>> {
        let prompt = self.save_as.as_ref()?;
        let status = match &prompt.stage {
            Stage::Typing { failure: None } => SaveAsStatus::Typing,
            Stage::Typing {
                failure: Some(reason),
            } => SaveAsStatus::Failed(reason),
            Stage::Replacing { .. } => SaveAsStatus::Replacing,
            Stage::Resolving | Stage::Writing { .. } => SaveAsStatus::Saving,
        };
        Some(SaveAsView {
            text: prompt.field.text(),
            caret: prompt.field.caret(),
            selection: prompt.field.selection(),
            status,
        })
    }

    /// Applies `event` to Save As and returns the effects it asks for.
    pub(super) fn save_as_event(&mut self, event: SaveAs) -> Vec<Effect> {
        if event == SaveAs::Open {
            let path = self.path().map(|path| path.display().to_string());
            self.save_as = Some(Prompt {
                field: TextField::new(path.unwrap_or_default()).edited(TextEdit::SelectAll),
                stage: Stage::Typing { failure: None },
            });
            return Vec::new();
        }
        let Some(prompt) = self.save_as.as_mut() else {
            return Vec::new();
        };
        match (event, &prompt.stage) {
            (SaveAs::Edit(edit), Stage::Typing { .. } | Stage::Replacing { .. }) => {
                prompt.field = prompt.field.edited(edit);
                prompt.stage = Stage::Typing { failure: None };
            }
            (SaveAs::Complete, Stage::Typing { .. } | Stage::Replacing { .. }) => {
                return vec![Effect::CompletePath(prompt.field.text().to_owned())];
            }
            (SaveAs::Completed(text), Stage::Typing { .. } | Stage::Replacing { .. }) => {
                prompt.field = TextField::new(text);
                prompt.stage = Stage::Typing { failure: None };
            }
            (SaveAs::Accept, Stage::Typing { .. }) if !prompt.field.text().trim().is_empty() => {
                prompt.stage = Stage::Resolving;
                return vec![Effect::ResolvePath(prompt.field.text().to_owned())];
            }
            (SaveAs::Accept, Stage::Replacing { path, expected }) => {
                let (path, expected) = (path.clone(), *expected);
                return self.write_as(path, expected);
            }
            (SaveAs::Resolved(path), Stage::Resolving) => return self.write_as(path, None),
            (SaveAs::Cancel, Stage::Replacing { .. }) => {
                prompt.stage = Stage::Typing { failure: None };
            }
            (SaveAs::Cancel, Stage::Typing { .. }) => self.save_as = None,
            (SaveAs::Failed { reason }, Stage::Writing { .. }) => {
                prompt.stage = Stage::Typing {
                    failure: Some(reason),
                };
            }
            (SaveAs::Refused { contents }, Stage::Writing { path, .. }) => {
                prompt.stage = Stage::Replacing {
                    path: path.clone(),
                    expected: contents.as_deref().map(Version::of),
                };
            }
            (
                SaveAs::Saved,
                Stage::Writing {
                    path,
                    version,
                    revision,
                },
            ) => {
                let (path, version, revision) = (path.clone(), *version, *revision);
                return self.saved_as(path, version, revision);
            }
            _ => {}
        }
        Vec::new()
    }

    /// Writes the document to `path`, which must be at `expected`, or not exist when `None`,
    /// after keeping a backup of a file it replaces.
    fn write_as(&mut self, path: PathBuf, expected: Option<Version>) -> Vec<Effect> {
        self.lift_trailing_blanks();
        let text = format::serialize(self.document());
        let backup = expected.map(|_| Effect::Disk(Disk::Backup { path: path.clone() }));
        let save = Effect::Disk(Disk::SaveAs {
            path: path.clone(),
            text: text.clone(),
            expected,
        });
        if let Some(prompt) = self.save_as.as_mut() {
            prompt.stage = Stage::Writing {
                path,
                version: Version::of(text.as_bytes()),
                revision: self.document.revision(),
            };
        }
        backup.into_iter().chain([save]).collect()
    }

    /// The document at `revision` was written to `path` as `version`: the prompt closes and the
    /// file becomes the open file, which autosave, backups and watching follow from now on.
    fn saved_as(&mut self, path: PathBuf, version: Version, revision: u64) -> Vec<Effect> {
        self.save_as = None;
        self.autosave.saved_as(revision);
        self.external.moved(version);
        self.switch_file(path.clone());
        vec![Effect::Watch(path)]
    }

    /// Whether Save As is writing the document, which autosave waits for.
    pub(super) fn saving_as(&self) -> bool {
        self.save_as
            .as_ref()
            .is_some_and(|prompt| matches!(prompt.stage, Stage::Resolving | Stage::Writing { .. }))
    }
}

#[cfg(test)]
mod tests {
    use std::{path::Path, time::Duration};

    use super::*;
    use crate::{
        app::{Access, Message, Origin, SaveState, saving::DELAY},
        document::format,
    };

    const FILE: &str = "/tmp/taskpaper-1.taskpaper";
    const NEW: &str = "/notes/todo.taskpaper";

    /// The temporary file holding `- a`, opened read-only.
    fn opened() -> State {
        State::new(format::parse("- a\n"), Some(PathBuf::from(FILE)))
            .with_origin(Origin::Temporary)
            .with_access(Access::ReadOnly)
    }

    fn event(state: &mut State, event: SaveAs) -> Vec<Effect> {
        state.update(Message::SaveAs(event))
    }

    fn typed(state: &mut State, text: &str) {
        for c in text.chars() {
            assert_eq!(event(state, SaveAs::Edit(TextEdit::Insert(c))), []);
        }
    }

    fn status(state: &State) -> Option<SaveAsStatus<'_>> {
        state.save_as().map(|view| view.status)
    }

    fn save_as(text: &str, expected: Option<&[u8]>) -> Effect {
        Effect::Disk(Disk::SaveAs {
            path: PathBuf::from(NEW),
            text: text.to_owned(),
            expected: expected.map(Version::of),
        })
    }

    /// `state` with Save As accepted for [`NEW`], waiting for the file to be written.
    fn accepted(state: &mut State) {
        event(state, SaveAs::Open);
        typed(state, NEW);
        assert_eq!(
            event(state, SaveAs::Accept),
            [Effect::ResolvePath(NEW.to_owned())]
        );
        assert_eq!(status(state), Some(SaveAsStatus::Saving));
        assert_eq!(
            event(state, SaveAs::Resolved(PathBuf::from(NEW))),
            [save_as("- a\n", None)],
            "no backup of a file that must not exist"
        );
    }

    // FR-DOC-06, file-io.md §2
    #[test]
    fn opens_with_the_files_path_selected_so_typing_replaces_it() {
        let mut state = opened();
        assert_eq!(state.save_as(), None);
        event(&mut state, SaveAs::Open);
        assert_eq!(
            state.save_as(),
            Some(SaveAsView {
                text: FILE,
                caret: FILE.len(),
                selection: Some(0..FILE.len()),
                status: SaveAsStatus::Typing,
            })
        );
        typed(&mut state, "~/a");
        assert_eq!(state.save_as().unwrap().text, "~/a");
        let mut unnamed = State::new(format::parse("- a\n"), None);
        event(&mut unnamed, SaveAs::Open);
        assert_eq!(unnamed.save_as().unwrap().text, "");
    }

    // FR-DOC-06, file-io.md §2
    #[test]
    fn writing_the_file_makes_it_the_open_file() {
        let mut state = opened();
        accepted(&mut state);
        assert_eq!(
            state.path(),
            Some(Path::new(FILE)),
            "not until it is written"
        );
        assert_eq!(
            event(&mut state, SaveAs::Saved),
            [Effect::Watch(PathBuf::from(NEW))]
        );
        assert_eq!(state.save_as(), None);
        assert_eq!(state.path(), Some(Path::new(NEW)));
        assert_eq!(state.origin(), Origin::Chosen, "no longer temporary");
        assert_eq!(state.access(), Access::ReadWrite);
        assert_eq!(state.save_state(), SaveState::Saved);
        state.update(Message::ToggleDone {
            today: jiff::civil::date(2026, 10, 5),
        });
        let effects = state.update(Message::Tick {
            now: DELAY,
            today: jiff::civil::date(2026, 10, 5),
        });
        assert_eq!(
            effects,
            [
                Effect::Disk(Disk::Backup {
                    path: PathBuf::from(NEW)
                }),
                Effect::Disk(Disk::Save {
                    path: PathBuf::from(NEW),
                    text: "- a @done(2026-10-05)\n".to_owned(),
                    expected: Some(Version::of(b"- a\n")),
                })
            ],
            "autosave and backups follow the new file"
        );
    }

    // FR-DOC-02, FR-DOC-06
    #[test]
    fn changes_written_by_save_as_are_saved_and_a_failed_save_is_forgotten() {
        let today = jiff::civil::date(2026, 10, 5);
        let mut state = State::new(format::parse("- a\n"), Some(PathBuf::from(FILE)));
        state.update(Message::ToggleDone { today });
        state.update(Message::Tick { now: DELAY, today });
        state.update(Message::SaveFailed {
            reason: "denied".to_owned(),
        });
        assert!(matches!(state.save_state(), SaveState::Failed { .. }));
        event(&mut state, SaveAs::Open);
        typed(&mut state, NEW);
        event(&mut state, SaveAs::Accept);
        assert_eq!(
            event(&mut state, SaveAs::Resolved(PathBuf::from(NEW))),
            [save_as("- a @done(2026-10-05)\n", None)]
        );
        event(&mut state, SaveAs::Saved);
        assert_eq!(state.save_state(), SaveState::Saved);
        assert_eq!(
            state.update(Message::Tick {
                now: DELAY * 3,
                today
            }),
            [],
            "nothing is left to save"
        );
    }

    // FR-DOC-06, file-io.md §2
    #[test]
    fn a_file_there_is_replaced_only_once_confirmed_and_after_a_backup() {
        let mut state = opened();
        accepted(&mut state);
        let theirs = b"other\n".to_vec();
        assert_eq!(
            event(
                &mut state,
                SaveAs::Refused {
                    contents: Some(theirs.clone())
                }
            ),
            []
        );
        assert_eq!(status(&state), Some(SaveAsStatus::Replacing));
        assert_eq!(event(&mut state, SaveAs::Cancel), []);
        assert_eq!(status(&state), Some(SaveAsStatus::Typing), "Esc goes back");
        event(&mut state, SaveAs::Accept);
        event(&mut state, SaveAs::Resolved(PathBuf::from(NEW)));
        event(
            &mut state,
            SaveAs::Refused {
                contents: Some(theirs.clone()),
            },
        );
        assert_eq!(
            event(&mut state, SaveAs::Accept),
            [
                Effect::Disk(Disk::Backup {
                    path: PathBuf::from(NEW)
                }),
                save_as("- a\n", Some(&theirs)),
            ]
        );
        event(&mut state, SaveAs::Saved);
        assert_eq!(state.path(), Some(Path::new(NEW)));
    }

    // FR-DOC-06
    #[test]
    fn a_failure_shows_its_reason_until_the_path_changes() {
        let mut state = opened();
        accepted(&mut state);
        let reason = "no such folder".to_owned();
        assert_eq!(event(&mut state, SaveAs::Failed { reason }), []);
        assert_eq!(status(&state), Some(SaveAsStatus::Failed("no such folder")));
        assert_eq!(state.path(), Some(Path::new(FILE)));
        typed(&mut state, "x");
        assert_eq!(status(&state), Some(SaveAsStatus::Typing));
        assert_eq!(event(&mut state, SaveAs::Cancel), []);
        assert_eq!(state.save_as(), None, "Esc closes it");
        assert_eq!(state.path(), Some(Path::new(FILE)));
    }

    // FR-DOC-06, file-io.md §2
    #[test]
    fn tab_asks_to_complete_the_path_and_takes_the_completion() {
        let mut state = opened();
        event(&mut state, SaveAs::Open);
        typed(&mut state, "~/no");
        assert_eq!(
            event(&mut state, SaveAs::Complete),
            [Effect::CompletePath("~/no".to_owned())]
        );
        event(&mut state, SaveAs::Completed("~/notes/".to_owned()));
        let view = state.save_as().unwrap();
        assert_eq!((view.text, view.caret), ("~/notes/", 8));
        accepted(&mut state);
        event(&mut state, SaveAs::Refused { contents: None });
        assert_eq!(
            event(&mut state, SaveAs::Complete),
            [Effect::CompletePath(NEW.to_owned())],
            "also while asking to replace"
        );
        event(&mut state, SaveAs::Completed("/notes/todo/".to_owned()));
        assert_eq!(state.save_as().unwrap().text, "/notes/todo/");
        assert_eq!(
            status(&state),
            Some(SaveAsStatus::Typing),
            "a new path is typed"
        );
    }

    // F-FILE-06, FR-DOC-06
    #[test]
    fn the_old_file_is_not_checked_while_the_new_one_is_written() {
        let mut state = opened();
        event(&mut state, SaveAs::Open);
        assert_eq!(
            state.update(Message::CheckFile),
            [Effect::Disk(Disk::Check {
                path: PathBuf::from(FILE)
            })]
        );
        accepted(&mut state);
        assert_eq!(state.update(Message::CheckFile), []);
        event(&mut state, SaveAs::Saved);
        assert_eq!(
            state.update(Message::CheckFile),
            [Effect::Disk(Disk::Check {
                path: PathBuf::from(NEW)
            })]
        );
    }

    #[test]
    fn a_blank_path_is_not_saved_to_and_keys_wait_while_writing() {
        let mut state = opened();
        event(&mut state, SaveAs::Open);
        event(&mut state, SaveAs::Edit(TextEdit::DeleteBackward));
        assert_eq!(event(&mut state, SaveAs::Accept), []);
        typed(&mut state, "  ");
        assert_eq!(event(&mut state, SaveAs::Accept), []);
        assert_eq!(status(&state), Some(SaveAsStatus::Typing));
        let mut state = opened();
        accepted(&mut state);
        for ignored in [
            SaveAs::Edit(TextEdit::Insert('x')),
            SaveAs::Complete,
            SaveAs::Accept,
            SaveAs::Cancel,
            SaveAs::Completed("x".to_owned()),
            SaveAs::Resolved(PathBuf::from("/x")),
        ] {
            assert_eq!(event(&mut state, ignored), []);
        }
        assert_eq!(state.save_as().unwrap().text, NEW);
        assert_eq!(status(&state), Some(SaveAsStatus::Saving));
    }

    #[test]
    fn reports_without_a_save_under_way_change_nothing() {
        let mut state = opened();
        for report in [
            SaveAs::Saved,
            SaveAs::Failed {
                reason: "x".to_owned(),
            },
            SaveAs::Refused { contents: None },
            SaveAs::Accept,
        ] {
            assert_eq!(event(&mut state, report.clone()), []);
            assert_eq!(state.save_as(), None);
        }
        event(&mut state, SaveAs::Open);
        for report in [
            SaveAs::Saved,
            SaveAs::Refused { contents: None },
            SaveAs::Resolved(PathBuf::from(NEW)),
        ] {
            assert_eq!(event(&mut state, report), []);
            assert_eq!(status(&state), Some(SaveAsStatus::Typing));
        }
        assert_eq!(state.path(), Some(Path::new(FILE)));
    }

    // FR-DOC-02, FR-DOC-06
    #[test]
    fn autosave_waits_for_the_file_being_written_then_saves_there() {
        let mut state = State::new(format::parse("- a\n"), Some(PathBuf::from(FILE)));
        accepted(&mut state);
        state.update(Message::ToggleDone {
            today: jiff::civil::date(2026, 10, 5),
        });
        let later = Message::Tick {
            now: Duration::from_secs(5),
            today: jiff::civil::date(2026, 10, 5),
        };
        assert_eq!(
            state.update(later.clone()),
            [],
            "nothing is saved meanwhile"
        );
        assert_eq!(state.deadline(), None);
        let effects = event(&mut state, SaveAs::Saved);
        assert_eq!(state.save_state(), SaveState::Unsaved);
        assert!(
            effects.contains(&Effect::Disk(Disk::Save {
                path: PathBuf::from(NEW),
                text: "- a @done(2026-10-05)\n".to_owned(),
                expected: Some(Version::of(b"- a\n")),
            })),
            "{effects:?}"
        );
    }
}
