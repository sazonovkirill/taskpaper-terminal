//! Carries out the core's I/O at the edge ([file-io.md](../../../docs/specs/file-io.md)):
//! opening files, watching them, saving, checking and backing them up on the disk worker,
//! keeping their layouts, recovery files and the clipboard.

pub(super) mod atomic;
mod backup;
mod browser;
mod clipboard;
mod paths;
mod recovery;
mod views;
mod watch;
mod worker;

use std::{
    ffi::OsString,
    fs, io,
    io::Write as _,
    os::unix::fs::OpenOptionsExt as _,
    path::{Path, PathBuf},
};

use jiff::civil::DateTime;
use thiserror::Error;

pub use backup::{
    BackupError, BackupSettings, Backups, KEEP as BACKUPS_KEPT, folder as backups_folder,
};
pub use browser::Browser;
pub use clipboard::{Clipboard, SystemClipboard};
pub use paths::{complete as complete_path, expand as expand_path};
pub use recovery::{Recovery, RecoveryError, folder as recovery_folder};
pub use views::{Views, ViewsError, file as views_file};
pub use watch::FileWatch;
pub use worker::Io;

use crate::{
    app::{Access, Origin, State, Version, demo},
    document::{DecodeError, Document, format},
};

/// Where a start without a path makes its file (D-15).
pub const TEMPORARY_FOLDER: &str = "/tmp";

/// How many names [`create_demo`] tries before giving up.
const NAME_ATTEMPTS: u32 = 100;

/// Why a file could not be opened.
#[derive(Debug, Error)]
pub enum OpenError {
    /// Reading failed: no permission, a directory, an I/O error.
    #[error("cannot read {}", path.display())]
    Read {
        /// The file asked for.
        path: PathBuf,
        /// What the system reported.
        #[source]
        source: io::Error,
    },
    /// The file is not UTF-8 (F-FILE-07).
    #[error("cannot open {}", path.display())]
    Decode {
        /// The file asked for.
        path: PathBuf,
        /// Where decoding failed.
        #[source]
        source: DecodeError,
    },
    /// The temporary file could not be made.
    #[error("cannot create {}", path.display())]
    Create {
        /// The file that was to be made.
        path: PathBuf,
        /// What the system reported.
        #[source]
        source: io::Error,
    },
    /// The file on disk could not be kept in a conflict copy before recovered changes replace it.
    #[error("cannot keep a copy of {} before opening it with the recovered changes", path.display())]
    Copy {
        /// The file asked for.
        path: PathBuf,
        /// Why the copy failed.
        #[source]
        source: BackupError,
    },
    /// The file does not exist and its folder does not either, so it could never be saved.
    #[error("cannot open {}: the folder {} does not exist", path.display(), folder.display())]
    MissingFolder {
        /// The file asked for.
        path: PathBuf,
        /// Its missing parent folder.
        folder: PathBuf,
    },
}

/// Opens the file at `path`: read-only when the user may not write it (file-io.md §1).
///
/// # Errors
///
/// [`OpenError`] when the file cannot be read, is not UTF-8, or its folder does not exist.
pub fn open(path: &Path) -> Result<State, OpenError> {
    let (document, version) = read_document(path)?;
    Ok(State::new(document, Some(path.to_owned()))
        .with_access(access(path))
        .with_version(version))
}

/// Writes the demo document, dated relative to `now`, to a new file
/// `taskpaper-<YYYYMMDD-HHMMSS>.taskpaper` in `folder` and opens it as the temporary file
/// (FR-DOC-05, file-io.md §1). A name already taken gets a `-2`, `-3`… suffix, so a file made in
/// the same second is never overwritten.
///
/// # Errors
///
/// [`OpenError::Create`] when no new file can be made in `folder`.
pub fn create_demo(folder: &Path, now: DateTime) -> Result<State, OpenError> {
    let text = demo::text(now.date());
    let stamp = now.strftime("%Y%m%d-%H%M%S");
    let create = |attempt| {
        let name = match attempt {
            1 => format!("taskpaper-{stamp}.taskpaper"),
            n => format!("taskpaper-{stamp}-{n}.taskpaper"),
        };
        let path = folder.join(name);
        match create_new(&path, &text) {
            Ok(()) => Ok(State::new(format::parse(&text), Some(path))
                .with_origin(Origin::Temporary)
                .with_version(Some(Version::of(text.as_bytes())))),
            Err(source) => Err(OpenError::Create { path, source }),
        }
    };
    for attempt in 1..NAME_ATTEMPTS {
        match create(attempt) {
            Err(OpenError::Create { source, .. })
                if source.kind() == io::ErrorKind::AlreadyExists => {}
            result => return result,
        }
    }
    create(NAME_ATTEMPTS)
}

/// Writes `text` to a file at `path` that does not exist yet. Only the user may read it, as it
/// sits in a folder every user can list.
fn create_new(path: &Path, text: &str) -> io::Result<()> {
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?
        .write_all(text.as_bytes())
}

/// Opens the file at `path` with the document of `recovered`, a recovery file of it, as changes
/// the file lacks, after writing the file as it is on disk to a conflict copy with `backups`,
/// since it may have changed after the recovered document was read from it (file-io.md §4).
///
/// # Errors
///
/// [`OpenError`] when the recovery file cannot be read or is not UTF-8, or the copy fails.
pub fn recover(path: &Path, recovered: &Path, backups: &Backups) -> Result<State, OpenError> {
    let bytes = fs::read(recovered).map_err(|source| OpenError::Read {
        path: recovered.to_owned(),
        source,
    })?;
    let document = decode(recovered, &bytes)?;
    let on_disk = fs::read(path).ok();
    if let Some(on_disk) = &on_disk {
        backups
            .conflict(path, on_disk)
            .map_err(|source| OpenError::Copy {
                path: path.to_owned(),
                source,
            })?;
    }
    // The changes are saved over the file as it is now, not reported as a change made elsewhere.
    let version = on_disk.as_deref().map(Version::of);
    Ok(State::recovered(document, Some(path.to_owned()))
        .with_access(access(path))
        .with_version(version))
}

/// Read-only when the file has no write permission or the system refuses to open it for
/// writing; a file that does not exist yet will be created.
fn access(path: &Path) -> Access {
    let refused = |error: io::Error| {
        matches!(
            error.kind(),
            io::ErrorKind::PermissionDenied | io::ErrorKind::ReadOnlyFilesystem
        )
    };
    if fs::metadata(path).is_ok_and(|metadata| metadata.permissions().readonly())
        || fs::OpenOptions::new()
            .append(true)
            .open(path)
            .is_err_and(refused)
    {
        Access::ReadOnly
    } else {
        Access::ReadWrite
    }
}

/// The app's folder `name` under an XDG base directory: `<base>/taskpaper-terminal/<name>`, where
/// `base` is the directory's variable, or `fallback` under the home folder when that is unset or
/// not absolute, as the XDG base directory specification says; also on macOS. `None` when
/// neither is known.
pub(super) fn app_folder(
    variable: Option<OsString>,
    home: Option<PathBuf>,
    fallback: &str,
    name: &str,
) -> Option<PathBuf> {
    let base = variable
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| home.map(|home| home.join(fallback)))?;
    Some(base.join("taskpaper-terminal").join(name))
}

/// Reads and parses the file at `path`, with the version read. A missing file in an existing
/// folder is an empty document, created on the first save (F-FILE-01), and has no version.
fn read_document(path: &Path) -> Result<(Document, Option<Version>), OpenError> {
    match std::fs::read(path) {
        Ok(bytes) => decode(path, &bytes).map(|document| (document, Some(Version::of(&bytes)))),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let folder = std::path::absolute(path)
                .ok()
                .and_then(|absolute| absolute.parent().map(Path::to_owned));
            match folder {
                Some(folder) if folder.is_dir() => Ok((Document::default(), None)),
                Some(folder) => Err(OpenError::MissingFolder {
                    path: path.to_owned(),
                    folder,
                }),
                None => Err(OpenError::Read {
                    path: path.to_owned(),
                    source: error,
                }),
            }
        }
        Err(source) => Err(OpenError::Read {
            path: path.to_owned(),
            source,
        }),
    }
}

/// Parses `bytes`, read from the file at `path`.
fn decode(path: &Path, bytes: &[u8]) -> Result<Document, OpenError> {
    format::parse_bytes(bytes).map_err(|source| OpenError::Decode {
        path: path.to_owned(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use std::error::Error as _;

    use super::*;
    use crate::{
        app::{Disk, Effect, Message},
        document::Item,
    };

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn texts(document: &Document) -> Vec<&str> {
        document
            .descendants(document.root())
            .filter_map(|(id, _)| document.get(id))
            .map(Item::text)
            .collect()
    }

    #[test]
    fn reads_and_parses_a_file() -> TestResult {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("todo.taskpaper");
        std::fs::write(&path, "A:\n\t- b\n")?;
        let (document, version) = read_document(&path)?;
        assert_eq!(texts(&document), ["A:", "- b"]);
        assert_eq!(version, Some(Version::of(b"A:\n\t- b\n")));
        Ok(())
    }

    // F-FILE-07
    #[test]
    fn refuses_a_file_that_is_not_utf8() -> TestResult {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("latin1.taskpaper");
        std::fs::write(&path, b"caf\xe9")?;
        let error = read_document(&path).unwrap_err();
        assert!(matches!(
            &error,
            OpenError::Decode { path: p, source: DecodeError::NotUtf8 { line: 1, offset: 3 } } if *p == path
        ));
        assert_eq!(error.to_string(), format!("cannot open {}", path.display()));
        assert!(error.source().is_some());
        Ok(())
    }

    #[test]
    fn missing_file_in_an_existing_folder_is_an_empty_document() -> TestResult {
        let dir = tempfile::tempdir()?;
        let (document, version) = read_document(&dir.path().join("new.taskpaper"))?;
        assert_eq!(version, None);
        assert_eq!(texts(&document), Vec::<&str>::new());
        assert_eq!(document.format(), &crate::document::FileFormat::default());
        Ok(())
    }

    #[test]
    fn refuses_an_empty_path() {
        assert!(matches!(
            read_document(Path::new("")),
            Err(OpenError::Read { .. })
        ));
    }

    #[test]
    fn refuses_a_missing_file_in_a_missing_folder() -> TestResult {
        let dir = tempfile::tempdir()?;
        let folder = dir.path().join("nowhere");
        let path = folder.join("new.taskpaper");
        let error = read_document(&path).unwrap_err();
        assert!(matches!(&error, OpenError::MissingFolder { folder: f, .. } if *f == folder));
        assert_eq!(
            error.to_string(),
            format!(
                "cannot open {}: the folder {} does not exist",
                path.display(),
                folder.display()
            )
        );
        Ok(())
    }

    #[test]
    fn refuses_a_path_that_cannot_be_read() -> TestResult {
        let dir = tempfile::tempdir()?;
        let error = read_document(dir.path()).unwrap_err();
        assert!(matches!(&error, OpenError::Read { path, .. } if path == dir.path()));
        assert_eq!(
            error.to_string(),
            format!("cannot read {}", dir.path().display())
        );
        assert!(error.source().is_some());
        Ok(())
    }

    fn noon() -> DateTime {
        jiff::civil::date(2026, 10, 5).at(12, 3, 4, 0)
    }

    // FR-DOC-05, D-27, file-io.md §1
    #[test]
    fn creates_the_demo_as_a_new_temporary_file() -> TestResult {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir()?;
        let state = create_demo(dir.path(), noon())?;
        let path = dir.path().join("taskpaper-20261005-120304.taskpaper");
        assert_eq!(state.path(), Some(path.as_path()));
        assert_eq!(state.origin(), Origin::Temporary);
        assert_eq!(state.access(), Access::ReadWrite);
        let text = demo::text(noon().date());
        assert_eq!(fs::read_to_string(&path)?, text);
        assert_eq!(format::serialize(state.document()), text);
        assert_eq!(fs::metadata(&path)?.permissions().mode() & 0o777, 0o600);
        let mut state = state;
        state.update(Message::FileChecked {
            contents: Some(text.into_bytes()),
        });
        assert_eq!(state.notice(), None);
        Ok(())
    }

    #[test]
    fn a_taken_name_gets_a_number_and_stays_as_it_was() -> TestResult {
        let dir = tempfile::tempdir()?;
        let taken = dir.path().join("taskpaper-20261005-120304.taskpaper");
        fs::write(&taken, "mine")?;
        let second = create_demo(dir.path(), noon())?;
        let third = create_demo(dir.path(), noon())?;
        assert_eq!(fs::read_to_string(&taken)?, "mine");
        assert_eq!(
            second.path(),
            Some(
                dir.path()
                    .join("taskpaper-20261005-120304-2.taskpaper")
                    .as_path()
            )
        );
        assert_eq!(
            third.path(),
            Some(
                dir.path()
                    .join("taskpaper-20261005-120304-3.taskpaper")
                    .as_path()
            )
        );
        Ok(())
    }

    #[test]
    fn gives_up_when_every_name_is_taken() -> TestResult {
        let dir = tempfile::tempdir()?;
        for _ in 0..NAME_ATTEMPTS {
            create_demo(dir.path(), noon())?;
        }
        let last = dir.path().join("taskpaper-20261005-120304-100.taskpaper");
        assert!(last.exists());
        let error = create_demo(dir.path(), noon()).unwrap_err();
        assert!(matches!(
            &error,
            OpenError::Create { path, source } if *path == last && source.kind() == io::ErrorKind::AlreadyExists
        ));
        Ok(())
    }

    #[test]
    fn refuses_a_folder_that_cannot_hold_the_demo() -> TestResult {
        let dir = tempfile::tempdir()?;
        let folder = dir.path().join("nowhere");
        let error = create_demo(&folder, noon()).unwrap_err();
        let path = folder.join("taskpaper-20261005-120304.taskpaper");
        assert!(matches!(&error, OpenError::Create { path: p, .. } if *p == path));
        assert_eq!(
            error.to_string(),
            format!("cannot create {}", path.display())
        );
        assert!(error.source().is_some());
        Ok(())
    }

    // file-io.md §1
    #[test]
    fn opens_a_file_read_only_when_it_may_not_be_written() -> TestResult {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("todo.taskpaper");
        fs::write(&path, "- a\n")?;
        let state = open(&path)?;
        assert_eq!(state.access(), Access::ReadWrite);
        assert_eq!(state.path(), Some(path.as_path()));
        assert_eq!(texts(state.document()), ["- a"]);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o444))?;
        assert_eq!(open(&path)?.access(), Access::ReadOnly);
        let new = open(&dir.path().join("new.taskpaper"))?;
        assert_eq!(
            new.access(),
            Access::ReadWrite,
            "a new file will be created"
        );
        assert!(matches!(
            open(&dir.path().join("missing/x.taskpaper")),
            Err(OpenError::MissingFolder { .. })
        ));
        Ok(())
    }

    fn nine_am() -> DateTime {
        jiff::civil::date(2026, 10, 5).at(9, 0, 0, 0)
    }

    /// Backups in `dir/backups`, at 09:00 on 2026-10-05.
    fn backups(dir: &Path) -> Backups {
        Backups::new(Some(dir.join("backups")), nine_am)
    }

    /// The contents of every copy under `dir`'s backups folder.
    fn copies(dir: &Path) -> io::Result<Vec<String>> {
        let mut copies = Vec::new();
        for folder in fs::read_dir(dir.join("backups"))? {
            for file in fs::read_dir(folder?.path())? {
                let file = file?.path();
                if file
                    .extension()
                    .is_some_and(|extension| extension == "taskpaper")
                {
                    copies.push(fs::read_to_string(file)?);
                }
            }
        }
        Ok(copies)
    }

    // file-io.md §4
    #[test]
    fn recovering_opens_the_file_with_the_recovery_files_document_unsaved() -> TestResult {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("todo.taskpaper");
        fs::write(&path, "- a\n")?;
        let recovered = dir.path().join("recovered.taskpaper");
        fs::write(&recovered, "- a\n- b\n")?;
        let state = recover(&path, &recovered, &backups(dir.path()))?;
        assert_eq!(
            copies(dir.path())?,
            ["- a\n"],
            "the file it replaces is kept"
        );
        assert_eq!(state.path(), Some(path.as_path()));
        assert_eq!(texts(state.document()), ["- a", "- b"]);
        assert_eq!(state.unsaved_text().as_deref(), Some("- a\n- b\n"));
        assert_eq!(state.access(), Access::ReadWrite);
        let mut state = state;
        let effects = state.update(Message::Tick {
            now: std::time::Duration::from_secs(1),
            today: jiff::civil::Date::default(),
        });
        assert_eq!(
            effects.last(),
            Some(&Effect::Disk(Disk::Save {
                path: path.clone(),
                text: "- a\n- b\n".to_owned(),
                expected: Some(Version::of(b"- a\n")),
            }))
        );
        fs::set_permissions(&path, fs::Permissions::from_mode(0o444))?;
        assert_eq!(
            recover(&path, &recovered, &backups(dir.path()))?.access(),
            Access::ReadOnly
        );
        Ok(())
    }

    // file-io.md §4
    #[test]
    fn recovering_a_file_that_is_gone_keeps_no_copy() -> TestResult {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("todo.taskpaper");
        let recovered = dir.path().join("recovered.taskpaper");
        fs::write(&recovered, "- b\n")?;
        let state = recover(&path, &recovered, &backups(dir.path()))?;
        assert_eq!(state.unsaved_text().as_deref(), Some("- b\n"));
        assert!(!dir.path().join("backups").exists());
        Ok(())
    }

    // file-io.md §4
    #[test]
    fn recovering_refuses_when_the_file_cannot_be_kept_in_a_copy() -> TestResult {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("todo.taskpaper");
        fs::write(&path, "- a\n")?;
        let recovered = dir.path().join("recovered.taskpaper");
        fs::write(&recovered, "- b\n")?;
        let error = recover(&path, &recovered, &Backups::new(None, nine_am)).err();
        assert!(
            matches!(&error, Some(OpenError::Copy { path: p, source: BackupError::NoFolder }) if *p == path),
            "{error:?}"
        );
        assert_eq!(
            error.map(|error| error.to_string()),
            Some(format!(
                "cannot keep a copy of {} before opening it with the recovered changes",
                path.display()
            ))
        );
        Ok(())
    }

    #[test]
    fn recovering_refuses_a_recovery_file_that_is_gone_or_not_utf8() -> TestResult {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("todo.taskpaper");
        let recovered = dir.path().join("recovered.taskpaper");
        assert!(matches!(
            recover(&path, &recovered, &backups(dir.path())),
            Err(OpenError::Read { path: p, source }) if p == recovered && source.kind() == io::ErrorKind::NotFound
        ));
        fs::write(&recovered, b"caf\xe9")?;
        assert!(matches!(
            recover(&path, &recovered, &backups(dir.path())),
            Err(OpenError::Decode { path: p, .. }) if p == recovered
        ));
        Ok(())
    }
}
