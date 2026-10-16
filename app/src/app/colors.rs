//! The Colors view: the blocks with a color tag, grouped by color
//! ([navigation.md §7.1](../../../docs/specs/navigation.md#71-other-special-views-fr-tag-07-d-80),
//! FR-TAG-10, D-83).

use super::{
    Row, State,
    pane::Pane,
    special_view::{SpecialView, ViewGroup, under_headings},
};

impl State {
    /// The Colors view's entries in `pane`, as [`State::matching_rows`] gives them, grouped by
    /// color in the order red, yellow, green, blue, in document order within a color (D-83,
    /// D-138).
    pub(super) fn colors_rows(&self, pane: &Pane) -> Vec<Row> {
        let mut entries: Vec<_> = self
            .matching_rows(pane, SpecialView::Colors)
            .into_iter()
            .filter_map(|row| Some((row.color?, row)))
            .collect();
        entries.sort_by_key(|(color, _)| color.number());
        under_headings(
            entries
                .into_iter()
                .map(|(color, row)| (ViewGroup::Color(color), row)),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use jiff::civil::date;

    use super::*;
    use crate::{
        app::{Heading, Location, Message, SelectionChange, SpecialView},
        document::{TagColor, format},
    };

    /// Colored blocks at several depths, out of the colors' order, among blocks without a color,
    /// one whose `@violet` is no color, a special node and a done project.
    const OUTLINE: &str = "\
Home: @blue
\t- Water plants @red
\t- Paint the hall
\tErrands:
\t\t- Post office @violet @green
\t\t- Library @violet
\t\t- Bank @red
\t\t\tC @hidden @green
Work: @done
\t- Report @yellow
- Plan the week @blue
";

    /// `text` opened in the Colors view.
    fn colors(text: &str) -> State {
        let mut state = State::new(format::parse(text), None);
        state.update(Message::Tick {
            now: Duration::ZERO,
            today: date(2026, 10, 5),
        });
        state.update(Message::GoTo(Location::View(SpecialView::Colors)));
        assert_eq!(state.location(), Location::View(SpecialView::Colors));
        state
    }

    /// Each row's text and path, joined as `path › text`, after the heading's color it starts,
    /// with a `*` for the first heading.
    fn listed(state: &State) -> Vec<String> {
        state
            .rows()
            .iter()
            .map(|row| {
                assert_eq!(row.depth, 0, "entries are flat");
                let heading = row.heading.map_or_default(|Heading { group, first }| {
                    let color = TagColor::ALL
                        .into_iter()
                        .find(|&color| group == ViewGroup::Color(color));
                    format!(
                        "[{}{}] ",
                        color.map_or("not a color", TagColor::name),
                        if first { "*" } else { "" }
                    )
                });
                let path = row
                    .path
                    .iter()
                    .map(|segment| &**segment)
                    .chain([&*row.text])
                    .collect::<Vec<_>>()
                    .join(" › ");
                heading + &path
            })
            .collect()
    }

    /// The display text of the focused block.
    fn focused(state: &State) -> String {
        state
            .focus()
            .and_then(|id| state.document().get(id))
            .map(crate::document::Item::display_text)
            .map(str::to_owned)
            .unwrap_or_default()
    }

    // FR-TAG-10, D-83, D-80, FR-SPC-01, D-138
    #[test]
    fn lists_colored_blocks_grouped_by_color_in_document_order_under_one_heading_each() {
        let state = colors(OUTLINE);
        assert_eq!(
            listed(&state),
            [
                "[Red*] Home › Water plants",
                "Home › Errands › Bank",
                "[Yellow] Work › Report",
                "[Green] Home › Errands › Post office",
                "[Blue] Home",
                "Plan the week",
            ]
        );
        assert_eq!(state.breadcrumbs(), Vec::<String>::new());
        assert_eq!(
            focused(&state),
            "Home",
            "the block focused at home is listed (D-114)"
        );
        assert_eq!(state.rows()[0].color, Some(TagColor::Red));
    }

    // D-80: the focus moves over the entries only, and the headings take no row.
    #[test]
    fn motion_goes_from_entry_to_entry_past_the_headings() {
        let mut state = colors(OUTLINE);
        state.update(Message::FocusFirst);
        state.update(Message::FocusNext);
        state.update(Message::FocusNext);
        assert_eq!(focused(&state), "Report");
        state.update(Message::FocusLast);
        assert_eq!(focused(&state), "Plan the week");
        state.update(Message::FocusPrevious);
        state.update(Message::FocusPrevious);
        assert_eq!(focused(&state), "Post office");
    }

    // FR-DONE-02, D-83
    #[test]
    fn done_blocks_follow_the_show_completed_setting() {
        let mut state = colors("- a @red @done\n- b @red\nP: @done\n\t- c @green\n");
        assert_eq!(listed(&state), ["[Red*] a", "b", "[Green] P › c"]);
        state.update(Message::ToggleShowDone);
        assert_eq!(listed(&state), ["[Red*] b"]);
    }

    // D-83, FR-TAG-05: a block given another color moves to that color's group, and one whose
    // color is removed leaves the view.
    #[test]
    fn the_list_follows_color_changes() {
        let mut state = colors("- a @red\n- b @red\n- c @blue\n");
        state.update(Message::SetColor(Some(TagColor::Green)));
        assert_eq!(listed(&state), ["[Red*] b", "[Green] a", "[Blue] c"]);
        assert_eq!(focused(&state), "a");
        state.update(Message::SetColor(None));
        assert_eq!(listed(&state), ["[Red*] b", "[Blue] c"]);
        let mut empty = colors("- a\n");
        assert_eq!(listed(&empty), Vec::<String>::new());
        empty.update(Message::FocusNext);
        assert_eq!(empty.focus(), None);
    }

    // D-80, D-57: only Today can be reordered.
    #[test]
    fn move_up_and_down_do_nothing() {
        let mut state = colors("- a @red\n- b @red\n");
        state.update(Message::FocusNext);
        state.update(Message::Select(SelectionChange::ExtendUp));
        let before = (listed(&state), format::serialize(state.document()));
        for message in [Message::MoveUp, Message::MoveDown] {
            assert_eq!(state.update(message.clone()), [], "{message:?}");
            assert_eq!(
                (listed(&state), format::serialize(state.document())),
                before,
                "{message:?}"
            );
        }
    }

    // FR-NAV-03, D-80: zooming in on an entry goes to it in the outline.
    #[test]
    fn zooming_in_on_an_entry_goes_to_it_in_the_outline() {
        let mut state = colors(OUTLINE);
        state.update(Message::FocusFirst);
        state.update(Message::FocusNext);
        state.update(Message::ZoomIn);
        assert_eq!(state.breadcrumbs(), ["Home", "Errands"]);
        assert_eq!(focused(&state), "Bank");
        state.update(Message::Back);
        assert_eq!(state.location(), Location::View(SpecialView::Colors));
    }
}
