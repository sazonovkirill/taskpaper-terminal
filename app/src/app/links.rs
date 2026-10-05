//! Opening the focused block's address in the system browser: which address a block has and
//! which the frontend may hand to an opener ([editing.md §5](../../../docs/specs/editing.md#5-block-commands),
//! FR-BLK-06, D-10).

use super::{Effect, Notice, State};
use crate::document::{Item, inline};

impl State {
    /// Asks the frontend to open the focused block's address, if it has one (FR-BLK-06).
    pub(super) fn open_url(&self) -> Vec<Effect> {
        self.focus
            .and_then(|id| self.document.get(id))
            .and_then(address)
            .map(Effect::OpenUrl)
            .into_iter()
            .collect()
    }

    /// Says that the browser could not be started on `url`, for `reason`.
    pub(super) fn open_url_failed(&mut self, url: String, reason: String) {
        self.notify(Notice::OpenUrlFailed { url, reason });
    }
}

/// The address of the line `item`: its `@url` value when that is a URL, else the first bare URL
/// in its text (D-10). A block with several bare URLs opens the first until a picker can offer
/// them all (editing.md §2.2).
fn address(item: &Item) -> Option<String> {
    item.tags()
        .into_iter()
        .find(|tag| tag.name == "url")
        .and_then(|tag| tag.value)
        .map(|value| value.trim().to_owned())
        .filter(|value| has_scheme(value))
        .or_else(|| {
            inline::urls(&item.display_text())
                .first()
                .map(|&url| url.to_owned())
        })
}

/// Whether `address` starts with a URL scheme, such as `https:` or `mailto:`, so that an opener
/// never reads it as an option or a file name.
fn has_scheme(address: &str) -> bool {
    address.split_once(':').is_some_and(|(scheme, _)| {
        let mut chars = scheme.chars();
        chars
            .next()
            .is_some_and(|first| first.is_ascii_alphabetic())
            && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app::{Message, Mode, Notice},
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

    // FR-BLK-06
    #[test]
    fn opens_a_url_blocks_address() {
        assert_eq!(opened("Guide @url(https://a.b/c)"), open("https://a.b/c"));
        assert_eq!(opened("Mail @url( mailto:x@y.z )"), open("mailto:x@y.z"));
        assert_eq!(
            opened("See https://t.x @url(https://a.b)"),
            open("https://a.b")
        );
    }

    // D-10
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
