//! Placement on screen that the core leaves to the frontend ([ui.md](../../../docs/specs/ui.md)).

use std::ops::Range;

use ratatui::layout::{Constraint, Layout, Rect};

/// Lines kept between the focused block and the top or bottom edge when the view is tall enough
/// ([navigation.md §2](../../../docs/specs/navigation.md#2-focus-and-motion-fr-nav-01)).
const SCROLL_MARGIN: usize = 2;

/// The first of `len` lines to draw in a view `height` lines tall so that the lines `focus` of
/// the focused block are visible with [`SCROLL_MARGIN`] lines around them where space allows,
/// or its first lines when it is taller than the view, scrolling as little as possible from
/// `offset` and never past the last line.
#[must_use]
pub fn scroll_offset(offset: usize, focus: Range<usize>, height: usize, len: usize) -> usize {
    // A view with no lines has nothing to keep visible; treating it as one line keeps the
    // bounds below ordered.
    let height = height.max(1);
    let margin = SCROLL_MARGIN.min((height - 1) / 2);
    let highest = focus.start.saturating_sub(margin);
    let lowest = (focus.end + margin).saturating_sub(height).min(highest);
    offset
        .clamp(lowest, highest)
        .min(len.saturating_sub(height))
}

/// Where a popup of `size` (width, height) anchored at the caret at `caret` goes inside `area`:
/// from the caret's column on the line below it, or above the caret's line when there is no room
/// below, moved left or up as far as it must to stay inside `area` (ui.md §5).
#[must_use]
pub fn anchored(area: Rect, caret: (u16, u16), size: (u16, u16)) -> Rect {
    let (x, line) = caret;
    let (width, height) = size;
    let below = line.saturating_add(1);
    let y = if below.saturating_add(height) <= area.bottom() {
        below
    } else {
        line.saturating_sub(height)
    };
    Rect::new(x, y, width, height).clamp(area)
}

/// The most rows a popup anchored at the caret on `line` can have inside `area`: those below the
/// line or those above it, whichever are more (ui.md §5).
#[must_use]
pub fn room(area: Rect, line: u16) -> u16 {
    let below = area.bottom().saturating_sub(line.saturating_add(1));
    let above = line.saturating_sub(area.top());
    below.max(above)
}

/// Where the secondary sidebar goes on a `screen` whose panes would fill `body`, and what is left
/// of `body` to the panes ([ui.md §1](../../../docs/specs/ui.md#1-layout), D-28): a quarter of the
/// screen's width but 32 to 40 columns at the right of `body`, taken from the panes, or drawn over
/// their right part on a screen narrower than 80 columns. A screen smaller than 40×10 has room
/// only for the focused pane, so no sidebar.
#[must_use]
pub fn secondary_sidebar(screen: Rect, body: Rect) -> (Rect, Option<Rect>) {
    if screen.width < 40 || screen.height < 10 {
        return (body, None);
    }
    let width = (screen.width / 4).clamp(32, 40);
    let [panes, sidebar] = body.layout(&Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Length(width),
    ]));
    if screen.width < 80 {
        (body, Some(sidebar))
    } else {
        (panes, Some(sidebar))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_popup_opens_below_the_caret_or_above_it_without_room() {
        let area = Rect::new(2, 1, 40, 10);
        assert_eq!(anchored(area, (5, 3), (10, 4)), Rect::new(5, 4, 10, 4));
        assert_eq!(
            anchored(area, (5, 6), (10, 4)),
            Rect::new(5, 7, 10, 4),
            "just fits"
        );
        assert_eq!(anchored(area, (5, 7), (10, 4)), Rect::new(5, 3, 10, 4));
        assert_eq!(anchored(area, (38, 3), (10, 4)), Rect::new(32, 4, 10, 4));
        assert_eq!(
            anchored(area, (5, 2), (50, 20)),
            Rect::new(2, 1, 40, 10),
            "a popup larger than the area fills it"
        );
    }

    #[test]
    fn a_popup_has_room_on_the_larger_side_of_the_caret() {
        let area = Rect::new(2, 1, 40, 10);
        assert_eq!(room(area, 1), 9, "the top line has the rest below");
        assert_eq!(room(area, 3), 7);
        assert_eq!(room(area, 7), 6, "more above");
        assert_eq!(room(area, 10), 9, "the bottom line has the rest above");
        assert_eq!(
            anchored(area, (5, 7), (10, room(area, 7))),
            Rect::new(5, 1, 10, 6)
        );
    }

    #[test]
    fn the_secondary_sidebar_takes_a_quarter_of_the_width_within_32_to_40_columns() {
        let body = |width| Rect::new(0, 0, width, 23);
        let screen = |width| Rect::new(0, 0, width, 24);
        assert_eq!(
            secondary_sidebar(screen(80), body(80)),
            (body(48), Some(Rect::new(48, 0, 32, 23)))
        );
        assert_eq!(
            secondary_sidebar(screen(140), body(140)),
            (body(105), Some(Rect::new(105, 0, 35, 23)))
        );
        assert_eq!(
            secondary_sidebar(screen(200), body(200)),
            (body(160), Some(Rect::new(160, 0, 40, 23)))
        );
        let below_left_sidebar = Rect::new(30, 1, 170, 20);
        assert_eq!(
            secondary_sidebar(screen(200), below_left_sidebar),
            (Rect::new(30, 1, 130, 20), Some(Rect::new(160, 1, 40, 20))),
            "it stays at the right of the body"
        );
    }

    #[test]
    fn a_narrow_screen_draws_the_secondary_sidebar_over_the_panes() {
        let body = Rect::new(0, 0, 79, 23);
        assert_eq!(
            secondary_sidebar(Rect::new(0, 0, 79, 24), body),
            (body, Some(Rect::new(47, 0, 32, 23)))
        );
        let body = Rect::new(0, 0, 40, 9);
        assert_eq!(
            secondary_sidebar(Rect::new(0, 0, 40, 10), body),
            (body, Some(Rect::new(8, 0, 32, 9)))
        );
    }

    #[test]
    fn a_screen_below_40_by_10_has_no_secondary_sidebar() {
        let body = Rect::new(0, 0, 39, 23);
        assert_eq!(
            secondary_sidebar(Rect::new(0, 0, 39, 24), body),
            (body, None)
        );
        let body = Rect::new(0, 0, 100, 8);
        assert_eq!(
            secondary_sidebar(Rect::new(0, 0, 100, 9), body),
            (body, None)
        );
    }

    /// Checks the offset for a focused block one line tall at line `focus`.
    #[track_caller]
    fn check(offset: usize, focus: usize, height: usize, len: usize, expected: usize) {
        assert_eq!(
            scroll_offset(offset, focus..focus + 1, height, len),
            expected
        );
    }

    // FR-UI-05
    #[test]
    fn keeps_every_line_of_a_wrapped_block_in_view_or_its_first_ones() {
        assert_eq!(scroll_offset(0, 6..9, 10, 100), 1, "two lines below it");
        assert_eq!(scroll_offset(0, 2..5, 10, 100), 0);
        assert_eq!(scroll_offset(20, 12..15, 10, 100), 10, "two lines above it");
        assert_eq!(
            scroll_offset(0, 20..40, 10, 100),
            18,
            "taller than the view"
        );
        assert_eq!(scroll_offset(30, 20..40, 10, 100), 18);
    }

    #[test]
    fn keeps_the_offset_while_focus_is_inside_the_margins() {
        check(0, 0, 10, 100, 0);
        check(0, 7, 10, 100, 0);
        check(5, 7, 10, 100, 5);
        check(5, 12, 10, 100, 5);
    }

    #[test]
    fn scrolls_down_to_keep_two_rows_below_focus() {
        check(0, 8, 10, 100, 1);
        check(0, 50, 10, 100, 43);
    }

    #[test]
    fn scrolls_up_to_keep_two_rows_above_focus() {
        check(10, 11, 10, 100, 9);
        check(50, 2, 10, 100, 0);
        check(50, 1, 10, 100, 0);
    }

    #[test]
    fn never_scrolls_past_the_last_row() {
        check(0, 99, 10, 100, 90);
        check(95, 97, 10, 100, 90);
        check(3, 4, 10, 5, 0);
    }

    #[test]
    fn shrinks_the_margin_in_short_views() {
        check(0, 3, 4, 100, 1);
        check(10, 5, 4, 100, 4);
        check(0, 3, 3, 100, 2);
        check(5, 4, 3, 100, 3);
        check(0, 3, 2, 100, 2);
        check(0, 3, 1, 100, 3);
        check(9, 3, 1, 100, 3);
    }

    #[test]
    fn a_view_without_rows_starts_at_the_focus() {
        check(0, 3, 0, 100, 3);
        check(0, 0, 0, 0, 0);
    }
}
