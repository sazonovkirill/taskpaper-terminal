//! Opening URLs in the system browser with the platform's opener program
//! ([editing.md §5](../../../../docs/specs/editing.md#5-block-commands), FR-BLK-06).

use std::{
    ffi::OsString,
    io,
    process::{Child, Command, Stdio},
    thread,
};

/// The platform's opener program.
#[cfg(target_os = "macos")]
const OPENER: &str = "open";
/// The platform's opener program.
#[cfg(not(target_os = "macos"))]
const OPENER: &str = "xdg-open";

/// Hands URLs to an opener program, which shows each in the user's browser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Browser {
    opener: OsString,
}

impl Browser {
    /// The platform's opener: `open` on macOS, `xdg-open` elsewhere.
    #[must_use]
    pub fn system() -> Self {
        Self::new(OPENER)
    }

    /// A browser that runs the program `opener` with each URL as its only argument.
    #[must_use]
    pub fn new(opener: impl Into<OsString>) -> Self {
        Self {
            opener: opener.into(),
        }
    }

    /// Starts the opener on `url` without waiting for it. It gets no terminal input or output,
    /// which the app's screen owns.
    ///
    /// # Errors
    ///
    /// The system's error when the opener cannot be started, as when it is not installed.
    pub fn open(&self, url: &str) -> io::Result<()> {
        let mut child = self.start(url)?;
        // Waiting on a thread of its own keeps the finished opener from lingering as a zombie
        // while the app runs; how it ended has nobody to tell.
        thread::spawn(move || {
            let _ = child.wait();
        });
        Ok(())
    }

    fn start(&self, url: &str) -> io::Result<Child> {
        Command::new(&self.opener)
            .arg(url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn the_system_browser_is_the_platforms_opener() {
        assert_eq!(Browser::system().opener, OPENER);
        #[cfg(target_os = "linux")]
        assert_eq!(OPENER, "xdg-open");
    }

    // FR-BLK-06
    #[test]
    fn the_opener_gets_the_url_as_its_only_argument() -> TestResult {
        let dir = tempfile::tempdir()?;
        let name = "a b?c=1&d";
        std::fs::write(dir.path().join(name), "")?;
        let ls = Browser::new("ls");
        let finds = |path: std::path::PathBuf| -> io::Result<bool> {
            let path = path.to_str().expect("a UTF-8 temporary path");
            Ok(ls.start(path)?.wait()?.success())
        };
        assert!(finds(dir.path().join(name))?, "ls finds the file");
        assert!(!finds(dir.path().join("missing"))?, "ls misses the file");
        Ok(())
    }

    // FR-BLK-06
    #[test]
    fn opening_starts_the_opener_or_says_why_it_cannot() {
        assert!(Browser::new("true").open("https://a.b").is_ok());
        let missing = Browser::new("tpt-no-such-opener").open("https://a.b");
        assert_eq!(
            missing.map_err(|error| error.kind()),
            Err(io::ErrorKind::NotFound)
        );
    }
}
