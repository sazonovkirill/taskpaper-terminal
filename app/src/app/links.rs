//! Opening the focused block's link in the system browser: which address a block has and
//! which the frontend may hand to an opener ([editing.md §5](../../../docs/specs/editing.md#5-block-commands),
//! FR-BLK-06, D-45).

use super::{Effect, Notice, State};
use crate::document::{
    Item, blocks,
    inline::{self, Syntax},
};

impl State {
    /// Asks the frontend to open the focused block's link, if it has one, also while blocks are
    /// selected (FR-BLK-06).
    pub(super) fn open_url(&self) -> Vec<Effect> {
        self.pane
            .focus
            .and_then(|id| {
                let kind = blocks::block(&self.document, id)?.kind();
                address(self.document.get(id)?, Syntax::of(kind))
            })
            .map(Effect::OpenUrl)
            .into_iter()
            .collect()
    }

    /// Says that the browser could not be started on `url`, for `reason`.
    pub(super) fn open_url_failed(&mut self, url: String, reason: String) {
        self.notify(Notice::OpenUrlFailed { url, reason });
    }
}

/// The address of the line `item`, whose text has `syntax`: its `@url` value when that is a URL,
/// else the first link in its text whose address is a URL (D-45, FR-FMT-03). A block with several
/// opens the first until a picker can offer them all (editing.md §2.2).
fn address(item: &Item, syntax: Syntax) -> Option<String> {
    item.tags()
        .iter()
        .find(|tag| tag.name == "url")
        .and_then(|tag| tag.value.clone())
        .map(|value| value.trim().to_owned())
        .filter(|value| inline::has_scheme(value))
        .or_else(|| {
            inline::urls(item.display_text(), syntax)
                .into_iter()
                .find(|url| inline::has_scheme(url))
                .map(str::to_owned)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app::{Message, Mode, Notice, SelectionChange},
        document::format,
    };

    /// The effects of `gx` on the first block of `text`.
    #[track_caller]
    fn opened(text: &str) -> Vec<Effect> {
        State::new(format::parse(text), None).update(Message::OpenUrl)
    }

    fn open(url: &str) -> Vec<Effect> {
        vec![Effect::OpenUrl(url.to_owned())]
    }

    // FR-BLK-06, D-45
    #[test]
    fn opens_the_address_of_a_lines_url_tag_first() {
        assert_eq!(opened("Guide @url(https://a.b/c)"), open("https://a.b/c"));
        assert_eq!(opened("Mail @url( mailto:x@y.z )"), open("mailto:x@y.z"));
        assert_eq!(
            opened("See https://t.x @url(https://a.b)"),
            open("https://a.b")
        );
        assert_eq!(opened("- Task @url(https://a.b)"), open("https://a.b"));
        assert_eq!(opened("P: @url(https://a.b)"), open("https://a.b"));
    }

    // D-45
    #[test]
    fn opens_the_first_bare_url_in_any_blocks_text() {
        assert_eq!(opened("- Read https://a.b."), open("https://a.b"));
        assert_eq!(
            opened("Docs: http://x.y and https://z.w"),
            open("http://x.y")
        );
        assert_eq!(opened("https://a.b @url"), open("https://a.b"));
        assert_eq!(opened("P https://a.b:"), open("https://a.b"));
    }

    // FR-FMT-03, D-45
    #[test]
    fn opens_the_first_markdown_link_with_a_url_in_any_block() {
        assert_eq!(
            opened("See [the guide](https://a.b/c) or https://x.y"),
            open("https://a.b/c")
        );
        assert_eq!(opened("Mail <mailto:x@y.z>"), open("mailto:x@y.z"));
        assert_eq!(
            opened("[local](notes.txt) then [web](https://a.b)"),
            open("https://a.b")
        );
        assert_eq!(opened("[local](notes.txt)"), []);
        assert_eq!(
            opened("- Read [it](https://a.b/c) or https://x.y"),
            open("https://a.b/c")
        );
        assert_eq!(opened("- <mailto:x@y.z>"), open("mailto:x@y.z"));
        assert_eq!(opened("P [it](https://a.b):"), open("https://a.b"));
        assert_eq!(opened("S [it](https://a.b) @section"), open("https://a.b"));
    }

    // FR-BLK-06, FR-OPS-01
    #[test]
    fn opens_the_link_of_the_focused_block_of_a_selection() {
        let mut state = State::new(format::parse("- https://a.b\n- https://c.d"), None);
        state.update(Message::Select(SelectionChange::ExtendDown));
        assert_eq!(state.update(Message::OpenUrl), open("https://c.d"));
        assert!(
            state.rows().iter().all(|row| row.selected),
            "the selection stays"
        );
    }

    // FR-BLK-06
    #[test]
    fn opens_nothing_without_a_url() {
        for text in [
            "- plain",
            "",
            "Bad @url(-help)",
            "Path @url(/etc/x)",
            "Odd @url(1x:y)",
            "Odd @url(a_b:c)",
            "Empty @url()",
            "`https://code.only`",
        ] {
            assert_eq!(opened(text), [], "{text:?}");
        }
        assert_eq!(opened("Docs @url(www.x) https://a.b"), open("https://a.b"));
    }

    // FR-BLK-06
    #[test]
    fn opening_ends_editing_and_a_failure_shows_a_notice() {
        let mut state = State::new(format::parse("- https://a.b"), None);
        state.update(Message::EditAtEnd);
        assert_eq!(state.update(Message::OpenUrl), open("https://a.b"));
        assert_eq!(state.mode(), Mode::Navigate);
        let failed = Message::OpenUrlFailed {
            url: "https://a.b".to_owned(),
            reason: "no opener".to_owned(),
        };
        state.update(Message::EditAtEnd);
        assert_eq!(state.update(failed), []);
        assert_eq!(state.mode(), Mode::Edit, "a report leaves editing alone");
        assert_eq!(
            state.notice(),
            Some(&Notice::OpenUrlFailed {
                url: "https://a.b".to_owned(),
                reason: "no opener".to_owned(),
            })
        );
    }
}
