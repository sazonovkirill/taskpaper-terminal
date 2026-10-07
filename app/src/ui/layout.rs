//! Placement on screen that the core leaves to the frontend ([ui.md](../../../docs/specs/ui.md)).

use std::ops::Range;

use ratatui::layout::{Constraint, Layout, Rect};

/// Lines kept between the focused block and the top or bottom edge when the view is tall enough
/// ([navigation.md §2](../../../docs/specs/navigation.md#2-focus-and-motion-fr-nav-01)).
const SCROLL_MARGIN: usize = 2;

/// The narrowest screen with room for the sidebar beside the panes (ui.md §1).
const SIDEBAR_SCREEN: u16 = 80;

/// The narrowest a pane gets, in columns ([navigation.md §5]).
///
/// [navigation.md §5]: ../../../docs/specs/navigation.md#5-panes-fr-ui-02--fr-ui-04-d-13
pub const PANE_MIN: u16 = 30;

/// How many columns narrowing or widening a pane takes or gives (navigation.md §5).
const PANE_STEP: u16 = 4;

/// The width of each pane in a strip `width` columns wide, from left to right, for panes whose
/// widths are `chosen`, `None` for one that takes its share: a chosen width is kept, within
/// [`PANE_MIN`] and the strip's width, and the others share what is left equally, the leftmost
/// taking the columns that do not divide, none narrower than [`PANE_MIN`]. No pane is wider than
/// the strip, so the focused one always fits in view, even on a strip narrower than
/// [`PANE_MIN`]. The panes may need more than the strip, which then scrolls ([`strip_offset`]),
/// or leave some of it empty.
#[must_use]
pub fn pane_widths(width: u16, chosen: &[Option<u16>]) -> Vec<u16> {
    let least = PANE_MIN.min(width);
    let fixed = |chosen: u16| chosen.clamp(least, width);
    let taken: u16 = chosen
        .iter()
        .flatten()
        .fold(0, |sum, &chosen| sum.saturating_add(fixed(chosen)));
    let sharing =
        u16::try_from(chosen.iter().filter(|chosen| chosen.is_none()).count()).unwrap_or(u16::MAX);
    let left = width.saturating_sub(taken);
    let share = left.checked_div(sharing).unwrap_or_default();
    let mut extra = left.checked_rem(sharing).unwrap_or_default();
    chosen
        .iter()
        .map(|chosen| {
            if let Some(chosen) = chosen {
                return fixed(*chosen);
            }
            let one = u16::from(extra > 0);
            extra -= one;
            (share + one).max(least)
        })
        .collect()
}

/// A pane `width` columns wide narrowed by a step, but no narrower than [`PANE_MIN`].
#[must_use]
pub fn narrower(width: u16) -> u16 {
    width.saturating_sub(PANE_STEP).max(PANE_MIN)
}

/// A pane `width` columns wide widened by a step, but no wider than a strip `strip` columns wide.
#[must_use]
pub fn wider(width: u16, strip: u16) -> u16 {
    width.saturating_add(PANE_STEP).min(strip)
}

/// How many columns of a strip of panes `total` columns wide to scroll past to show it in a view
/// `view` columns wide, scrolling as little as possible from `offset` to keep the focused pane,
/// at the columns `focused`, fully in view, or its start when it is wider than the view, and
/// never past the strip's end ([navigation.md §5]).
///
/// [navigation.md §5]: ../../../docs/specs/navigation.md#5-panes-fr-ui-02--fr-ui-04-d-13
#[must_use]
pub fn strip_offset(offset: u16, focused: Range<u16>, view: u16, total: u16) -> u16 {
    offset
        .max(focused.end.saturating_sub(view))
        .min(focused.start)
        .min(total.saturating_sub(view))
}

/// The sidebar's width on a screen `width` columns wide: a quarter of it, within 24 to 32
/// columns, or `None` below 80 columns, where the sidebar hides (ui.md §1).
#[must_use]
pub fn sidebar_width(width: u16) -> Option<u16> {
    (width >= SIDEBAR_SCREEN).then(|| (width / 4).clamp(24, 32))
}

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

/// Where an outline view starts drawing: a row and how many of its screen lines are scrolled
/// past.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Scroll {
    /// The first row on screen, whole or in part.
    pub row: usize,
    /// The screen lines of that row above the view.
    pub line: usize,
}

/// Where a view `height` lines tall over `rows` rows starts, as [`scroll_offset`] places it from
/// `scroll` to show the row `focus`, one of the `rows` (the first line when there is none), or
/// with the focus `anchor` lines from the top, as after a page motion; returns it with the screen
/// line the focus starts on. `row_height` gives a row's screen lines; it is asked only for the
/// rows near the focus and at the end, so the cost does not grow with the outline.
pub fn scroll_rows(
    scroll: Scroll,
    focus: Option<usize>,
    anchor: Option<usize>,
    height: usize,
    rows: usize,
    mut row_height: impl FnMut(usize) -> usize,
) -> (Scroll, usize) {
    let height = height.max(1);
    let at = focus.unwrap_or_default();
    // A window of whole rows from far enough above the focus to hold any start the scroll can
    // end at, measured in lines from its first row.
    let reach = height.max(anchor.unwrap_or_default());
    let mut first = at;
    let mut before = 0;
    while first > 0 && before < reach {
        first -= 1;
        before += row_height(first);
    }
    let focus_lines = focus.map_or(1, &mut row_height);
    let offset = match anchor {
        Some(line) => before.saturating_sub(line),
        // A start above the window is at least `height` lines above the focus, so it scrolls to
        // the focus as any start there would; one below the focus scrolls up to it.
        None if scroll.row < first => 0,
        None if scroll.row > at => usize::MAX,
        None => (first..scroll.row).map(&mut row_height).sum::<usize>() + scroll.line,
    };
    // The lines from the window's start to the end, counted far enough past the focus that the
    // end only limits the scroll when it is that near.
    let mut len = before;
    let mut row = at;
    while row < rows && len < before + height {
        len += row_height(row);
        row += 1;
    }
    let start = scroll_offset(offset, before..before + focus_lines, height, len);
    let mut scroll = Scroll {
        row: first,
        line: start,
    };
    while scroll.row < at {
        let lines = row_height(scroll.row);
        if scroll.line < lines {
            break;
        }
        scroll.line -= lines;
        scroll.row += 1;
    }
    (scroll, before - start)
}

/// Where a view `height` lines tall over `rows` rows starts after scrolling `lines` screen lines
/// down from `scroll`, or up when negative, as the wheel does (ui.md §6): no further up than
/// the first line, nor down than where the last line is at the bottom, which a view past it
/// after rows went away comes back to. `row_height` gives a row's screen lines; it is asked only
/// for rows near the view.
pub fn scroll_by(
    scroll: Scroll,
    lines: isize,
    height: usize,
    rows: usize,
    mut row_height: impl FnMut(usize) -> usize,
) -> Scroll {
    let Some(last) = rows.checked_sub(1) else {
        return Scroll::default();
    };
    let height = height.max(1);
    let mut scroll = if scroll.row > last {
        Scroll { row: last, line: 0 }
    } else {
        scroll
    };
    let mut left = lines_below(scroll, height, rows, &mut row_height);
    for _ in 0..lines.max(0) {
        if left <= height {
            break;
        }
        scroll = if scroll.line + 1 < row_height(scroll.row) {
            Scroll {
                line: scroll.line + 1,
                ..scroll
            }
        } else {
            Scroll {
                row: scroll.row + 1,
                line: 0,
            }
        };
        left = lines_below(scroll, height, rows, &mut row_height);
    }
    let mut up = lines.min(0).unsigned_abs();
    while (up > 0 || left < height)
        && let Some(earlier) = line_above(scroll, &mut row_height)
    {
        scroll = earlier;
        up = up.saturating_sub(1);
        left += 1;
    }
    scroll
}

/// The screen lines of `rows` rows from a view's start at `scroll` to the end, counted only until
/// they are more than `height`.
fn lines_below(
    scroll: Scroll,
    height: usize,
    rows: usize,
    row_height: &mut impl FnMut(usize) -> usize,
) -> usize {
    let mut lines = row_height(scroll.row).saturating_sub(scroll.line);
    let mut row = scroll.row + 1;
    while lines <= height && row < rows {
        lines += row_height(row);
        row += 1;
    }
    lines
}

/// The view's start one screen line above `scroll`, unless it is at the first line.
fn line_above(scroll: Scroll, row_height: &mut impl FnMut(usize) -> usize) -> Option<Scroll> {
    if let Some(line) = scroll.line.checked_sub(1) {
        return Some(Scroll { line, ..scroll });
    }
    let row = scroll.row.checked_sub(1)?;
    Some(Scroll {
        row,
        line: row_height(row).saturating_sub(1),
    })
}

/// The screen line, counted from the top of a view `height` lines tall that starts at
/// `scroll`, where the row `focus` starts, if the view shows that line.
pub fn line_of(
    scroll: Scroll,
    focus: usize,
    height: usize,
    mut row_height: impl FnMut(usize) -> usize,
) -> Option<usize> {
    if focus < scroll.row || (focus == scroll.row && scroll.line > 0) {
        return None;
    }
    let mut line = 0;
    for row in scroll.row..focus {
        line += row_height(row);
        if line >= height + scroll.line {
            return None;
        }
    }
    Some(line - scroll.line)
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

    // FR-UI-01
    #[test]
    fn the_sidebar_takes_a_quarter_of_the_screen_within_24_to_32_columns_from_80_on() {
        assert_eq!(sidebar_width(79), None);
        assert_eq!(sidebar_width(0), None);
        assert_eq!(sidebar_width(80), Some(24));
        assert_eq!(sidebar_width(103), Some(25));
        assert_eq!(sidebar_width(128), Some(32));
        assert_eq!(sidebar_width(500), Some(32));
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

    // FR-UI-02, FR-UI-03, navigation.md §5
    #[test]
    fn panes_share_the_strip_equally_the_leftmost_taking_what_does_not_divide() {
        assert_eq!(pane_widths(80, &[None]), [80]);
        assert_eq!(pane_widths(80, &[None, None]), [40, 40]);
        assert_eq!(pane_widths(100, &[None, None, None]), [34, 33, 33]);
        assert_eq!(pane_widths(101, &[None, None, None]), [34, 34, 33]);
    }

    // FR-UI-03, navigation.md §5
    #[test]
    fn panes_are_never_narrower_than_30_columns_nor_wider_than_the_strip() {
        assert_eq!(pane_widths(80, &[None, None, None]), [30, 30, 30]);
        assert_eq!(
            pane_widths(20, &[None, None]),
            [20, 20],
            "nor wider than a strip below 30"
        );
        assert_eq!(pane_widths(0, &[]), Vec::<u16>::new());
    }

    // FR-UI-03, navigation.md §5
    #[test]
    fn a_chosen_width_is_kept_and_the_other_panes_share_the_rest() {
        assert_eq!(pane_widths(100, &[Some(36), None]), [36, 64]);
        assert_eq!(pane_widths(100, &[None, Some(44), None]), [30, 44, 30]);
        assert_eq!(
            pane_widths(100, &[Some(36), Some(40)]),
            [36, 40],
            "the rest of the strip stays empty"
        );
        assert_eq!(
            pane_widths(100, &[Some(12), Some(140)]),
            [30, 100],
            "within 30 columns and the strip"
        );
        assert_eq!(pane_widths(20, &[Some(26)]), [20], "a strip below 30");
    }

    // FR-UI-03, navigation.md §5
    #[test]
    fn narrowing_and_widening_go_by_4_columns_within_30_and_the_strip() {
        assert_eq!(narrower(40), 36);
        assert_eq!(narrower(33), 30);
        assert_eq!(narrower(30), 30);
        assert_eq!(narrower(3), 30);
        assert_eq!(wider(40, 100), 44);
        assert_eq!(wider(98, 100), 100);
        assert_eq!(wider(100, 100), 100);
        assert_eq!(wider(20, 20), 20, "a strip below 30");
    }

    // FR-UI-03, navigation.md §5
    #[test]
    fn the_strip_scrolls_as_little_as_it_must_to_show_the_focused_pane_whole() {
        assert_eq!(strip_offset(0, 0..40, 80, 120), 0);
        assert_eq!(strip_offset(0, 80..120, 80, 120), 40, "to its right edge");
        assert_eq!(strip_offset(40, 40..80, 80, 120), 40, "already in view");
        assert_eq!(strip_offset(40, 0..40, 80, 120), 0, "back to its left edge");
        assert_eq!(strip_offset(0, 30..60, 50, 90), 10);
        assert_eq!(
            strip_offset(70, 30..60, 50, 90),
            30,
            "from a scroll past it"
        );
        assert_eq!(strip_offset(40, 0..40, 80, 40), 0, "never past the end");
        assert_eq!(
            strip_offset(0, 30..100, 50, 100),
            30,
            "its start when wider"
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

    /// Where the view starts as a line counted over all `heights`, after placing it as the
    /// outline did before it measured only the rows near the focus.
    fn scroll_over_all_rows(
        offset: usize,
        focus: Option<usize>,
        anchor: Option<usize>,
        height: usize,
        heights: &[usize],
    ) -> usize {
        let start = |index: usize| heights.iter().take(index).sum::<usize>();
        let focus = focus.map_or(0..1, |index| start(index)..start(index + 1));
        let offset = anchor.map_or(offset, |line| focus.start.saturating_sub(line));
        scroll_offset(offset, focus, height, start(heights.len()))
    }

    // N-PERF-02
    #[test]
    fn a_view_scrolls_by_whole_rows_and_the_lines_of_its_first_one() {
        let heights = [1, 3, 1, 2, 1, 1, 1, 1];
        let scroll = |scroll, focus, anchor| {
            scroll_rows(scroll, focus, anchor, 4, heights.len(), |row| heights[row])
        };
        let top = Scroll::default();
        assert_eq!(scroll(top, Some(1), None), (top, 1), "in view");
        assert_eq!(
            scroll(top, Some(2), None),
            (Scroll { row: 1, line: 1 }, 2),
            "inside a wrapped row, a line below the focus"
        );
        assert_eq!(
            scroll(top, Some(5), Some(2)),
            (Scroll { row: 3, line: 1 }, 2),
            "the focus on the anchor line"
        );
        assert_eq!(scroll(top, None, None), (top, 0), "without a focus");
        let inside = Scroll { row: 0, line: 5 };
        assert_eq!(
            scroll_rows(inside, Some(1), None, 4, 6, |row| [6, 1, 1, 1, 1, 1][row]),
            (inside, 1),
            "a start inside a tall row stays while the focus is in view"
        );
        let heights = [6, 6, 1, 1, 1, 1, 1];
        assert_eq!(
            scroll_rows(inside, Some(2), None, 4, heights.len(), |row| heights[row]),
            (Scroll { row: 1, line: 4 }, 2),
            "a start far above the focus scrolls down to it"
        );
    }

    // ui.md §6
    #[test]
    fn the_wheel_scrolls_by_lines_between_the_first_and_the_last_page() {
        let heights = [1, 3, 1, 2, 1];
        let within = |rows, row, line, lines| {
            scroll_by(Scroll { row, line }, lines, 4, rows, |row| heights[row])
        };
        let by = |row, line, lines| within(heights.len(), row, line, lines);
        assert_eq!(by(0, 0, 2), Scroll { row: 1, line: 1 });
        assert_eq!(by(1, 1, -2), Scroll { row: 0, line: 0 });
        assert_eq!(by(0, 0, -1), Scroll { row: 0, line: 0 }, "at the top");
        assert_eq!(by(0, 0, 20), Scroll { row: 2, line: 0 }, "the last page");
        assert_eq!(
            by(4, 0, 0),
            Scroll { row: 2, line: 0 },
            "back to the last page"
        );
        assert_eq!(by(9, 0, 0), Scroll { row: 2, line: 0 }, "rows went away");
        assert_eq!(within(0, 3, 0, 1), Scroll::default(), "no rows");
        assert_eq!(within(2, 0, 0, 1), Scroll::default(), "everything fits");
    }

    // ui.md §6
    #[test]
    fn a_rows_line_is_found_while_the_view_shows_its_first_line() {
        let heights = [1, 3, 1, 2, 1];
        let of = |row, line, focus| line_of(Scroll { row, line }, focus, 4, |row| heights[row]);
        assert_eq!(of(0, 0, 0), Some(0));
        assert_eq!(of(0, 0, 1), Some(1));
        assert_eq!(of(0, 0, 2), None, "below the view");
        assert_eq!(of(1, 1, 2), Some(2));
        assert_eq!(of(1, 1, 3), Some(3));
        assert_eq!(of(1, 1, 1), None, "its first line is above the view");
        assert_eq!(of(2, 0, 1), None, "above the view");
    }

    proptest::proptest! {
        /// Scrolling by lines measures only rows near the view and lands where counting every
        /// line would.
        #[test]
        fn scrolling_by_lines_matches_counting_every_line(
            heights in proptest::collection::vec(1..=12usize, 1..80),
            height in 0..24usize,
            offset in 0..400usize,
            lines in -30..30isize,
        ) {
            let start = |index: usize| heights.iter().take(index).sum::<usize>();
            let total = start(heights.len());
            let offset = offset % total;
            let row = (0..heights.len()).take_while(|&row| start(row + 1) <= offset).count();
            let scroll = Scroll { row, line: offset - start(row) };
            let mut asked = 0;
            let placed = scroll_by(scroll, lines, height, heights.len(), |row| {
                asked += 1;
                heights[row]
            });
            let last = total.saturating_sub(height.max(1));
            let expected = offset.saturating_add_signed(lines).min(last);
            proptest::prop_assert_eq!(start(placed.row) + placed.line, expected);
            proptest::prop_assert!(placed.line < heights[placed.row], "{placed:?}");
            let reach = height.max(1) + 2;
            let most = (lines.unsigned_abs() + 2) * (reach + 2) + reach * 2;
            proptest::prop_assert!(asked <= most, "asked {asked} rows");
        }
    }

    proptest::proptest! {
        /// Measuring only the rows near the focus scrolls exactly as measuring every row did,
        /// and asks for a number of rows that does not grow with the outline.
        #[test]
        fn scrolling_by_rows_matches_scrolling_over_every_line(
            heights in proptest::collection::vec(1..=12usize, 1..80),
            focus in proptest::option::of(0..80usize),
            anchor in proptest::option::of(0..20usize),
            height in 0..24usize,
            offset in 0..400usize,
        ) {
            let focus = focus.map(|at| at % heights.len());
            let start = |index: usize| heights.iter().take(index).sum::<usize>();
            let offset = offset % start(heights.len());
            let row = (0..heights.len()).take_while(|&row| start(row + 1) <= offset).count();
            let scroll = Scroll { row, line: offset - start(row) };
            let mut asked = 0;
            let (placed, focus_line) =
                scroll_rows(scroll, focus, anchor, height, heights.len(), |row| {
                    asked += 1;
                    heights[row]
                });
            let expected = scroll_over_all_rows(offset, focus, anchor, height, &heights);
            proptest::prop_assert_eq!(start(placed.row) + placed.line, expected);
            proptest::prop_assert!(placed.line < heights[placed.row], "{placed:?}");
            proptest::prop_assert_eq!(
                focus_line,
                focus.map_or(0, start) - expected,
            );
            let reach = height.max(anchor.unwrap_or_default()).max(1);
            proptest::prop_assert!(asked <= 3 * reach + height + 2, "asked {asked} rows");
        }
    }
}
