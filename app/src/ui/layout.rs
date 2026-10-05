//! Placement on screen that the core leaves to the frontend ([ui.md](../../../docs/specs/ui.md)).

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

#[cfg(test)]
mod tests {
    use super::*;

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
