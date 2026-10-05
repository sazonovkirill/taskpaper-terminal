//! Watching the open file for changes made outside the app
//! ([file-io.md §3](../../../../docs/specs/file-io.md#3-external-changes), F-FILE-06): each burst
//! of changes in its folder becomes one [`Message::CheckFile`], and the check tells whether the
//! file itself changed.

use std::{fs, path::Path, sync::mpsc::Sender, time::Duration};

use notify_debouncer_full::{
    DebounceEventResult, Debouncer, RecommendedCache, new_debouncer,
    notify::{self, RecommendedWatcher, RecursiveMode},
};

use crate::{app::Message, ui::Input};

/// How long the folder must stay quiet before a burst of changes in it is reported.
const QUIET: Duration = Duration::from_millis(200);

/// Watches a file until dropped.
#[derive(Debug)]
pub struct Watcher {
    _debouncer: Debouncer<RecommendedWatcher, RecommendedCache>,
}

/// Watches the folder of the file at `path`, or of a link's target, and sends
/// [`Message::CheckFile`] to `inputs` after each burst of changes in it. The folder is watched
/// rather than the file, because many editors save by renaming a new file over the old one, and
/// the file may not exist yet.
///
/// # Errors
///
/// The error of the system's watcher, or why `path` cannot be made absolute.
pub fn watch(path: &Path, inputs: Sender<Input>) -> notify::Result<Watcher> {
    let file = fs::canonicalize(path).or_else(|_| std::path::absolute(path))?;
    let folder = file.parent().unwrap_or(&file);
    let mut debouncer = new_debouncer(QUIET, None, ask_for_checks(inputs))?;
    debouncer.watch(folder, RecursiveMode::NonRecursive)?;
    Ok(Watcher {
        _debouncer: debouncer,
    })
}

/// Asks `inputs` for a check of the file after each burst. Reading the file again is cheap, so
/// neither which file changed nor an error of the watcher, which may hide a change, needs
/// telling apart.
fn ask_for_checks(inputs: Sender<Input>) -> impl FnMut(DebounceEventResult) + Send + 'static {
    move |_| {
        // A closed channel means the event loop has ended, and nobody is left to tell.
        let _ = inputs.send(Input::Core(Message::CheckFile));
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    // F-FILE-06
    #[test]
    fn a_change_to_the_file_asks_for_a_check() -> TestResult {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("todo.taskpaper");
        fs::write(&path, "- a\n")?;
        let (inputs, received) = mpsc::channel();
        let _watcher = watch(&path, inputs)?;
        fs::write(&path, "- b\n")?;
        assert_eq!(
            received.recv_timeout(Duration::from_secs(10))?,
            Input::Core(Message::CheckFile)
        );
        Ok(())
    }

    #[test]
    fn watching_a_file_in_a_missing_folder_fails() {
        let (inputs, _) = mpsc::channel();
        assert!(watch(Path::new("/nonexistent-folder/todo.taskpaper"), inputs).is_err());
    }
}
