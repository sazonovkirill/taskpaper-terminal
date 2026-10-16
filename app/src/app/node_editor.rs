//! The Edit Node dialog that `e` opens: one block's type, text, completion, parent,
//! dates, star, flag and every other tag, changed in a draft and applied as one step to undo
//! ([editing.md §7](../../../docs/specs/editing.md#7-edit-node-dialog-fr-mode-09), FR-MODE-09,
//! D-103).

use std::ops::Range;

use jiff::civil::Date;

use super::{
    Location, MenuInput, Notice, State,
    finder::{Candidate, Finder, FinderView},
    picker::Picked,
    text_field::{Motion, TextEdit, TextField},
};
use crate::document::{
    BlockKind, DateTag, HiddenData, ItemId, Transaction, blocks,
    dates::{self, DateError},
    format::{Typed, breaks_line, tags},
    metadata,
    properties::{self, Properties, TagError},
    structure,
};

/// The tags with rows of their own, which the Tags rows leave out.
const OWN_ROWS: [&str; 5] = ["done", "start", "due", "star", "flag"];

/// A key of the open dialog, as an intent; its parent finder's keys are menu keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeInput {
    /// Open the dialog on the focused block (`e`).
    Open,
    /// Focus the next row.
    Next,
    /// Focus the previous row.
    Previous,
    /// A text key: it edits the Text row or an open field, and on another row a space acts on
    /// it, a character starts its field or does what the row binds it to, `←`/`→` step its value
    /// and `Backspace`/`Delete` remove it.
    Key(TextEdit),
    /// Give the next type from any row (`Ctrl+T`).
    CycleType,
    /// Accept the open field, else apply the changes and close.
    Accept,
    /// Leave the open field, else close, changing nothing.
    Cancel,
}

/// A row of the dialog, which the focus can be on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeRow {
    /// The text.
    Text,
    /// Project, task or note.
    Type,
    /// `@done`.
    Done,
    /// The block it is a child of.
    Parent,
    /// `@start`.
    Start,
    /// `@due`.
    Due,
    /// `@star`.
    Starred,
    /// `@flag`.
    Flagged,
    /// The tag at this position in [`NodeEditorView::tags`].
    Tag(usize),
    /// The row that adds a tag.
    AddTag,
}

/// The value of a date tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateValue<'a> {
    /// A date.
    Date(Date),
    /// A value that is no date, or none.
    Written(Option<&'a str>),
}

/// A tag as a Tags row shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TagView<'a> {
    /// Its name.
    pub name: &'a str,
    /// Its value, or `None` for a bare tag.
    pub value: Option<&'a str>,
    /// Whether the app reads it (FR-TAG-02).
    pub supported: bool,
}

/// What the open field would do on `Enter`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldOutcome {
    /// Set this date, or remove it when `None`; with an error, nothing, because the text is no
    /// date.
    Date(Result<Option<Date>, DateError>),
    /// Write this tag when `true`, or remove it when `false`, the text being blank; with an
    /// error, nothing.
    Tag(Result<bool, TagError>),
}

/// A field open over a row, as a frontend shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldView<'a> {
    /// The text typed.
    pub text: &'a str,
    /// The caret's byte offset into `text`.
    pub caret: usize,
    /// The selected bytes of `text`, if any.
    pub selection: Option<Range<usize>>,
    /// What `Enter` would do.
    pub outcome: FieldOutcome,
}

/// The open dialog as a frontend shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeEditorView<'a> {
    /// The rows the focus can be on, in order.
    pub rows: Vec<NodeRow>,
    /// The position of the focused row in `rows`.
    pub focus: usize,
    /// The text.
    pub text: &'a str,
    /// The caret's byte offset into `text`.
    pub caret: usize,
    /// The selected bytes of `text`, if any.
    pub selection: Option<Range<usize>>,
    /// Project, task or text.
    pub typed: Typed,
    /// The value of `@done` when it is done, `Some(None)` when bare, `None` when not done.
    pub done: Option<Option<&'a str>>,
    /// The path to the parent, its own text last, or `None` at the top level.
    pub parent: Option<Vec<String>>,
    /// `@start`, if any.
    pub start: Option<DateValue<'a>>,
    /// `@due`, if any.
    pub due: Option<DateValue<'a>>,
    /// Whether it has `@star`.
    pub starred: bool,
    /// Whether it has `@flag`.
    pub flagged: bool,
    /// Every other tag, in order.
    pub tags: Vec<TagView<'a>>,
    /// The special nodes it owns.
    pub hidden: Vec<HiddenData>,
    /// The field open over the focused row, if any.
    pub field: Option<FieldView<'a>>,
}

/// What an open field changes.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Target {
    /// This date.
    Date(DateTag),
    /// The draft's tag of this name.
    Tag(String),
    /// A new tag.
    NewTag,
}

/// The open dialog: the block, its parent and properties as they were and as changed, the
/// focused row, and the field or parent finder open over it.
#[derive(Debug, Clone)]
pub(super) struct NodeEditor {
    id: ItemId,
    parent: ItemId,
    original: Properties,
    draft: Properties,
    text: TextField,
    /// The parent chosen; the document's root for the top level.
    destination: ItemId,
    /// The document's root, which only a project can move under (D-56).
    root: ItemId,
    focus: usize,
    field: Option<(Target, TextField)>,
    finder: Option<Finder<ItemId>>,
}

/// What `Enter` would do with `text` in a field over `target`, on `today`.
fn outcome(target: &Target, text: &str, today: Date) -> FieldOutcome {
    match *target {
        Target::Date(tag) => FieldOutcome::Date(match dates::parse(text, today, tag) {
            Ok(date) => Ok(Some(date)),
            Err(DateError::Empty) => Ok(None),
            Err(error) => Err(error),
        }),
        Target::Tag(_) | Target::NewTag => {
            FieldOutcome::Tag(properties::parse_tag(text).map(|tag| tag.is_some()))
        }
    }
}

impl NodeEditor {
    /// The draft's tags the Tags rows list, in order.
    fn listed(&self) -> impl Iterator<Item = &(String, Option<String>)> {
        self.draft
            .tags
            .iter()
            .filter(|(name, _)| !OWN_ROWS.contains(&name.as_str()))
    }

    /// The rows the focus can be on.
    fn rows(&self) -> Vec<NodeRow> {
        let fixed = [
            NodeRow::Text,
            NodeRow::Type,
            NodeRow::Done,
            NodeRow::Parent,
            NodeRow::Start,
            NodeRow::Due,
            NodeRow::Starred,
            NodeRow::Flagged,
        ];
        let tags = (0..self.listed().count()).map(NodeRow::Tag);
        fixed
            .into_iter()
            .chain(tags)
            .chain([NodeRow::AddTag])
            .collect()
    }

    /// The focused row, which removing a row never leaves past the last.
    fn row(&self) -> NodeRow {
        self.rows()
            .get(self.focus)
            .copied()
            .unwrap_or(NodeRow::AddTag)
    }

    /// Moves the focus to the next row, or the previous one, stopping at the first and last.
    fn step(&mut self, forward: bool) {
        let last = self.rows().len() - 1;
        self.focus = if forward {
            (self.focus + 1).min(last)
        } else {
            self.focus.saturating_sub(1)
        };
    }

    /// Gives the type after the draft's, or before it when not `forward`: task, note, project.
    fn cycle(&mut self, forward: bool) {
        self.retype(match (self.draft.typed, forward) {
            (Typed::Task, true) | (Typed::Project, false) => Typed::Text,
            (Typed::Section | Typed::Text, true) | (Typed::Task, false) => Typed::Project,
            (Typed::Project, true) | (Typed::Section | Typed::Text, false) => Typed::Task,
        });
    }

    /// Gives the draft `typed`; a block that is no longer a project cannot stay chosen for the
    /// top level, so its parent goes back to its own (D-56).
    fn retype(&mut self, typed: Typed) {
        self.draft.typed = typed;
        if typed != Typed::Project && self.destination == self.root {
            self.destination = self.parent;
        }
    }

    /// Adds the bare tag `name`, or removes it when the draft has it.
    fn toggle(&mut self, name: &str) {
        if self.draft.tag(name).is_some() {
            self.draft.remove(name);
        } else {
            self.draft.set(name, None);
        }
    }

    /// The draft's date `tag` as a date, when its value is one.
    fn date(&self, tag: DateTag) -> Option<Date> {
        self.draft.tag(date_name(tag))??.parse().ok()
    }

    /// Moves the date `tag` a day forward, or back, from today when there is none.
    fn shift(&mut self, tag: DateTag, forward: bool, today: Date) {
        let from = self.date(tag).unwrap_or(today);
        let to = if forward {
            from.tomorrow()
        } else {
            from.yesterday()
        };
        if let Ok(to) = to
            && metadata::writable(to)
        {
            self.draft.set(date_name(tag), Some(to.to_string()));
        }
    }

    /// Opens a field over the focused row for `target`, holding `text`.
    fn open_field(&mut self, target: Target, text: TextField) {
        self.field = Some((target, text));
    }

    /// The text a field over `target` opens with on `Space`: the date as `YYYY-MM-DD`, or as
    /// written, selected; the tag without its `@`, the caret at its end.
    fn field_text(&self, target: &Target) -> TextField {
        match target {
            Target::Date(tag) => {
                let text = self
                    .draft
                    .tag(date_name(*tag))
                    .flatten()
                    .unwrap_or_default();
                TextField::new(text.to_owned()).edited(TextEdit::SelectAll)
            }
            Target::Tag(name) => TextField::new(match self.draft.tag(name).flatten() {
                Some(value) => format!("{name}({value})"),
                None => name.clone(),
            }),
            Target::NewTag => TextField::new(String::new()),
        }
    }

    /// Writes `field`, open over `target`, into the draft, or keeps it open when its text is no
    /// date or tag.
    fn accept_field(&mut self, target: Target, field: TextField, today: Date) {
        match (outcome(&target, field.text(), today), target) {
            (FieldOutcome::Date(Ok(date)), Target::Date(tag)) => match date {
                Some(date) => self.draft.set(date_name(tag), Some(date.to_string())),
                None => self.draft.remove(date_name(tag)),
            },
            (FieldOutcome::Tag(Ok(_)), Target::Tag(name)) => {
                match properties::parse_tag(field.text()).ok().flatten() {
                    Some((new, value)) => self.draft.rename(&name, &new, value),
                    None => self.draft.remove(&name),
                }
            }
            (FieldOutcome::Tag(Ok(_)), Target::NewTag) => {
                if let Ok(Some((name, value))) = properties::parse_tag(field.text()) {
                    self.draft.set(&name, value);
                }
                self.focus = self.rows().len() - 1;
            }
            (_, target) => self.field = Some((target, field)),
        }
    }

    /// Applies `key` to `row`, any row but Text and Parent, on `today`.
    fn row_key(&mut self, row: NodeRow, key: RowKey, today: Date) {
        let typed = match key {
            RowKey::Typed(c) => Some(c),
            _ => None,
        };
        match (row, key) {
            (NodeRow::Type, RowKey::Typed(' ')) => self.cycle(true),
            (NodeRow::Type, RowKey::Typed('p')) => self.retype(Typed::Project),
            (NodeRow::Type, RowKey::Typed('t')) => self.retype(Typed::Task),
            (NodeRow::Type, RowKey::Typed('n')) => self.retype(Typed::Text),
            (NodeRow::Type, RowKey::Step(forward)) => self.cycle(forward),
            (NodeRow::Done, RowKey::Typed(' ' | 'x')) => {
                if self.draft.tag("done").is_some() {
                    self.draft.remove("done");
                } else {
                    self.draft.set("done", Some(today.to_string()));
                }
            }
            (NodeRow::Start | NodeRow::Due, _) => {
                let tag = if row == NodeRow::Start {
                    DateTag::Start
                } else {
                    DateTag::Due
                };
                match key {
                    RowKey::Remove => self.draft.remove(date_name(tag)),
                    RowKey::Step(forward) => self.shift(tag, forward, today),
                    _ => open_typed(self, Some(Target::Date(tag)), typed),
                }
            }
            (NodeRow::Starred, RowKey::Typed(' ' | 's' | '*')) => self.toggle("star"),
            (NodeRow::Flagged, RowKey::Typed(' ' | '!')) => self.toggle("flag"),
            (NodeRow::Tag(at), _) => {
                let name = self.listed().nth(at).map(|(name, _)| name.clone());
                if key == RowKey::Remove {
                    if let Some(name) = name {
                        self.draft.remove(&name);
                    }
                } else {
                    open_typed(self, name.map(Target::Tag), typed);
                }
            }
            (NodeRow::AddTag, _) => open_typed(self, Some(Target::NewTag), typed),
            _ => {}
        }
    }
}

/// What a text key does on a row without text: types a character, removes the value, steps it
/// forward (`true`) or back, or nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowKey {
    Typed(char),
    Remove,
    Step(bool),
    Other,
}

impl RowKey {
    fn of(edit: TextEdit) -> Self {
        match edit {
            TextEdit::Insert(c) => Self::Typed(c),
            TextEdit::DeleteBackward | TextEdit::DeleteForward => Self::Remove,
            TextEdit::Move {
                motion: motion @ (Motion::Left | Motion::Right),
                select: false,
            } => Self::Step(motion == Motion::Right),
            _ => Self::Other,
        }
    }
}

/// The tag name of the date `tag`.
fn date_name(tag: DateTag) -> &'static str {
    match tag {
        DateTag::Start => "start",
        DateTag::Due => "due",
    }
}

impl State {
    /// The Edit Node dialog, while it is open.
    #[must_use]
    pub fn node_editor(&self) -> Option<NodeEditorView<'_>> {
        let editor = self.node_editor.as_ref()?;
        let draft = &editor.draft;
        let date = |tag: DateTag| {
            draft.tag(date_name(tag)).map(|value| {
                editor
                    .date(tag)
                    .map_or(DateValue::Written(value), DateValue::Date)
            })
        };
        let kind = match draft.typed {
            Typed::Project => BlockKind::Project,
            Typed::Task => BlockKind::Task,
            Typed::Section | Typed::Text => BlockKind::Text,
        };
        let tags = editor
            .listed()
            .map(|(name, value)| TagView {
                name,
                value: value.as_deref(),
                supported: blocks::supported(name, kind),
            })
            .collect();
        let field = editor.field.as_ref().map(|(target, field)| FieldView {
            text: field.text(),
            caret: field.caret(),
            selection: field.selection(),
            outcome: outcome(target, field.text(), self.today),
        });
        Some(NodeEditorView {
            rows: editor.rows(),
            focus: editor.focus,
            text: editor.text.text(),
            caret: editor.text.caret(),
            selection: editor.text.selection(),
            typed: draft.typed,
            done: draft.tag("done"),
            parent: blocks::block(&self.document, editor.destination)
                .map(|parent| self.path_to(&parent)),
            start: date(DateTag::Start),
            due: date(DateTag::Due),
            starred: draft.tag("star").is_some(),
            flagged: draft.tag("flag").is_some(),
            tags,
            hidden: blocks::hidden_data(&self.document, editor.id, &self.attached)
                .unwrap_or_default(),
            field,
        })
    }

    /// The dialog's parent finder, while it is open.
    #[must_use]
    pub fn node_editor_finder(&self) -> Option<FinderView<'_, ItemId>> {
        self.node_editor.as_ref()?.finder.as_ref().map(Finder::view)
    }

    /// Applies `input`: opens the dialog on the focused block, or changes, applies or closes the
    /// open one.
    pub(super) fn node_editor_input(&mut self, input: NodeInput) {
        if input == NodeInput::Open {
            self.open_node_editor();
            return;
        }
        let Some(mut editor) = self.node_editor.take() else {
            return;
        };
        let today = self.today;
        match input {
            NodeInput::Next if editor.field.is_none() => editor.step(true),
            NodeInput::Previous if editor.field.is_none() => editor.step(false),
            NodeInput::Open | NodeInput::Next | NodeInput::Previous => {}
            NodeInput::CycleType => {
                if editor.field.is_none() {
                    editor.cycle(true);
                }
            }
            NodeInput::Key(edit) => self.node_key(&mut editor, edit),
            NodeInput::Accept => {
                if let Some((target, field)) = editor.field.take() {
                    editor.accept_field(target, field, today);
                    self.node_editor = Some(editor);
                    return;
                }
                self.apply_node_editor(editor);
                self.resume_editing();
                return;
            }
            NodeInput::Cancel if editor.field.is_some() => editor.field = None,
            NodeInput::Cancel => {
                self.resume_editing();
                return;
            }
        }
        self.node_editor = Some(editor);
    }

    /// `e`: opens the dialog on the focused block, if there is one.
    fn open_node_editor(&mut self) {
        let Some((id, item)) = self
            .focused()
            .and_then(|(block, _)| Some((block.id(), self.document.get(block.id())?)))
        else {
            return;
        };
        let parent = item.parent().unwrap_or(self.document.root());
        let draft = Properties::of(item.text());
        self.node_editor = Some(NodeEditor {
            id,
            parent,
            text: TextField::new(draft.text.clone()),
            original: draft.clone(),
            draft,
            destination: parent,
            root: self.document.root(),
            focus: 0,
            field: None,
            finder: None,
        });
    }

    /// Applies the text key `edit` to the open dialog's focused row or field.
    fn node_key(&mut self, editor: &mut NodeEditor, edit: TextEdit) {
        if let TextEdit::Insert(c) = edit
            && breaks_line(c)
        {
            return;
        }
        if let Some((_, field)) = &mut editor.field {
            *field = field.edited(edit);
            return;
        }
        let row = editor.row();
        if row == NodeRow::Text {
            let text = editor.text.edited(edit);
            if tags::adds_tag(editor.text.text(), text.text()) {
                self.notify(Notice::TypedTagInNodeEditor);
            } else {
                editor.text = text;
                editor.draft.text = editor.text.text().to_owned();
            }
            return;
        }
        let key = RowKey::of(edit);
        if row == NodeRow::Parent {
            if let RowKey::Typed(c) = key {
                let mut finder = Finder::new(self.parent_candidates(editor));
                if c != ' ' {
                    finder.input(MenuInput::Type(c));
                }
                editor.finder = Some(finder);
            }
        } else {
            editor.row_key(row, key, self.today);
        }
    }

    /// The blocks the dialog's block can move under, as Move to lists them, after Home when its
    /// type is Project (D-56).
    fn parent_candidates(&self, editor: &NodeEditor) -> Vec<Candidate<ItemId>> {
        let moving: Vec<_> = blocks::block(&self.document, editor.id)
            .into_iter()
            .collect();
        let home = (editor.draft.typed == Typed::Project).then(|| Candidate {
            value: self.document.root(),
            text: "Home".into(),
            detail: "".into(),
            tier: 0,
        });
        home.into_iter()
            .chain(self.block_candidates(&moving))
            .collect()
    }

    /// Types `text` into the open dialog's field as [`TextField::pasted`] does, or into its Text
    /// row, as [`State::one_line`] gives it, as its characters would be typed,
    /// so that no tag is typed; `false` when no dialog is open. Other rows and the parent finder
    /// take no text.
    pub(super) fn node_editor_paste(&mut self, text: &str) -> bool {
        let Some(mut editor) = self.node_editor.take() else {
            return false;
        };
        if editor.finder.is_none() {
            if let Some((_, field)) = &mut editor.field {
                *field = field.pasted(text);
            } else if editor.row() == NodeRow::Text {
                for c in self.one_line(text).chars() {
                    self.node_key(&mut editor, TextEdit::Insert(c));
                }
            }
        }
        self.node_editor = Some(editor);
        true
    }

    /// Applies `input` to the dialog's parent finder: choosing makes the highlighted block the
    /// parent; `Esc` closes it.
    pub(super) fn node_editor_finder_input(&mut self, input: MenuInput) {
        let Some(editor) = &mut self.node_editor else {
            return;
        };
        let Some(closed) = editor
            .finder
            .as_mut()
            .and_then(|finder| finder.input(input))
        else {
            return;
        };
        editor.finder = None;
        if let Picked::Chosen(id) = closed {
            editor.destination = id;
        }
    }

    /// Writes the dialog's changes to its block, if it is still there, as one step to undo:
    /// its line, then the move under its new parent, following it there (editing.md §7).
    fn apply_node_editor(&mut self, editor: NodeEditor) {
        let Some(item) = self.document.get(editor.id) else {
            return;
        };
        let before = item.text().to_owned();
        let mut draft = editor.draft;
        editor.text.text().clone_into(&mut draft.text);
        let line = if draft == editor.original {
            before.clone()
        } else {
            draft.line(&before)
        };
        let mut ops = Vec::new();
        if line != before {
            ops.push(crate::document::Op::SetText {
                id: editor.id,
                before,
                after: line,
            });
        }
        let moves = editor.destination != editor.parent;
        if moves {
            let mut changed = self.document.clone();
            changed
                .apply(&mut Transaction::new(ops.clone(), (), ()))
                .expect("the op is built from the line as it is");
            let to = blocks::block(&changed, editor.destination);
            let moving: Vec<_> = blocks::block(&changed, editor.id).into_iter().collect();
            ops.extend(
                structure::move_to(&changed, &moving, to.as_ref())
                    .into_iter()
                    .flatten(),
            );
        }
        if ops.is_empty() {
            return;
        }
        let revision = self.document.revision();
        self.commit_focusing(ops, Some(editor.id));
        if moves && self.document.revision() != revision {
            self.show_moved(editor.destination, Some(editor.id));
        }
    }

    /// Shows blocks that moved under the line `to`, a block or the document's root, with the
    /// focus on `focus`: `to` and the blocks above it in the zoom expand, and a `to` outside the
    /// zoom becomes the zoom, the root being home (FR-OPS-05).
    pub(super) fn show_moved(&mut self, to: ItemId, focus: Option<ItemId>) {
        let home = to == self.document.root();
        let above: Vec<ItemId> =
            std::iter::successors(Some(to), |&line| self.document.get(line)?.parent()).collect();
        let zoom = self.zoom();
        if zoom.is_none_or(|zoom| above.contains(&zoom)) {
            for line in above.iter().take_while(|&&line| Some(line) != zoom) {
                self.folds.remove(*line);
            }
        } else {
            self.go_to(Location::Outline((!home).then_some(to)));
            self.pane.focus = focus;
        }
    }
}

/// Opens a field over `target`, if any, on `Space`, holding its value, or on another
/// character, empty with that character typed.
fn open_typed(editor: &mut NodeEditor, target: Option<Target>, typed: Option<char>) {
    match (target, typed) {
        (Some(target), Some(' ')) => {
            let text = editor.field_text(&target);
            editor.open_field(target, text);
        }
        (Some(target), Some(c)) => editor.open_field(target, TextField::new(c.to_string())),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use jiff::civil::date;

    use super::*;
    use crate::{
        app::{Access, Message, Paste},
        document::{Document, format},
    };

    const TODAY: Date = date(2026, 10, 5);

    /// `text` on `TODAY` with the focus moved down `index` rows.
    fn open(text: &str, index: usize) -> State {
        let mut state = State::new(format::parse(text), None);
        state.update(Message::Tick {
            now: Duration::ZERO,
            today: TODAY,
        });
        for _ in 0..index {
            state.update(Message::FocusNext);
        }
        state
    }

    /// `text` with the dialog open on the block `index` rows down.
    fn dialog(text: &str, index: usize) -> State {
        let mut state = open(text, index);
        input(&mut state, NodeInput::Open);
        state
    }

    fn input(state: &mut State, input: NodeInput) {
        assert_eq!(state.update(Message::NodeEditor(input)), []);
    }

    /// Types `text`, each character a key.
    fn typed(state: &mut State, text: &str) {
        for c in text.chars() {
            input(state, NodeInput::Key(TextEdit::Insert(c)));
        }
    }

    fn key(state: &mut State, edit: TextEdit) {
        input(state, NodeInput::Key(edit));
    }

    fn arrow(state: &mut State, motion: Motion) {
        key(
            state,
            TextEdit::Move {
                motion,
                select: false,
            },
        );
    }

    /// Focuses `row`, which must be listed.
    fn focus(state: &mut State, row: NodeRow) {
        let rows = state.node_editor().unwrap().rows;
        let at = rows.iter().position(|&each| each == row).unwrap();
        for _ in 0..rows.len() {
            input(state, NodeInput::Previous);
        }
        for _ in 0..at {
            input(state, NodeInput::Next);
        }
        assert_eq!(view(state).rows[view(state).focus], row);
    }

    fn view(state: &State) -> NodeEditorView<'_> {
        state.node_editor().unwrap()
    }

    /// Each line's text.
    fn texts(state: &State) -> Vec<String> {
        let document = state.document();
        document
            .descendants(document.root())
            .map(|(id, depth)| {
                format!("{}{}", "\t".repeat(depth), document.get(id).unwrap().text())
            })
            .collect()
    }

    /// Applies the dialog's changes and returns each line's text.
    fn applied(state: &mut State) -> Vec<String> {
        input(state, NodeInput::Accept);
        assert_eq!(state.node_editor(), None);
        texts(state)
    }

    // editing.md §7, D-3, FR-TAG-02, FR-SPC-01
    #[test]
    fn it_opens_on_the_focused_block_showing_every_property() {
        let state = dialog(
            "P:\n\t- Milk @due(2026-10-08) @done(2026-10-01) @x @start(soon) @star @flag @section\n\t\tLOG @hidden @log\n\t\t\tl",
            1,
        );
        let shown = view(&state);
        assert_eq!(shown.text, "Milk");
        assert_eq!(shown.caret, 4, "the caret at the end");
        assert_eq!(shown.typed, Typed::Task);
        assert_eq!(shown.done, Some(Some("2026-10-01")));
        assert_eq!(shown.parent, Some(vec!["P".to_owned()]));
        assert_eq!(shown.start, Some(DateValue::Written(Some("soon"))));
        assert_eq!(shown.due, Some(DateValue::Date(date(2026, 10, 8))));
        assert!(shown.starred && shown.flagged);
        assert_eq!(
            shown.tags,
            [
                TagView {
                    name: "x",
                    value: None,
                    supported: false
                },
                TagView {
                    name: "section",
                    value: None,
                    supported: false
                },
            ],
            "only the tags without rows of their own; @section is a note's"
        );
        assert_eq!(
            shown.rows,
            [
                NodeRow::Text,
                NodeRow::Type,
                NodeRow::Done,
                NodeRow::Parent,
                NodeRow::Start,
                NodeRow::Due,
                NodeRow::Starred,
                NodeRow::Flagged,
                NodeRow::Tag(0),
                NodeRow::Tag(1),
                NodeRow::AddTag,
            ]
        );
        assert_eq!(shown.focus, 0);
        assert_eq!(
            shown.hidden,
            [HiddenData {
                tags: vec!["@hidden".to_owned(), "@log".to_owned()],
                lines: 2,
            }]
        );
        assert_eq!(shown.field, None);
        let state = dialog("Work: @section\n- a", 0);
        let shown = view(&state);
        assert_eq!(
            (shown.typed, shown.text, shown.parent),
            (Typed::Project, "Work", None)
        );
        assert_eq!((shown.start, shown.due, shown.done), (None, None, None));
        assert!(!shown.starred && !shown.flagged);
        assert_eq!(shown.hidden, []);
        assert!(!shown.tags[0].supported, "a project's @section is not read");
        let state = dialog("Later @section", 0);
        assert!(view(&state).tags[0].supported);
    }

    #[test]
    fn nothing_opens_in_an_empty_document_and_input_without_it_does_nothing() {
        let mut state = State::new(Document::default(), None);
        input(&mut state, NodeInput::Open);
        assert_eq!(state.node_editor(), None);
        let mut state = open("- a", 0);
        for each in [
            NodeInput::Next,
            NodeInput::Key(TextEdit::Insert('x')),
            NodeInput::Accept,
            NodeInput::Cancel,
        ] {
            input(&mut state, each);
        }
        state.update(Message::Menu(
            crate::app::Menu::NodeEditor,
            MenuInput::Choose,
        ));
        assert_eq!(texts(&state), ["- a"]);
        assert_eq!(state.node_editor(), None);
        assert_eq!(state.node_editor_finder(), None);
    }

    // editing.md §7
    #[test]
    fn next_and_previous_stop_at_the_first_and_last_row() {
        let mut state = dialog("- a", 0);
        input(&mut state, NodeInput::Previous);
        assert_eq!(view(&state).focus, 0);
        for _ in 0..20 {
            input(&mut state, NodeInput::Next);
        }
        assert_eq!(view(&state).rows[view(&state).focus], NodeRow::AddTag);
        input(&mut state, NodeInput::Previous);
        assert_eq!(view(&state).rows[view(&state).focus], NodeRow::Flagged);
    }

    // editing.md §7, §2
    #[test]
    fn the_text_row_edits_the_text_but_takes_no_tag_or_line_break() {
        let mut state = dialog("- a @x", 0);
        typed(&mut state, " b\n");
        key(&mut state, TextEdit::DeleteBackward);
        typed(&mut state, "c");
        assert_eq!(view(&state).text, "a c");
        typed(&mut state, " @");
        assert_eq!(view(&state).text, "a c ", "the @ is not typed");
        typed(&mut state, "y");
        assert_eq!(view(&state).text, "a c y");
        assert_eq!(state.notice(), Some(&Notice::TypedTagInNodeEditor));
        assert_eq!(applied(&mut state), ["- a c y @x"]);
    }

    // editing.md §7
    #[test]
    fn the_type_row_and_ctrl_t_change_the_type() {
        let mut state = dialog("- a", 0);
        input(&mut state, NodeInput::CycleType);
        assert_eq!(view(&state).typed, Typed::Text, "from the Text row too");
        focus(&mut state, NodeRow::Type);
        let mut seen = Vec::new();
        for c in " tnpq".chars() {
            typed(&mut state, &c.to_string());
            seen.push(view(&state).typed);
        }
        arrow(&mut state, Motion::Right);
        seen.push(view(&state).typed);
        arrow(&mut state, Motion::Left);
        arrow(&mut state, Motion::Left);
        seen.push(view(&state).typed);
        key(&mut state, TextEdit::DeleteBackward);
        seen.push(view(&state).typed);
        assert_eq!(
            seen,
            [
                Typed::Project,
                Typed::Task,
                Typed::Text,
                Typed::Project,
                Typed::Project,
                Typed::Task,
                Typed::Text,
                Typed::Text,
            ]
        );
        assert_eq!(applied(&mut state), ["a"]);
    }

    // editing.md §7, D-4
    #[test]
    fn the_done_row_adds_done_with_today_or_removes_it() {
        let mut state = dialog("- a @done", 0);
        focus(&mut state, NodeRow::Done);
        typed(&mut state, "y");
        assert_eq!(view(&state).done, Some(None));
        typed(&mut state, "x");
        assert_eq!(view(&state).done, None);
        typed(&mut state, " ");
        assert_eq!(view(&state).done, Some(Some("2026-10-05")));
        assert_eq!(applied(&mut state), ["- a @done(2026-10-05)"]);
    }

    // editing.md §7
    #[test]
    fn star_and_flag_toggle() {
        let mut state = dialog("- a @star", 0);
        focus(&mut state, NodeRow::Starred);
        typed(&mut state, "*y");
        assert!(!view(&state).starred);
        typed(&mut state, " ");
        assert!(view(&state).starred);
        typed(&mut state, "s");
        assert!(!view(&state).starred, "s as in navigate mode (D-130)");
        typed(&mut state, "*");
        assert!(view(&state).starred);
        focus(&mut state, NodeRow::Flagged);
        typed(&mut state, "!y");
        assert!(view(&state).flagged);
        typed(&mut state, " ");
        assert!(!view(&state).flagged);
        typed(&mut state, "!");
        assert_eq!(applied(&mut state), ["- a @star @flag"]);
    }

    // editing.md §7, §5
    #[test]
    fn a_date_field_takes_what_the_date_prompt_takes() {
        let mut state = dialog("- a @due(2026-10-08)", 0);
        focus(&mut state, NodeRow::Start);
        typed(&mut state, "tomorrow");
        let field = view(&state).field.unwrap();
        assert_eq!(field.text, "tomorrow");
        assert_eq!(
            field.outcome,
            FieldOutcome::Date(Ok(Some(date(2026, 10, 6))))
        );
        input(&mut state, NodeInput::Next);
        input(&mut state, NodeInput::Previous);
        input(&mut state, NodeInput::CycleType);
        assert_eq!(view(&state).typed, Typed::Task, "a field keeps the keys");
        assert_eq!(view(&state).rows[view(&state).focus], NodeRow::Start);
        input(&mut state, NodeInput::Accept);
        assert_eq!(view(&state).field, None);
        assert_eq!(view(&state).start, Some(DateValue::Date(date(2026, 10, 6))));
        focus(&mut state, NodeRow::Due);
        typed(&mut state, " ");
        let field = view(&state).field.unwrap();
        assert_eq!(
            (field.text, field.selection),
            ("2026-10-08", Some(0..10)),
            "Space opens it on the date, selected"
        );
        typed(&mut state, "zz");
        assert!(matches!(
            view(&state).field.unwrap().outcome,
            FieldOutcome::Date(Err(_))
        ));
        input(&mut state, NodeInput::Accept);
        assert!(view(&state).field.is_some(), "no date, so it stays open");
        key(&mut state, TextEdit::SelectAll);
        key(&mut state, TextEdit::DeleteBackward);
        assert_eq!(
            view(&state).field.unwrap().outcome,
            FieldOutcome::Date(Ok(None))
        );
        input(&mut state, NodeInput::Accept);
        assert_eq!(view(&state).due, None, "an empty field removes it");
        typed(&mut state, "1");
        input(&mut state, NodeInput::Cancel);
        assert_eq!(view(&state).field, None, "Esc leaves the field");
        assert!(state.node_editor().is_some(), "the dialog stays");
        assert_eq!(applied(&mut state), ["- a @start(2026-10-06)"]);
    }

    // editing.md §7, D-26
    #[test]
    fn arrows_move_a_date_a_day_and_delete_removes_it() {
        let mut state = dialog("- a @start(2026-10-08) @due(soon)", 0);
        focus(&mut state, NodeRow::Start);
        arrow(&mut state, Motion::Right);
        assert_eq!(view(&state).start, Some(DateValue::Date(date(2026, 10, 9))));
        arrow(&mut state, Motion::Left);
        arrow(&mut state, Motion::Left);
        assert_eq!(view(&state).start, Some(DateValue::Date(date(2026, 10, 7))));
        key(
            &mut state,
            TextEdit::Move {
                motion: Motion::Left,
                select: true,
            },
        );
        key(&mut state, TextEdit::DeleteForward);
        assert_eq!(view(&state).start, None);
        focus(&mut state, NodeRow::Due);
        arrow(&mut state, Motion::Left);
        assert_eq!(
            view(&state).due,
            Some(DateValue::Date(date(2026, 10, 4))),
            "from today when the value is no date"
        );
        assert_eq!(applied(&mut state), ["- a @due(2026-10-04)"]);
        let mut state = dialog("- a @due(9999-12-31)", 0);
        focus(&mut state, NodeRow::Due);
        arrow(&mut state, Motion::Right);
        assert_eq!(
            view(&state).due,
            Some(DateValue::Date(date(9999, 12, 31))),
            "no further than a date can be written"
        );
    }

    // editing.md §7
    #[test]
    fn a_tag_row_edits_renames_or_removes_its_tag() {
        let mut state = dialog("- a @x(1) b @y @z", 0);
        focus(&mut state, NodeRow::Tag(0));
        typed(&mut state, " ");
        let field = view(&state).field.unwrap();
        assert_eq!((field.text, field.caret), ("x(1)", 4));
        assert_eq!(field.outcome, FieldOutcome::Tag(Ok(true)));
        key(&mut state, TextEdit::SelectAll);
        typed(&mut state, "w(2)");
        input(&mut state, NodeInput::Accept);
        focus(&mut state, NodeRow::Tag(1));
        typed(&mut state, " ");
        input(&mut state, NodeInput::Accept);
        assert_eq!(
            view(&state)
                .tags
                .iter()
                .map(|tag| tag.name)
                .collect::<Vec<_>>(),
            ["w", "y", "z"],
            "renamed where it stood, an unchanged one left"
        );
        typed(&mut state, "z(3)");
        input(&mut state, NodeInput::Accept);
        assert_eq!(
            view(&state)
                .tags
                .iter()
                .map(|tag| (tag.name, tag.value))
                .collect::<Vec<_>>(),
            [("w", Some("2")), ("z", Some("3"))],
            "renaming to a name it has merges the two"
        );
        assert_eq!(view(&state).rows[view(&state).focus], NodeRow::Tag(1));
        key(&mut state, TextEdit::DeleteBackward);
        assert_eq!(view(&state).rows[view(&state).focus], NodeRow::AddTag);
        focus(&mut state, NodeRow::Tag(0));
        typed(&mut state, " ");
        key(&mut state, TextEdit::SelectAll);
        key(&mut state, TextEdit::DeleteBackward);
        input(&mut state, NodeInput::Accept);
        assert!(view(&state).tags.is_empty(), "an emptied field removes it");
        assert_eq!(applied(&mut state), ["- a b"]);
    }

    // editing.md §7
    #[test]
    fn add_tag_adds_one_or_sets_the_value_of_one_the_block_has() {
        let mut state = dialog("- a @x", 0);
        focus(&mut state, NodeRow::AddTag);
        arrow(&mut state, Motion::Right);
        assert_eq!(view(&state).field, None);
        typed(&mut state, "@y(1)");
        input(&mut state, NodeInput::Accept);
        assert_eq!(
            view(&state).rows[view(&state).focus],
            NodeRow::AddTag,
            "the focus stays for the next tag"
        );
        typed(&mut state, " ");
        assert_eq!(view(&state).field.unwrap().text, "", "it opens empty");
        typed(&mut state, "x(2)");
        input(&mut state, NodeInput::Accept);
        typed(&mut state, "due(today)");
        input(&mut state, NodeInput::Accept);
        assert_eq!(
            view(&state).due,
            Some(DateValue::Written(Some("today"))),
            "a name with a row of its own changes that row"
        );
        typed(&mut state, "@");
        assert_eq!(
            view(&state).field.unwrap().outcome,
            FieldOutcome::Tag(Ok(false)),
            "a lone @ is no tag, so Enter adds none"
        );
        key(&mut state, TextEdit::DeleteBackward);
        typed(&mut state, "a b");
        assert_eq!(
            view(&state).field.unwrap().outcome,
            FieldOutcome::Tag(Err(TagError::Name))
        );
        input(&mut state, NodeInput::Accept);
        assert!(view(&state).field.is_some(), "no tag, so it stays open");
        input(&mut state, NodeInput::Cancel);
        typed(&mut state, " ");
        input(&mut state, NodeInput::Accept);
        assert_eq!(view(&state).field, None, "a blank field adds nothing");
        assert_eq!(applied(&mut state), ["- a @x(2) @y(1) @due(today)"]);
    }

    // editing.md §7, D-56, FR-OPS-05, FR-OPS-08
    #[test]
    fn the_parent_finder_moves_the_block_with_its_subtree_as_one_step() {
        let text = "A:\n\t- x\nB:\n\t- y\n\t\t- z";
        let mut state = dialog(text, 3);
        focus(&mut state, NodeRow::Parent);
        arrow(&mut state, Motion::Right);
        assert_eq!(
            state.node_editor_finder(),
            None,
            "only a character opens it"
        );
        typed(&mut state, " ");
        let listed: Vec<_> = state
            .node_editor_finder()
            .unwrap()
            .entries
            .iter()
            .map(|entry| entry.text.to_string())
            .collect();
        assert_eq!(listed, ["A", "B", "x"], "no Home for a task");
        state.update(Message::Menu(
            crate::app::Menu::NodeEditor,
            MenuInput::Dismiss,
        ));
        assert_eq!(state.node_editor_finder(), None);
        assert_eq!(view(&state).parent, Some(vec!["B".to_owned()]));
        typed(&mut state, "x");
        for menu in [MenuInput::Erase, MenuInput::Type('A')] {
            state.update(Message::Menu(crate::app::Menu::NodeEditor, menu));
        }
        assert_eq!(state.node_editor_finder().unwrap().query, "A");
        state.update(Message::Menu(
            crate::app::Menu::NodeEditor,
            MenuInput::Choose,
        ));
        assert_eq!(state.node_editor_finder(), None);
        assert_eq!(view(&state).parent, Some(vec!["A".to_owned()]));
        typed(&mut state, "e");
        assert_eq!(view(&state).text, "y", "a key the finder took is not typed");
        state.update(Message::Menu(
            crate::app::Menu::NodeEditor,
            MenuInput::Dismiss,
        ));
        input(&mut state, NodeInput::CycleType);
        input(&mut state, NodeInput::CycleType);
        assert_eq!(view(&state).typed, Typed::Project);
        typed(&mut state, " ");
        assert_eq!(
            state.node_editor_finder().unwrap().entries[0].value,
            state.document().root(),
            "Home first for a project"
        );
        state.update(Message::Menu(
            crate::app::Menu::NodeEditor,
            MenuInput::Dismiss,
        ));
        input(&mut state, NodeInput::CycleType);
        assert_eq!(
            applied(&mut state),
            ["A:", "\t- x", "\t- y", "\t\t- z", "B:"]
        );
        assert_eq!(state.focus(), Some(state.rows()[2].id));
        state.update(Message::Undo);
        assert_eq!(texts(&state), text.split('\n').collect::<Vec<_>>());
    }

    // editing.md §7, FR-OPS-05
    #[test]
    fn a_retyped_block_moved_out_of_the_zoom_is_followed_there() {
        let mut state = open("A:\n\t- x\nB:\n\t- y", 2);
        state.update(Message::ZoomIn);
        state.update(Message::FocusNext);
        input(&mut state, NodeInput::Open);
        typed(&mut state, "!");
        input(&mut state, NodeInput::CycleType);
        input(&mut state, NodeInput::CycleType);
        focus(&mut state, NodeRow::Parent);
        typed(&mut state, "A");
        state.update(Message::Menu(
            crate::app::Menu::NodeEditor,
            MenuInput::Choose,
        ));
        assert_eq!(applied(&mut state), ["A:", "\t- x", "\ty!:", "B:"]);
        let a = state.rows()[0].id;
        assert_eq!(state.zoom(), Some(a), "A becomes the zoom");
        let moved = state.document().get(a).unwrap().children()[1];
        assert_eq!(state.focus(), Some(moved));
    }

    // editing.md §7
    #[test]
    fn nothing_changed_changes_nothing() {
        let mut state = dialog("- a  @x b", 0);
        typed(&mut state, "c");
        key(&mut state, TextEdit::DeleteBackward);
        let revision = state.document().revision();
        assert_eq!(applied(&mut state), ["- a  @x b"]);
        assert_eq!(state.document().revision(), revision);
        let mut state = dialog("- a", 0);
        typed(&mut state, "b");
        input(&mut state, NodeInput::Cancel);
        assert_eq!(texts(&state), ["- a"], "Esc changes nothing");
    }

    // file-io.md §1, editing.md §7
    #[test]
    fn a_read_only_file_shows_it_but_refuses_the_change() {
        let mut state = open("- a", 0).with_access(Access::ReadOnly);
        input(&mut state, NodeInput::Open);
        typed(&mut state, "b");
        assert_eq!(applied(&mut state), ["- a"]);
        assert_eq!(state.notice(), Some(&Notice::ReadOnly));
        let mut state = open("A:\n\t- x\nB:", 0).with_access(Access::ReadOnly);
        state.update(Message::ZoomIn);
        let a = state.zoom();
        state.update(Message::FocusNext);
        input(&mut state, NodeInput::Open);
        focus(&mut state, NodeRow::Parent);
        typed(&mut state, "B");
        state.update(Message::Menu(
            crate::app::Menu::NodeEditor,
            MenuInput::Choose,
        ));
        assert_eq!(applied(&mut state), ["A:", "\t- x", "B:"]);
        assert_eq!(state.zoom(), a, "a refused move is not followed");
    }

    #[test]
    fn a_block_gone_meanwhile_is_not_changed() {
        let mut state = dialog("- a\n- b", 0);
        typed(&mut state, "c");
        let mut editor = state.node_editor.take().unwrap();
        state.update(Message::Delete);
        editor.destination = editor.parent;
        state.node_editor = Some(editor);
        assert_eq!(applied(&mut state), ["- b"]);
    }

    // FR-OPS-06, editing.md §7
    #[test]
    fn pasted_text_goes_to_the_text_row_or_the_open_field_only() {
        let paste = |state: &mut State, text: &str| {
            state.update(Message::Pasted {
                text: text.to_owned(),
                at: Paste::Below,
            });
        };
        let mut state = open("- x @star\n- a", 0);
        state.update(Message::Copy);
        state.update(Message::FocusNext);
        input(&mut state, NodeInput::Open);
        paste(&mut state, "- x @star\n");
        assert_eq!(view(&state).text, "ax", "the copied block's text");
        let mut state = dialog("- a", 0);
        paste(&mut state, " b\r\nc\n");
        assert_eq!(view(&state).text, "a b c", "its lines joined with spaces");
        focus(&mut state, NodeRow::Done);
        paste(&mut state, "x");
        assert_eq!(view(&state).done, None, "a row without a field takes none");
        focus(&mut state, NodeRow::AddTag);
        typed(&mut state, "t");
        paste(&mut state, "ag");
        assert_eq!(view(&state).field.unwrap().text, "tag");
        input(&mut state, NodeInput::Cancel);
        focus(&mut state, NodeRow::Parent);
        typed(&mut state, " ");
        paste(&mut state, "zz");
        assert_eq!(state.node_editor_finder().unwrap().query, "");
        assert_eq!(texts(&state), ["- a"]);
        let mut state = open("- a @star", 0);
        state.go_to(crate::app::Location::View(crate::app::SpecialView::Starred));
        input(&mut state, NodeInput::Open);
        paste(&mut state, "b");
        assert_eq!(view(&state).text, "ab", "in a special view too");
    }

    // editing.md §7, D-56
    #[test]
    fn a_block_that_stops_being_a_project_leaves_home_as_its_parent() {
        let mut state = dialog("A:\n\tB:", 1);
        focus(&mut state, NodeRow::Parent);
        typed(&mut state, " ");
        state.update(Message::Menu(
            crate::app::Menu::NodeEditor,
            MenuInput::Choose,
        ));
        assert_eq!(view(&state).parent, None, "Home");
        focus(&mut state, NodeRow::Type);
        typed(&mut state, "t");
        assert_eq!(view(&state).parent, Some(vec!["A".to_owned()]));
        assert_eq!(applied(&mut state), ["A:", "\t- B"]);
    }
}
