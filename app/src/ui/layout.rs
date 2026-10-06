//! Placement on screen that the core leaves to the frontend ([ui.md](../../../docs/specs/ui.md)).

use ratatui::layout::Rect;

/// Rows kept between the focused row and the top or bottom edge when the view is tall enough
/// ([navigation.md §2](../../../docs/specs/navigation.md#2-focus-and-motion-fr-nav-01)).
const SCROLL_MARGIN: usize = 2;

/// The first of `len` rows to draw in a view `height` rows tall so that the row `focus` is
/// visible with [`SCROLL_MARGIN`] rows around it where space allows, scrolling as little as
/// possible from `offset` and never past the last row.
#[must_use]
pub fn scroll_offset(offset: usize, focus: usize, height: usize, len: usize) -> usize {
    // A view with no rows has nothing to keep visible; treating it as one row keeps the bounds
    // below ordered.
    let height = height.max(1);
    let margin = SCROLL_MARGIN.min((height - 1) / 2);
    let highest = focus.saturating_sub(margin);
    let lowest = (focus + 1 + margin).saturating_sub(height).min(highest);
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

    #[track_caller]
    fn check(offset: usize, focus: usize, height: usize, len: usize, expected: usize) {
        assert_eq!(scroll_offset(offset, focus, height, len), expected);
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
