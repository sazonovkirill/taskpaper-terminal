//! A block's line as the properties the Edit Node dialog changes: its type, its text and its
//! tags, read from the line and written back to it
//! ([editing.md §7](../../../docs/specs/editing.md#7-edit-node-dialog-fr-mode-09), D-103).

use super::format::{
    self, LineParts, Typed,
    canonical::PLACEHOLDER,
    tags::{self, Tag},
};
use super::model::ItemKind;

/// The tag that makes a text block a section (D-52).
const SECTION: &str = "section";

/// A block's type, text and tags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Properties {
    /// Project, task or text; a section is text with `@section` among the tags.
    pub typed: Typed,
    /// The text as edit mode edits it, without a project's `:`, which the type gives.
    pub text: String,
    /// Each tag's name and value, `None` for a bare tag, in the line's order; a name occurs once,
    /// with its first occurrence's value, as TaskPaper reads it.
    pub tags: Vec<(String, Option<String>)>,
    /// Each tag renamed since the line was read, as its name in the line and its name now, so
    /// that only the occurrence listed changes and a later duplicate stays.
    renamed: Vec<(String, String)>,
}

impl Properties {
    /// The properties of `line`.
    #[must_use]
    pub fn of(line: &str) -> Self {
        let typed = match format::kind(line) {
            ItemKind::Project => Typed::Project,
            ItemKind::Task => Typed::Task,
            ItemKind::Note => Typed::Text,
        };
        let (_, mut text) = LineParts::split(line);
        if typed == Typed::Project {
            text.pop();
        }
        let mut tags: Vec<(String, Option<String>)> = Vec::new();
        for Tag { name, value, .. } in tags::scan(line) {
            if tags.iter().all(|(seen, _)| *seen != name) {
                tags.push((name, value.filter(|value| !value.is_empty())));
            }
        }
        Self {
            typed,
            text,
            tags,
            renamed: Vec::new(),
        }
    }

    /// The value of the tag `name`: `None` without it, `Some(None)` when it is bare.
    #[must_use]
    pub fn tag(&self, name: &str) -> Option<Option<&str>> {
        self.tags
            .iter()
            .find(|(tag, _)| tag == name)
            .map(|(_, value)| value.as_deref())
    }

    /// Gives the tag `name` the value `value`, where it stands or after the others, an empty
    /// value making it bare.
    pub fn set(&mut self, name: &str, value: Option<String>) {
        let value = value.filter(|value| !value.is_empty());
        match self.tags.iter_mut().find(|(tag, _)| tag == name) {
            Some((_, old)) => *old = value,
            None => self.tags.push((name.to_owned(), value)),
        }
    }

    /// Renames the tag `from` to `to` with the value `value`, where it stands, an empty value
    /// making it bare; another tag `to` goes, as a name occurs once.
    pub fn rename(&mut self, from: &str, to: &str, value: Option<String>) {
        let value = value.filter(|value| !value.is_empty());
        self.tags.retain(|(tag, _)| tag != to || tag == from);
        for tag in &mut self.tags {
            if tag.0 == from {
                *tag = (to.to_owned(), value.clone());
            }
        }
        match self.renamed.iter_mut().find(|(_, now)| now == from) {
            Some((_, now)) => to.clone_into(now),
            None => self.renamed.push((from.to_owned(), to.to_owned())),
        }
        self.renamed.retain(|(was, now)| was != now);
    }

    /// Takes the tag `name` off.
    pub fn remove(&mut self, name: &str) {
        self.tags.retain(|(tag, _)| tag != name);
        self.renamed.retain(|(_, now)| now != name);
    }

    /// The line `before` with these properties, changed only where they differ from its own: a
    /// changed text or type rewrites it as edit mode writes a line back, then each tag it loses
    /// goes, every occurrence, a renamed tag takes its name where its first occurrence stands, the
    /// later ones kept, or goes when the line has its new name already, each changed tag takes its
    /// value where it stands and each new one goes at the end (editing.md §7). A task without text but with tags gets the placeholder,
    /// as `- @tag` would read back as text, and so does a line left empty.
    #[must_use]
    pub fn line(&self, before: &str) -> String {
        let old = Self::of(before);
        let text = if self.typed == Typed::Task
            && self.text.trim_matches(tags::is_whitespace).is_empty()
            && !self.tags.is_empty()
        {
            PLACEHOLDER
        } else {
            &self.text
        };
        let mut line = before.to_owned();
        if self.typed != old.typed || text != old.text {
            let typed = if self.typed == Typed::Text && self.tag(SECTION).is_some() {
                Typed::Section
            } else {
                self.typed
            };
            let (parts, _) = LineParts::split(before);
            let (parts, text) = parts.retyped(typed, text);
            line = parts.line(&text);
        }
        for (name, _) in &old.tags {
            if self.tag(name).is_none() && self.renamed.iter().all(|(was, _)| was != name) {
                line = tags::remove(&line, name);
            }
        }
        for (was, now) in &self.renamed {
            line = if Self::of(&line).tag(now).is_some() {
                tags::remove_first(&line, was)
            } else {
                tags::rename(&line, was, now)
            };
        }
        for (name, value) in &self.tags {
            if Self::of(&line).tag(name) != Some(value.as_deref()) {
                line = tags::set(&line, name, value.as_deref().unwrap_or_default());
            }
        }
        if line.is_empty() && !before.is_empty() {
            PLACEHOLDER.to_owned()
        } else {
            line
        }
    }
}

/// What the Edit Node dialog's tag field holds when it is no tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TagError {
    /// The name holds a character a tag name cannot, or none.
    #[error("not a tag name")]
    Name,
    /// The name is one TaskPaper keeps for itself, `id`, `text` or `type`.
    #[error("a name TaskPaper keeps for itself")]
    Reserved,
    /// The name is `hidden`, which only special nodes carry (special-nodes.md §1).
    #[error("@hidden is kept for hidden data")]
    Hidden,
    /// The value lacks the parenthesis that closes it.
    #[error("the value needs its closing )")]
    Value,
    /// Text follows the parenthesis that closes the value, such as a second tag.
    #[error("nothing can follow the value's closing )")]
    Trailing,
}

/// The tag `text` names: `name` or `name(value)`, a leading `@` and surrounding whitespace
/// dropped; an empty value makes a bare tag, and parentheses in a value pair up. `Ok(None)` for blank text, which removes a tag. Any
/// value reads back as itself, since [`tags::set`] escapes its parentheses.
///
/// # Errors
///
/// What keeps `text` from being a tag.
pub fn parse_tag(text: &str) -> Result<Option<(String, Option<String>)>, TagError> {
    let text = text.trim_matches(tags::is_whitespace);
    let text = text.strip_prefix('@').unwrap_or(text);
    if text.is_empty() {
        return Ok(None);
    }
    let (name, value) = match text.split_once('(') {
        Some((name, rest)) => match closed(rest).ok_or(TagError::Value)? {
            (value, "") => (name, Some(value)),
            _ => return Err(TagError::Trailing),
        },
        None => (text, None),
    };
    if matches!(name, "id" | "text" | "type") {
        return Err(TagError::Reserved);
    }
    if name == "hidden" {
        return Err(TagError::Hidden);
    }
    let bare = format!("@{name}");
    if !matches!(tags::scan(&bare).as_slice(), [tag] if tag.name == name && tag.span.end == bare.len())
    {
        return Err(TagError::Name);
    }
    let value = value.filter(|value| !value.is_empty());
    Ok(Some((name.to_owned(), value.map(str::to_owned))))
}

/// `text` split at the `)` that closes a value whose `(` came just before it, and after it.
fn closed(text: &str) -> Option<(&str, &str)> {
    let mut depth = 0_usize;
    for (at, character) in text.char_indices() {
        match character {
            '(' => depth += 1,
            ')' if depth == 0 => return text.get(..at).zip(text.get(at + 1..)),
            ')' => depth -= 1,
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The line `before` becomes once `change` changes its properties.
    #[track_caller]
    fn changed(before: &str, change: impl FnOnce(&mut Properties)) -> String {
        let mut properties = Properties::of(before);
        change(&mut properties);
        properties.line(before)
    }

    fn tags(pairs: &[(&str, Option<&str>)]) -> Vec<(String, Option<String>)> {
        pairs
            .iter()
            .map(|&(name, value)| (name.to_owned(), value.map(str::to_owned)))
            .collect()
    }

    // editing.md §7
    #[test]
    fn reads_the_type_the_text_without_a_projects_colon_and_each_tag_once() {
        let task = Properties::of("- Buy @due(2026-10-08) @x @due(2026-11-01) @y()");
        assert_eq!(task.typed, Typed::Task);
        assert_eq!(task.text, "Buy");
        assert_eq!(
            task.tags,
            tags(&[("due", Some("2026-10-08")), ("x", None), ("y", None)]),
            "the first occurrence counts, and an empty value is bare"
        );
        let project = Properties::of("Work: @star");
        assert_eq!(
            (project.typed, project.text.as_str()),
            (Typed::Project, "Work")
        );
        let section = Properties::of("Later @section");
        assert_eq!(
            (section.typed, section.text.as_str()),
            (Typed::Text, "Later")
        );
        assert_eq!(section.tag("section"), Some(None));
        assert_eq!(section.tag("x"), None);
        assert_eq!(task.tag("due"), Some(Some("2026-10-08")));
    }

    // editing.md §7
    #[test]
    fn an_unchanged_line_is_written_as_it_was() {
        for line in [
            "- a @x b @y",
            "Work: @x",
            "a @x @x(2)",
            "- …",
            "",
            "---",
            "- @x",
        ] {
            assert_eq!(changed(line, |_| {}), line);
        }
    }

    // editing.md §7, §2
    #[test]
    fn a_changed_text_or_type_is_written_back_as_edit_mode_writes_it() {
        assert_eq!(
            changed("- a @x b @y", |p| p.text = "c".into()),
            "- c @x @y",
            "mid-line tags move to the end"
        );
        assert_eq!(changed("- a @x", |p| p.typed = Typed::Project), "a: @x");
        assert_eq!(changed("a: @x", |p| p.typed = Typed::Task), "- a @x");
        assert_eq!(changed("a: @x", |p| p.typed = Typed::Text), "a @x");
        assert_eq!(
            changed("- a: @x", |p| p.typed = Typed::Text),
            "a @x",
            "colons that would make a project go"
        );
        assert_eq!(
            changed("a @section @x", |p| p.typed = Typed::Task),
            "- a @x @section",
            "a listed @section stays"
        );
        assert_eq!(
            changed("a @x @section", |p| p.text = "b".into()),
            "b @x @section"
        );
        assert_eq!(changed("Work:", |p| p.text = "Home".into()), "Home:");
    }

    // editing.md §7, file-format.md §3.1
    #[test]
    fn a_task_without_text_but_with_tags_gets_the_placeholder() {
        assert_eq!(changed("- ", |p| p.set("x", None)), "- … @x");
        assert_eq!(changed("- a @x", |p| p.text = " ".into()), "- … @x");
        assert_eq!(changed("- a", |p| p.text = String::new()), "- ");
        assert_eq!(
            changed("a @x", |p| {
                p.text = String::new();
                p.remove("x");
            }),
            "…",
            "an empty line gets it too"
        );
    }

    // editing.md §7
    #[test]
    fn tags_are_removed_changed_in_place_and_added_at_the_end() {
        assert_eq!(
            changed("- a @x(1) b @y @x(2)", |p| p.remove("x")),
            "- a b @y",
            "every occurrence goes"
        );
        assert_eq!(
            changed("- a @x(1) b @y", |p| p.set("x", Some("(2)".into()))),
            r"- a @x(\(2\)) b @y"
        );
        assert_eq!(
            changed("- a @y", |p| {
                p.set("z", None);
                p.set("x", Some(String::new()));
            }),
            "- a @y @z @x",
            "in the order added, an empty value bare"
        );
        assert_eq!(
            changed("- a @y(1)", |p| p.set("y", None)),
            "- a @y",
            "a value can go"
        );
    }

    // editing.md §7
    #[test]
    fn a_renamed_tag_changes_where_it_stands_and_a_duplicate_stays() {
        assert_eq!(
            changed("- a @context(phone) @x @context(work) @y(1)", |p| {
                p.rename("context", "place", Some("home".into()));
            }),
            "- a @place(home) @x @context(work) @y(1)",
            "the duplicate the dialog never listed stays"
        );
        assert_eq!(
            changed("- a @x(1) @y @x(2)", |p| {
                p.rename("x", "z", None);
                p.rename("z", "w", Some("3".into()));
            }),
            "- a @w(3) @y @x(2)",
            "renamed twice"
        );
        assert_eq!(
            changed("- a @x(1) @y @x(2)", |p| {
                p.rename("x", "z", None);
                p.rename("z", "x", Some("1".into()));
            }),
            "- a @x(1) @y @x(2)",
            "renamed back"
        );
        assert_eq!(
            changed("- a @x(1) b @y @x(2)", |p| {
                p.rename("x", "y", Some("3".into()));
            }),
            "- a b @y(3) @x(2)",
            "onto a name the line has: the listed one goes, its value on the other"
        );
        assert_eq!(
            changed("- a @x @x(2)", |p| {
                p.rename("x", "z", None);
                p.remove("z");
            }),
            "- a",
            "renamed and removed: every occurrence goes"
        );
        assert_eq!(
            changed("- a", |p| {
                p.set("n", None);
                p.rename("n", "m", Some("1".into()));
            }),
            "- a @m(1)",
            "a tag added and renamed goes at the end"
        );
    }

    // editing.md §7
    #[test]
    fn a_tag_is_a_name_and_an_optional_value() {
        let tag =
            |name: &str, value: Option<&str>| Ok(Some((name.to_owned(), value.map(str::to_owned))));
        assert_eq!(parse_tag("  "), Ok(None));
        assert_eq!(parse_tag("@"), Ok(None));
        assert_eq!(parse_tag(" @context "), tag("context", None));
        assert_eq!(
            parse_tag("due(next (week))"),
            tag("due", Some("next (week)"))
        );
        assert_eq!(parse_tag("x()"), tag("x", None));
        assert_eq!(parse_tag("x(a"), Err(TagError::Value));
        assert_eq!(parse_tag("x(a (b) c)"), tag("x", Some("a (b) c")));
        assert_eq!(parse_tag("x(a(b)"), Err(TagError::Value));
        assert_eq!(parse_tag("x(a)b"), Err(TagError::Trailing));
        assert_eq!(parse_tag("x(a) b"), Err(TagError::Trailing));
        assert_eq!(
            parse_tag("context(phone)context(home)"),
            Err(TagError::Trailing),
            "one tag at a time"
        );
        assert_eq!(parse_tag("x(a)(b)"), Err(TagError::Trailing));
        assert_eq!(parse_tag("a b"), Err(TagError::Name));
        assert_eq!(parse_tag("(a)"), Err(TagError::Name));
        assert_eq!(parse_tag("a:b"), Err(TagError::Name));
        assert_eq!(parse_tag("type"), Err(TagError::Reserved));
        assert_eq!(parse_tag("hidden(x)"), Err(TagError::Hidden));
    }
}
