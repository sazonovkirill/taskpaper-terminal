//! `views.json`: the layout of each file the app opened, keyed by the file's canonical path, so
//! that the next launch of the file shows it as it was left
//! ([file-io.md §5](../../../../docs/specs/file-io.md#5-view-state-persistence), FR-UI-04, OQ-05).

use std::{
    ffi::OsString,
    fs::{self, Permissions},
    io,
    os::unix::fs::PermissionsExt as _,
    path::{Path, PathBuf},
};

use serde_json::{Map, Value};
use thiserror::Error;

use super::atomic;
use crate::app::{State, ViewState};

/// Only the user may read the file, since it holds text from the outlines of every file opened.
const MODE: u32 = 0o600;

/// Why a layout could not be kept.
#[derive(Debug, Error)]
pub enum ViewsError {
    /// Neither `$XDG_STATE_HOME` nor a home folder says where `views.json` goes.
    #[error("no place for views.json: neither $XDG_STATE_HOME nor a home folder is set")]
    NoFile,
    /// Reading or writing the file failed.
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// Where `views.json` goes: `$XDG_STATE_HOME/taskpaper-terminal/views.json`, or under
/// `~/.local/state` when that is unset or not absolute; also on macOS. `None` when neither is
/// known.
#[must_use]
pub fn file(xdg_state_home: Option<OsString>, home: Option<PathBuf>) -> Option<PathBuf> {
    super::app_folder(xdg_state_home, home, ".local/state", "views.json")
}

/// The layouts kept in one `views.json`.
#[derive(Debug, Clone)]
pub struct Views {
    file: Option<PathBuf>,
}

impl Views {
    /// The layouts kept in `file`, or none anywhere when it is `None`.
    #[must_use]
    pub fn new(file: Option<PathBuf>) -> Self {
        Self { file }
    }

    /// `state` with the layout kept for its file, when there is one to read. View state never
    /// blocks opening a file, so a missing or unreadable `views.json`, or a layout in it that
    /// cannot be read, leaves `state` as it is; so does a file with no layout to keep.
    #[must_use]
    pub fn restore(&self, state: State) -> State {
        let view = state
            .view_state()
            .and(state.path())
            .and_then(|path| self.load(path));
        match view {
            Some(view) => state.with_view_state(view),
            None => state,
        }
    }

    /// The layout kept for the file at `path`, if any can be read.
    fn load(&self, path: &Path) -> Option<ViewState> {
        let bytes = fs::read(self.file.as_ref()?).ok()?;
        let mut views: Map<String, Value> = serde_json::from_slice(&bytes).ok()?;
        serde_json::from_value(views.remove(&key(path))?).ok()
    }

    /// Keeps `view` as the layout of the file at `path`, replacing `views.json` atomically and
    /// keeping the layouts of the other files in it. A `views.json` that is not a set of layouts
    /// is replaced, as nothing in it could be read back.
    ///
    /// # Errors
    ///
    /// [`ViewsError`] when there is no place for the file or it could not be read or written.
    ///
    /// # Panics
    ///
    /// Never: a layout holds only what JSON can hold, and JSON always writes to memory.
    pub fn store(&self, path: &Path, view: &ViewState) -> Result<(), ViewsError> {
        let file = self.file.as_deref().ok_or(ViewsError::NoFile)?;
        let mut views: Map<String, Value> = match fs::read(file) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Map::new(),
            Err(error) => return Err(error.into()),
        };
        let view = serde_json::to_value(view).expect("a layout holds only what JSON can hold");
        views.insert(key(path), view);
        file.parent().map(fs::create_dir_all).transpose()?;
        let text = serde_json::to_vec_pretty(&views).expect("JSON values always write to memory");
        atomic::replace(file, &text, Some(Permissions::from_mode(MODE)))?;
        Ok(())
    }
}

/// The key of the file at `path`: its canonical path, or its absolute path while it does not
/// exist.
fn key(path: &Path) -> String {
    fs::canonicalize(path)
        .or_else(|_| std::path::absolute(path))
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::symlink;

    use super::*;
    use crate::app::{Message, Origin, PaneChange};

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    /// The file `todo.taskpaper` in `dir`, holding two projects, opened.
    fn opened(dir: &Path) -> Result<State, Box<dyn std::error::Error>> {
        let path = dir.join("todo.taskpaper");
        if !path.exists() {
            fs::write(&path, "Work:\n\t- Report\nHome:\n")?;
        }
        Ok(super::super::open(&path)?)
    }

    /// The open file with its first block opened in a second pane, which has the keys.
    fn split(dir: &Path) -> Result<State, Box<dyn std::error::Error>> {
        let mut state = opened(dir)?;
        state.update(Message::Pane(PaneChange::Open));
        Ok(state)
    }

    /// Each pane's breadcrumbs.
    fn crumbs(state: &State) -> Vec<Vec<String>> {
        state
            .panes()
            .into_iter()
            .map(|pane| pane.breadcrumbs)
            .collect()
    }

    fn views(dir: &Path) -> Views {
        Views::new(Some(dir.join("state/views.json")))
    }

    /// Keeps `state`'s layout with `views`.
    fn store(views: &Views, state: &State) -> Result<(), ViewsError> {
        let view = state.view_state().expect("a layout to keep");
        views.store(state.path().expect("a file"), &view)
    }

    // FR-UI-04, file-io.md §5
    #[test]
    fn the_file_is_under_xdg_state_home_else_the_home_folder() {
        let home = Some(PathBuf::from("/home/me"));
        assert_eq!(
            file(Some("/state".into()), home.clone()),
            Some(PathBuf::from("/state/taskpaper-terminal/views.json"))
        );
        let default = Some(PathBuf::from(
            "/home/me/.local/state/taskpaper-terminal/views.json",
        ));
        assert_eq!(file(None, home.clone()), default);
        assert_eq!(file(Some("state".into()), home), default);
        assert_eq!(file(None, None), None);
    }

    // FR-UI-04, file-io.md §5
    #[test]
    fn a_kept_layout_comes_back_on_the_next_launch_of_the_file() -> TestResult {
        let dir = tempfile::tempdir()?;
        let views = views(dir.path());
        let state = split(dir.path())?;
        store(&views, &state)?;
        let restored = views.restore(opened(dir.path())?);
        assert_eq!(crumbs(&restored), [vec![], vec!["Work".to_owned()]]);
        assert_eq!(restored.view_state(), state.view_state());
        let file = dir.path().join("state/views.json");
        assert_eq!(
            fs::metadata(&file)?.permissions().mode() & 0o777,
            MODE,
            "only the user may read it"
        );
        Ok(())
    }

    // file-io.md §5
    #[test]
    fn layouts_are_kept_by_the_files_canonical_path_in_json() -> TestResult {
        let dir = tempfile::tempdir()?;
        let views = views(dir.path());
        let state = split(dir.path())?;
        store(&views, &state)?;
        let link = dir.path().join("link.taskpaper");
        symlink(dir.path().join("todo.taskpaper"), &link)?;
        let through_link = views.restore(super::super::open(&link)?);
        assert_eq!(crumbs(&through_link).len(), 2, "the file through a link");
        let kept: Value = serde_json::from_slice(&fs::read(dir.path().join("state/views.json"))?)?;
        let canonical = fs::canonicalize(dir.path().join("todo.taskpaper"))?;
        assert_eq!(
            kept,
            serde_json::json!({
                canonical.to_string_lossy(): {
                    "panes": [
                        {"history": ["home"], "at": 0, "focus": [["Work", 0]], "width": null,
                         "show_completed": true},
                        {"history": [{"zoom": [["Work", 0]]}], "at": 0,
                         "focus": [["Work", 0], ["Report", 0]], "width": null,
                         "show_completed": true}
                    ],
                    "focused": 1,
                    "sidebar": true
                }
            })
        );
        Ok(())
    }

    // file-io.md §5
    #[test]
    fn a_new_file_is_kept_by_its_absolute_path() -> TestResult {
        let dir = tempfile::tempdir()?;
        let views = views(dir.path());
        let path = dir.path().join("new.taskpaper");
        let mut state = super::super::open(&path)?;
        state.update(Message::ToggleSidebar);
        store(&views, &state)?;
        assert_eq!(views.restore(super::super::open(&path)?).sidebar(), None);
        Ok(())
    }

    // file-io.md §5
    #[test]
    fn keeping_a_layout_keeps_those_of_other_files() -> TestResult {
        let dir = tempfile::tempdir()?;
        let views = views(dir.path());
        let other = tempfile::tempdir()?;
        store(&views, &split(other.path())?)?;
        store(&views, &opened(dir.path())?)?;
        store(&views, &split(dir.path())?)?;
        assert_eq!(views.restore(opened(other.path())?).panes().len(), 2);
        assert_eq!(views.restore(opened(dir.path())?).panes().len(), 2);
        Ok(())
    }

    // file-io.md §5: view state never blocks opening a file.
    #[test]
    fn a_missing_or_unreadable_layout_leaves_the_file_as_it_opens() -> TestResult {
        let dir = tempfile::tempdir()?;
        let views = views(dir.path());
        let single = |views: &Views| -> Result<usize, Box<dyn std::error::Error>> {
            Ok(views.restore(opened(dir.path())?).panes().len())
        };
        assert_eq!(single(&views)?, 1, "no views.json");
        assert_eq!(single(&Views::new(None))?, 1, "no place for it");
        let file = dir.path().join("state/views.json");
        fs::create_dir_all(dir.path().join("state"))?;
        for broken in ["not json", "[1, 2]", "{}"] {
            fs::write(&file, broken)?;
            assert_eq!(single(&views)?, 1, "{broken}");
        }
        let key = key(&dir.path().join("todo.taskpaper"));
        fs::write(&file, serde_json::json!({ key: {"panes": 3} }).to_string())?;
        assert_eq!(single(&views)?, 1, "a layout of another shape");
        store(&views, &split(dir.path())?)?;
        assert_eq!(single(&views)?, 2, "replaced by one that reads back");
        Ok(())
    }

    // file-io.md §5
    #[test]
    fn the_temporary_file_has_no_layout_to_restore() -> TestResult {
        let dir = tempfile::tempdir()?;
        let views = views(dir.path());
        store(&views, &split(dir.path())?)?;
        let temporary = opened(dir.path())?.with_origin(Origin::Temporary);
        assert_eq!(views.restore(temporary).panes().len(), 1);
        Ok(())
    }

    // file-io.md §5
    #[test]
    fn keeping_fails_without_a_place_or_when_the_file_cannot_be_read() -> TestResult {
        let dir = tempfile::tempdir()?;
        let state = opened(dir.path())?;
        let error = store(&Views::new(None), &state).unwrap_err();
        assert!(matches!(error, ViewsError::NoFile));
        assert_eq!(
            error.to_string(),
            "no place for views.json: neither $XDG_STATE_HOME nor a home folder is set"
        );
        let folder = dir.path().join("views.json");
        fs::create_dir(&folder)?;
        let error = store(&Views::new(Some(folder.clone())), &state).unwrap_err();
        assert!(matches!(error, ViewsError::Io(_)), "{error:?}");
        assert!(folder.is_dir(), "left as it was");
        let looped = dir.path().join("looped.json");
        symlink(&looped, &looped)?;
        let error = store(&Views::new(Some(looped.clone())), &state).unwrap_err();
        assert!(matches!(error, ViewsError::Io(_)), "{error:?}");
        assert!(
            looped.is_symlink(),
            "a file that cannot be read is not replaced"
        );
        let blocked = dir.path().join("todo.taskpaper/views.json");
        let error = store(&Views::new(Some(blocked)), &state).unwrap_err();
        assert!(matches!(error, ViewsError::Io(_)), "{error:?}");
        Ok(())
    }
}
