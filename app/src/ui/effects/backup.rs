//! Backups: before the app first changes a file on a given day, a copy of the file as it is on
//! disk goes to a folder of its own under the backups folder, which keeps the newest [`KEEP`]
//! daily copies ([file-io.md §2](../../../../docs/specs/file-io.md#2-saving), FR-DOC-07); the
//! same folder keeps the newest [`KEEP`] conflict copies, the versions a conflict choice
//! discards ([file-io.md §3](../../../../docs/specs/file-io.md#3-external-changes)).

use std::{
    ffi::OsString,
    fs::{self, Permissions},
    io,
    path::{Path, PathBuf},
};

use jiff::{
    SignedDuration,
    civil::{Date, DateTime},
};
use thiserror::Error;

use super::atomic;

/// How many daily copies, and how many conflict copies, each file keeps (`backups.keep`).
pub const KEEP: usize = 30;

/// The file in a file's backup folder that names the file.
const SOURCE: &str = "source-path.txt";

/// How a conflict copy's local time of writing is written in its name.
const CONFLICT_TIME: &str = "%Y-%m-%d-%H%M%S";

/// What ends a conflict copy's name.
const CONFLICT_SUFFIX: &str = "-conflict.taskpaper";

/// Why a backup could not be made.
#[derive(Debug, Error)]
pub enum BackupError {
    /// Neither `$XDG_DATA_HOME` nor a home folder says where backups go.
    #[error("no folder for backups: neither $XDG_DATA_HOME nor a home folder is set")]
    NoFolder,
    /// Reading the file or writing the copy failed.
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// The backups folder: `$XDG_DATA_HOME/taskpaper-terminal/backups`, or under `~/.local/share`
/// when that is unset or not absolute, as the XDG base directory specification says; also on
/// macOS. `None` when neither is known.
#[must_use]
pub fn folder(xdg_data_home: Option<OsString>, home: Option<PathBuf>) -> Option<PathBuf> {
    super::app_folder(xdg_data_home, home, ".local/share", "backups")
}

/// The backups of the files one session opens: where they go, what time it is, and which file
/// was checked on which day, so each file is checked once a day.
#[derive(Debug)]
pub struct Backups {
    folder: Option<PathBuf>,
    now: fn() -> DateTime,
    checked: Option<(PathBuf, Date)>,
}

impl Backups {
    /// Backups in `folder`, dated by the local time `now` returns.
    #[must_use]
    pub fn new(folder: Option<PathBuf>, now: fn() -> DateTime) -> Self {
        Self {
            folder,
            now,
            checked: None,
        }
    }

    /// Makes sure today's copy of `file` exists, unless `file` was checked today already. A
    /// failure is not retried until the next day.
    ///
    /// # Errors
    ///
    /// [`BackupError`] when the copy could not be made.
    pub fn ensure(&mut self, file: &Path) -> Result<(), BackupError> {
        let check = (file.to_owned(), (self.now)().date());
        if self.checked.as_ref() == Some(&check) {
            return Ok(());
        }
        let (file, today) = self.checked.insert(check);
        let folder = self.folder.as_deref().ok_or(BackupError::NoFolder)?;
        Ok(daily(file, folder, *today)?)
    }

    /// Writes `contents` to a new conflict copy of `file` and returns its path.
    ///
    /// # Errors
    ///
    /// [`BackupError`] when the copy could not be written.
    pub fn conflict(&self, file: &Path, contents: &[u8]) -> Result<PathBuf, BackupError> {
        let folder = self.folder.as_deref().ok_or(BackupError::NoFolder)?;
        Ok(conflict(file, folder, (self.now)(), contents)?)
    }
}

/// Copies `file` as it is on disk to `<YYYY-MM-DD>.taskpaper`, dated `today`, in its folder under
/// `backups`, unless that copy exists, the file is missing or empty, or it equals the newest copy;
/// then drops the oldest copies beyond [`KEEP`].
fn daily(file: &Path, backups: &Path, today: Date) -> io::Result<()> {
    let bytes = match fs::read(file) {
        Ok(bytes) if !bytes.is_empty() => bytes,
        Ok(_) => return Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    let (source, permissions) = locate(file)?;
    let folder = backups.join(folder_name(&source));
    fs::create_dir_all(&folder)?;
    let copy = folder.join(daily_name(today));
    if fs::exists(&copy)? {
        return Ok(());
    }
    let mut dates = daily_copies(&folder)?;
    if let Some(&newest) = dates.first()
        && fs::read(folder.join(daily_name(newest)))? == bytes
    {
        return Ok(());
    }
    write_copy(&source, &copy, &bytes, permissions)?;
    dates.push(today);
    dates.sort_unstable_by(|a, b| b.cmp(a));
    prune(&folder, dates, daily_name)
}

/// Writes `contents` to `<YYYY-MM-DD>-<HHMMSS>-conflict.taskpaper`, named by the local time
/// `now`, or the first later second no copy is named by, in `file`'s folder under `backups`; then
/// drops the oldest conflict copies beyond [`KEEP`]. Returns the copy's path. A file gone from
/// disk still has its folder of copies.
fn conflict(file: &Path, backups: &Path, now: DateTime, contents: &[u8]) -> io::Result<PathBuf> {
    let (source, permissions) = locate(file)?;
    let folder = backups.join(folder_name(&source));
    fs::create_dir_all(&folder)?;
    let mut time = now;
    while fs::exists(folder.join(conflict_name(time)))? {
        time = time.saturating_add(SignedDuration::from_secs(1));
    }
    let copy = folder.join(conflict_name(time));
    write_copy(&source, &copy, contents, permissions)?;
    let copies = copies(&folder, parse_conflict_name)?;
    prune(&folder, copies, conflict_name)?;
    Ok(copy)
}

/// The canonical path of `file` and its permissions, or, when it is gone, the canonical path of
/// its folder joined with its name, and no permissions.
fn locate(file: &Path) -> io::Result<(PathBuf, Option<Permissions>)> {
    match fs::canonicalize(file) {
        Ok(source) => {
            let permissions = fs::metadata(&source)?.permissions();
            Ok((source, Some(permissions)))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let file = std::path::absolute(file)?;
            let folder = fs::canonicalize(file.parent().unwrap_or(&file))?;
            Ok((folder.join(file.file_name().unwrap_or_default()), None))
        }
        Err(error) => Err(error),
    }
}

/// Writes `contents` to `copy` with `permissions`, or those of a new file, recording the path
/// of `source` in the folder's [`SOURCE`] file.
fn write_copy(
    source: &Path,
    copy: &Path,
    contents: &[u8],
    permissions: Option<Permissions>,
) -> io::Result<()> {
    let folder = copy.parent().unwrap_or(copy);
    let path = source.as_os_str().as_encoded_bytes();
    atomic::replace(&folder.join(SOURCE), path, permissions.clone())?;
    atomic::replace(copy, contents, permissions)
}

/// Deletes the copies in `folder` named by `name` from `newest_first` beyond the first [`KEEP`].
fn prune<T>(folder: &Path, newest_first: Vec<T>, name: fn(T) -> String) -> io::Result<()> {
    for old in newest_first.into_iter().skip(KEEP) {
        fs::remove_file(folder.join(name(old)))?;
    }
    Ok(())
}

/// `<file name>-<hash>`, where the hash of the canonical path tells apart files with one name.
pub(super) fn folder_name(source: &Path) -> OsString {
    let mut name = source.file_name().unwrap_or_default().to_os_string();
    name.push(format!(
        "-{:016x}",
        fnv1a(source.as_os_str().as_encoded_bytes())
    ));
    name
}

/// The 64-bit FNV-1a hash, written out so folder names stay the same across Rust releases.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, &byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

fn daily_name(date: Date) -> String {
    format!("{date}.taskpaper")
}

/// The dates of the daily copies in `folder`, newest first; other files are not copies.
fn daily_copies(folder: &Path) -> io::Result<Vec<Date>> {
    copies(folder, |name| {
        let date = name.strip_suffix(".taskpaper")?.parse::<Date>().ok()?;
        (daily_name(date) == name).then_some(date)
    })
}

fn conflict_name(time: DateTime) -> String {
    format!("{}{CONFLICT_SUFFIX}", time.strftime(CONFLICT_TIME))
}

fn parse_conflict_name(name: &str) -> Option<DateTime> {
    let time = DateTime::strptime(CONFLICT_TIME, name.strip_suffix(CONFLICT_SUFFIX)?).ok()?;
    (conflict_name(time) == name).then_some(time)
}

/// What `parse` reads from the names of the copies in `folder`, newest first; files it reads
/// nothing from are not copies.
fn copies<T: Ord>(folder: &Path, parse: impl Fn(&str) -> Option<T>) -> io::Result<Vec<T>> {
    let mut found = Vec::new();
    for entry in fs::read_dir(folder)? {
        found.extend(entry?.file_name().to_str().and_then(&parse));
    }
    found.sort_unstable_by(|a, b| b.cmp(a));
    Ok(found)
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt as _;

    use jiff::civil::date;

    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    const TODAY: Date = date(2026, 10, 5);

    const NOW: DateTime = TODAY.at(9, 30, 0, 0);

    /// A file `todo.taskpaper` holding `text` and an empty backups folder, both in `dir`.
    fn setup(dir: &Path, text: &str) -> io::Result<(PathBuf, PathBuf)> {
        let file = dir.join("todo.taskpaper");
        fs::write(&file, text)?;
        let backups = dir.join("backups");
        Ok((file, backups))
    }

    /// The backup folder of `file` under `backups`.
    fn backup_folder(file: &Path, backups: &Path) -> io::Result<PathBuf> {
        Ok(backups.join(folder_name(&fs::canonicalize(file)?)))
    }

    /// The names in `folder`, sorted.
    fn names(folder: &Path) -> io::Result<Vec<String>> {
        let mut names = fs::read_dir(folder)?
            .map(|entry| Ok(entry?.file_name().to_string_lossy().into_owned()))
            .collect::<io::Result<Vec<_>>>()?;
        names.sort();
        Ok(names)
    }

    #[test]
    fn hashes_with_fnv1a() {
        assert_eq!(fnv1a(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a(b"foobar"), 0x8594_4171_f739_67e8);
    }

    // FR-DOC-07
    #[test]
    fn the_folder_is_under_xdg_data_home_else_the_home_folder() {
        let home = Some(PathBuf::from("/home/me"));
        assert_eq!(
            folder(Some("/data".into()), home.clone()),
            Some(PathBuf::from("/data/taskpaper-terminal/backups"))
        );
        let default = Some(PathBuf::from(
            "/home/me/.local/share/taskpaper-terminal/backups",
        ));
        assert_eq!(folder(None, home.clone()), default);
        assert_eq!(folder(Some("".into()), home.clone()), default);
        assert_eq!(folder(Some("data".into()), home), default);
        assert_eq!(folder(None, None), None);
    }

    // FR-DOC-07, F-FILE-09
    #[test]
    fn copies_the_file_into_a_folder_named_after_it() -> TestResult {
        let dir = tempfile::tempdir()?;
        let (file, backups) = setup(dir.path(), "- a\n")?;
        fs::set_permissions(&file, fs::Permissions::from_mode(0o600))?;
        daily(&file, &backups, TODAY)?;
        let source = fs::canonicalize(&file)?;
        let folder = backups.join(format!(
            "todo.taskpaper-{:016x}",
            fnv1a(source.as_os_str().as_encoded_bytes())
        ));
        assert_eq!(names(&folder)?, ["2026-10-05.taskpaper", SOURCE]);
        let copy = folder.join("2026-10-05.taskpaper");
        assert_eq!(fs::read_to_string(&copy)?, "- a\n");
        assert_eq!(
            fs::metadata(&copy)?.permissions().mode() & 0o777,
            0o600,
            "a copy is as private as its file"
        );
        assert_eq!(
            fs::read_to_string(folder.join(SOURCE))?,
            source.to_string_lossy()
        );
        Ok(())
    }

    #[test]
    fn files_with_one_name_in_different_folders_get_different_folders() -> TestResult {
        let dir = tempfile::tempdir()?;
        let backups = dir.path().join("backups");
        for sub in ["a", "b"] {
            fs::create_dir(dir.path().join(sub))?;
            let file = dir.path().join(sub).join("todo.taskpaper");
            fs::write(&file, sub)?;
            daily(&file, &backups, TODAY)?;
        }
        assert_eq!(names(&backups)?.len(), 2);
        Ok(())
    }

    // F-FILE-09
    #[test]
    fn keeps_the_first_copy_of_the_day_and_copies_again_the_next_day() -> TestResult {
        let dir = tempfile::tempdir()?;
        let (file, backups) = setup(dir.path(), "first")?;
        daily(&file, &backups, TODAY)?;
        fs::write(&file, "second")?;
        daily(&file, &backups, TODAY)?;
        let folder = backup_folder(&file, &backups)?;
        assert_eq!(
            fs::read_to_string(folder.join("2026-10-05.taskpaper"))?,
            "first"
        );
        daily(&file, &backups, date(2026, 10, 6))?;
        assert_eq!(
            fs::read_to_string(folder.join("2026-10-06.taskpaper"))?,
            "second"
        );
        Ok(())
    }

    #[test]
    fn skips_a_file_equal_to_its_newest_copy() -> TestResult {
        let dir = tempfile::tempdir()?;
        let (file, backups) = setup(dir.path(), "same")?;
        daily(&file, &backups, date(2026, 9, 1))?;
        daily(&file, &backups, TODAY)?;
        let folder = backup_folder(&file, &backups)?;
        assert_eq!(names(&folder)?, ["2026-09-01.taskpaper", SOURCE]);
        Ok(())
    }

    #[test]
    fn skips_a_missing_or_empty_file() -> TestResult {
        let dir = tempfile::tempdir()?;
        let (file, backups) = setup(dir.path(), "")?;
        daily(&file, &backups, TODAY)?;
        daily(&dir.path().join("new.taskpaper"), &backups, TODAY)?;
        assert!(!fs::exists(&backups)?);
        Ok(())
    }

    // FR-DOC-07
    #[test]
    fn keeps_the_newest_daily_copies_and_leaves_other_files_alone() -> TestResult {
        let dir = tempfile::tempdir()?;
        let (file, backups) = setup(dir.path(), "today")?;
        let folder = backup_folder(&file, &backups)?;
        fs::create_dir_all(&folder)?;
        let first = date(2026, 1, 1);
        for day in 0..KEEP {
            let old = first.checked_add(jiff::Span::new().days(i64::try_from(day)?))?;
            fs::write(folder.join(daily_name(old)), format!("{day}"))?;
        }
        let others = [
            "2026-01-01-093000-conflict.taskpaper",
            "notes.txt",
            "1.taskpaper",
        ];
        for other in others {
            fs::write(folder.join(other), "")?;
        }
        daily(&file, &backups, TODAY)?;
        let names = names(&folder)?;
        assert!(!names.contains(&"2026-01-01.taskpaper".to_owned()));
        assert!(names.contains(&"2026-01-02.taskpaper".to_owned()));
        assert!(names.contains(&"2026-10-05.taskpaper".to_owned()));
        assert_eq!(names.len(), KEEP + others.len() + 1, "{names:?}");
        Ok(())
    }

    #[test]
    fn fails_when_the_file_cannot_be_read_or_copied() -> TestResult {
        let dir = tempfile::tempdir()?;
        let (file, backups) = setup(dir.path(), "x")?;
        assert!(daily(dir.path(), &backups, TODAY).is_err());
        fs::write(&backups, "")?;
        assert!(daily(&file, &backups, TODAY).is_err());
        Ok(())
    }

    // FR-DOC-07
    #[test]
    fn ensure_checks_each_file_once_a_day() -> TestResult {
        let dir = tempfile::tempdir()?;
        let (file, backups) = setup(dir.path(), "")?;
        let mut session = Backups::new(Some(backups.clone()), || NOW);
        session.ensure(&file)?;
        fs::write(&file, "changed by the app")?;
        session.ensure(&file)?;
        assert!(
            !fs::exists(&backups)?,
            "the file was empty when first checked today"
        );
        let other = dir.path().join("other.taskpaper");
        fs::write(&other, "other")?;
        session.ensure(&other)?;
        assert_eq!(names(&backups)?.len(), 1);
        Ok(())
    }

    #[test]
    fn ensure_fails_once_a_day_without_a_folder() -> TestResult {
        let dir = tempfile::tempdir()?;
        let (file, _) = setup(dir.path(), "x")?;
        let mut session = Backups::new(None, || NOW);
        let error = session.ensure(&file).unwrap_err();
        assert!(matches!(error, BackupError::NoFolder));
        assert_eq!(
            error.to_string(),
            "no folder for backups: neither $XDG_DATA_HOME nor a home folder is set"
        );
        session.ensure(&file)?;
        Ok(())
    }

    // file-io.md §3
    #[test]
    fn writes_conflict_copies_named_by_the_time_beside_the_daily_ones() -> TestResult {
        let dir = tempfile::tempdir()?;
        let (file, backups) = setup(dir.path(), "on disk")?;
        fs::set_permissions(&file, fs::Permissions::from_mode(0o600))?;
        let session = Backups::new(Some(backups.clone()), || NOW);
        let first = session.conflict(&file, b"theirs")?;
        let second = session.conflict(&file, b"mine")?;
        let folder = backup_folder(&file, &backups)?;
        assert_eq!(first, folder.join("2026-10-05-093000-conflict.taskpaper"));
        assert_eq!(
            second,
            folder.join("2026-10-05-093001-conflict.taskpaper"),
            "a copy never replaces one written in the same second"
        );
        assert_eq!(fs::read_to_string(&first)?, "theirs");
        assert_eq!(fs::read_to_string(&second)?, "mine");
        assert_eq!(fs::metadata(&first)?.permissions().mode() & 0o777, 0o600);
        assert_eq!(
            fs::read_to_string(folder.join(SOURCE))?,
            fs::canonicalize(&file)?.to_string_lossy()
        );
        assert_eq!(fs::read_to_string(&file)?, "on disk");
        Ok(())
    }

    // file-io.md §2–3
    #[test]
    fn keeps_the_newest_conflict_copies_counted_apart_from_daily_ones() -> TestResult {
        let dir = tempfile::tempdir()?;
        let (file, backups) = setup(dir.path(), "x")?;
        let folder = backup_folder(&file, &backups)?;
        fs::create_dir_all(&folder)?;
        let first = date(2026, 1, 1).at(8, 0, 0, 0);
        for minute in 0..KEEP {
            let old = first.checked_add(jiff::Span::new().minutes(i64::try_from(minute)?))?;
            fs::write(folder.join(conflict_name(old)), "")?;
        }
        let others = [
            "2026-01-01.taskpaper",
            "2026-01-01-0800-conflict.taskpaper",
            "2026-01-01-080000-conflict.txt",
        ];
        for other in others {
            fs::write(folder.join(other), "")?;
        }
        conflict(&file, &backups, NOW, b"new")?;
        let names = names(&folder)?;
        assert!(!names.contains(&"2026-01-01-080000-conflict.taskpaper".to_owned()));
        assert!(names.contains(&"2026-01-01-080100-conflict.taskpaper".to_owned()));
        assert!(names.contains(&"2026-10-05-093000-conflict.taskpaper".to_owned()));
        assert_eq!(names.len(), KEEP + others.len() + 1, "{names:?}");
        Ok(())
    }

    #[test]
    fn a_conflict_copy_fails_without_a_folder_or_a_readable_file() -> TestResult {
        let dir = tempfile::tempdir()?;
        let (file, backups) = setup(dir.path(), "x")?;
        let error = Backups::new(None, || NOW)
            .conflict(&file, b"x")
            .unwrap_err();
        assert!(matches!(error, BackupError::NoFolder));
        let session = Backups::new(Some(backups), || NOW);
        let missing = session.conflict(&dir.path().join("gone/todo.taskpaper"), b"x");
        assert!(matches!(missing, Err(BackupError::Io(_))));
        let looped = dir.path().join("loop.taskpaper");
        std::os::unix::fs::symlink(&looped, &looped)?;
        let unreadable = session.conflict(&looped, b"x");
        assert!(matches!(unreadable, Err(BackupError::Io(_))));
        Ok(())
    }

    // file-io.md §3
    #[test]
    fn a_file_gone_from_disk_gets_its_conflict_copies_beside_the_others() -> TestResult {
        let dir = tempfile::tempdir()?;
        let (file, backups) = setup(dir.path(), "on disk")?;
        let folder = backup_folder(&file, &backups)?;
        fs::remove_file(&file)?;
        let session = Backups::new(Some(backups), || NOW);
        let copy = session.conflict(&file, b"mine")?;
        assert_eq!(copy, folder.join("2026-10-05-093000-conflict.taskpaper"));
        assert_eq!(fs::read_to_string(&copy)?, "mine");
        assert!(!fs::exists(&file)?);
        Ok(())
    }
}
