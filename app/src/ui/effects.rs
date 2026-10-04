//! Carries out the core's I/O at the edge ([file-io.md](../../../docs/specs/file-io.md)).

use std::{
    io,
    path::{Path, PathBuf},
};

use thiserror::Error;

use crate::document::{DecodeError, Document, format};

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
    /// The file does not exist and its folder does not either, so it could never be saved.
    #[error("cannot open {}: the folder {} does not exist", path.display(), folder.display())]
    MissingFolder {
        /// The file asked for.
        path: PathBuf,
        /// Its missing parent folder.
        folder: PathBuf,
    },
}

/// Reads and parses the file at `path`. A missing file in an existing folder is an empty
/// document, created on the first save (F-FILE-01).
///
/// # Errors
///
/// [`OpenError`] when the file cannot be read, is not UTF-8, or its folder does not exist.
pub fn read_document(path: &Path) -> Result<Document, OpenError> {
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
}
