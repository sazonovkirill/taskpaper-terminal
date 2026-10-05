//! A line, or a block's lines, split into the text the user edits and what edit mode keeps around
//! it: the type syntax and the tags ([editing.md §2](../../../../docs/specs/editing.md#2-edit-mode),
//! OQ-24, OQ-27).

use super::{canonical::PLACEHOLDER, display_text, kind, tags};
use crate::document::ItemKind;

/// A line's type syntax and tags, which stay while the text between them is edited. The default
/// is a note's: no syntax and no tags.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LineParts {
    /// A task's dash and the whitespace after it; empty for other types.
    prefix: String,
    /// Whether a project's `:` follows the text.
    project: bool,
    /// Every tag occurrence as written, without the whitespace before it, in order.
    tags: Vec<String>,
}

impl LineParts {
    /// The parts of `line`, and its display text, which is what edit mode edits.
    #[must_use]
    pub fn split(line: &str) -> (Self, String) {
        let prefix = match kind(line) {
            ItemKind::Task => line.chars().take(2).collect(),
            ItemKind::Project | ItemKind::Note => String::new(),
        };
        let tags = tags::scan(line)
            .into_iter()
            .filter_map(|tag| line.get(tag.span))
            .map(|tag| tag.trim_start_matches(tags::is_whitespace).to_owned())
            .collect();
        let parts = Self {
            prefix,
            project: kind(line) == ItemKind::Project,
            tags,
        };
        (parts, display_text(line))
    }

    /// What follows the text on screen as part of the type: a project's `:`, else nothing.
    #[must_use]
    pub fn suffix(&self) -> &'static str {
        if self.project { ":" } else { "" }
    }

    /// The line holding `text`: the type syntax around it and the tags after it in their
    /// original order, so tags from the middle of the line move to its end (OQ-24). The text of a
    /// task that would stop being one, because only whitespace stands before its tags, becomes
    /// the placeholder.
    #[must_use]
    pub fn line(&self, text: &str) -> String {
        let line = self.compose(text);
        if !self.prefix.is_empty() && kind(&line) != ItemKind::Task {
            return self.compose(PLACEHOLDER);
        }
        line
    }

    fn compose(&self, text: &str) -> String {
        let mut line = format!("{}{text}{}", self.prefix, self.suffix());
        for tag in &self.tags {
            if !line.is_empty() && !line.ends_with(tags::is_whitespace) {
                line.push(' ');
            }
            line.push_str(tag);
        }
        line
    }

    /// When `text` makes the line a task although these are not a task's parts: the task's parts
    /// and the text after its dash and whitespace, which give the same line. The dash then shows
    /// as the task's marker instead of as text (OQ-27).
    #[must_use]
    pub fn as_task(&self, text: &str) -> Option<(Self, String)> {
        if !self.prefix.is_empty() || kind(&self.line(text)) != ItemKind::Task {
            return None;
        }
        let split = text.char_indices().nth(2).map_or(text.len(), |(at, _)| at);
        let (prefix, rest) = text.split_at(split);
        let parts = Self {
            prefix: prefix.to_owned(),
            project: false,
            tags: self.tags.clone(),
        };
        Some((parts, format!("{rest}{}", self.suffix())))
    }
}

/// The lines of a block as edit mode edits them: one text with a line break between the lines'
/// display texts, each line keeping its own type syntax and tags (editing.md §2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockLines {
    /// Each line as written, with its parts and display text.
    lines: Vec<(String, LineParts, String)>,
}

impl BlockLines {
    /// The block made of `lines`, and the text edit mode edits: their display texts joined by
    /// line breaks.
    #[must_use]
    pub fn split<'a>(lines: impl IntoIterator<Item = &'a str>) -> (Self, String) {
        let lines: Vec<(String, LineParts, String)> = lines
            .into_iter()
            .map(|line| {
                let (parts, text) = LineParts::split(line);
                (line.to_owned(), parts, text)
            })
            .collect();
        let text = lines
            .iter()
            .map(|(_, _, text)| text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        (Self { lines }, text)
    }

    /// The first line's parts, which edit mode may change as the text gives the line a type
    /// (OQ-27).
    #[must_use]
    pub fn first(&self) -> LineParts {
        self.lines
            .first()
            .map(|(_, parts, _)| parts.clone())
            .unwrap_or_default()
    }

    /// The lines that `text` gives, with `first` as the first line's parts: each with the index
    /// of the line it rewrites, or `None` for an added line. The unchanged lines at either end of
    /// the text keep their own lines, and the changed lines between them rewrite the lines they
    /// replace in order, so a line keeps its tags while its text is edited and the first line
    /// always rewrites the first line. The tags of the lines that are gone move to the end of the
    /// first line, so none is lost (FR-DOC-03). A line whose text and parts are as they were is
    /// written as it was, so its mid-line tags stay where they are.
    #[must_use]
    pub fn lines(&self, first: &LineParts, text: &str) -> Vec<(Option<usize>, String)> {
        let new: Vec<&str> = text.split('\n').collect();
        let old: Vec<&str> = self
            .lines
            .iter()
            .map(|(_, _, text)| text.as_str())
            .collect();
        let prefix = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
        let room = old.len().min(new.len()).saturating_sub(prefix.max(1));
        let suffix = old
            .iter()
            .rev()
            .zip(new.iter().rev())
            .take(room)
            .take_while(|(a, b)| a == b)
            .count();
        let origin = |index: usize| {
            if index >= new.len() - suffix {
                Some(old.len() - (new.len() - index))
            } else {
                Some(index).filter(|&index| index < old.len() - suffix)
            }
        };
        let gone = new.len().min(old.len()) - suffix..old.len() - suffix;
        let mut first = first.clone();
        first.tags.extend(
            self.lines
                .get(gone)
                .unwrap_or_default()
                .iter()
                .flat_map(|(_, parts, _)| parts.tags.iter().cloned()),
        );
        let new_line = LineParts::default();
        new.iter()
            .enumerate()
            .map(|(index, text)| {
                let from = origin(index);
                let parts = match from {
                    Some(0) => &first,
                    Some(at) => self.lines.get(at).map_or(&new_line, |(_, parts, _)| parts),
                    None => &new_line,
                };
                let line = match from.and_then(|at| self.lines.get(at)) {
                    Some((line, was, old)) if old == text && was == parts => line.clone(),
                    _ => parts.line(text),
                };
                (from, line)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `line` with its display text replaced by `text`.
    #[track_caller]
    fn edited(line: &str, text: &str) -> String {
        LineParts::split(line).0.line(text)
    }

    #[test]
    fn splits_off_the_display_text() {
        for (line, text) in [
            ("- Buy milk @due(1)", "Buy milk"),
            ("Inbox: @a", "Inbox"),
            ("note @x more", "note more"),
            ("", ""),
        ] {
            assert_eq!(LineParts::split(line).1, text, "{line:?}");
        }
    }

    // editing.md §2
    #[test]
    fn the_unchanged_text_gives_the_line_back_when_tags_trail() {
        for line in [
            "- Buy milk @due(1)",
            "-\tBuy milk",
            "Inbox: @a @b(x)",
            "Inbox:",
            "a note",
            "f() @code",
            "@x",
            "",
            "-  two spaces",
        ] {
            assert_eq!(edited(line, &LineParts::split(line).1), line);
        }
    }

    // editing.md §2, OQ-24
    #[test]
    fn edited_text_keeps_the_type_syntax_and_appends_the_tags_in_order() {
        assert_eq!(edited("- a @due(1) b @x", "c"), "- c @due(1) @x");
        assert_eq!(edited("Inbox @a later: @b", "Mail"), "Mail: @a @b");
        assert_eq!(edited("@today note", "text"), "text @today");
        assert_eq!(edited("- a @x", "b "), "- b @x", "no second space");
        assert_eq!(edited("a @x", ""), "@x");
        assert_eq!(edited("- a @x @x", "b"), "- b @x @x", "duplicates are kept");
        assert_eq!(edited("a @id", "b @id"), "b @id", "reserved names are text");
        assert_eq!(edited("- a", ""), "- ");
        assert_eq!(edited("P:", ""), ":");
    }

    // OQ-27
    #[test]
    fn the_text_decides_a_type_change() {
        assert_eq!(kind(&edited("note", "Questions:")), ItemKind::Project);
        assert_eq!(kind(&edited("note @x", "- a")), ItemKind::Task);
        assert_eq!(kind(&edited("- a", "b:")), ItemKind::Task);
        assert_eq!(kind(&edited("P:", "- a")), ItemKind::Task);
    }

    // file-format §4
    #[test]
    fn a_task_left_without_text_before_its_tags_gets_the_placeholder() {
        assert_eq!(edited("- a @done", ""), "- … @done");
        assert_eq!(edited("- a @done", "  "), "- … @done");
        assert_eq!(edited("- a", "  "), "-   ", "without tags it stays a task");
    }

    #[test]
    fn the_suffix_is_a_projects_colon() {
        assert_eq!(LineParts::split("P: @x").0.suffix(), ":");
        assert_eq!(LineParts::split("- P:").0.suffix(), "");
        assert_eq!(LineParts::split("P").0.suffix(), "");
    }

    // OQ-27
    #[test]
    fn text_starting_a_task_turns_into_the_tasks_parts() {
        let (note, _) = LineParts::split("a @x");
        let (task, text) = note.as_task("- b").unwrap();
        assert_eq!(text, "b");
        assert_eq!(task.line(&text), note.line("- b"));
        assert_eq!(task.line(&text), "- b @x");
        let (project, _) = LineParts::split("P:");
        let (task, text) = project.as_task("-\u{3000}P").unwrap();
        assert_eq!(text, "P:", "the project's colon becomes text");
        assert_eq!(task.line(&text), "-\u{3000}P:");
        assert_eq!(task.suffix(), "");
    }

    #[test]
    fn text_that_starts_no_task_keeps_the_parts() {
        let (note, _) = LineParts::split("a @x");
        assert_eq!(note.as_task("b - c"), None);
        assert_eq!(note.as_task("-"), None);
        assert_eq!(note.as_task("- "), None, "a dash before tags is a note");
        let (task, _) = LineParts::split("- a");
        assert_eq!(task.as_task("- b"), None, "already a task");
    }

    /// The lines `text` gives the block of `lines`, with its first line's parts as they were.
    #[track_caller]
    fn relined(lines: &[&str], text: &str) -> Vec<(Option<usize>, String)> {
        let (block, _) = BlockLines::split(lines.iter().copied());
        block.lines(&block.first(), text)
    }

    #[test]
    fn a_block_edits_its_display_texts_joined_by_line_breaks() {
        let (block, text) = BlockLines::split(["a @x", "", "- b"]);
        assert_eq!(text, "a\n\nb");
        assert_eq!(block.first(), LineParts::split("a @x").0);
        let (none, text) = BlockLines::split([]);
        assert_eq!((none.first(), text), (LineParts::default(), String::new()));
    }

    // editing.md §2
    #[test]
    fn unchanged_lines_are_written_as_they_were() {
        assert_eq!(
            relined(&["a @x b", "c @y d"], "a b\nc d"),
            [(Some(0), "a @x b".into()), (Some(1), "c @y d".into())]
        );
    }

    // editing.md §2, OQ-24
    #[test]
    fn a_changed_line_keeps_its_own_tags() {
        assert_eq!(
            relined(&["a @x", "b @y", "c @z"], "a\nB\nc"),
            [
                (Some(0), "a @x".into()),
                (Some(1), "B @y".into()),
                (Some(2), "c @z".into())
            ]
        );
    }

    // editing.md §2
    #[test]
    fn added_lines_are_new_notes_between_the_lines_around_them() {
        assert_eq!(
            relined(&["a @x", "b @y"], "a1\na2\nb"),
            [
                (Some(0), "a1 @x".into()),
                (None, "a2".into()),
                (Some(1), "b @y".into())
            ]
        );
        assert_eq!(
            relined(&["a"], "a\n"),
            [(Some(0), "a".into()), (None, String::new())]
        );
        assert_eq!(
            relined(&["a", "a"], "a\na\na"),
            [
                (Some(0), "a".into()),
                (Some(1), "a".into()),
                (None, "a".into())
            ]
        );
    }

    // editing.md §2, FR-DOC-03
    #[test]
    fn the_tags_of_removed_lines_move_to_the_first_line() {
        assert_eq!(
            relined(&["a @x", "b @y", "c @z"], "a\nbc"),
            [(Some(0), "a @x @z".into()), (Some(1), "bc @y".into())]
        );
        assert_eq!(
            relined(&["a @x", "b @y"], "b"),
            [(Some(0), "b @x @y".into())],
            "the first line stays the first"
        );
        assert_eq!(relined(&["a", "b", "c"], "c"), [(Some(0), "c".into())]);
        assert_eq!(
            relined(&["a", "b @y", "c @z", "d"], "a\nd"),
            [(Some(0), "a @y @z".into()), (Some(3), "d".into())]
        );
    }

    // OQ-27
    #[test]
    fn the_first_lines_parts_are_the_ones_given() {
        let (block, _) = BlockLines::split(["a @x"]);
        let (task, text) = block.first().as_task("- b").unwrap();
        assert_eq!(block.lines(&task, &text), [(Some(0), "- b @x".into())]);
        assert_eq!(
            relined(&["a", "b"], "a\n- b"),
            [(Some(0), "a".into()), (Some(1), "- b".into())],
            "a later line reads as typed"
        );
    }
}
