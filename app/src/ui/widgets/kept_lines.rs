//! Keeping the screen lines the outline laid its rows out in, while each row and what it is
//! drawn with stay the same, since laying out a long block takes much longer than a frame
//! (N-PERF-02).

use std::{cell::RefCell, collections::HashMap, mem, rc::Rc};

use jiff::civil::Date;
use ratatui::text::Line;

use super::super::theme::{IconSet, Theme};
use crate::{app::Row, document::ItemId};

/// What every row is laid out with: wrap on or off, the local date the chips count from, the
/// icons and the theme.
pub(super) type Drawn = (bool, Date, IconSet, Theme);

/// A row's screen lines and the date chips at the right of its first line.
pub(super) type Laid = (Rc<[Line<'static>]>, Line<'static>);

/// The screen lines of the rows laid out since the frame before last, each kept for its row and
/// width. Rows no frame lays out are let go, so it holds about the rows on screen.
#[derive(Debug, Default)]
pub struct KeptLines {
    // The outline view is a `Copy` widget that lays rows out through shared references, several
    // times a frame, so the lines it keeps change behind one.
    kept: RefCell<Kept>,
}

#[derive(Debug, Default)]
struct Kept {
    drawn: Option<Drawn>,
    /// The rows laid out this frame and the frame before, by block and width.
    current: HashMap<(ItemId, usize), (Row, Laid)>,
    previous: HashMap<(ItemId, usize), (Row, Laid)>,
}

impl KeptLines {
    /// Starts a frame: the rows the frame before did not lay out are let go.
    pub fn next_frame(&mut self) {
        let kept = self.kept.get_mut();
        kept.previous = mem::take(&mut kept.current);
    }

    /// The lines of `row` `width` columns wide drawn with `drawn`: those kept for it, or else
    /// those `lay_out` gives, kept from now on.
    pub(super) fn get(
        &self,
        row: &Row,
        width: usize,
        drawn: Drawn,
        lay_out: impl FnOnce() -> Laid,
    ) -> Laid {
        let mut kept = self.kept.borrow_mut();
        if kept.drawn != Some(drawn) {
            *kept = Kept {
                drawn: Some(drawn),
                ..Kept::default()
            };
        }
        let key = (row.id, width);
        let found = match kept.current.get(&key) {
            Some((was, laid)) if was == row => Some(laid.clone()),
            _ => kept
                .previous
                .remove(&key)
                .filter(|(was, _)| was == row)
                .map(|(_, laid)| laid),
        };
        let laid = found.unwrap_or_else(lay_out);
        kept.current.insert(key, (row.clone(), laid.clone()));
        laid
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use jiff::civil::date;

    use super::*;
    use crate::{
        app::State,
        document::format,
        ui::{ColorSupport, theme::BuiltIn},
    };

    /// What the rows are drawn with in these tests.
    fn drawn() -> Drawn {
        (
            true,
            date(2026, 10, 5),
            IconSet::Unicode,
            BuiltIn::DEFAULT.colors(ColorSupport::Ansi, None),
        )
    }

    /// The rows of `text`.
    fn rows(text: &str) -> Vec<Row> {
        State::new(format::parse(text), None).rows().to_vec()
    }

    /// Lays `row` out through `kept`, counting each time it is laid out in `laid`, as one line
    /// reading `text`.
    fn get(kept: &KeptLines, row: &Row, width: usize, drawn: Drawn, laid: &Cell<usize>) -> String {
        let (lines, _) = kept.get(row, width, drawn, || {
            laid.set(laid.get() + 1);
            (Rc::from([Line::raw(row.text.to_string())]), Line::default())
        });
        lines.iter().map(ToString::to_string).collect()
    }

    // N-PERF-02
    #[test]
    fn a_row_is_laid_out_once_while_it_and_what_it_is_drawn_with_stay_the_same() {
        let mut kept = KeptLines::default();
        let laid = Cell::new(0);
        let [a, b] = <[Row; 2]>::try_from(rows("- a\n- b")).unwrap();
        assert_eq!(get(&kept, &a, 9, drawn(), &laid), "a");
        assert_eq!(get(&kept, &b, 9, drawn(), &laid), "b");
        assert_eq!(get(&kept, &a, 9, drawn(), &laid), "a");
        kept.next_frame();
        assert_eq!(get(&kept, &a, 9, drawn(), &laid), "a");
        kept.next_frame();
        assert_eq!(get(&kept, &a, 9, drawn(), &laid), "a");
        assert_eq!(laid.get(), 2, "a and b, each once");
    }

    // N-PERF-02, FR-UI-05
    #[test]
    fn a_row_is_laid_out_again_once_it_its_width_or_what_it_is_drawn_with_changes() {
        let kept = KeptLines::default();
        let laid = Cell::new(0);
        let a = rows("- a").remove(0);
        get(&kept, &a, 9, drawn(), &laid);
        let changed = rows("- a2").remove(0);
        assert_eq!(
            get(&kept, &changed, 9, drawn(), &laid),
            "a2",
            "the same block, changed"
        );
        assert_eq!(laid.get(), 2);
        get(&kept, &changed, 8, drawn(), &laid);
        assert_eq!(laid.get(), 3, "another width");
        get(&kept, &changed, 9, drawn(), &laid);
        assert_eq!(laid.get(), 3, "both widths kept, as for panes side by side");
        let (wrap, today, icons, theme) = drawn();
        for other in [
            (!wrap, today, icons, theme),
            (wrap, date(2026, 10, 6), icons, theme),
            (wrap, today, IconSet::Nerd, theme),
            (
                wrap,
                today,
                icons,
                BuiltIn::named("nord")
                    .unwrap()
                    .colors(ColorSupport::Ansi, None),
            ),
        ] {
            let before = laid.get();
            get(&kept, &changed, 9, other, &laid);
            assert_eq!(laid.get(), before + 1, "{other:?}");
        }
    }

    // N-PERF-02
    #[test]
    fn rows_no_frame_lays_out_are_let_go() {
        let mut kept = KeptLines::default();
        let laid = Cell::new(0);
        let a = rows("- a").remove(0);
        get(&kept, &a, 9, drawn(), &laid);
        kept.next_frame();
        kept.next_frame();
        get(&kept, &a, 9, drawn(), &laid);
        assert_eq!(laid.get(), 2);
    }
}
