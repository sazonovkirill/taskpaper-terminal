//! Recovery files: after a crash, the document with its unsaved changes goes to a file of its own
//! in the recovery folder, and the next start on the same file offers to open the file with them
//! ([file-io.md §4](../../../../docs/specs/file-io.md#4-crash-safety), N-REL-01).

use std::{
    ffi::{OsStr, OsString},
    fs,
    io::{self, BufRead, Write},
    path::{Path, PathBuf},
};

use jiff::{SignedDuration, civil::DateTime};
use thiserror::Error;

use super::{atomic, backup};

/// What the recovery file of a document without a file is named after.
const UNTITLED: &str = "untitled";

/// The local time in a recovery file's name, as in the name of a `/tmp` document.
const STAMP: &str = "%Y%m%d-%H%M%S";

/// Why a recovery file could not be written.
#[derive(Debug, Error)]
pub enum RecoveryError {
    /// Neither `$XDG_STATE_HOME` nor a home folder says where recovery files go.
    #[error("no folder for recovery files: neither $XDG_STATE_HOME nor a home folder is set")]
    NoFolder,
    /// Writing the file failed.
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// The recovery folder: `$XDG_STATE_HOME/taskpaper-terminal/recovery`, or under
/// `~/.local/state` when that is unset or not absolute; also on macOS. `None` when neither is
/// known.
#[must_use]
pub fn folder(xdg_state_home: Option<OsString>, home: Option<PathBuf>) -> Option<PathBuf> {
    super::app_folder(xdg_state_home, home, ".local/state", "recovery")
}

/// Where recovery files go and the clock that names them.
#[derive(Debug, Clone)]
pub struct Recovery {
    folder: Option<PathBuf>,
    now: fn() -> DateTime,
}

impl Recovery {
    /// Recovery files in `folder`, named with the local time `now` returns.
    #[must_use]
    pub fn new(folder: Option<PathBuf>, now: fn() -> DateTime) -> Self {
        Self { folder, now }
    }

    /// Writes `text`, the document opened from `file` or from no file, to a new recovery file
    /// `<name>-<YYYYMMDD-HHMMSS>.taskpaper`, a later second when that name is taken, and returns
    /// its path. `<name>` is the name of the file's backup folder, which tells apart files with
    /// one name, or `untitled`. The recovery file is as private as the file.
    ///
    /// # Errors
    ///
    /// [`RecoveryError`] when there is no recovery folder or writing failed.
    pub fn write(&self, file: Option<&Path>, text: &str) -> Result<PathBuf, RecoveryError> {
        let folder = self.folder.as_deref().ok_or(RecoveryError::NoFolder)?;
        let name = match file {
            Some(file) => name(file)?,
            None => OsString::from(UNTITLED),
        };
        fs::create_dir_all(folder)?;
        let permissions = file
            .and_then(|file| fs::metadata(file).ok())
            .map(|metadata| metadata.permissions());
        let mut time = (self.now)();
        loop {
            let path = folder.join(file_name(&name, time));
            let Err(error) = atomic::create(&path, text.as_bytes(), permissions.clone()) else {
                return Ok(path);
            };
            match error.kind() {
                io::ErrorKind::AlreadyExists => {
                    time = time.saturating_add(SignedDuration::from_secs(1));
                }
                _ => return Err(error.into()),
            }
        }
    }

    /// The newest recovery file of `file` written no earlier than `file` last changed, or the
    /// newest of any when `file` does not exist. Equal times count, because the clock that dates
    /// files can be too coarse to tell apart a save and a crash right after it; of two recovery
    /// files written at one time, the later name wins.
    ///
    /// # Errors
    ///
    /// The I/O error that kept the recovery folder or `file` from being read.
    pub fn find(&self, file: &Path) -> io::Result<Option<PathBuf>> {
        let Some(folder) = &self.folder else {
            return Ok(None);
        };
        let entries = match fs::read_dir(folder) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let name = name(file)?;
        // A file that does not exist, or whose time cannot be read, is older than any recovery
        // file; opening it then reports what is wrong with it.
        let changed = fs::metadata(file)
            .and_then(|metadata| metadata.modified())
            .ok();
        let mut found = Vec::new();
        for entry in entries {
            let entry = entry?;
            if is_recovery_of(&name, &entry.file_name()) {
                found.push((entry.metadata()?.modified()?, entry.path()));
            }
        }
        Ok(found
            .into_iter()
            .filter(|&(written, _)| changed.is_none_or(|changed| written >= changed))
            .max()
            .map(|(_, path)| path))
    }

    /// The recovery file to open `file` with, if the user wants one: when a recovery file of
    /// `file` is newer, asks on `ask` whether to open `file` with its changes and reads the
    /// answer, yes or no, from `answer` (file-io.md §4).
    ///
    /// # Errors
    ///
    /// The I/O error of looking for recovery files or of asking.
    pub fn offer(
        &self,
        file: &Path,
        answer: &mut dyn BufRead,
        ask: &mut dyn Write,
    ) -> io::Result<Option<PathBuf>> {
        let Some(found) = self.find(file)? else {
            return Ok(None);
        };
        write!(
            ask,
            "{} holds changes to {} that were not saved when tpt stopped.\nOpen the file with these changes? [y/N] ",
            found.display(),
            file.display()
        )?;
        ask.flush()?;
        let mut reply = String::new();
        answer.read_line(&mut reply)?;
        let yes = matches!(reply.trim().to_lowercase().as_str(), "y" | "yes");
        Ok(yes.then_some(found))
    }
}

/// What the recovery files of `file` are named after: its backup folder's name, from its
/// canonical path, or from its folder's canonical path while the file does not exist yet.
fn name(file: &Path) -> io::Result<OsString> {
    let source = fs::canonicalize(file).or_else(|error| {
        let absolute = std::path::absolute(file)?;
        match (absolute.parent(), absolute.file_name()) {
            (Some(folder), Some(name)) if error.kind() == io::ErrorKind::NotFound => {
                Ok(fs::canonicalize(folder)?.join(name))
            }
            _ => Err(error),
        }
    })?;
    Ok(backup::folder_name(&source))
}

fn file_name(name: &OsStr, time: DateTime) -> OsString {
    let mut file = name.to_owned();
    file.push(format!("-{}.taskpaper", time.strftime(STAMP)));
    file
}

/// Whether `file` is the name of a recovery file of the file named `name`.
fn is_recovery_of(name: &OsStr, file: &OsStr) -> bool {
    file.as_encoded_bytes()
        .strip_prefix(name.as_encoded_bytes())
        .and_then(|rest| str::from_utf8(rest).ok())
        .and_then(|rest| rest.strip_prefix('-')?.strip_suffix(".taskpaper"))
        .and_then(|stamp| DateTime::strptime(STAMP, stamp).ok())
        .is_some_and(|time| file_name(name, time) == file)
}

#[cfg(test)]
mod tests {
    use std::{
        fs::File,
        os::unix::fs::PermissionsExt as _,
        time::{Duration, SystemTime},
    };

    use jiff::civil::date;

    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn crash_time() -> DateTime {
        date(2026, 10, 5).at(9, 7, 3, 0)
    }

    /// Recovery files in `dir/recovery`.
    fn recovery(dir: &Path) -> Recovery {
        Recovery::new(Some(dir.join("recovery")), crash_time)
    }

    /// Sets the time `file` last changed to `seconds` after the Unix epoch.
    fn touch(file: &Path, seconds: u64) -> io::Result<()> {
        File::options()
            .append(true)
            .open(file)?
            .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(seconds))
    }

    fn names(folder: &Path) -> io::Result<Vec<String>> {
        let mut names = fs::read_dir(folder)?
            .map(|entry| Ok(entry?.file_name().to_string_lossy().into_owned()))
            .collect::<io::Result<Vec<_>>>()?;
        names.sort();
        Ok(names)
    }

    /// Asks with `recovery` whether to open `file` with a recovery file, answering `reply`;
    /// returns the recovery file accepted and the question.
    fn offer(
        recovery: &Recovery,
        file: &Path,
        reply: &str,
    ) -> io::Result<(Option<PathBuf>, String)> {
        let mut asked = Vec::new();
        let accepted = recovery.offer(file, &mut reply.as_bytes(), &mut asked)?;
        Ok((accepted, String::from_utf8_lossy(&asked).into_owned()))
    }

    // N-REL-01
    #[test]
    fn the_folder_is_under_xdg_state_home_else_the_home_folder() {
        let home = Some(PathBuf::from("/home/me"));
        assert_eq!(
            folder(Some("/state".into()), home.clone()),
            Some(PathBuf::from("/state/taskpaper-terminal/recovery"))
        );
        let default = Some(PathBuf::from(
            "/home/me/.local/state/taskpaper-terminal/recovery",
        ));
        assert_eq!(folder(None, home.clone()), default);
        assert_eq!(folder(Some("state".into()), home), default);
        assert_eq!(folder(None, None), None);
    }

    // N-REL-01
    #[test]
    fn writes_the_text_to_a_file_named_after_the_file_and_the_time() -> TestResult {
        let dir = tempfile::tempdir()?;
        let file = dir.path().join("todo.taskpaper");
        fs::write(&file, "- on disk\n")?;
        fs::set_permissions(&file, fs::Permissions::from_mode(0o600))?;
        let written = recovery(dir.path()).write(Some(&file), "- unsaved\n")?;
        let name = backup::folder_name(&fs::canonicalize(&file)?);
        let expected = format!("{}-20261005-090703.taskpaper", name.to_string_lossy());
        assert_eq!(written, dir.path().join("recovery").join(&expected));
        assert_eq!(fs::read_to_string(&written)?, "- unsaved\n");
        assert_eq!(fs::read_to_string(&file)?, "- on disk\n");
        assert_eq!(
            fs::metadata(&written)?.permissions().mode() & 0o777,
            0o600,
            "a recovery file is as private as its file"
        );
        assert_eq!(names(&dir.path().join("recovery"))?, [expected]);
        Ok(())
    }

    // file-io.md §4
    #[test]
    fn a_second_recovery_in_one_second_takes_the_next_one_and_keeps_the_first() -> TestResult {
        let dir = tempfile::tempdir()?;
        let file = dir.path().join("todo.taskpaper");
        fs::write(&file, "- on disk\n")?;
        touch(&file, 1000)?;
        let recovery = recovery(dir.path());
        let first = recovery.write(Some(&file), "- first\n")?;
        let second = recovery.write(Some(&file), "- second\n")?;
        let third = recovery.write(Some(&file), "- third\n")?;
        let name = name(&file)?.to_string_lossy().into_owned();
        for (written, stamp, text) in [
            (&first, "090703", "- first\n"),
            (&second, "090704", "- second\n"),
            (&third, "090705", "- third\n"),
        ] {
            assert_eq!(
                written.file_name(),
                Some(OsStr::new(&format!("{name}-20261005-{stamp}.taskpaper")))
            );
            assert_eq!(fs::read_to_string(written)?, text);
        }
        assert_eq!(recovery.find(&file)?, Some(third));
        Ok(())
    }

    #[test]
    fn a_document_without_a_file_or_with_a_new_one_is_recovered_too() -> TestResult {
        let dir = tempfile::tempdir()?;
        let recovery = recovery(dir.path());
        let untitled = recovery.write(None, "- a\n")?;
        assert_eq!(
            untitled.file_name(),
            Some(OsStr::new("untitled-20261005-090703.taskpaper"))
        );
        let new = dir.path().join("new.taskpaper");
        let written = recovery.write(Some(&new), "- b\n")?;
        assert_eq!(fs::read_to_string(&written)?, "- b\n");
        assert!(!new.exists(), "the file is left alone");
        assert_eq!(recovery.find(&new)?, Some(written));
        Ok(())
    }

    #[test]
    fn writing_fails_without_a_folder_or_when_it_cannot_be_made() -> TestResult {
        let dir = tempfile::tempdir()?;
        let file = dir.path().join("todo.taskpaper");
        assert!(matches!(
            Recovery::new(None, crash_time).write(Some(&file), "x"),
            Err(RecoveryError::NoFolder)
        ));
        fs::write(dir.path().join("recovery"), "a file where the folder goes")?;
        let error = recovery(dir.path()).write(Some(&file), "x").unwrap_err();
        assert!(matches!(error, RecoveryError::Io(_)), "{error:?}");
        assert_ne!(error.to_string(), "");
        let dir = tempfile::tempdir()?;
        fs::write(dir.path().join("todo.taskpaper"), "-")?;
        let inside_a_file = dir.path().join("todo.taskpaper/x");
        assert!(matches!(
            recovery(dir.path()).write(Some(&inside_a_file), "x"),
            Err(RecoveryError::Io(_))
        ));
        // A name the file system takes whose recovery file's name is too long for it: a failure
        // that is no taken name, which permissions cannot cause for root.
        let dir = tempfile::tempdir()?;
        let long = dir.path().join("x".repeat(240));
        let written = recovery(dir.path()).write(Some(&long), "x");
        assert!(
            matches!(written, Err(RecoveryError::Io(ref error)) if error.kind() == io::ErrorKind::InvalidFilename),
            "{written:?}"
        );
        assert!(
            RecoveryError::NoFolder
                .to_string()
                .starts_with("no folder for recovery files")
        );
        Ok(())
    }

    // file-io.md §4
    #[test]
    fn finds_the_newest_recovery_file_newer_than_the_file() -> TestResult {
        let dir = tempfile::tempdir()?;
        let file = dir.path().join("todo.taskpaper");
        fs::write(&file, "- a\n")?;
        touch(&file, 2000)?;
        let folder = dir.path().join("recovery");
        fs::create_dir(&folder)?;
        let name = name(&file)?.to_string_lossy().into_owned();
        let copy = |stamp: &str, seconds| -> io::Result<PathBuf> {
            let path = folder.join(format!("{name}-{stamp}.taskpaper"));
            fs::write(&path, stamp)?;
            touch(&path, seconds)?;
            Ok(path)
        };
        let recovery = recovery(dir.path());
        copy("20261001-120000", 1000)?;
        assert_eq!(recovery.find(&file)?, None, "older than the file");
        let as_old = copy("20261002-120000", 2000)?;
        assert_eq!(recovery.find(&file)?, Some(as_old), "as old as the file");
        let older = copy("20261003-120000", 3000)?;
        let newest = copy("20261004-120000", 4000)?;
        copy("20261005-120000", 2500)?;
        assert_eq!(recovery.find(&file)?, Some(newest.clone()));
        fs::remove_file(&newest)?;
        assert_eq!(recovery.find(&file)?, Some(older));
        let later_name = copy("20261003-120001", 3000)?;
        assert_eq!(
            recovery.find(&file)?,
            Some(later_name),
            "a tie goes to the later name"
        );
        Ok(())
    }

    #[test]
    fn ignores_the_recovery_files_of_other_files_and_other_names() -> TestResult {
        let dir = tempfile::tempdir()?;
        let recovery = recovery(dir.path());
        for sub in ["a", "b"] {
            fs::create_dir(dir.path().join(sub))?;
        }
        let file = dir.path().join("a/todo.taskpaper");
        let other = dir.path().join("b/todo.taskpaper");
        fs::write(&file, "- a\n")?;
        touch(&file, 1000)?;
        recovery.write(Some(&other), "- b\n")?;
        let folder = dir.path().join("recovery");
        let name = name(&file)?.to_string_lossy().into_owned();
        for odd in [
            format!("{name}-20261005-0907.taskpaper"),
            format!("{name}-20261005-090703.txt"),
            format!("{name}20261005-090703.taskpaper"),
            format!("{name}-2026105-090703.taskpaper"),
            format!("{name}-x-20261005-090703.taskpaper"),
            name.clone(),
        ] {
            fs::write(folder.join(odd), "-")?;
        }
        assert_eq!(recovery.find(&file)?, None);
        Ok(())
    }

    #[test]
    fn finds_nothing_without_a_folder() -> TestResult {
        let dir = tempfile::tempdir()?;
        let file = dir.path().join("todo.taskpaper");
        assert_eq!(Recovery::new(None, crash_time).find(&file)?, None);
        assert_eq!(recovery(dir.path()).find(&file)?, None);
        Ok(())
    }

    #[test]
    fn looking_fails_when_the_folder_or_the_file_cannot_be_read() -> TestResult {
        let dir = tempfile::tempdir()?;
        let file = dir.path().join("todo.taskpaper");
        fs::write(dir.path().join("recovery"), "a file where the folder goes")?;
        assert!(recovery(dir.path()).find(&file).is_err());
        let dir = tempfile::tempdir()?;
        recovery(dir.path()).write(None, "-")?;
        let missing = dir.path().join("missing/todo.taskpaper");
        assert!(recovery(dir.path()).find(&missing).is_err());
        assert!(
            recovery(dir.path())
                .find(&dir.path().join("missing/.."))
                .is_err()
        );
        let file = dir.path().join("todo.taskpaper/x");
        fs::write(dir.path().join("todo.taskpaper"), "-")?;
        assert!(recovery(dir.path()).find(&file).is_err());
        Ok(())
    }

    // file-io.md §4
    #[test]
    fn offers_a_newer_recovery_file_and_takes_it_on_yes() -> TestResult {
        let dir = tempfile::tempdir()?;
        let file = dir.path().join("todo.taskpaper");
        fs::write(&file, "- a\n")?;
        touch(&file, 1000)?;
        let recovery = recovery(dir.path());
        let found = recovery.write(Some(&file), "- b\n")?;
        let (accepted, asked) = offer(&recovery, &file, "y\n")?;
        assert_eq!(accepted.as_ref(), Some(&found));
        assert_eq!(
            asked,
            format!(
                "{} holds changes to {} that were not saved when tpt stopped.\nOpen the file with these changes? [y/N] ",
                found.display(),
                file.display()
            )
        );
        for yes in ["yes\n", " Y "] {
            assert_eq!(
                offer(&recovery, &file, yes)?.0.as_ref(),
                Some(&found),
                "{yes:?}"
            );
        }
        for no in ["n\n", "\n", "", "yep\n"] {
            assert_eq!(offer(&recovery, &file, no)?.0, None, "{no:?}");
        }
        Ok(())
    }

    #[test]
    fn asks_nothing_without_a_newer_recovery_file() -> TestResult {
        let dir = tempfile::tempdir()?;
        let file = dir.path().join("todo.taskpaper");
        fs::write(&file, "- a\n")?;
        assert_eq!(
            offer(&recovery(dir.path()), &file, "y\n")?,
            (None, String::new())
        );
        Ok(())
    }

    #[test]
    fn a_failed_question_is_an_error() -> TestResult {
        let dir = tempfile::tempdir()?;
        let file = dir.path().join("todo.taskpaper");
        let recovery = recovery(dir.path());
        recovery.write(Some(&file), "- b\n")?;
        let mut full: &mut [u8] = &mut [];
        let error = recovery
            .offer(&file, &mut "y\n".as_bytes(), &mut full)
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::WriteZero);
        let mut unreadable: &[u8] = b"\xff\n";
        assert!(
            recovery
                .offer(&file, &mut unreadable, &mut Vec::new())
                .is_err()
        );
        Ok(())
    }
}
