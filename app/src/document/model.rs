//! The document as a tree of lines ([data-model.md §1–2](../../../docs/specs/data-model.md)).

mod edit;

use std::{
    collections::{HashSet, VecDeque},
    sync::{Arc, OnceLock},
};

use slotmap::{SlotMap, new_key_type};

use super::{
    BlockKind,
    blocks::Facts,
    format::{self, LineOrigin},
    inline::{self, Syntax},
};

pub use edit::{EditError, Op, Snapshot, Transaction};

new_key_type! {
    /// Identifies one item (line) for the session; stale ids fail lookups instead of aliasing.
    pub struct ItemId;
}

/// A parsed `.taskpaper` document: one item per line under a virtual root.
#[derive(Debug)]
pub struct Document {
    /// Every line of the session; a removed line's slot is emptied, not freed, so undo can put
    /// the line back under the same id.
    items: SlotMap<ItemId, Option<Arc<Item>>>,
    root: ItemId,
    format: FileFormat,
    revision: u64,
    /// The lines each of the latest transactions changed, oldest first, with the revision it
    /// made, so that views can tell what changed since they were computed (N-PERF-02).
    changes: VecDeque<(u64, Vec<ItemId>)>,
    /// Shared only by this document's stamps, so that no other document, a copy included, has
    /// equal ones.
    identity: Arc<()>,
    /// The lines the transactions applied since [`Document::take_touched`] last took them
    /// changed, put in place or moved.
    touched: Vec<ItemId>,
}

impl Clone for Document {
    /// A copy that changes apart from this document, so it gets an identity of its own.
    fn clone(&self) -> Self {
        Self {
            items: self.items.clone(),
            root: self.root,
            format: self.format.clone(),
            revision: self.revision,
            // Its stamps differ from this one's, so this one's log could never answer for them.
            changes: VecDeque::new(),
            identity: Arc::default(),
            touched: self.touched.clone(),
        }
    }
}

/// A document's content at one point: two stamps are equal only when they come from the same
/// document at the same revision, so a view computed from one fits the other.
#[derive(Debug, Clone)]
pub struct Stamp {
    identity: Arc<()>,
    revision: u64,
}

impl PartialEq for Stamp {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.identity, &other.identity) && self.revision == other.revision
    }
}

impl Eq for Stamp {}

/// One line of the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    parent: Option<ItemId>,
    children: Vec<ItemId>,
    text: Line,
    extra_indent: usize,
    original: Option<LineOrigin>,
}

/// TaskPaper's line type ([file-format.md §4](../../../docs/specs/file-format.md#4-item-type)).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    /// A line ending in `:`.
    Project,
    /// A line starting with `-` and whitespace.
    Task,
    /// Anything else, including blank lines.
    Note,
}

/// How the file was written, remembered so it can be written back the same way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileFormat {
    /// The indentation of one level.
    pub indent: IndentUnit,
    /// The dominant line ending.
    pub line_ending: LineEnding,
    /// Whether the file started with a UTF-8 byte order mark.
    pub bom: bool,
    /// Whether the text ended with a line break.
    pub trailing_newline: bool,
}

/// The indentation of one outline level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndentUnit {
    /// One tab per level.
    Tab,
    /// The given number of spaces per level.
    Spaces(usize),
}

/// A line ending.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    /// `\n`
    Lf,
    /// `\r\n`
    CrLf,
    /// `\r`
    Cr,
}

impl Default for FileFormat {
    /// The format of a new file: tabs, `\n`, no BOM and a trailing newline.
    fn default() -> Self {
        Self {
            indent: IndentUnit::Tab,
            line_ending: LineEnding::Lf,
            bom: false,
            trailing_newline: true,
        }
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new(FileFormat::default())
    }
}

impl Document {
    /// An empty document written in `format`.
    #[must_use]
    pub fn new(format: FileFormat) -> Self {
        let mut items = SlotMap::with_key();
        let root = items.insert(Some(Arc::new(Item {
            parent: None,
            children: Vec::new(),
            text: Line::new(String::new()),
            extra_indent: 0,
            original: None,
        })));
        Self {
            items,
            root,
            format,
            revision: 0,
            changes: VecDeque::new(),
            identity: Arc::default(),
            touched: Vec::new(),
        }
    }

    /// The virtual root, parent of the top-level items; it is never written to the file.
    #[must_use]
    pub fn root(&self) -> ItemId {
        self.root
    }

    /// The item with this id, or `None` if it does not exist.
    #[must_use]
    pub fn get(&self, id: ItemId) -> Option<&Item> {
        self.items.get(id)?.as_deref()
    }

    /// How the file was written.
    #[must_use]
    pub fn format(&self) -> &FileFormat {
        &self.format
    }

    /// Counts the transactions applied so far, so views and saving can tell that it changed.
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// The document's content as it is now: its stamp changes with every applied transaction.
    #[must_use]
    pub fn stamp(&self) -> Stamp {
        Stamp {
            identity: Arc::clone(&self.identity),
            revision: self.revision,
        }
    }

    /// The lines changed since the document's `stamp`: those put in or taken out and those whose
    /// text, extra indent, parent or children changed. `None` when `stamp` is another document's
    /// or too old to tell.
    #[must_use]
    pub fn changed_since(&self, stamp: &Stamp) -> Option<HashSet<ItemId>> {
        if !Arc::ptr_eq(&stamp.identity, &self.identity) {
            return None;
        }
        if stamp.revision == self.revision {
            return Some(HashSet::new());
        }
        let (oldest, _) = self.changes.front()?;
        (*oldest <= stamp.revision + 1).then(|| {
            self.changes
                .iter()
                .filter(|(revision, _)| *revision > stamp.revision)
                .flat_map(|(_, lines)| lines.iter().copied())
                .collect()
        })
    }

    /// Every item below `id` in document order, with its depth relative to `id`'s children (0).
    pub fn descendants(&self, id: ItemId) -> impl Iterator<Item = (ItemId, usize)> + '_ {
        let mut stack: Vec<(ItemId, usize)> = self
            .children_of(id)
            .iter()
            .rev()
            .map(|&child| (child, 0))
            .collect();
        std::iter::from_fn(move || {
            let (id, depth) = stack.pop()?;
            stack.extend(
                self.children_of(id)
                    .iter()
                    .rev()
                    .map(|&child| (child, depth + 1)),
            );
            Some((id, depth))
        })
    }

    /// Appends a new last child of `parent`; used while parsing, before anything can refer to it.
    pub(crate) fn push_child(
        &mut self,
        parent: ItemId,
        text: String,
        extra_indent: usize,
    ) -> ItemId {
        let id = self.items.insert(Some(Arc::new(Item {
            parent: Some(parent),
            children: Vec::new(),
            text: Line::new(text),
            extra_indent,
            original: None,
        })));
        if let Some(parent) = self.item_mut(parent) {
            parent.children.push(id);
        }
        id
    }

    /// Records the line `id` was read from; used while parsing.
    pub(crate) fn set_original(&mut self, id: ItemId, original: LineOrigin) {
        if let Some(item) = self.item_mut(id) {
            item.original = Some(original);
        }
    }

    /// The item with this id to change, copied first while a copy of the document shares it.
    fn item_mut(&mut self, id: ItemId) -> Option<&mut Item> {
        self.items.get_mut(id)?.as_mut().map(Arc::make_mut)
    }

    fn children_of(&self, id: ItemId) -> &[ItemId] {
        self.get(id).map_or(&[], |item| &item.children)
    }
}

impl Item {
    /// The parent item; the root's children have the root as parent, the root has none.
    #[must_use]
    pub fn parent(&self) -> Option<ItemId> {
        self.parent
    }

    /// The child items in document order.
    #[must_use]
    pub fn children(&self) -> &[ItemId] {
        &self.children
    }

    /// The line without its indentation; the single source of truth for everything derived.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text.text
    }

    /// Indent levels beyond the expected depth, kept so indent jumps are written back unchanged.
    #[must_use]
    pub fn extra_indent(&self) -> usize {
        self.extra_indent
    }

    /// The line this item was read from, if it was read from a file.
    pub(crate) fn original(&self) -> Option<&LineOrigin> {
        self.original.as_ref()
    }

    /// The TaskPaper type of this line.
    #[must_use]
    pub fn kind(&self) -> ItemKind {
        self.text.kind
    }

    /// The text without the task's `- `, the project's `:` and every tag.
    #[must_use]
    pub fn display_text(&self) -> &str {
        &self.text.display_text
    }

    /// [`Item::display_text`] shared rather than copied, for views that keep it.
    #[must_use]
    pub fn shared_display_text(&self) -> Arc<str> {
        Arc::clone(&self.text.display_text)
    }

    /// The tags on this line, first occurrence per name.
    #[must_use]
    pub fn tags(&self) -> &[format::tags::Tag] {
        &self.text.tags
    }

    /// What the line says about its block, worked out the first time it is read.
    pub(super) fn facts(&self) -> &Facts {
        self.text.facts.get_or_init(|| Facts::of(self))
    }

    /// The display text without the delimiters of its formatting, as lists of blocks such as the
    /// Navigator show it; worked out the first time it is read.
    #[must_use]
    pub fn plain_text(&self) -> &str {
        self.text.plain.get_or_init(|| {
            let syntax = Syntax::of(BlockKind::from(self.text.kind));
            inline::runs(&self.text.display_text, syntax)
                .iter()
                .map(|run| run.text)
                .collect()
        })
    }
}

/// A line's text with what it says, derived when the text is set rather than each time it is
/// read, since every view reads it for every line (data-model.md §1).
#[derive(Debug, Clone)]
struct Line {
    text: String,
    kind: ItemKind,
    tags: Vec<format::tags::Tag>,
    display_text: Arc<str>,
    /// Only some views read these, so they are derived on the first read rather than for every
    /// line.
    plain: OnceLock<String>,
    facts: OnceLock<Facts>,
}

impl Line {
    fn new(text: String) -> Self {
        let (kind, tags, display_text) = format::read(&text);
        Self {
            text,
            kind,
            tags,
            display_text: display_text.into(),
            plain: OnceLock::new(),
            facts: OnceLock::new(),
        }
    }
}

impl PartialEq for Line {
    /// Lines are equal when their texts are, since everything else is derived from the text.
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text
    }
}

impl Eq for Line {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_document_has_only_a_root() {
        let document = Document::default();
        let root = document.get(document.root()).unwrap();
        assert_eq!(root.children(), []);
        assert_eq!(root.parent(), None);
        assert_eq!(document.descendants(document.root()).count(), 0);
    }

    #[test]
    fn default_format_is_the_new_file_format() {
        assert_eq!(
            FileFormat::default(),
            FileFormat {
                indent: IndentUnit::Tab,
                line_ending: LineEnding::Lf,
                bom: false,
                trailing_newline: true,
            }
        );
        assert_eq!(Document::default().format(), &FileFormat::default());
    }

    #[test]
    fn push_child_links_parent_and_child() {
        let mut document = Document::default();
        let root = document.root();
        let a = document.push_child(root, "A:".into(), 0);
        let b = document.push_child(a, "- b".into(), 2);
        assert_eq!(document.get(root).unwrap().children(), [a]);
        assert_eq!(document.get(a).unwrap().children(), [b]);
        assert_eq!(document.get(b).unwrap().parent(), Some(a));
        assert_eq!(document.get(a).unwrap().parent(), Some(root));
        assert_eq!(document.get(b).unwrap().text(), "- b");
        assert_eq!(document.get(b).unwrap().extra_indent(), 2);
    }

    #[test]
    fn descendants_walk_depth_first_with_relative_depth() {
        let mut document = Document::default();
        let root = document.root();
        let a = document.push_child(root, "a".into(), 0);
        let b = document.push_child(a, "b".into(), 0);
        let c = document.push_child(b, "c".into(), 0);
        let d = document.push_child(root, "d".into(), 0);
        assert_eq!(
            document.descendants(root).collect::<Vec<_>>(),
            [(a, 0), (b, 1), (c, 2), (d, 0)]
        );
        assert_eq!(
            document.descendants(a).collect::<Vec<_>>(),
            [(b, 0), (c, 1)]
        );
    }

    #[test]
    fn stale_ids_find_nothing() {
        let mut other = Document::default();
        let stale = other.push_child(other.root(), "x".into(), 0);
        let document = Document::default();
        assert!(document.get(stale).is_none());
        assert_eq!(document.descendants(stale).count(), 0);
    }

    #[test]
    fn item_kind_and_display_text_follow_the_text() {
        let mut document = Document::default();
        let root = document.root();
        let project = document.push_child(root, "Inbox: @a".into(), 0);
        let task = document.push_child(root, "- Milk".into(), 0);
        let item = document.get(project).unwrap();
        assert_eq!(item.kind(), ItemKind::Project);
        assert_eq!(item.display_text(), "Inbox");
        assert_eq!(document.get(task).unwrap().kind(), ItemKind::Task);
        assert_eq!(document.get(task).unwrap().display_text(), "Milk");
    }

    #[test]
    fn item_tags_follow_the_text() {
        let mut document = Document::default();
        let root = document.root();
        let task = document.push_child(root, "- Milk @done @done(2026-10-01)".into(), 0);
        let tags = document.get(task).unwrap().tags();
        assert_eq!(
            tags,
            [format::tags::Tag {
                span: 6..12,
                name: "done".to_owned(),
                value: None,
            }]
        );
    }
}
