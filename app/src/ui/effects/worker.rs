//! The disk worker: carries out the core's [`Disk`] effects in order on a thread of its own, so
//! the event loop never waits on the disk (N-UX-01), and sends their results back as messages.

use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, Sender},
    thread::{self, JoinHandle},
};

use toml_edit::Value;

use super::{BackupSettings, Backups, MissingFolder, Views, atomic};
use crate::{
    app::{Disk, Message, SaveAs, Version, ViewState},
    ui::{Input, config},
};

/// The queue of disk work and the thread that does it. Dropping it waits for the queued work.
#[derive(Debug)]
pub struct Io {
    /// `None` only while dropping, to close the queue before waiting for the thread.
    jobs: Option<Sender<Job>>,
    worker: Option<JoinHandle<()>>,
}

#[derive(Debug)]
enum Job {
    Disk(Disk),
    /// Keep this layout for the file at this path.
    StoreView(PathBuf, ViewState),
    /// Write this value for this setting, a dotted key, to the settings file at this path, or
    /// remove the setting when there is none.
    StoreSetting(PathBuf, &'static str, Option<Value>),
    /// Make backups as these settings say from now on.
    SetBackups(BackupSettings),
    /// Answers once every job before it is done and its result sent.
    Flush(Sender<()>),
}

impl Io {
    /// Starts the worker, which sends each result to `results` as an [`Input::Core`] message,
    /// makes backups with `backups` and keeps layouts in `views`.
    #[must_use]
    pub fn start(results: Sender<Input>, backups: Backups, views: Views) -> Self {
        let (jobs, queue) = mpsc::channel();
        let worker = thread::spawn(move || work(&queue, &results, backups, &views));
        Self {
            jobs: Some(jobs),
            worker: Some(worker),
        }
    }

    /// Queues `disk` behind the work queued before it.
    pub fn submit(&self, disk: Disk) {
        self.send(Job::Disk(disk));
    }

    /// Queues keeping `view` as the layout of the file at `path` behind the work queued before
    /// it (file-io.md §5).
    pub fn store_view(&self, path: PathBuf, view: ViewState) {
        self.send(Job::StoreView(path, view));
    }

    /// Queues writing `value` for the setting `key`, dotted as in `backups.keep`, to the settings
    /// file at `path`, or removing the setting from it when `value` is `None`, behind the work
    /// queued before it; a write that fails comes back as [`Message::ConfigProblems`] (FR-SET-04).
    pub fn store_setting(&self, path: PathBuf, key: &'static str, value: Option<Value>) {
        self.send(Job::StoreSetting(path, key, value));
    }

    /// Queues making backups as `settings` say from then on, behind the work queued before it
    /// (file-io.md §2).
    pub fn set_backups(&self, settings: BackupSettings) {
        self.send(Job::SetBackups(settings));
    }

    /// Waits until the work queued so far is done and its results are sent.
    ///
    /// # Panics
    ///
    /// Never: the worker runs, answering every flush, until the queue closes on dropping.
    pub fn flush(&self) {
        let (done, finished) = mpsc::channel();
        self.send(Job::Flush(done));
        finished
            .recv()
            .expect("the worker answers every flush before it stops");
    }

    fn send(&self, job: Job) {
        self.jobs
            .as_ref()
            .and_then(|jobs| jobs.send(job).ok())
            .expect("the worker runs until the queue closes, which only dropping does");
    }
}

impl Drop for Io {
    fn drop(&mut self) {
        drop(self.jobs.take());
        if let Some(worker) = self.worker.take() {
            // The worker never panics; if it did, the panic was reported on its own thread and
            // there is nothing left to do here.
            let _ = worker.join();
        }
    }
}

/// Runs the jobs from `queue` until it closes.
fn work(queue: &Receiver<Job>, results: &Sender<Input>, mut backups: Backups, views: &Views) {
    for job in queue {
        let message = match job {
            Job::Disk(Disk::Save {
                path,
                text,
                expected,
            }) => Some(match write(&path, &text.serialize(), expected) {
                Written::Done(version) => Message::Saved { version },
                Written::Failed(reason) => Message::SaveFailed { reason },
                Written::Refused(contents) => Message::SaveRefused { contents },
            }),
            Job::Disk(Disk::SaveAs {
                path,
                text,
                expected,
            }) => Some(Message::SaveAs(
                match write(&path, &text.serialize(), expected) {
                    Written::Done(version) => SaveAs::Saved { version },
                    Written::Failed(reason) => SaveAs::Failed { reason },
                    Written::Refused(contents) => SaveAs::Refused { contents },
                },
            )),
            Job::Disk(Disk::Check { path }) => check(&path),
            Job::Disk(Disk::ConflictCopy { path, contents }) => {
                Some(match backups.conflict(&path, &contents) {
                    Ok(copy) => Message::ConflictCopied { copy },
                    Err(error) => Message::ConflictCopyFailed {
                        reason: error.to_string(),
                    },
                })
            }
            Job::Disk(Disk::Backup { path }) => {
                backups
                    .ensure(&path)
                    .err()
                    .map(|error| Message::BackupFailed {
                        reason: error.to_string(),
                    })
            }
            Job::StoreView(path, view) => {
                // A layout that cannot be kept loses only the arrangement of the panes on the
                // next launch, never anything of the file, and the spec has the status line say
                // nothing of it (file-io.md §5).
                let _ = views.store(&path, &view);
                None
            }
            Job::StoreSetting(path, key, value) => match value {
                Some(value) => config::set(&path, key, value),
                None => config::remove(&path, key),
            }
            .err()
            .map(|error| Message::ConfigProblems(vec![error.problem(key)])),
            Job::SetBackups(settings) => {
                backups.set(&settings);
                None
            }
            Job::Flush(done) => {
                // The flusher waits for this answer and only stops waiting by being dropped.
                let _ = done.send(());
                None
            }
        };
        if let Some(message) = message {
            // A closed channel means the event loop has ended, and nobody is left to tell.
            let _ = results.send(Input::Core(message));
        }
    }
}

/// How writing a file went.
enum Written {
    /// Done, leaving the file at this version.
    Done(Version),
    /// Not done, because the file was no longer the version expected; these are its bytes now,
    /// or `None` when there is no file.
    Refused(Option<Vec<u8>>),
    /// Failed, for this reason.
    Failed(String),
}

/// Writes `text` to `path`, unless just before the rename the file is no longer at `expected`.
fn write(path: &Path, text: &str, expected: Option<Version>) -> Written {
    let written = atomic::prepare(path, text.as_bytes()).and_then(|prepared| {
        let found = read(prepared.target())?;
        if found.as_deref().map(Version::of) == expected {
            prepared
                .commit()
                .map(|()| Written::Done(Version::of(text.as_bytes())))
        } else {
            Ok(Written::Refused(found))
        }
    });
    written.unwrap_or_else(|error| Written::Failed(reason(path, &error)))
}

/// Why writing the file at `path` failed with `error`: a missing folder in the words opening
/// uses, rather than the error about the temporary file beside the file.
fn reason(path: &Path, error: &io::Error) -> String {
    match MissingFolder::of(path) {
        Some(missing) if error.kind() == io::ErrorKind::NotFound => missing.to_string(),
        _ => error.to_string(),
    }
}

/// What is at `path` now. A file that cannot be read is not reported: the next save reads it
/// again and reports why it fails.
fn check(path: &Path) -> Option<Message> {
    read(path)
        .ok()
        .map(|contents| Message::FileChecked { contents })
}

/// The bytes of the file at `path`, or `None` when there is none.
fn read(path: &Path) -> io::Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use jiff::civil::date;

    use super::*;
    use crate::{app::DocumentText, document::format};

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    /// A worker with backups in `dir/backups`, and the channel its results arrive on.
    fn start(dir: &Path) -> (Io, Receiver<Input>) {
        let (results, inputs) = mpsc::channel();
        let backups = Backups::new(Some(dir.join("backups")), || {
            date(2026, 10, 5).at(9, 30, 0, 0)
        });
        let views = Views::new(Some(dir.join("state/views.json")));
        (Io::start(results, backups, views), inputs)
    }

    fn results(inputs: &Receiver<Input>) -> Vec<Input> {
        inputs.try_iter().collect()
    }

    /// A save of `text` to `path`, where there is no file yet.
    fn save_job(path: &Path, text: &str) -> Disk {
        save_over(path, text, None)
    }

    /// A save of `text` to `path`, where the file holds `expected`.
    fn save_over(path: &Path, text: &str, expected: Option<&str>) -> Disk {
        Disk::Save {
            path: path.to_owned(),
            text: DocumentText::of(&format::parse(text)),
            expected: expected.map(|text| Version::of(text.as_bytes())),
        }
    }

    /// The report of a save that wrote `text`.
    fn saved(text: &str) -> Input {
        Input::Core(Message::Saved {
            version: Version::of(text.as_bytes()),
        })
    }

    // F-FILE-04, N-UX-01
    #[test]
    fn saves_in_order_and_reports_each_result() -> TestResult {
        let dir = tempfile::tempdir()?;
        let (io, inputs) = start(dir.path());
        let path = dir.path().join("todo.taskpaper");
        io.submit(save_job(&path, "one"));
        io.submit(save_job(&dir.path().join("missing/x.taskpaper"), "x"));
        io.submit(save_over(&path, "two", Some("one")));
        io.flush();
        let results = results(&inputs);
        assert_eq!(results.len(), 3);
        assert_eq!(results[0], saved("one"));
        assert_eq!(
            results[1],
            Input::Core(Message::SaveFailed {
                reason: format!(
                    "the folder {} does not exist",
                    dir.path().join("missing").display()
                )
            })
        );
        assert_eq!(results[2], saved("two"));
        assert_eq!(fs::read_to_string(&path)?, "two");
        Ok(())
    }

    // FR-DOC-06, F-FILE-04
    #[test]
    fn saves_as_another_file_and_reports_each_result_as_save_as() -> TestResult {
        let dir = tempfile::tempdir()?;
        let (io, inputs) = start(dir.path());
        let path = dir.path().join("new.taskpaper");
        let save_as = |path: &Path, text: &str, expected: Option<&str>| Disk::SaveAs {
            path: path.to_owned(),
            text: DocumentText::of(&format::parse(text)),
            expected: expected.map(|text| Version::of(text.as_bytes())),
        };
        let saved = |text: &str| {
            Input::Core(Message::SaveAs(SaveAs::Saved {
                version: Version::of(text.as_bytes()),
            }))
        };
        io.submit(save_as(&path, "one", None));
        io.submit(save_as(&path, "two", None));
        io.submit(save_as(&path, "two", Some("one")));
        io.submit(save_as(&dir.path().join("missing/x.taskpaper"), "x", None));
        io.flush();
        let results = results(&inputs);
        assert_eq!(results.len(), 4);
        assert_eq!(results[0], saved("one"));
        assert_eq!(
            results[1],
            Input::Core(Message::SaveAs(SaveAs::Refused {
                contents: Some(b"one".to_vec())
            })),
            "a file that must not be there is not replaced"
        );
        assert_eq!(results[2], saved("two"));
        assert_eq!(
            results[3],
            Input::Core(Message::SaveAs(SaveAs::Failed {
                reason: format!(
                    "the folder {} does not exist",
                    dir.path().join("missing").display()
                )
            }))
        );
        assert_eq!(fs::read_to_string(&path)?, "two");
        Ok(())
    }

    // FR-DOC-07, F-FILE-09
    #[test]
    fn backs_up_before_a_save_queued_after_it() -> TestResult {
        let dir = tempfile::tempdir()?;
        let (io, inputs) = start(dir.path());
        let path = dir.path().join("todo.taskpaper");
        fs::write(&path, "on disk")?;
        io.submit(Disk::Backup { path: path.clone() });
        io.submit(save_over(&path, "saved", Some("on disk")));
        drop(io);
        assert_eq!(results(&inputs), [saved("saved")]);
        let backups = dir.path().join("backups");
        let folder: PathBuf = fs::read_dir(&backups)?.next().unwrap()?.path();
        assert_eq!(
            fs::read_to_string(folder.join("2026-10-05.taskpaper"))?,
            "on disk"
        );
        Ok(())
    }

    // FR-SET-04, N-UX-01
    #[test]
    fn stores_a_setting_and_reports_one_it_cannot_store() -> TestResult {
        let dir = tempfile::tempdir()?;
        let (io, inputs) = start(dir.path());
        let path = dir.path().join("settings/config.toml");
        io.store_setting(path.clone(), "theme", Some("nord".into()));
        io.store_setting(dir.path().to_owned(), "theme", Some("nord".into()));
        io.flush();
        assert!(
            matches!(&results(&inputs)[..], [Input::Core(Message::ConfigProblems(problems))] if problems.len() == 1 && problems[0].starts_with("`theme` not saved: ")),
            "only the folder that is no file is reported"
        );
        assert_eq!(fs::read_to_string(&path)?, "theme = \"nord\"\n");
        Ok(())
    }

    // FR-SET-02, file-io.md §2
    #[test]
    fn makes_backups_as_the_settings_changed_later_say() -> TestResult {
        let dir = tempfile::tempdir()?;
        let (io, inputs) = start(dir.path());
        let path = dir.path().join("todo.taskpaper");
        fs::write(&path, "- a\n")?;
        io.set_backups(BackupSettings {
            enabled: false,
            ..BackupSettings::default()
        });
        io.submit(Disk::Backup { path });
        io.flush();
        assert_eq!(results(&inputs), []);
        assert!(!fs::exists(dir.path().join("backups"))?, "no copy made");
        Ok(())
    }

    // FR-SET-04, ui.md §8
    #[test]
    fn removes_a_setting() -> TestResult {
        let dir = tempfile::tempdir()?;
        let (io, inputs) = start(dir.path());
        let path = dir.path().join("config.toml");
        fs::write(&path, "theme = \"nord\"\nicons = \"none\"\n")?;
        io.store_setting(path.clone(), "theme", None);
        io.flush();
        assert_eq!(results(&inputs), []);
        assert_eq!(fs::read_to_string(&path)?, "icons = \"none\"\n");
        Ok(())
    }

    #[test]
    fn reports_a_failed_backup() -> TestResult {
        let dir = tempfile::tempdir()?;
        let (io, inputs) = start(dir.path());
        io.submit(Disk::Backup {
            path: dir.path().to_owned(),
        });
        io.flush();
        assert!(matches!(
            results(&inputs).as_slice(),
            [Input::Core(Message::BackupFailed { reason })] if reason.contains("directory")
        ));
        Ok(())
    }

    // FR-UI-04, file-io.md §5
    #[test]
    fn keeps_layouts_in_order_with_saves_and_reports_nothing() -> TestResult {
        let dir = tempfile::tempdir()?;
        let (io, inputs) = start(dir.path());
        let path = dir.path().join("todo.taskpaper");
        let mut state = super::super::open(&path)?;
        state.update(Message::Pane(crate::app::PaneChange::OpenLocation));
        io.submit(save_job(&path, "Work:\n"));
        io.store_view(path.clone(), state.view_state().expect("a layout"));
        io.flush();
        assert_eq!(results(&inputs), [saved("Work:\n")]);
        let views = Views::new(Some(dir.path().join("state/views.json")));
        let restored = views.restore(super::super::open(&path)?);
        assert_eq!(restored.panes().len(), 2, "kept for the saved file");
        Ok(())
    }

    #[test]
    fn results_after_the_event_loop_ended_are_dropped() -> TestResult {
        let dir = tempfile::tempdir()?;
        let (io, inputs) = start(dir.path());
        drop(inputs);
        let path = dir.path().join("todo.taskpaper");
        io.submit(save_job(&path, "kept"));
        io.flush();
        assert_eq!(fs::read_to_string(&path)?, "kept");
        Ok(())
    }

    // file-io.md §3
    #[test]
    fn a_save_is_refused_when_the_file_changed_and_leaves_it_alone() -> TestResult {
        let dir = tempfile::tempdir()?;
        let (io, inputs) = start(dir.path());
        let path = dir.path().join("todo.taskpaper");
        fs::write(&path, "theirs")?;
        io.submit(save_over(&path, "mine", Some("known")));
        io.submit(save_job(&path, "mine"));
        let gone = dir.path().join("gone.taskpaper");
        io.submit(save_over(&gone, "mine", Some("known")));
        io.flush();
        let theirs = Some(b"theirs".to_vec());
        assert_eq!(
            results(&inputs),
            [
                Input::Core(Message::SaveRefused {
                    contents: theirs.clone()
                }),
                Input::Core(Message::SaveRefused { contents: theirs }),
                Input::Core(Message::SaveRefused { contents: None }),
            ]
        );
        assert_eq!(fs::read_to_string(&path)?, "theirs");
        assert!(!fs::exists(&gone)?);
        assert_eq!(
            fs::read_dir(dir.path())?.count(),
            1,
            "no temporary file is left"
        );
        Ok(())
    }

    #[test]
    fn a_save_under_a_file_says_what_the_system_reported() -> TestResult {
        let dir = tempfile::tempdir()?;
        let (io, inputs) = start(dir.path());
        let file = dir.path().join("file");
        fs::write(&file, "")?;
        io.submit(save_job(&file.join("x.taskpaper"), "mine"));
        io.flush();
        assert!(
            matches!(
                results(&inputs).as_slice(),
                [Input::Core(Message::SaveFailed { reason })] if reason.contains("Not a directory")
            ),
            "only a folder that is missing is reported as missing"
        );
        Ok(())
    }

    #[test]
    fn a_save_fails_when_the_file_cannot_be_read() -> TestResult {
        let dir = tempfile::tempdir()?;
        let (io, inputs) = start(dir.path());
        let link = dir.path().join("loop.taskpaper");
        std::os::unix::fs::symlink(&link, &link)?;
        io.submit(save_job(&link, "mine"));
        io.flush();
        assert!(matches!(
            results(&inputs).as_slice(),
            [Input::Core(Message::SaveFailed { reason })] if reason.contains("symbolic links")
        ));
        Ok(())
    }

    // F-FILE-06
    #[test]
    fn checks_report_the_file_or_its_absence() -> TestResult {
        let dir = tempfile::tempdir()?;
        let (io, inputs) = start(dir.path());
        let path = dir.path().join("todo.taskpaper");
        fs::write(&path, "on disk")?;
        io.submit(Disk::Check { path: path.clone() });
        io.submit(Disk::Check {
            path: dir.path().join("gone.taskpaper"),
        });
        io.submit(Disk::Check {
            path: dir.path().to_owned(),
        });
        io.flush();
        assert_eq!(
            results(&inputs),
            [
                Input::Core(Message::FileChecked {
                    contents: Some(b"on disk".to_vec())
                }),
                Input::Core(Message::FileChecked { contents: None }),
            ],
            "a path that cannot be read reports nothing"
        );
        Ok(())
    }

    // file-io.md §3
    #[test]
    fn writes_conflict_copies_and_reports_where() -> TestResult {
        let dir = tempfile::tempdir()?;
        let (io, inputs) = start(dir.path());
        let path = dir.path().join("todo.taskpaper");
        fs::write(&path, "on disk")?;
        io.submit(Disk::ConflictCopy {
            path: path.clone(),
            contents: b"theirs".to_vec(),
        });
        io.submit(Disk::ConflictCopy {
            path: dir.path().join("gone/todo.taskpaper"),
            contents: b"mine".to_vec(),
        });
        io.flush();
        let results = results(&inputs);
        assert!(
            matches!(
                results.as_slice(),
                [
                    Input::Core(Message::ConflictCopied { .. }),
                    Input::Core(Message::ConflictCopyFailed { .. }),
                ]
            ),
            "{results:?}"
        );
        let copies: Vec<_> = results
            .iter()
            .filter_map(|input| match input {
                Input::Core(Message::ConflictCopied { copy }) => Some(copy),
                _ => None,
            })
            .collect();
        assert_eq!(fs::read_to_string(copies[0])?, "theirs");
        assert!(copies[0].starts_with(dir.path().join("backups")));
        assert!(
            matches!(&results[1], Input::Core(Message::ConflictCopyFailed { reason }) if reason.contains("No such file")),
            "{results:?}"
        );
        Ok(())
    }
}
