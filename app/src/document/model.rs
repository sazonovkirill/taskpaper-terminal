//! The document as a tree of lines ([data-model.md §1–2](../../../docs/specs/data-model.md)).

mod edit;

use slotmap::{SlotMap, new_key_type};

use super::format;

pub use edit::{EditError, Op, Snapshot, Transaction};

new_key_type! {
    /// Identifies one item (line) for the session; stale ids fail lookups instead of aliasing.
    pub struct ItemId;
}

/// A parsed `.taskpaper` document: one item per line under a virtual root.
#[derive(Debug, Clone)]
pub struct Document {
    /// Every line of the session; a removed line's slot is emptied, not freed, so undo can put
    /// the line back under the same id.
    items: SlotMap<ItemId, Option<Item>>,
    root: ItemId,
    format: FileFormat,
    revision: u64,
}

/// One line of the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    parent: Option<ItemId>,
    children: Vec<ItemId>,
    text: String,
    extra_indent: usize,
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
        let root = items.insert(Some(Item {
            parent: None,
            children: Vec::new(),
            text: String::new(),
            extra_indent: 0,
        }));
        Self {
            items,
            root,
            format,
            revision: 0,
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
        self.items.get(id)?.as_ref()
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
        let id = self.items.insert(Some(Item {
            parent: Some(parent),
            children: Vec::new(),
            text,
            extra_indent,
        }));
        if let Some(Some(parent)) = self.items.get_mut(parent) {
            parent.children.push(id);
        }
        id
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
        &self.text
    }

    /// Indent levels beyond the expected depth, kept so indent jumps are written back unchanged.
    #[must_use]
    pub fn extra_indent(&self) -> usize {
        self.extra_indent
    }

    /// The TaskPaper type of this line.
    #[must_use]
    pub fn kind(&self) -> ItemKind {
        format::kind(&self.text)
    }

    /// The text without the task's `- `, the project's `:` and every tag.
    #[must_use]
    pub fn display_text(&self) -> String {
        format::display_text(&self.text)
    }

    /// The tags on this line, first occurrence per name.
    #[must_use]
    pub fn tags(&self) -> Vec<format::tags::Tag> {
        format::tags(&self.text)
    }
}

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
