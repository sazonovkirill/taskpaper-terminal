//! Replacing a file so that it is never seen half written: the text goes to a temporary file
//! beside it, which is renamed over it ([file-io.md §2](../../../../docs/specs/file-io.md#2-saving),
//! F-FILE-04), after a last look at the file if the caller wants one
//! ([§3](../../../../docs/specs/file-io.md#3-external-changes)).

use std::{
    ffi::OsString,
    fs::{self, File, Permissions},
    io::{self, Write as _},
    os::unix::fs::PermissionsExt as _,
    path::{Path, PathBuf},
};

use tempfile::NamedTempFile;

/// What a new file gets before the umask applies, as most programs create files.
const NEW_FILE_MODE: u32 = 0o666;

/// New contents written beside the file they will replace, which is left as it was until they
/// are committed; dropping them removes them.
#[derive(Debug)]
pub struct Prepared {
    temp: NamedTempFile,
    target: PathBuf,
}

/// Writes `contents` beside the file at `path`, ready to replace it keeping its permissions; a new
/// file gets the default ones. A symbolic link stays a link: the file it points to is replaced.
///
/// # Errors
///
/// The I/O error of the step that failed; the file is left as it was.
pub fn prepare(path: &Path, contents: &[u8]) -> io::Result<Prepared> {
    let (target, permissions) = resolve(path)?;
    let target = std::path::absolute(target)?;
    let temp = write_beside(&target, contents, permissions)?;
    Ok(Prepared { temp, target })
}

impl Prepared {
    /// The absolute path of the file the contents will replace: a link's target.
    #[must_use]
    pub fn target(&self) -> &Path {
        &self.target
    }

    /// Replaces the file with the contents.
    ///
    /// # Errors
    ///
    /// The I/O error of the rename; the file is then left as it was.
    pub fn commit(self) -> io::Result<()> {
        self.temp.persist(&self.target)?;
        // Some systems refuse to sync a folder. The rename has replaced the file already; syncing
        // the folder only makes the rename survive a power cut sooner, so a refusal is no failure.
        let folder = self.target.parent().unwrap_or(&self.target);
        let _ = File::open(folder).and_then(|folder| folder.sync_all());
        Ok(())
    }
}

/// Replaces the file at `target`, which is not a link, with `contents` and the given
/// permissions, or the default ones for a new file.
///
/// # Errors
///
/// The I/O error of the step that failed; the file is then left as it was.
pub fn replace(target: &Path, contents: &[u8], permissions: Option<Permissions>) -> io::Result<()> {
    let target = std::path::absolute(target)?;
    let temp = write_beside(&target, contents, permissions)?;
    Prepared { temp, target }.commit()
}

/// Creates the file at `target`, which is not a link, with `contents` and the given
/// permissions, or the default ones, unless a file is there already, even one another process
/// creates meanwhile.
///
/// # Errors
///
/// [`io::ErrorKind::AlreadyExists`] when `target` exists, or the I/O error of the step that
/// failed; nothing is created then.
pub fn create(target: &Path, contents: &[u8], permissions: Option<Permissions>) -> io::Result<()> {
    let target = std::path::absolute(target)?;
    write_beside(&target, contents, permissions)?.persist_noclobber(&target)?;
    // A folder that refuses to sync is no failure, as in `Prepared::commit`.
    let folder = target.parent().unwrap_or(&target);
    let _ = File::open(folder).and_then(|folder| folder.sync_all());
    Ok(())
}

/// A temporary file beside `target`, an absolute path, holding `contents` with the given
/// permissions, or the default ones, synced to disk.
fn write_beside(
    target: &Path,
    contents: &[u8],
    permissions: Option<Permissions>,
) -> io::Result<NamedTempFile> {
    let folder = target.parent().unwrap_or(target);
    let mut prefix = OsString::from(".");
    prefix.push(target.file_name().unwrap_or_default());
    prefix.push(".tpt-");
    let mut builder = tempfile::Builder::new();
    builder.prefix(&prefix).rand_bytes(4);
    if permissions.is_none() {
        builder.permissions(Permissions::from_mode(NEW_FILE_MODE));
    }
    let mut temp = builder.tempfile_in(folder)?;
    temp.write_all(contents)?;
    if let Some(permissions) = permissions {
        temp.as_file().set_permissions(permissions)?;
    }
    temp.as_file().sync_all()?;
    Ok(temp)
}

/// The file `path` stands for, which is the file a symbolic link points to, with its
/// permissions, or `None` for a file that does not exist yet.
fn resolve(path: &Path) -> io::Result<(PathBuf, Option<Permissions>)> {
    match fs::canonicalize(path) {
        Ok(target) => {
            let permissions = fs::metadata(&target)?.permissions();
            Ok((target, Some(permissions)))
        }
        // Nothing is there, not even a dangling link, so the write creates the file; if the
        // path cannot hold one, creating it reports why.
        Err(_) if fs::symlink_metadata(path).is_err() => Ok((path.to_owned(), None)),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::symlink;

    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    /// Replaces the file at `path` with `contents` at once.
    fn write(path: &Path, contents: &[u8]) -> io::Result<()> {
        prepare(path, contents)?.commit()
    }

    fn mode(path: &Path) -> io::Result<u32> {
        Ok(fs::metadata(path)?.permissions().mode() & 0o7777)
    }

    /// The names in `folder`, sorted.
    fn names(folder: &Path) -> io::Result<Vec<String>> {
        let mut names = fs::read_dir(folder)?
            .map(|entry| Ok(entry?.file_name().to_string_lossy().into_owned()))
            .collect::<io::Result<Vec<_>>>()?;
        names.sort();
        Ok(names)
    }

    // F-FILE-04
    #[test]
    fn replaces_a_file_keeping_its_permissions_and_no_temporary_file() -> TestResult {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("todo.taskpaper");
        fs::write(&path, "old")?;
        fs::set_permissions(&path, Permissions::from_mode(0o640))?;
        write(&path, b"new")?;
        assert_eq!(fs::read_to_string(&path)?, "new");
        assert_eq!(mode(&path)?, 0o640);
        assert_eq!(names(dir.path())?, ["todo.taskpaper"]);
        Ok(())
    }

    // F-FILE-01
    #[test]
    fn creates_a_new_file_with_default_permissions() -> TestResult {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("new.taskpaper");
        write(&path, b"- a\n")?;
        assert_eq!(fs::read_to_string(&path)?, "- a\n");
        assert_ne!(
            mode(&path)? & 0o044,
            0,
            "readable by others unless the umask forbids it, not 0600 like a temporary file"
        );
        Ok(())
    }

    #[test]
    fn creates_a_file_only_where_none_is() -> TestResult {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("new.taskpaper");
        create(&path, b"- a\n", Some(Permissions::from_mode(0o600)))?;
        assert_eq!(fs::read_to_string(&path)?, "- a\n");
        assert_eq!(mode(&path)?, 0o600);
        let error = create(&path, b"- b\n", None).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read_to_string(&path)?, "- a\n");
        assert_eq!(
            names(dir.path())?,
            ["new.taskpaper"],
            "no temporary file is left"
        );
        Ok(())
    }

    #[test]
    fn writes_through_a_symbolic_link_and_keeps_the_link() -> TestResult {
        let dir = tempfile::tempdir()?;
        let real = dir.path().join("real.taskpaper");
        let link = dir.path().join("link.taskpaper");
        fs::write(&real, "old")?;
        symlink(&real, &link)?;
        write(&link, b"new")?;
        assert!(fs::symlink_metadata(&link)?.is_symlink());
        assert_eq!(fs::read_to_string(&real)?, "new");
        assert_eq!(names(dir.path())?, ["link.taskpaper", "real.taskpaper"]);
        Ok(())
    }

    #[test]
    fn a_failed_write_leaves_the_file_as_it_was() -> TestResult {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("todo.taskpaper");
        fs::write(&path, "old")?;
        let missing = dir.path().join("missing/todo.taskpaper");
        assert_eq!(
            write(&missing, b"new").unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
        assert_eq!(fs::read_to_string(&path)?, "old");
        Ok(())
    }

    #[test]
    fn a_dangling_link_fails() -> TestResult {
        let dir = tempfile::tempdir()?;
        let link = dir.path().join("link.taskpaper");
        symlink(dir.path().join("gone.taskpaper"), &link)?;
        assert!(write(&link, b"x").is_err());
        Ok(())
    }

    #[test]
    fn a_path_under_a_file_fails() -> TestResult {
        let dir = tempfile::tempdir()?;
        let file = dir.path().join("file");
        fs::write(&file, "")?;
        assert_eq!(
            write(&file.join("child.taskpaper"), b"x")
                .unwrap_err()
                .kind(),
            io::ErrorKind::NotADirectory
        );
        Ok(())
    }

    // file-io.md §3
    #[test]
    fn prepared_contents_replace_the_file_only_when_committed() -> TestResult {
        let dir = tempfile::tempdir()?;
        let real = dir.path().join("real.taskpaper");
        let link = dir.path().join("link.taskpaper");
        fs::write(&real, "old")?;
        fs::set_permissions(&real, Permissions::from_mode(0o640))?;
        symlink(&real, &link)?;
        let prepared = prepare(&link, b"new")?;
        assert_eq!(prepared.target(), fs::canonicalize(&real)?);
        assert_eq!(fs::read_to_string(&real)?, "old");
        assert_eq!(
            names(dir.path())?.len(),
            3,
            "the new contents wait beside it"
        );
        prepared.commit()?;
        assert_eq!(fs::read_to_string(&real)?, "new");
        assert_eq!(mode(&real)?, 0o640);
        assert_eq!(names(dir.path())?, ["link.taskpaper", "real.taskpaper"]);
        Ok(())
    }

    #[test]
    fn dropped_contents_leave_the_file_and_no_temporary_file() -> TestResult {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("todo.taskpaper");
        fs::write(&path, "theirs")?;
        drop(prepare(&path, b"mine")?);
        assert_eq!(fs::read_to_string(&path)?, "theirs");
        assert_eq!(names(dir.path())?, ["todo.taskpaper"]);
        assert!(prepare(&dir.path().join("missing/x"), b"x").is_err());
        Ok(())
    }
}
