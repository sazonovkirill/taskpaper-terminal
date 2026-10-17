//! The Keymap dialog ([ui.md §11](../../../../docs/specs/ui.md#11-keymap-dialog-fr-key-07), D-140): every
//! binding on one sheet over 90% of the screen, its groups in columns as on a printed cheat
//! sheet, scrolled when they do not fit.

use std::borrow::Cow;

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Clear, Padding, Widget},
};

use super::wrap;
use crate::ui::theme::Theme;

/// A group of the Keymap dialog: its heading and each command's name with how its keys read.
pub type SheetGroup = (Cow<'static, str>, Vec<(Cow<'static, str>, String)>);

/// The share of the screen's width and height the dialog takes.
const SHARE: u16 = 90;

/// The widest a column of groups gets before keys that do not fit beside their command's name
/// go below it.
const COLUMN: usize = 40;

/// The blank columns between two columns of groups.
const GAP: u16 = 3;

/// What the bottom of the frame says while the sheet fits.
const HINT: &str = " Esc: close ";

/// What the bottom of the frame says while the sheet scrolls.
const SCROLL_HINT: &str = " j/k: scroll · Esc: close ";

/// The Keymap dialog: a frame titled "Keymap" with each group's heading in the headers color
/// over its commands, each command's name indented with its keys at its column's right in the
/// primary color, or below it where they do not fit beside it; the groups flow down columns as
/// wide as the widest group up to 40 columns, as many columns as fit, each group whole and a
/// blank line between two groups of a column.
#[derive(Debug)]
pub struct KeymapDialog<'a> {
    groups: &'a [SheetGroup],
    theme: &'a Theme,
    scroll: usize,
}

/// A line of a column: a group's heading, a command's name and keys, or the blank line between
/// two groups.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Item<'a> {
    Heading(&'a str),
    Command(&'a str, &'a str),
    Blank,
}

impl<'a> KeymapDialog<'a> {
    /// The dialog listing `groups` in `theme`, from the top.
    #[must_use]
    pub fn new(groups: &'a [SheetGroup], theme: &'a Theme) -> Self {
        Self {
            groups,
            theme,
            scroll: 0,
        }
    }

    /// The dialog with its sheet scrolled `scroll` lines down, as far as it goes.
    #[must_use]
    pub fn scrolled(self, scroll: usize) -> Self {
        Self { scroll, ..self }
    }

    /// Where the dialog goes on `screen`: centered, 90% of its width and height.
    #[must_use]
    pub fn area(screen: Rect) -> Rect {
        screen.centered(Constraint::Percentage(SHARE), Constraint::Percentage(SHARE))
    }

    /// How far the sheet scrolls when the dialog is drawn on `screen`, and how many lines a page
    /// scrolls: the lines it shows, at least one.
    #[must_use]
    pub fn scroll_range(&self, screen: Rect) -> (usize, usize) {
        let sheet = Self::frame("").inner(Self::area(screen));
        let height = usize::from(sheet.height);
        let lines = self
            .columns(sheet.width)
            .iter()
            .map(Vec::len)
            .max()
            .unwrap_or_default();
        (lines.saturating_sub(height), height.max(1))
    }

    /// The dialog's frame, `hint` at its bottom right.
    fn frame(hint: &str) -> Block<'_> {
        Block::bordered()
            .title(Line::from(" Keymap ").bold())
            .title_bottom(Line::from(hint).dim().right_aligned())
            .padding(Padding::horizontal(1))
    }

    /// The columns a group's lines take: its heading's, or a command's indented name, a space
    /// and its keys.
    fn width((heading, entries): &SheetGroup) -> usize {
        entries
            .iter()
            .map(|(name, keys)| line_width(name, keys))
            .chain([width(heading)])
            .max()
            .unwrap_or_default()
    }

    /// The columns of lines the groups flow down on a sheet `width` columns wide: as many as fit
    /// columns as wide as the widest group, but no wider than [`COLUMN`] or the sheet, at least
    /// one, and as short as they can be.
    fn columns(&self, width: u16) -> Vec<Vec<Item<'a>>> {
        let column = self
            .groups
            .iter()
            .map(Self::width)
            .max()
            .unwrap_or_default()
            .min(COLUMN)
            .min(usize::from(width));
        let gap = usize::from(GAP);
        let fit = ((usize::from(width) + gap) / (column + gap)).max(1);
        let groups: Vec<Vec<Item<'a>>> = self
            .groups
            .iter()
            .map(|group| Self::lines(group, column))
            .collect();
        let total = groups.iter().map(|lines| lines.len() + 1).sum();
        (1..=total)
            .map(|limit| pack(&groups, limit))
            .find(|columns| columns.len() <= fit)
            .unwrap_or_default()
    }

    /// The lines of `group` in a column `column` wide: its heading, then each command's name with
    /// its keys, or the keys on a line of their own below a name they do not fit beside.
    fn lines((heading, entries): &'a SheetGroup, column: usize) -> Vec<Item<'a>> {
        let mut lines = vec![Item::Heading(heading)];
        for (name, keys) in entries {
            if line_width(name, keys) <= column {
                lines.push(Item::Command(name, keys));
            } else {
                lines.extend([Item::Command(name, ""), Item::Command("", keys)]);
            }
        }
        lines
    }

    /// `item` drawn on `row`.
    fn draw(&self, item: Item<'_>, row: Rect, buf: &mut Buffer) {
        match item {
            Item::Heading(heading) => {
                Line::styled(
                    heading.to_owned(),
                    Style::new().fg(self.theme.headers).bold(),
                )
                .render(row, buf);
            }
            Item::Command(name, keys) => {
                let keys = Line::styled(keys.to_owned(), Style::new().fg(self.theme.primary));
                let keys = wrap::truncate(&keys, usize::from(row.width.saturating_sub(1)));
                let width = u16::try_from(keys.width()).unwrap_or(u16::MAX);
                let [name_area, keys_area] = row.layout(
                    &Layout::horizontal([Constraint::Fill(1), Constraint::Length(width)])
                        .spacing(1),
                );
                wrap::truncate(
                    &Line::from(format!(" {name}")),
                    usize::from(name_area.width),
                )
                .render(name_area, buf);
                keys.right_aligned().render(keys_area, buf);
            }
            Item::Blank => {}
        }
    }
}

impl Widget for KeymapDialog<'_> {
    fn render(self, screen: Rect, buf: &mut Buffer) {
        let area = Self::area(screen);
        Clear.render(area, buf);
        let (last, _) = self.scroll_range(screen);
        let frame = Self::frame(if last > 0 { SCROLL_HINT } else { HINT });
        let sheet = frame.inner(area);
        frame.render(area, buf);
        let columns = self.columns(sheet.width);
        let count = u16::try_from(columns.len()).unwrap_or(u16::MAX).max(1);
        let width = sheet.width.saturating_sub(GAP * (count - 1)) / count;
        let scroll = self.scroll.min(last);
        for (at, column) in (0..).zip(&columns) {
            let x = sheet.x.saturating_add(at * (width + GAP));
            let area = Rect::new(x, sheet.y, width, sheet.height).intersection(sheet);
            for (&item, row) in column.iter().skip(scroll).zip(area.rows()) {
                self.draw(item, row, buf);
            }
        }
    }
}

/// `groups`, each a group's lines, in order down columns of at most `limit` lines, a blank line
/// between two groups of a column; a group that does not fit below the last one starts a new
/// column, where it may be taller than `limit`.
fn pack<'a>(groups: &[Vec<Item<'a>>], limit: usize) -> Vec<Vec<Item<'a>>> {
    let mut columns: Vec<Vec<Item<'a>>> = Vec::new();
    for lines in groups {
        let mut column = match columns.pop() {
            Some(mut column) if column.len() + 1 + lines.len() <= limit => {
                column.push(Item::Blank);
                column
            }
            Some(full) => {
                columns.push(full);
                Vec::new()
            }
            None => Vec::new(),
        };
        column.extend_from_slice(lines);
        columns.push(column);
    }
    columns
}

/// The columns a command's line takes: its indented name, a space and its keys.
fn line_width(name: &str, keys: &str) -> usize {
    1 + width(name) + 1 + width(keys)
}

/// How many columns `text` takes on screen.
fn width(text: &str) -> usize {
    Span::raw(text).width()
}

#[cfg(test)]
mod tests {
    use super::super::tests::THEME;
    use super::*;

    /// Every row of the dialog listing `groups`, scrolled `scroll` lines, drawn on a screen
    /// `width` × `height` cells.
    fn render(groups: &[SheetGroup], scroll: usize, width: u16, height: u16) -> Vec<String> {
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        KeymapDialog::new(groups, &THEME)
            .scrolled(scroll)
            .render(area, &mut buf);
        (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
            })
            .collect()
    }

    /// A group headed `heading` of `count` commands, each with the keys `k`.
    fn group(heading: &'static str, count: usize) -> SheetGroup {
        let entries = (1..=count)
            .map(|n| (Cow::from(format!("{heading} {n}")), "k".to_owned()))
            .collect();
        (heading.into(), entries)
    }

    // FR-KEY-07, D-140
    #[test]
    fn the_dialog_takes_90_percent_of_the_screen_centered() {
        assert_eq!(
            KeymapDialog::area(Rect::new(0, 0, 100, 40)),
            Rect::new(5, 2, 90, 36)
        );
    }

    // FR-KEY-07, D-140
    #[test]
    fn groups_flow_down_as_many_columns_as_fit_as_short_as_they_can_be_each_group_whole() {
        let groups = [group("Ab", 3), group("Cd", 2), group("Ef", 1)];
        assert_eq!(
            render(&groups, 0, 30, 10),
            [
                "",
                "  ┌ Keymap ─────────────────┐",
                "  │ Ab           Cd         │",
                "  │  Ab 1    k    Cd 1    k │",
                "  │  Ab 2    k    Cd 2    k │",
                "  │  Ab 3    k              │",
                "  │              Ef         │",
                "  │               Ef 1    k │",
                "  │                         │",
                "  └───────────── Esc: close ┘",
            ]
        );
        assert_eq!(
            render(&groups, 0, 40, 6)[1..4],
            [
                "  │ Ab         Cd         Ef         │",
                "  │  Ab 1  k    Cd 1  k    Ef 1  k   │",
                "  │  Ab 2  k    Cd 2  k              │",
            ],
            "a column a group where they fit"
        );
    }

    // FR-KEY-07, D-140
    #[test]
    fn a_blank_line_parts_two_groups_of_a_column_and_counts_toward_its_height() {
        let headings = [group("Ab", 0), group("Cd", 0)];
        assert_eq!(
            render(&headings, 0, 20, 6)[1..3],
            [" │ Ab      Cd     │", " │                │"],
            "two lines each side by side rather than three in one column"
        );
        let groups = [group("Ab", 0), group("Cd", 3)];
        assert_eq!(
            render(&groups, 0, 12, 10)[1..8],
            [
                " ┌ Keymap ┐",
                " │ Ab     │",
                " │        │",
                " │ Cd     │",
                " │  Cd 1  │",
                " │      k │",
                " │  Cd 2  │",
            ]
        );
    }

    // FR-KEY-07, D-140
    #[test]
    fn keys_go_below_their_name_from_one_column_too_wide_on() {
        let keys = |keys: &str| {
            [(
                Cow::from("Group"),
                vec![(Cow::from("Name"), keys.to_owned())],
            )]
        };
        assert_eq!(
            render(&keys("123456789012345678"), 0, 30, 8)[2..4],
            [
                "  │  Name                   │",
                "  │      123456789012345678 │"
            ],
            "a line one column wider than the sheet"
        );
        assert_eq!(
            render(&keys("12345678901234567"), 0, 30, 8)[2..4],
            [
                "  │  Name 12345678901234567 │",
                "  │                         │"
            ]
        );
    }

    // FR-KEY-07, D-140
    #[test]
    fn a_sheet_too_tall_for_the_screen_scrolls_no_further_than_its_last_line_and_says_how() {
        let groups = [group("Ab", 6), group("Cd", 6)];
        let dialog = KeymapDialog::new(&groups, &THEME);
        let screen = Rect::new(0, 0, 20, 10);
        assert_eq!(dialog.scroll_range(screen), (8, 7), "15 lines, 7 shown");
        let rows = render(&groups, 3, 20, 10);
        assert_eq!(rows[2], " │  Ab 3        k │");
        assert_eq!(
            render(&[group("Ab", 20)], 0, 40, 12)[10],
            "  └──────── j/k: scroll · Esc: close ┘"
        );
        assert_eq!(render(&groups, 99, 20, 10), render(&groups, 8, 20, 10));
    }

    // FR-KEY-07, D-140
    #[test]
    fn keys_that_do_not_fit_beside_a_name_go_below_it_and_what_does_not_fit_is_cut() {
        let groups = [(
            Cow::from("Group"),
            vec![(Cow::from("A very long command name"), "Ctrl+X".to_owned())],
        )];
        assert_eq!(
            render(&groups, 0, 22, 6)[2..4],
            [" │  A very long c…  │", " │           Ctrl+X │"]
        );
        let groups = [(
            Cow::from("Group"),
            vec![(Cow::from("Name"), "Ctrl+Shift+←, Ctrl+W ←".to_owned())],
        )];
        assert_eq!(
            render(&groups, 0, 22, 6)[2..4],
            [" │  Name            │", " │  Ctrl+Shift+←, … │"],
            "keys too wide for the column go below the name, cut at the end"
        );
    }
}
