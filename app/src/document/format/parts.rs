//! A line split into the text the user edits and what edit mode keeps around it: the type syntax
//! and the tags ([editing.md §2](../../../../docs/specs/editing.md#2-edit-mode),
//! OQ-24, OQ-27).

use super::{canonical::PLACEHOLDER, display_text, kind, tags};
use crate::document::ItemKind;

/// A type the user gives a block: when adding it (FR-MODE-06, D-26) or from the command menu
/// (D-44).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Typed {
    /// A project.
    Project,
    /// A section.
    Section,
    /// A task.
    Task,
    /// Text.
    Text,
    /// A divider.
    Divider,
}

/// The tag that gives a line its type, which a type change takes off unless it makes a section
/// (editing.md §5, D-45).
const SECTION: &str = "section";

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

impl LineParts {
    /// The parts and text that give the line holding `text` the type `typed`: the old type's
    /// syntax and `@section` go, the new type's come and every other tag, `@url` included, stays
    /// (editing.md §5, D-45). A divider's text is `---` (D-44).
    #[must_use]
    pub fn retyped(&self, typed: Typed, text: &str) -> (Self, String) {
        let section = typed == Typed::Section;
        let mut tags: Vec<String> = self
            .tags
            .iter()
            .filter(|tag| section || tags::scan(tag).iter().all(|found| found.name != SECTION))
            .cloned()
            .collect();
        let sectioned = tags
            .iter()
            .any(|tag| tags::scan(tag).iter().any(|found| found.name == SECTION));
        if section && !sectioned {
            tags.push("@section".to_owned());
        }
        let prefix = match typed {
            Typed::Task if self.prefix.is_empty() => "- ".to_owned(),
            Typed::Task => self.prefix.clone(),
            _ => String::new(),
        };
        let text = if typed == Typed::Divider { "---" } else { text };
        let parts = Self {
            prefix,
            project: typed == Typed::Project,
            tags,
        };
        (parts, text.to_owned())
    }

    /// These parts with the tag `name` set to `value` as [`tags::set`] sets it on a line: the
    /// first occurrence's value replaced, else the tag added after the others.
    #[must_use]
    pub fn tagged(mut self, name: &str, value: &str) -> Self {
        let found = self
            .tags
            .iter_mut()
            .find(|tag| tags::scan(tag).iter().any(|found| found.name == name));
        match found {
            Some(tag) => *tag = tags::set(tag, name, value),
            None => self.tags.push(tags::set("", name, value)),
        }
        self
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

    /// The line that `line` becomes when its display text takes the type `typed`.
    #[track_caller]
    fn retyped(line: &str, typed: Typed) -> String {
        let (parts, text) = LineParts::split(line);
        let (parts, text) = parts.retyped(typed, &text);
        parts.line(&text)
    }

    // editing.md §5, D-44
    #[test]
    fn a_type_change_swaps_the_type_syntax_and_keeps_other_tags() {
        let types = [Typed::Project, Typed::Task, Typed::Text, Typed::Section];
        let lines = ["P: @x", "- P @x", "P @x", "P @x @section"];
        for from in lines {
            for (typed, to) in types.into_iter().zip(lines) {
                assert_eq!(retyped(from, typed), to, "{from:?} to {typed:?}");
            }
        }
        let linked = [
            "P: @url(u) @x",
            "- P @url(u) @x",
            "P @url(u) @x",
            "P @url(u) @x @section",
        ];
        for from in linked {
            for (typed, to) in types.into_iter().zip(linked) {
                assert_eq!(retyped(from, typed), to, "{from:?} keeps @url as {typed:?}");
            }
        }
        assert_eq!(retyped("-\ta", Typed::Task), "-\ta", "kept as it is");
        assert_eq!(
            retyped("a @section @x", Typed::Section),
            "a @section @x",
            "the tag keeps its place"
        );
        assert_eq!(retyped("f() @code", Typed::Task), "- f() @code");
    }

    // D-44
    #[test]
    fn a_divider_takes_dashes_for_its_text_and_keeps_other_tags() {
        assert_eq!(retyped("- a @x @section", Typed::Divider), "--- @x");
        assert_eq!(retyped("P:", Typed::Divider), "---");
    }

    // FR-MODE-06
    #[test]
    fn an_empty_line_takes_the_syntax_of_its_type() {
        for (typed, line) in [
            (Typed::Project, ":"),
            (Typed::Section, "@section"),
            (Typed::Task, "- "),
            (Typed::Text, ""),
            (Typed::Divider, "---"),
        ] {
            assert_eq!(retyped("", typed), line, "{typed:?}");
        }
    }

    // FR-MODE-05, FR-DONE-01
    #[test]
    fn a_tag_set_on_the_parts_replaces_its_value_or_comes_last() {
        let tagged = |line: &str| {
            let (parts, text) = LineParts::split(line);
            parts.tagged("done", "2026-10-05").line(&text)
        };
        assert_eq!(tagged("- a @x"), "- a @x @done(2026-10-05)");
        assert_eq!(tagged("- a @done @x"), "- a @done(2026-10-05) @x");
        let (task, _) = LineParts::split("- a");
        assert_eq!(
            task.tagged("done", "2026-10-05").line(""),
            "- \u{2026} @done(2026-10-05)",
            "an empty task keeps the placeholder"
        );
        assert_eq!(tagged(""), "@done(2026-10-05)");
    }
}
