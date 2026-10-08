//! Blocks, what the user navigates: every line that is not a special node is one block, whose
//! type is derived from its text ([data-model.md §3](../../../docs/specs/data-model.md#3-blocks)).

use jiff::civil::Date;

use super::{
    format::tags,
    model::{Document, Item, ItemId, ItemKind},
};

/// The type of a block, as TaskPaper's (FR-BLK-01–03, D-52).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    /// A project line.
    Project,
    /// A task line.
    Task,
    /// A line of any other text, blank lines included, sections and dividers among them.
    Text,
}

/// A color tag, which tints a block's background (FR-TAG-05, D-48).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagColor {
    /// `@red`.
    Red,
    /// `@orange`.
    Orange,
    /// `@yellow`.
    Yellow,
    /// `@green`.
    Green,
    /// `@blue`.
    Blue,
    /// `@violet`.
    Violet,
}

impl TagColor {
    /// Every color, in the order the color command cycles through them (D-26).
    pub const ALL: [Self; 6] = [
        Self::Red,
        Self::Orange,
        Self::Yellow,
        Self::Green,
        Self::Blue,
        Self::Violet,
    ];

    /// The tag's name, without `@`.
    #[must_use]
    pub fn tag(self) -> &'static str {
        match self {
            Self::Red => "red",
            Self::Orange => "orange",
            Self::Yellow => "yellow",
            Self::Green => "green",
            Self::Blue => "blue",
            Self::Violet => "violet",
        }
    }

    /// The color's name as the app shows it, such as `Red`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Red => "Red",
            Self::Orange => "Orange",
            Self::Yellow => "Yellow",
            Self::Green => "Green",
            Self::Blue => "Blue",
            Self::Violet => "Violet",
        }
    }

    /// The color's number, 1 for red to 6 for violet in the order of [`Self::ALL`], by which
    /// commands such as `c1` choose it (D-48).
    #[must_use]
    pub fn number(self) -> u32 {
        match self {
            Self::Red => 1,
            Self::Orange => 2,
            Self::Yellow => 3,
            Self::Green => 4,
            Self::Blue => 5,
            Self::Violet => 6,
        }
    }

    /// The color after `current` in the cycle none, red, orange, yellow, green, blue, violet and
    /// back to none (editing.md §5).
    #[must_use]
    pub fn cycle(current: Option<Self>) -> Option<Self> {
        match current {
            None => Self::ALL.first().copied(),
            Some(color) => Self::ALL
                .iter()
                .skip_while(|&&other| other != color)
                .nth(1)
                .copied(),
        }
    }
}

/// One block: a line of the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    id: ItemId,
    facts: Facts,
}

/// What a line says about its block, read from its type, tags and text; its line keeps it
/// until the text changes (data-model.md §1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent facts read from a line's tags, not states of one machine"
)]
pub(super) struct Facts {
    kind: BlockKind,
    done: bool,
    done_date: Option<Date>,
    start: Option<Date>,
    due: Option<Date>,
    starred: bool,
    bookmark: bool,
    flag: bool,
    color: Option<TagColor>,
    url: bool,
    section: bool,
    divider: bool,
}

impl Facts {
    /// What the line `item` says.
    pub(super) fn of(item: &Item) -> Self {
        let kind = BlockKind::from(item.kind());
        let text = kind == BlockKind::Text;
        let tags = item.tags();
        let has = |name| tags.iter().any(|tag| tag.name == name);
        Self {
            kind,
            done: has("done"),
            done_date: date_tag(tags, "done"),
            start: date_tag(tags, "start"),
            due: date_tag(tags, "due"),
            starred: has("star"),
            bookmark: has("bookmark"),
            flag: has("flag"),
            color: color(tags),
            url: has("url"),
            section: text && has("section"),
            divider: text && divider_title(item.display_text()).is_some(),
        }
    }
}

impl Block {
    /// The block of the line `item`, whose id is `id`.
    fn of(id: ItemId, item: &Item) -> Self {
        Self {
            id,
            facts: *item.facts(),
        }
    }

    /// The block's id: the id of its line.
    #[must_use]
    pub fn id(&self) -> ItemId {
        self.id
    }

    /// Its type.
    #[must_use]
    pub fn kind(&self) -> BlockKind {
        self.facts.kind
    }

    /// Whether its line carries `@done`, with or without a date (D-17).
    #[must_use]
    pub fn done(&self) -> bool {
        self.facts.done
    }

    /// The date of its `@done`, or `None` when it is not done or its value is not a date (D-84).
    #[must_use]
    pub fn done_date(&self) -> Option<Date> {
        self.facts.done_date
    }

    /// The date of its `@start`, or `None` without one or when its value is not a date.
    #[must_use]
    pub fn start(&self) -> Option<Date> {
        self.facts.start
    }

    /// The date of its `@due`, or `None` without one or when its value is not a date.
    #[must_use]
    pub fn due(&self) -> Option<Date> {
        self.facts.due
    }

    /// Whether its line carries `@star`, with or without a value (FR-TAG-04, D-88).
    #[must_use]
    pub fn starred(&self) -> bool {
        self.facts.starred
    }

    /// Whether its line carries `@bookmark`, with or without a value (D-11).
    #[must_use]
    pub fn bookmark(&self) -> bool {
        self.facts.bookmark
    }

    /// Whether its line carries `@flag`, with or without a value (D-91).
    #[must_use]
    pub fn flag(&self) -> bool {
        self.facts.flag
    }

    /// Its color: that of the first color tag on its line (D-5).
    #[must_use]
    pub fn color(&self) -> Option<TagColor> {
        self.facts.color
    }

    /// Whether its line carries `@url`, the address its title links to (FR-BLK-06, D-45).
    #[must_use]
    pub fn url(&self) -> bool {
        self.facts.url
    }

    /// Whether it is a section: a text block carrying `@section`, which a task or project
    /// carries without effect (FR-BLK-04, D-52).
    #[must_use]
    pub fn section(&self) -> bool {
        self.facts.section
    }

    /// Whether it is a divider: a text block whose text, tags aside, is `---`, or `---Title---`
    /// for a titled one (FR-BLK-07, D-52, D-76).
    #[must_use]
    pub fn divider(&self) -> bool {
        self.facts.divider
    }
}

/// The title of a divider whose display text, trimmed, is `text`: empty for `---`, and the text
/// between the dashes, trimmed, for `---Title---` when it holds more than dashes; `None` for any
/// other text (FR-BLK-07, D-76).
#[must_use]
pub fn divider_title(text: &str) -> Option<&str> {
    let text = text.trim();
    if text == "---" {
        return Some("");
    }
    let title = text.strip_prefix("---")?.strip_suffix("---")?.trim();
    title.contains(|c| c != '-').then_some(title)
}

/// The blocks formed by the children of the line `parent` (or the root), in order. Special nodes
/// (`@hidden` lines and their subtrees) form no block.
#[must_use]
pub fn children(document: &Document, parent: ItemId) -> Vec<Block> {
    each_child(document, parent).collect()
}

/// The blocks [`children`] gives, one at a time, for walks over the whole outline that would
/// otherwise collect each line's children.
pub fn each_child(document: &Document, parent: ItemId) -> impl Iterator<Item = Block> + '_ {
    document
        .get(parent)
        .map(Item::children)
        .unwrap_or_default()
        .iter()
        .filter_map(|&id| Some((id, document.get(id)?)))
        .filter(|(_, item)| !is_special(item))
        .map(|(id, item)| Block::of(id, item))
}

/// The block of the line `id`, or `None` for the root, a special node or a line not in
/// `document`.
#[must_use]
pub fn block(document: &Document, id: ItemId) -> Option<Block> {
    let item = document.get(id)?;
    item.parent()?;
    (!is_special(item)).then(|| Block::of(id, item))
}

/// The block whose children include the block `id`, or `None` at the top level.
#[must_use]
pub fn parent(document: &Document, id: ItemId) -> Option<Block> {
    block(document, document.get(id)?.parent()?)
}

/// What a block's details list: every tag on its line and the special nodes it owns (D-3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Details {
    /// Each tag on the line as written, such as `@due(2026-10-08)`, in order, duplicates
    /// included, with whether the app supports it (FR-TAG-02).
    pub tags: Vec<(String, bool)>,
    /// The special nodes among the line's children, which no feature of the app shows yet
    /// (special-nodes.md §1).
    pub hidden: Vec<HiddenData>,
}

/// A special node as a block's details list it: preserved data the app does not show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HiddenData {
    /// The tags on its first line as written, in order.
    pub tags: Vec<String>,
    /// How many lines it has, its first line included.
    pub lines: usize,
}

/// The tags the app reads (FR-TAG-02); any other tag is preserved and hidden, and listed in the
/// details as unsupported (D-3), as `@section` is on a task or project (D-52).
const SUPPORTED: [&str; 14] = [
    "start", "due", "star", "red", "orange", "yellow", "green", "blue", "violet", "section", "url",
    "done", "bookmark", "flag",
];

/// The details of the block `id`, or `None` when it is no block of `document`.
#[must_use]
pub fn details(document: &Document, id: ItemId) -> Option<Details> {
    let item = document.get(id)?;
    let text = block(document, id)?.kind() == BlockKind::Text;
    let hidden = item
        .children()
        .iter()
        .filter_map(|&child| Some((child, document.get(child)?)))
        .filter(|(_, child)| is_special(child))
        .map(|(child, line)| HiddenData {
            tags: written_tags(line.text()).map(|(tag, _)| tag).collect(),
            lines: document.descendants(child).count() + 1,
        })
        .collect();
    Some(Details {
        tags: written_tags(item.text())
            .map(|(tag, name)| {
                let supported = SUPPORTED.contains(&name.as_str()) && (text || name != "section");
                (tag, supported)
            })
            .collect(),
        hidden,
    })
}

/// Each tag of `text` as written, without the whitespace before it, with its name.
fn written_tags(text: &str) -> impl Iterator<Item = (String, String)> + '_ {
    tags::scan(text).into_iter().filter_map(|tag| {
        let written = text.get(tag.span)?.trim_start_matches(tags::is_whitespace);
        Some((written.to_owned(), tag.name))
    })
}

/// Whether `item` starts a special node, which with its subtree forms no block (FR-SPC-01).
#[must_use]
pub fn is_special(item: &Item) -> bool {
    has_tag(item, "hidden")
}

/// The type of the line `item` (data-model §3).
impl From<ItemKind> for BlockKind {
    /// The block type of a line of TaskPaper type `kind`.
    fn from(kind: ItemKind) -> Self {
        match kind {
            ItemKind::Task => Self::Task,
            ItemKind::Project => Self::Project,
            ItemKind::Note => Self::Text,
        }
    }
}

fn has_tag(item: &Item, name: &str) -> bool {
    item.tags().iter().any(|tag| tag.name == name)
}

/// The color of the first color tag among `tags`.
fn color(tags: &[tags::Tag]) -> Option<TagColor> {
    tags.iter().find_map(|tag| {
        TagColor::ALL
            .into_iter()
            .find(|color| color.tag() == tag.name)
    })
}

/// The date the first tag `name` among `tags` holds, read as an ISO 8601 date such as the
/// `YYYY-MM-DD` the app writes (D-6).
fn date_tag(tags: &[tags::Tag], name: &str) -> Option<Date> {
    let tag = tags.iter().find(|tag| tag.name == name)?;
    tag.value.as_deref()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::format::parse;

    /// Each block below `parent`, recursively, as `depth kind text` plus ` section`, ` divider`
    /// and ` done` when it is one.
    fn outline(document: &Document) -> Vec<String> {
        fn walk(document: &Document, parent: ItemId, depth: usize, out: &mut Vec<String>) {
            for block in children(document, parent) {
                let text = document.get(block.id()).unwrap().text();
                let flags: String = [
                    (block.section(), " section"),
                    (block.divider(), " divider"),
                    (block.done(), " done"),
                ]
                .into_iter()
                .filter_map(|(on, flag)| on.then_some(flag))
                .collect();
                out.push(format!("{depth} {:?} {text:?}{flags}", block.kind()));
                walk(document, block.id(), depth + 1, out);
            }
        }
        let mut out = Vec::new();
        walk(document, document.root(), 0, &mut out);
        out
    }

    #[track_caller]
    fn check(text: &str, expected: &[&str]) {
        assert_eq!(outline(&parse(text)), expected, "blocks of {text:?}");
    }

    // FR-BLK-01, FR-BLK-02, FR-BLK-03, FR-BLK-04, FR-BLK-07, D-52
    #[test]
    fn every_line_is_a_block_of_its_type_and_sections_and_dividers_are_text() {
        check(
            "P:\n\t- t\n\tS @section\n\t---\n\tn",
            &[
                r#"0 Project "P:""#,
                r#"1 Task "- t""#,
                r#"1 Text "S @section" section"#,
                r#"1 Text "---" divider"#,
                r#"1 Text "n""#,
            ],
        );
    }

    // D-43
    #[test]
    fn consecutive_text_lines_and_blank_lines_are_blocks_of_their_own() {
        check(
            "a\n\nb\n\tc\n- t",
            &[
                r#"0 Text "a""#,
                r#"0 Text """#,
                r#"0 Text "b""#,
                r#"1 Text "c""#,
                r#"0 Task "- t""#,
            ],
        );
    }

    // D-43
    #[test]
    fn code_is_text_whose_tag_changes_nothing() {
        check(
            "f() @code\n- t @code\n--- @code",
            &[
                r#"0 Text "f() @code""#,
                r#"0 Task "- t @code""#,
                r#"0 Text "--- @code" divider"#,
            ],
        );
    }

    // FR-BLK-07, D-52
    #[test]
    fn divider_ignores_tags_and_surrounding_spaces_but_not_other_text() {
        check("--- @red", &[r#"0 Text "--- @red" divider"#]);
        check("---  ", &[r#"0 Text "---  " divider"#]);
        check(
            "--- @section",
            &[r#"0 Text "--- @section" section divider"#],
        );
        check("----", &[r#"0 Text "----""#]);
        check("- ---", &[r#"0 Task "- ---""#]);
    }

    // FR-BLK-07, D-76
    #[test]
    fn a_titled_divider_is_text_between_dashes_its_title_trimmed() {
        check("---Plan--- @red", &[r#"0 Text "---Plan--- @red" divider"#]);
        check("- ---Plan---", &[r#"0 Task "- ---Plan---""#]);
        let titles = [
            "---",
            " --- ",
            "---Plan---",
            "--- Next week ---",
            "---a-b---",
            "------",
            "-------",
            "---- ---",
            "--Plan--",
            "---Plan",
            "Plan---",
            "---Plan---!",
        ]
        .map(divider_title);
        assert_eq!(
            titles,
            [
                Some(""),
                Some(""),
                Some("Plan"),
                Some("Next week"),
                Some("a-b"),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ]
        );
    }

    // FR-BLK-04, D-52
    #[test]
    fn the_section_tag_makes_no_section_of_a_task_or_project() {
        check(
            "- t @section\nP: @section",
            &[r#"0 Task "- t @section""#, r#"0 Project "P: @section""#],
        );
    }

    // FR-BLK-06, D-45
    #[test]
    fn a_url_tag_links_a_block_of_any_type_without_changing_it() {
        let document = parse("L @url(https://x.y)\n- t @url(u)\nP: @url\nS @section @url(u)\n- n");
        let linked: Vec<(BlockKind, bool, bool)> = children(&document, document.root())
            .iter()
            .map(|block| (block.kind(), block.section(), block.url()))
            .collect();
        assert_eq!(
            linked,
            [
                (BlockKind::Text, false, true),
                (BlockKind::Task, false, true),
                (BlockKind::Project, false, true),
                (BlockKind::Text, true, true),
                (BlockKind::Task, false, false),
            ]
        );
    }

    // FR-SPC-01
    #[test]
    fn special_nodes_and_their_subtrees_are_skipped() {
        check(
            "a\n\tC @hidden @comments\n\t\tcomment\nX @hidden\nd",
            &[r#"0 Text "a""#, r#"0 Text "d""#],
        );
    }

    // D-17
    #[test]
    fn done_comes_from_the_line_with_or_without_a_date() {
        check(
            "- a @done\n- b @done(2026-10-04)\n- c @done(soon)\nm @done\n- d",
            &[
                r#"0 Task "- a @done" done"#,
                r#"0 Task "- b @done(2026-10-04)" done"#,
                r#"0 Task "- c @done(soon)" done"#,
                r#"0 Text "m @done" done"#,
                r#"0 Task "- d""#,
            ],
        );
    }

    /// The start and due dates of each top-level block of `text`.
    fn dates(text: &str) -> Vec<(Option<Date>, Option<Date>)> {
        let document = parse(text);
        children(&document, document.root())
            .iter()
            .map(|block| (block.start(), block.due()))
            .collect()
    }

    // FR-TAG-02, FR-TAG-03
    #[test]
    fn start_and_due_come_from_their_tags_when_they_hold_dates() {
        let day = |d| Some(jiff::civil::date(2026, 10, d));
        assert_eq!(
            dates(
                "- a @start(2026-10-05) @due(2026-10-06)\n- b @due(2026-10-07)\nP: @start(2026-10-08)\n- c"
            ),
            [
                (day(5), day(6)),
                (None, day(7)),
                (day(8), None),
                (None, None)
            ]
        );
    }

    // FR-TAG-03, D-6
    #[test]
    fn a_date_tag_without_a_date_gives_no_date() {
        assert_eq!(
            dates(
                "- a @due\n- b @due()\n- c @due(soon)\n- d @due(2026-13-01)\n- e @Due(2026-10-05)\n- f @due(2026-02-30)"
            ),
            [(None, None); 6]
        );
    }

    // D-84
    #[test]
    fn the_done_date_comes_from_a_done_tag_holding_a_date() {
        let document = parse(
            "- a @done(2026-10-05)\n- b @done\n- c @done(soon)\n- d\n- e @done(2026-10-04) @done(2026-10-09)",
        );
        let dates: Vec<_> = children(&document, document.root())
            .iter()
            .map(Block::done_date)
            .collect();
        let day = |d| Some(jiff::civil::date(2026, 10, d));
        assert_eq!(dates, [day(5), None, None, None, day(4)]);
    }

    #[test]
    fn the_first_date_tag_of_a_name_counts() {
        assert_eq!(
            dates("- a @due(2026-10-05) @due(2026-10-09)"),
            [(None, Some(jiff::civil::date(2026, 10, 5)))]
        );
    }

    /// Whether each top-level block of `text` has `@star`, `@bookmark` and `@flag`, and its
    /// color.
    fn marks(text: &str) -> Vec<(bool, bool, bool, Option<TagColor>)> {
        let document = parse(text);
        children(&document, document.root())
            .iter()
            .map(|block| {
                (
                    block.starred(),
                    block.bookmark(),
                    block.flag(),
                    block.color(),
                )
            })
            .collect()
    }

    // FR-TAG-02, FR-TAG-04, FR-TAG-05, D-5, D-11, D-88, D-91: `@today` means nothing
    #[test]
    fn star_bookmark_flag_and_color_come_from_their_tags() {
        use TagColor::{Blue, Green, Orange, Red, Violet, Yellow};
        assert_eq!(
            marks(
                "- a @star
- b @bookmark(x)
P: @blue @red
- c @violet
- d @orange @x
- e @yellow
- f @green @star @bookmark @flag
- g @Red @stars @redish @flags
- h @today
- i @flag(x)"
            ),
            [
                (true, false, false, None),
                (false, true, false, None),
                (false, false, false, Some(Blue)),
                (false, false, false, Some(Violet)),
                (false, false, false, Some(Orange)),
                (false, false, false, Some(Yellow)),
                (true, true, true, Some(Green)),
                (false, false, false, None),
                (false, false, false, None),
                (false, false, true, None),
            ]
        );
        assert_eq!(marks("- a @red(x)"), [(false, false, false, Some(Red))]);
    }

    // D-48
    #[test]
    fn colors_are_numbered_from_one_in_their_order() {
        let numbers: Vec<u32> = TagColor::ALL.into_iter().map(TagColor::number).collect();
        assert_eq!(numbers, [1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn colors_are_named_for_the_app_to_show() {
        let names: Vec<&str> = TagColor::ALL.into_iter().map(TagColor::name).collect();
        assert_eq!(
            names,
            ["Red", "Orange", "Yellow", "Green", "Blue", "Violet"]
        );
    }

    // D-26, editing.md §5
    #[test]
    fn colors_cycle_from_none_through_the_six_and_back() {
        let cycle: Vec<_> =
            std::iter::successors(Some(None), |&color| Some(TagColor::cycle(color)))
                .take(8)
                .map(|color| color.map(TagColor::tag))
                .collect();
        assert_eq!(
            cycle,
            [
                None,
                Some("red"),
                Some("orange"),
                Some("yellow"),
                Some("green"),
                Some("blue"),
                Some("violet"),
                None
            ]
        );
    }

    #[test]
    fn empty_document_and_unknown_parent_have_no_blocks() {
        let document = parse("");
        assert_eq!(children(&document, document.root()), Vec::new());
        let other = parse("a");
        let stale = children(&other, other.root())[0].id();
        assert_eq!(children(&document, stale), Vec::new());
    }

    #[test]
    fn finds_a_block_and_its_parent_by_id() {
        let document = parse("P:\n\ta\n\t\t- t\n\th @hidden\n- u");
        let top = children(&document, document.root());
        let note = &children(&document, top[0].id())[0];
        let task = &children(&document, note.id())[0];
        assert_eq!(block(&document, note.id()).as_ref(), Some(note));
        assert_eq!(block(&document, task.id()).as_ref(), Some(task));
        assert_eq!(parent(&document, task.id()).as_ref(), Some(note));
        assert_eq!(parent(&document, note.id()).as_ref(), Some(&top[0]));
        assert_eq!(parent(&document, top[1].id()), None);
        let hidden = document.get(top[0].id()).unwrap().children()[1];
        assert_eq!(block(&document, hidden), None, "a special node");
        assert_eq!(block(&document, document.root()), None);
        assert_eq!(parent(&document, document.root()), None);
    }

    // FR-SPC-01
    #[test]
    fn a_line_tagged_hidden_is_special() {
        let document = parse("a @hidden\nb @hidden(x)\nc @hide\nd hidden\n- e @x @hidden");
        let special: Vec<bool> = document
            .descendants(document.root())
            .map(|(id, _)| is_special(document.get(id).unwrap()))
            .collect();
        assert_eq!(special, [true, true, false, false, true]);
    }

    // D-3, FR-TAG-01, FR-TAG-02
    #[test]
    fn details_list_every_tag_as_written_and_mark_the_unsupported_ones() {
        let document = parse("- a @due(2026-10-08) @estimate(2 h) @red @code @x(a\\)b) @x\n- b");
        let top = children(&document, document.root());
        let tag = |written: &str, supported| (written.to_owned(), supported);
        assert_eq!(
            details(&document, top[0].id()),
            Some(Details {
                tags: vec![
                    tag("@due(2026-10-08)", true),
                    tag("@estimate(2 h)", false),
                    tag("@red", true),
                    tag("@code", false),
                    tag("@x(a\\)b)", false),
                    tag("@x", false),
                ],
                hidden: Vec::new(),
            })
        );
        assert_eq!(
            details(&document, top[1].id()),
            Some(Details {
                tags: Vec::new(),
                hidden: Vec::new()
            })
        );
        let every = parse(&format!("a @{}", SUPPORTED.join(" @")));
        let first = children(&every, every.root())[0].id();
        let supported = details(&every, first).unwrap().tags;
        assert_eq!(supported.len(), 14);
        assert!(supported.iter().all(|&(_, supported)| supported));
    }

    // D-52
    #[test]
    fn a_section_tag_on_a_task_or_project_is_listed_as_unsupported() {
        let document = parse("- a @section @red\nP: @section\nn @section");
        let listed: Vec<Vec<(String, bool)>> = children(&document, document.root())
            .iter()
            .map(|block| details(&document, block.id()).unwrap().tags)
            .collect();
        let tag = |written: &str, supported| (written.to_owned(), supported);
        assert_eq!(
            listed,
            [
                vec![tag("@section", false), tag("@red", true)],
                vec![tag("@section", false)],
                vec![tag("@section", true)],
            ]
        );
    }

    // FR-SPC-01, special-nodes.md §1
    #[test]
    fn details_list_the_special_nodes_a_block_owns_with_their_tags_and_lines() {
        let document = parse(
            "P:\n\t- t\n\tLOG @hidden @log(x)\n\t\tone\n\t\t\ttwo\n\tX @hidden\n\t- u\n\t\tY @hidden",
        );
        let project = children(&document, document.root())[0].id();
        assert_eq!(
            details(&document, project).unwrap().hidden,
            [
                HiddenData {
                    tags: vec!["@hidden".to_owned(), "@log(x)".to_owned()],
                    lines: 3,
                },
                HiddenData {
                    tags: vec!["@hidden".to_owned()],
                    lines: 1,
                },
            ]
        );
    }

    #[test]
    fn only_blocks_have_details() {
        let document = parse("a\n\tX @hidden");
        let hidden = document
            .get(children(&document, document.root())[0].id())
            .unwrap()
            .children()[0];
        assert_eq!(details(&document, hidden), None);
        assert_eq!(details(&document, document.root()), None);
    }
}
