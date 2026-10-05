//! Carries out the core's I/O at the edge ([file-io.md](../../../docs/specs/file-io.md)):
//! opening files, and saving and backing them up on the disk worker.

mod atomic;
mod backup;
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

pub use backup::{BackupError, Backups, folder as backups_folder};
pub use worker::Io;

use crate::{
    app::{Access, Origin, State, demo},
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
    let document = read_document(path)?;
    Ok(State::new(document, Some(path.to_owned())).with_access(access(path)))
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
            Ok(()) => {
                Ok(State::new(format::parse(&text), Some(path)).with_origin(Origin::Temporary))
            }
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
fn app_folder(
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

/// Reads and parses the file at `path`. A missing file in an existing folder is an empty
/// document, created on the first save (F-FILE-01).
fn read_document(path: &Path) -> Result<Document, OpenError> {
    match std::fs::read(path) {
        Ok(bytes) => format::parse_bytes(&bytes).map_err(|source| OpenError::Decode {
            path: path.to_owned(),
            source,
        }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let folder = std::path::absolute(path)
                .ok()
                .and_then(|absolute| absolute.parent().map(Path::to_owned));
            match folder {
                Some(folder) if folder.is_dir() => Ok(Document::default()),
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

#[cfg(test)]
mod tests {
    use std::error::Error as _;

    use super::*;
    use crate::document::Item;

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
        let document = read_document(&path)?;
        assert_eq!(texts(&document), ["A:", "- b"]);
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
        let document = read_document(&dir.path().join("new.taskpaper"))?;
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
}
