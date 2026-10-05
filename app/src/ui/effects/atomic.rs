//! Replacing a file so that it is never seen half written: the text goes to a temporary file
//! beside it, which is renamed over it ([file-io.md §2](../../../../docs/specs/file-io.md#2-saving),
//! F-FILE-04).

use std::{
    ffi::OsString,
    fs::{self, File, Permissions},
    io::{self, Write as _},
    os::unix::fs::PermissionsExt as _,
    path::{Path, PathBuf},
};

/// What a new file gets before the umask applies, as most programs create files.
const NEW_FILE_MODE: u32 = 0o666;

/// Replaces the file at `path` with `contents`, keeping its permissions; a new file gets the
/// default ones. A symbolic link stays a link: the file it points to is replaced.
///
/// # Errors
///
/// The I/O error of the step that failed; the file is then left as it was.
pub fn write(path: &Path, contents: &[u8]) -> io::Result<()> {
    let (target, permissions) = resolve(path)?;
    replace(&target, contents, permissions)
}

/// Replaces the file at `target`, which is not a link, with `contents` and the given
/// permissions, or the default ones for a new file.
///
/// # Errors
///
/// The I/O error of the step that failed; the file is then left as it was.
pub fn replace(target: &Path, contents: &[u8], permissions: Option<Permissions>) -> io::Result<()> {
    let target = std::path::absolute(target)?;
    let folder = target.parent().unwrap_or(&target);
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
    temp.persist(&target)?;
    // Some systems refuse to sync a folder. The rename has replaced the file already; syncing
    // the folder only makes the rename survive a power cut sooner, so a refusal is no failure.
    let _ = File::open(folder).and_then(|folder| folder.sync_all());
    Ok(())
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
}
