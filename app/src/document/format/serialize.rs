//! Writing a [`Document`] as `.taskpaper` text ([file-format.md §6](../../../../docs/specs/file-format.md#6-serialization)):
//! decides which lines can be written back from their original bytes and how the others are indented and ended.

use std::borrow::Cow;

use super::super::model::{Document, IndentUnit, Item, ItemId, LineEnding};
use super::{BOM, LineOrigin, is_whitespace, leading_space_runs};

/// The file contents for `document`, BOM included: an unedited document gives back the exact text
/// it was parsed from (F-FILE-03), and edited lines use the remembered indent unit and line ending.
#[must_use]
pub fn serialize(document: &Document) -> String {
    let format = document.format();
    let lines = lines_with_depth(document);
    let mut contents = line_contents(&lines, format.indent, true);
    if !keeps_indent_unit(&contents, format.indent) {
        contents = line_contents(&lines, format.indent, false);
    }
    let mut text = String::new();
    if format.bom {
        text.push_str(BOM);
    }
    let last = lines.len().checked_sub(1);
    let mut previous_break = "";
    for (index, (content, &(item, _))) in contents.iter().zip(&lines).enumerate() {
        text.push_str(content);
        // An empty last line is only read back when a line break ends it (file-format.md §3.1).
        if Some(index) != last || format.trailing_newline || content.is_empty() {
            let mut this_break = line_break(item, format.line_ending);
            // A `\r` directly followed by `\n` reads back as one break, losing this empty line.
            if content.is_empty() && previous_break == "\r" && this_break.starts_with('\n') {
                this_break = "\r";
            }
            text.push_str(this_break);
            previous_break = this_break;
        }
    }
    text
}

/// The lines `tops`, none inside another and in document order, with their subtrees as
/// TaskPaper text for the clipboard: tags included, tab-indented relative to the shallowest of
/// them and each ended by `\n`, whatever the file uses ([editing.md §4](../../../../docs/specs/editing.md#4-structural-operations-fr-ops)).
/// Lines not in `document` are left out.
#[must_use]
pub fn serialize_blocks(document: &Document, tops: &[ItemId]) -> String {
    let ids: Vec<ItemId> = document
        .descendants(document.root())
        .map(|(id, _)| id)
        .collect();
    let lines = lines_with_depth(document);
    let runs: Vec<&[(&Item, usize)]> = tops
        .iter()
        .filter_map(|&top| {
            let start = ids.iter().position(|&id| id == top)?;
            lines.get(start..=start + document.descendants(top).count())
        })
        .collect();
    let shallowest = runs
        .iter()
        .filter_map(|run| run.first().map(|&(_, depth)| depth))
        .min()
        .unwrap_or_default();
    let mut text = String::new();
    for &(item, depth) in runs.iter().copied().flatten() {
        text.push_str(&indent(IndentUnit::Tab, depth.saturating_sub(shallowest)));
        text.push_str(item.text());
        text.push('\n');
    }
    text
}

/// Each line without its break: the original line where `reuse_originals` and it still reads
/// back the same, otherwise indent and text.
fn line_contents<'a>(
    lines: &[(&'a Item, usize)],
    unit: IndentUnit,
    reuse_originals: bool,
) -> Vec<Cow<'a, str>> {
    lines
        .iter()
        .zip(next_text_depths(lines))
        .map(|(&(item, depth), next_text_depth)| {
            match item.original().filter(|original| {
                reuse_originals && rereads_as(original, item, depth, next_text_depth)
            }) {
                Some(original) => Cow::Borrowed(original.line.as_str()),
                None => Cow::Owned(indent(unit, depth) + item.text()),
            }
        })
        .collect()
}

/// Whether the lines read back with `unit`: the depths of original lines were read with the
/// file's smallest run of leading spaces, so an edit that removes that run would change them.
fn keeps_indent_unit(contents: &[Cow<'_, str>], unit: IndentUnit) -> bool {
    match unit {
        IndentUnit::Tab => true,
        IndentUnit::Spaces(per_level) => contents
            .iter()
            .flat_map(|line| leading_space_runs(line))
            .min()
            .is_none_or(|run| run == per_level),
    }
}

/// The break that followed the item's original line, so an edited or moved line keeps it; the
/// remembered line ending for new lines and for a former last line without one.
fn line_break(item: &Item, line_ending: LineEnding) -> &'static str {
    item.original()
        .map(|original| original.line_break)
        .filter(|line_break| !line_break.is_empty())
        .unwrap_or(match line_ending {
            LineEnding::Lf => "\n",
            LineEnding::CrLf => "\r\n",
            LineEnding::Cr => "\r",
        })
}

/// Whether the original line, written where `item` now is, parses back to its text and depth.
fn rereads_as(
    original: &LineOrigin,
    item: &Item,
    depth: usize,
    next_text_depth: Option<usize>,
) -> bool {
    if original.line.get(original.text_start..) != Some(item.text()) {
        return false;
    }
    let reread_depth = if is_blank(item) {
        next_text_depth.map_or(0, |next| original.depth.max(next))
    } else {
        original.depth
    };
    reread_depth == depth
}

/// Every item in document order with its absolute depth ([data-model.md §2](../../../../docs/specs/data-model.md#2-derived-line-attributes)).
fn lines_with_depth(document: &Document) -> Vec<(&Item, usize)> {
    let mut ancestor_depths: Vec<usize> = Vec::new();
    document
        .descendants(document.root())
        .filter_map(|(id, level)| document.get(id).map(|item| (item, level)))
        .map(|(item, level)| {
            ancestor_depths.truncate(level);
            let depth = ancestor_depths.last().map_or(0, |parent| parent + 1) + item.extra_indent();
            ancestor_depths.push(depth);
            (item, depth)
        })
        .collect()
}

/// For each line, the depth of the next line that is not blank, which a blank line takes when deeper.
fn next_text_depths(lines: &[(&Item, usize)]) -> Vec<Option<usize>> {
    let mut next = None;
    let mut depths: Vec<_> = lines
        .iter()
        .rev()
        .map(|&(item, depth)| {
            let after = next;
            if !is_blank(item) {
                next = Some(depth);
            }
            after
        })
        .collect();
    depths.reverse();
    depths
}

fn is_blank(item: &Item) -> bool {
    item.text().chars().all(is_whitespace)
}

fn indent(unit: IndentUnit, depth: usize) -> String {
    match unit {
        IndentUnit::Tab => "\t".repeat(depth),
        IndentUnit::Spaces(per_level) => " ".repeat(per_level.saturating_mul(depth)),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use proptest::prelude::*;

    use super::super::super::model::{FileFormat, ItemId, Op, Transaction};
    use super::super::{parse, parse_bytes, tests::outline};
    use super::*;

    #[track_caller]
    fn check_round_trip(text: &str) {
        let document = parse_bytes(text.as_bytes()).unwrap();
        assert_eq!(serialize(&document), text, "round trip of {text:?}");
    }

    fn line_origin(
        line: &str,
        text_start: usize,
        depth: usize,
        line_break: &'static str,
    ) -> LineOrigin {
        LineOrigin {
            line: line.to_owned(),
            text_start,
            depth,
            line_break,
        }
    }

    /// Appends an item that was read from `original`, as the parser leaves it.
    fn push_read(
        document: &mut Document,
        parent: ItemId,
        text: &str,
        extra_indent: usize,
        original: LineOrigin,
    ) -> ItemId {
        let id = document.push_child(parent, text.to_owned(), extra_indent);
        document.set_original(id, original);
        id
    }

    // F-FILE-03, F-FILE-08
    #[test]
    fn unedited_text_is_written_back_byte_for_byte() {
        let cases = [
            "",
            "A",
            "A\n",
            "\n",
            "\n\n",
            "\r\n",
            "a\r",
            "A:\n\t- b\n\t\tnote\n\t- c\nB:\n",
            "A:\r\n\t- b\r\n",
            "A:\r\t- b\r",
            "a\r\nb\nc\rd\r\n",
            "a\u{C}b\u{2029}c\u{2B7F}d\u{2B7F}",
            "a\u{2028}b\n",
            "\u{FEFF}A:\n\t- b\n",
            "\u{FEFF}\u{FEFF}A",
            "A\n    B\n        C\n    D\n",
            "A\n    B\n  C\n",
            "A\n  B\n   C",
            "A\n  B\n\t   \tC\n",
            "A\n \tB\n    C\n",
            "A\n\t\t\tB\n\tC\n",
            "\t\tA\nB\n",
            "A\n\n\tB\n",
            "A\n\t\t\nB\n",
            "A\n\t \u{A0}\n\tB\n",
            "A\n\tB\n\t\n\t\t\n",
            "\t\n\t\t",
            "Inbox:  \n- task  \t\n",
            "  - Milk",
            "\t- Milk\n",
        ];
        for text in cases {
            check_round_trip(text);
        }
    }

    /// Each item as `depth extra_indent kind "text" tags`, one per line.
    fn item_tree(document: &Document) -> String {
        let lines: Vec<_> = outline(document)
            .into_iter()
            .zip(document.descendants(document.root()))
            .map(|(line, (id, _))| {
                let item = document.get(id).unwrap();
                let tags: Vec<_> = item
                    .tags()
                    .into_iter()
                    .map(|tag| match tag.value {
                        Some(value) => format!("@{}({value})", tag.name),
                        None => format!("@{}", tag.name),
                    })
                    .collect();
                format!("{line} {:?} [{}]", item.kind(), tags.join(" "))
            })
            .collect();
        lines.join("\n")
    }

    // N-REL-02
    #[test]
    fn corpus_files_round_trip_and_parse_to_their_snapshot() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/corpus");
        let mut paths: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        paths.sort();
        assert!(!paths.is_empty(), "the corpus has files");
        for path in paths {
            let bytes = std::fs::read(&path).unwrap();
            let document = parse_bytes(&bytes).unwrap();
            assert_eq!(serialize(&document).as_bytes(), bytes, "{}", path.display());
            let name = path.file_stem().unwrap().to_string_lossy().into_owned();
            insta::assert_snapshot!(name, item_tree(&document));
        }
    }

    #[test]
    fn new_documents_are_empty() {
        assert_eq!(serialize(&Document::default()), "");
        let with_bom = Document::new(FileFormat {
            bom: true,
            ..FileFormat::default()
        });
        assert_eq!(serialize(&with_bom), "\u{FEFF}");
    }

    /// A document of new lines `A`, `B` under it, `C` under `B` and `D` two levels under `A`.
    fn new_lines(format: FileFormat) -> Document {
        let mut document = Document::new(format);
        let root = document.root();
        let a = document.push_child(root, "A:".into(), 0);
        let b = document.push_child(a, "- B".into(), 0);
        document.push_child(b, "C".into(), 0);
        document.push_child(a, "D".into(), 1);
        document
    }

    // F-FILE-08
    #[test]
    fn new_lines_use_the_remembered_format() {
        let cases = [
            (FileFormat::default(), "A:\n\t- B\n\t\tC\n\t\tD\n"),
            (
                FileFormat {
                    indent: IndentUnit::Spaces(4),
                    ..FileFormat::default()
                },
                "A:\n    - B\n        C\n        D\n",
            ),
            (
                FileFormat {
                    line_ending: LineEnding::CrLf,
                    trailing_newline: false,
                    ..FileFormat::default()
                },
                "A:\r\n\t- B\r\n\t\tC\r\n\t\tD",
            ),
            (
                FileFormat {
                    line_ending: LineEnding::Cr,
                    bom: true,
                    ..FileFormat::default()
                },
                "\u{FEFF}A:\r\t- B\r\t\tC\r\t\tD\r",
            ),
        ];
        for (format, expected) in cases {
            assert_eq!(
                serialize(&new_lines(format.clone())),
                expected,
                "{format:?}"
            );
        }
    }

    #[test]
    fn top_level_items_keep_their_extra_indent() {
        let mut document = Document::default();
        let root = document.root();
        document.push_child(root, "A".into(), 2);
        document.push_child(root, String::new(), 1);
        assert_eq!(serialize(&document), "\t\tA\n\t\n");
    }

    #[test]
    fn edited_text_is_written_with_its_indent_and_keeps_its_line_break() {
        let mut document = Document::new(FileFormat {
            indent: IndentUnit::Spaces(2),
            ..FileFormat::default()
        });
        let root = document.root();
        let a = push_read(&mut document, root, "A", 0, line_origin("A", 0, 0, "\r\n"));
        push_read(
            &mut document,
            a,
            "- new",
            0,
            line_origin("   - old", 3, 1, "\r"),
        );
        assert_eq!(serialize(&document), "A\r\n  - new\r");
    }

    #[test]
    fn lines_are_reindented_when_reusing_them_would_change_the_space_unit() {
        // "P:\n   Q:\n        a\n         b\n  s\n" with the 2-space line `s` deleted.
        let mut document = Document::new(FileFormat {
            indent: IndentUnit::Spaces(2),
            ..FileFormat::default()
        });
        let root = document.root();
        let p = push_read(&mut document, root, "P:", 0, line_origin("P:", 0, 0, "\n"));
        let q = push_read(
            &mut document,
            p,
            "Q:",
            0,
            line_origin("   Q:", 3, 1, "\r\n"),
        );
        push_read(
            &mut document,
            q,
            "a",
            2,
            line_origin("        a", 8, 4, "\n"),
        );
        push_read(
            &mut document,
            q,
            "b",
            2,
            line_origin("         b", 9, 4, "\n"),
        );
        let text = serialize(&document);
        assert_eq!(text, "P:\n  Q:\r\n        a\n        b\n");
        assert_eq!(outline(&parse(&text)), outline(&document));
    }

    // F-FILE-03
    #[test]
    fn edits_change_only_their_lines_and_undo_restores_the_bytes() {
        let text = "A:\r\n  - b\n\n  - c  \r\n";
        let mut document = parse_bytes(text.as_bytes()).unwrap();
        let ids: Vec<_> = document
            .descendants(document.root())
            .map(|(id, _)| id)
            .collect();
        let (b, c) = (ids[1], ids[3]);
        let mut edit = Transaction::new(
            vec![
                Op::SetText {
                    id: b,
                    before: "- b".into(),
                    after: "- B".into(),
                },
                Op::SetText {
                    id: c,
                    before: "- c  ".into(),
                    after: "- c".into(),
                },
            ],
            (),
            (),
        );
        document.apply(&mut edit).unwrap();
        assert_eq!(serialize(&document), "A:\r\n  - B\n\n  - c\r\n");
        document.apply(&mut edit.inverse()).unwrap();
        assert_eq!(serialize(&document), text);
    }

    // F-FILE-03, N-REL-02
    #[test]
    fn a_blank_line_after_a_carriage_return_is_not_read_back_as_part_of_it() {
        let cases: [(&[u8], &str); 3] = [
            (b"- a\r- b\n\n\n- c\n", "- a\r\r\r- c\n- b\n"),
            (b"- a\r- b\n\r\n- c\n", "- a\r\r\n- c\n- b\n"),
            (b"- a\r- b\n- c\n", "- a\r- c\n- b\n"),
        ];
        for (bytes, expected) in cases {
            let mut document = parse_bytes(bytes).unwrap();
            let root = document.root();
            let children = document.get(root).unwrap().children().to_vec();
            let mut edit = Transaction::new(
                vec![Op::Move {
                    id: children[1],
                    from: (root, 1),
                    to: (root, children.len() - 1),
                }],
                (),
                (),
            );
            document.apply(&mut edit).unwrap();
            let text = serialize(&document);
            assert_eq!(text, expected);
            assert_eq!(outline(&parse(&text)), outline(&document), "{text:?}");
        }
    }

    #[test]
    fn a_line_whose_depth_changed_is_reindented() {
        let mut document = Document::default();
        let root = document.root();
        push_read(&mut document, root, "A", 0, line_origin("\tA", 1, 1, "\n"));
        assert_eq!(serialize(&document), "A\n");
    }

    #[test]
    fn line_breaks_follow_the_lines_and_the_end_of_the_file() {
        let mut document = Document::new(FileFormat {
            line_ending: LineEnding::CrLf,
            trailing_newline: false,
            ..FileFormat::default()
        });
        let root = document.root();
        push_read(
            &mut document,
            root,
            "was last",
            0,
            line_origin("was last", 0, 0, ""),
        );
        push_read(
            &mut document,
            root,
            "now last",
            0,
            line_origin("now last", 0, 0, "\u{C}"),
        );
        assert_eq!(serialize(&document), "was last\r\nnow last");
    }

    // file-format §3.1, §6
    #[test]
    fn a_last_empty_line_ends_with_a_line_break_even_without_a_trailing_newline() {
        let mut document = parse("P:");
        let root = document.root();
        document.push_child(root, String::new(), 0);
        assert_eq!(serialize(&document), "P:\n\n");
        let mut document = parse("");
        let root = document.root();
        document.push_child(root, String::new(), 0);
        assert_eq!(serialize(&document), "\n");
        assert_eq!(
            serialize(&parse("P:\n  ")),
            "P:\n  ",
            "a blank line written as read"
        );
    }

    #[test]
    fn a_blank_line_is_kept_while_the_next_line_still_gives_it_its_depth() {
        let mut document = Document::default();
        let root = document.root();
        let a = document.push_child(root, "A".into(), 0);
        push_read(&mut document, a, "", 0, line_origin("", 0, 0, "\n"));
        document.push_child(a, "B".into(), 0);
        assert_eq!(serialize(&document), "A\n\n\tB\n");
    }

    #[test]
    fn a_blank_line_is_reindented_when_the_next_line_no_longer_gives_it_its_depth() {
        let mut document = Document::default();
        let root = document.root();
        let a = document.push_child(root, "A".into(), 0);
        push_read(&mut document, a, "", 0, line_origin("", 0, 0, "\n"));
        document.push_child(root, "C".into(), 0);
        assert_eq!(serialize(&document), "A\n\t\nC\n");
    }

    #[test]
    fn a_blank_line_moved_above_a_shallower_line_loses_its_indentation() {
        let mut document = Document::default();
        let root = document.root();
        push_read(&mut document, root, "", 0, line_origin("\t", 1, 1, "\n"));
        document.push_child(root, "B".into(), 0);
        assert_eq!(serialize(&document), "\nB\n");
    }

    #[test]
    fn whitespace_only_lines_do_not_give_a_blank_line_its_depth() {
        let mut document = Document::default();
        let root = document.root();
        let a = document.push_child(root, "A".into(), 0);
        push_read(&mut document, a, "", 0, line_origin("", 0, 0, "\n"));
        document.push_child(a, " ".into(), 0);
        assert_eq!(serialize(&document), "A\n\t\n\t \n");
    }

    /// One generated line: how many levels to go up first, the extra indent and the text.
    type LineSpec = (usize, usize, String);

    /// Builds an outline that is canonical (file-format.md §3.1): sibling extra indents never
    /// increase (placement), empty items have no children and trailing empty items are top-level.
    fn canonical(specs: &[LineSpec], format: FileFormat) -> Document {
        let trailing_empty = specs
            .iter()
            .rposition(|(_, _, text)| !text.is_empty())
            .map_or(0, |last_text| last_text + 1);
        let mut document = Document::new(format);
        let mut path: Vec<(ItemId, bool, Option<usize>)> = vec![(document.root(), false, None)];
        for (index, (up, extra, text)) in specs.iter().enumerate() {
            let up = if index >= trailing_empty {
                path.len()
            } else {
                *up
            };
            path.truncate(path.len().saturating_sub(up).max(1));
            while path.last().is_some_and(|&(_, empty, _)| empty) {
                path.pop();
            }
            let (parent, _, last_extra) = path.last_mut().unwrap();
            let extra = if index >= trailing_empty {
                0
            } else {
                (*extra).min(last_extra.unwrap_or(*extra))
            };
            *last_extra = Some(extra);
            let parent = *parent;
            let id = document.push_child(parent, text.clone(), extra);
            path.push((id, text.is_empty(), None));
        }
        document
    }

    fn line_specs() -> impl Strategy<Value = Vec<LineSpec>> {
        let text = "([a-z:@()\\\\\\-é][a-z:@()\\\\\\- \t\u{2028}é]{0,8})?";
        prop::collection::vec((0..4usize, 0..3usize, text), 0..12)
    }

    fn formats() -> impl Strategy<Value = FileFormat> {
        let line_ending = prop_oneof![
            Just(LineEnding::Lf),
            Just(LineEnding::CrLf),
            Just(LineEnding::Cr)
        ];
        (line_ending, any::<bool>(), any::<bool>()).prop_map(
            |(line_ending, bom, trailing_newline)| FileFormat {
                indent: IndentUnit::Tab,
                line_ending,
                bom,
                trailing_newline,
            },
        )
    }

    fn file_text() -> impl Strategy<Value = String> {
        let piece = prop_oneof![
            Just("\n"),
            Just("\r\n"),
            Just("\r"),
            Just("\u{C}"),
            Just("\u{2029}"),
            Just("\u{2B7F}"),
            Just("\u{2028}"),
            Just("\u{FEFF}"),
            Just("\t"),
            Just(" "),
            Just("  "),
            Just("\u{A0}"),
            Just("- "),
            Just(":"),
            Just(" @a(1)"),
            Just("x"),
            Just("é"),
        ];
        prop_oneof![
            prop::collection::vec(piece, 0..40).prop_map(|pieces| pieces.concat()),
            any::<String>(),
        ]
    }

    proptest! {
        // F-FILE-03, N-REL-02
        #[test]
        fn any_unedited_text_round_trips(text in file_text()) {
            let document = parse_bytes(text.as_bytes()).unwrap();
            prop_assert_eq!(serialize(&document), text);
        }

        // F-FILE-03, N-REL-02
        #[test]
        fn canonical_outlines_parse_back_to_the_same_outline(
            specs in line_specs(),
            format in formats(),
        ) {
            let document = canonical(&specs, format);
            let text = serialize(&document);
            let reparsed = parse_bytes(text.as_bytes()).unwrap();
            prop_assert_eq!(outline(&reparsed), outline(&document), "{:?}", text);
            prop_assert_eq!(serialize(&reparsed), text);
        }
    }

    #[test]
    fn parse_without_bom_keeps_a_leading_bom_in_the_text() {
        let document = parse("\u{FEFF}A");
        assert_eq!(serialize(&document), "\u{FEFF}A");
        assert!(!document.format().bom);
    }

    /// The text `serialize_blocks` gives for the lines of `text` reading each of `tops`.
    #[track_caller]
    fn copied(text: &str, tops: &[&str]) -> String {
        let document = parse(text);
        let ids: Vec<ItemId> = tops
            .iter()
            .map(|top| {
                document
                    .descendants(document.root())
                    .map(|(id, _)| id)
                    .find(|&id| document.get(id).unwrap().text() == *top)
                    .unwrap()
            })
            .collect();
        serialize_blocks(&document, &ids)
    }

    // FR-OPS-06, special-nodes.md §1
    #[test]
    fn copied_blocks_keep_their_subtrees_tags_and_special_nodes_with_tabs_and_line_feeds() {
        assert_eq!(
            copied(
                "x\r\nP: @a\r\n  - b @due(1)\r\n    n\r\n  C @hidden\r\n    c\r\ny",
                &["P: @a"]
            ),
            "P: @a\n\t- b @due(1)\n\t\tn\n\tC @hidden\n\t\tc\n"
        );
    }

    // FR-OPS-06, editing.md §4
    #[test]
    fn copied_blocks_are_indented_relative_to_the_shallowest() {
        assert_eq!(
            copied("P:\n\t- a\n\t\t- b\n\t- c\nQ:", &["- a", "- c"]),
            "- a\n\t- b\n- c\n"
        );
        assert_eq!(
            copied("P:\n\t- a\n\t\t- b\nQ:", &["- b", "Q:"]),
            "\t\t- b\nQ:\n",
            "a deeper first block keeps its depth below a later shallower one"
        );
        assert_eq!(
            copied("a\n\t\t\tb\nc", &["a", "c"]),
            "a\n\t\t\tb\nc\n",
            "an indent jump is kept"
        );
        assert_eq!(copied("a\n\nb", &[""]), "\n", "a blank line");
    }

    #[test]
    fn copying_nothing_or_lines_not_in_the_document_gives_no_text() {
        let document = parse("a");
        let other = parse("x\ny");
        let stranger = other.descendants(other.root()).nth(1).unwrap().0;
        assert_eq!(serialize_blocks(&document, &[]), "");
        assert_eq!(serialize_blocks(&document, &[stranger]), "");
    }
}
