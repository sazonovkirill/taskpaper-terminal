//! Daily backups: before the app first changes a file on a given day, a copy of the file as it is
//! on disk goes to a folder of its own under the backups folder, which keeps the newest
//! [`KEEP`] daily copies ([file-io.md §2](../../../../docs/specs/file-io.md#2-saving), FR-DOC-07).

use std::{
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
};

use jiff::civil::Date;
use thiserror::Error;

use super::atomic;

/// How many daily copies each file keeps (`backups.keep`).
pub const KEEP: usize = 30;

/// The file in a file's backup folder that names the file.
const SOURCE: &str = "source-path.txt";

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
    let data = xdg_data_home
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| home.map(|home| home.join(".local/share")))?;
    Some(data.join("taskpaper-terminal/backups"))
}

/// The daily backups of the files one session opens: where they go, what day it is, and which
/// file was checked on which day, so each file is checked once a day.
#[derive(Debug)]
pub struct Backups {
    folder: Option<PathBuf>,
    today: fn() -> Date,
    checked: Option<(PathBuf, Date)>,
}

impl Backups {
    /// Backups in `folder`, dated by the local date `today` returns.
    #[must_use]
    pub fn new(folder: Option<PathBuf>, today: fn() -> Date) -> Self {
        Self {
            folder,
            today,
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
        let check = (file.to_owned(), (self.today)());
        if self.checked.as_ref() == Some(&check) {
            return Ok(());
        }
        let (file, today) = self.checked.insert(check);
        let folder = self.folder.as_deref().ok_or(BackupError::NoFolder)?;
        Ok(daily(file, folder, *today)?)
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
    let source = fs::canonicalize(file)?;
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
    let permissions = fs::metadata(&source)?.permissions();
    let path = source.as_os_str().as_encoded_bytes();
    atomic::replace(&folder.join(SOURCE), path, Some(permissions.clone()))?;
    atomic::replace(&copy, &bytes, Some(permissions))?;
    dates.push(today);
    dates.sort_unstable_by(|a, b| b.cmp(a));
    for old in dates.into_iter().skip(KEEP) {
        fs::remove_file(folder.join(daily_name(old)))?;
    }
    Ok(())
}

/// `<file name>-<hash>`, where the hash of the canonical path tells apart files with one name.
fn folder_name(source: &Path) -> OsString {
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
    let mut dates = Vec::new();
    for entry in fs::read_dir(folder)? {
        let name = entry?.file_name();
        let date = name.to_str().and_then(|name| {
            let date = name.strip_suffix(".taskpaper")?.parse::<Date>().ok()?;
            (daily_name(date) == name).then_some(date)
        });
        dates.extend(date);
    }
    dates.sort_unstable_by(|a, b| b.cmp(a));
    Ok(dates)
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt as _;

    use jiff::civil::date;

    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    const TODAY: Date = date(2026, 10, 5);

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
        let mut session = Backups::new(Some(backups.clone()), || TODAY);
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
        let mut session = Backups::new(None, || TODAY);
        let error = session.ensure(&file).unwrap_err();
        assert!(matches!(error, BackupError::NoFolder));
        assert_eq!(
            error.to_string(),
            "no folder for backups: neither $XDG_DATA_HOME nor a home folder is set"
        );
        session.ensure(&file)?;
        Ok(())
    }
}
