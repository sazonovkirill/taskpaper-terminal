//! Paths as the user types them in the Save As prompt and reads them on the status line: `~` for
//! the home folder, and completing the folder being typed
//! ([file-io.md §2](../../../../docs/specs/file-io.md#2-saving), FR-DOC-06,
//! [ui.md §1](../../../../docs/specs/ui.md#1-layout)).

use std::{
    fs,
    path::{Path, PathBuf},
};

/// The path `text` names: `~` alone or before a `/` stands for `home` when there is one, and a
/// relative path is taken from the working folder.
#[must_use]
pub fn expand(text: &str, home: Option<&Path>) -> PathBuf {
    let path = match (text.strip_prefix('~'), home) {
        (Some(""), Some(home)) => home.to_owned(),
        (Some(rest), Some(home)) if rest.starts_with('/') => {
            home.join(rest.trim_start_matches('/'))
        }
        _ => PathBuf::from(text),
    };
    std::path::absolute(&path).unwrap_or(path)
}

/// `path` as the user reads it: `~` for `home` when it is there or inside it.
#[must_use]
pub fn abbreviate(path: &Path, home: Option<&Path>) -> String {
    match home.and_then(|home| path.strip_prefix(home).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".to_owned(),
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

/// `text` with the name after its last `/` completed to the folders there whose names start
/// with it: as far as they agree, with a `/` after a single one. A name starting with `.` is
/// offered only when what is typed starts with one. `None` when nothing matches or nothing is
/// left to add.
#[must_use]
pub fn complete(text: &str, home: Option<&Path>) -> Option<String> {
    let (folder, typed) = match text.rfind('/') {
        Some(at) => text.split_at(at + 1),
        None if text == "~" => return home.map(|_| "~/".to_owned()),
        None => ("", text),
    };
    let listed = expand(if folder.is_empty() { "." } else { folder }, home);
    let names: Vec<String> = fs::read_dir(listed)
        .ok()?
        .filter_map(Result::ok)
        .filter(|entry| fs::metadata(entry.path()).is_ok_and(|metadata| metadata.is_dir()))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| {
            name.starts_with(typed) && (typed.starts_with('.') || !name.starts_with('.'))
        })
        .collect();
    let completed = match names.as_slice() {
        [] => return None,
        [name] => format!("{folder}{name}/"),
        [first, rest @ ..] => {
            let shared = rest.iter().fold(first.as_str(), |shared, name| {
                let end = shared
                    .char_indices()
                    .zip(name.chars())
                    .find(|((_, a), b)| a != b)
                    .map_or(shared.len().min(name.len()), |((at, _), _)| at);
                shared.get(..end).unwrap_or_default()
            });
            format!("{folder}{shared}")
        }
    };
    (completed != text).then_some(completed)
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    // FR-DOC-06, file-io.md §2
    #[test]
    fn a_tilde_stands_for_home_and_a_relative_path_for_one_in_the_working_folder() {
        let home = Path::new("/home/ann");
        assert_eq!(expand("~", Some(home)), home);
        assert_eq!(
            expand("~/notes/a.taskpaper", Some(home)),
            Path::new("/home/ann/notes/a.taskpaper")
        );
        assert_eq!(
            expand("/tmp/a.taskpaper", Some(home)),
            Path::new("/tmp/a.taskpaper")
        );
        assert_eq!(
            expand("~ann/a", Some(home)),
            std::path::absolute("~ann/a").unwrap(),
            "only the user's own home"
        );
        assert_eq!(
            expand("~/a", None),
            std::path::absolute("~/a").unwrap(),
            "without a home, `~` is a name"
        );
        assert_eq!(
            expand("a.taskpaper", Some(home)),
            std::env::current_dir().unwrap().join("a.taskpaper")
        );
    }

    // ui.md §1
    #[test]
    fn a_path_in_the_home_folder_reads_with_a_tilde() {
        let home = Path::new("/home/ann");
        assert_eq!(abbreviate(Path::new("/home/ann"), Some(home)), "~");
        assert_eq!(
            abbreviate(Path::new("/home/ann/notes/a.taskpaper"), Some(home)),
            "~/notes/a.taskpaper"
        );
        assert_eq!(
            abbreviate(Path::new("/home/annex/a.taskpaper"), Some(home)),
            "/home/annex/a.taskpaper",
            "a folder whose name only starts like home's"
        );
        assert_eq!(
            abbreviate(Path::new("/tmp/a.taskpaper"), Some(home)),
            "/tmp/a.taskpaper"
        );
        assert_eq!(
            abbreviate(Path::new("/home/ann/a.taskpaper"), None),
            "/home/ann/a.taskpaper",
            "without a home"
        );
    }

    /// A folder holding the folders `notes`, `novels`, `.hidden` and `n.taskpaper`, a file.
    fn folders() -> Result<tempfile::TempDir, std::io::Error> {
        let dir = tempfile::tempdir()?;
        for name in ["notes", "novels", ".hidden", "other"] {
            fs::create_dir(dir.path().join(name))?;
        }
        fs::write(dir.path().join("n.taskpaper"), "")?;
        fs::write(dir.path().join("notes.taskpaper"), "")?;
        Ok(dir)
    }

    // FR-DOC-06, file-io.md §2
    #[test]
    fn completes_the_folder_being_typed() -> TestResult {
        let dir = folders()?;
        let root = format!("{}/", dir.path().display());
        let complete = |typed: &str| complete(&format!("{root}{typed}"), None);
        assert_eq!(complete("ot"), Some(format!("{root}other/")));
        assert_eq!(
            complete("n"),
            Some(format!("{root}no")),
            "as far as they agree"
        );
        assert_eq!(complete("no"), None, "nothing left to add");
        assert_eq!(complete("notes"), Some(format!("{root}notes/")));
        assert_eq!(complete("x"), None, "nothing matches");
        assert_eq!(complete("."), Some(format!("{root}.hidden/")));
        assert_eq!(complete("missing/a"), None, "no such folder");
        assert_eq!(
            super::complete("no-such-folder-here", None),
            None,
            "a name alone is looked for in the working folder"
        );
        assert_eq!(
            complete(""),
            None,
            "the folders agree on nothing, and hidden ones are not offered"
        );
        Ok(())
    }

    // FR-DOC-06, file-io.md §2
    #[test]
    fn completes_under_home_keeping_the_tilde() -> TestResult {
        let home = folders()?;
        let home = Some(home.path());
        assert_eq!(complete("~", home), Some("~/".to_owned()));
        assert_eq!(complete("~", None), None);
        assert_eq!(
            complete("no-such-folder-here", home),
            None,
            "only `~` alone stands for home"
        );
        assert_eq!(complete("~/ot", home), Some("~/other/".to_owned()));
        assert_eq!(complete("~/no", home), None);
        Ok(())
    }

    #[test]
    fn completes_folders_with_names_that_share_a_wide_start() -> TestResult {
        let dir = tempfile::tempdir()?;
        fs::create_dir(dir.path().join("Ölstand"))?;
        fs::create_dir(dir.path().join("Ölpreis"))?;
        fs::create_dir(dir.path().join("Öl"))?;
        let root = format!("{}/", dir.path().display());
        assert_eq!(
            complete(&format!("{root}Ö"), None),
            Some(format!("{root}Öl"))
        );
        Ok(())
    }
}
