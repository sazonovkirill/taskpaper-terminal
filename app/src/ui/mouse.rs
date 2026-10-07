//! The mouse: what is under the pointer, as the last frame drew it, and the intents a click, an
//! Alt+click, a right-click or the wheel there gives
//! ([ui.md §6](../../../docs/specs/ui.md#6-mouse-fr-key-05-d-16), FR-KEY-05, D-16).

use std::time::Duration;

use crossterm::event::{Event, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Position, Rect};

use super::{Tui, widgets};
use crate::{
    app::{Fold, Message, Motion, PaneChange, PaneId, SelectionChange, SidebarInput, TextEdit},
    document::ItemId,
};

/// How many screen lines a step of the wheel scrolls.
const WHEEL_LINES: isize = 3;

/// Where the last frame drew what the pointer can act on.
#[derive(Debug, Default)]
pub(super) struct Drawn {
    /// The sidebar's area and its first line on screen.
    pub(super) sidebar: Option<(Rect, usize)>,
    /// The panes on screen.
    pub(super) panes: Vec<DrawnPane>,
    /// The shortcuts panel's area.
    pub(super) shortcuts: Option<Rect>,
}

/// A pane as the last frame drew it.
#[derive(Debug)]
pub(super) struct DrawnPane {
    /// Its position among the panes, counted from 0 at the left.
    pub(super) at: usize,
    pub(super) id: PaneId,
    /// The part of the screen it shows, which leaves out columns cut off at an edge.
    pub(super) shown: Rect,
    /// How many of its columns are cut off at the left edge.
    pub(super) skipped: u16,
    /// Its width without the rule on its right edge.
    pub(super) width: u16,
}

/// What is under the pointer, by where it was drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Spot {
    /// A line of the sidebar, counted from its first, and a column after its gutter.
    Sidebar { line: usize, column: u16 },
    /// The header of the pane at `at` among `Drawn::panes`, at a column of its breadcrumbs, or
    /// on its gutter or rule.
    Header { pane: usize, column: Option<u16> },
    /// A screen line of the outline of the pane at `pane` among `Drawn::panes`, counted from its
    /// top, and a column counted from its left edge.
    Outline {
        pane: usize,
        line: usize,
        column: u16,
    },
    /// The shortcuts panel.
    Shortcuts,
}

impl Drawn {
    /// What the last frame drew at `position`.
    fn spot(&self, position: Position) -> Option<Spot> {
        if self.shortcuts.is_some_and(|area| area.contains(position)) {
            return Some(Spot::Shortcuts);
        }
        if let Some((area, scroll)) = self.sidebar
            && area.contains(position)
        {
            // The sidebar's rightmost column is its rule.
            let column = position.x - area.x;
            return (column + 1 < area.width).then(|| Spot::Sidebar {
                line: usize::from(position.y - area.y) + scroll,
                column: column.saturating_sub(widgets::GUTTER_WIDTH),
            });
        }
        let (index, pane) = self
            .panes
            .iter()
            .enumerate()
            .find(|(_, pane)| pane.shown.contains(position))?;
        let column = position.x - pane.shown.x + pane.skipped;
        if column >= pane.width {
            return None;
        }
        Some(match position.y - pane.shown.y {
            0 => Spot::Header {
                pane: index,
                column: column.checked_sub(widgets::GUTTER_WIDTH),
            },
            1 => Spot::Header {
                pane: index,
                column: None,
            },
            line => Spot::Outline {
                pane: index,
                line: usize::from(line - 2),
                column,
            },
        })
    }
}

/// What a gesture acts on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Target {
    /// The sidebar entry at this index, on its fold marker when it has a fold.
    Entry { at: usize, fold: Option<Fold> },
    /// The header of the pane at `pane`, on its breadcrumb at this depth if any.
    Header { pane: usize, crumb: Option<usize> },
    /// The block `id` of the pane at `pane`, on its fold marker when `fold`, and whether it is
    /// selected.
    Block {
        pane: usize,
        id: ItemId,
        fold: bool,
        selected: bool,
    },
    /// The text being edited, at this byte offset.
    Text(usize),
}

/// What the pointer did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Gesture {
    Click,
    AltClick,
    RightClick,
}

/// What a mouse event asks for: a press of a button, or the wheel scrolling by that many
/// screen lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Press(Gesture),
    Wheel(isize),
}

/// What `mouse` asks for, if anything: moving the pointer, dragging, releasing a button and the
/// other buttons and wheel directions ask for nothing (ui.md §6).
fn action(mouse: MouseEvent) -> Option<Action> {
    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) if mouse.modifiers.contains(KeyModifiers::ALT) => {
            Some(Action::Press(Gesture::AltClick))
        }
        MouseEventKind::Down(MouseButton::Left) => Some(Action::Press(Gesture::Click)),
        MouseEventKind::Down(MouseButton::Right) => Some(Action::Press(Gesture::RightClick)),
        MouseEventKind::ScrollDown => Some(Action::Wheel(WHEEL_LINES)),
        MouseEventKind::ScrollUp => Some(Action::Wheel(-WHEEL_LINES)),
        MouseEventKind::Down(MouseButton::Middle)
        | MouseEventKind::Up(_)
        | MouseEventKind::Drag(_)
        | MouseEventKind::Moved
        | MouseEventKind::ScrollLeft
        | MouseEventKind::ScrollRight => None,
    }
}

/// Whether the frontend may act on `event`: every event but a mouse event that asks for
/// nothing, such as each move of the pointer, which the event loop drops rather than draw a
/// frame for (ui.md §6).
#[must_use]
pub fn acts_on(event: &Event) -> bool {
    match event {
        Event::Mouse(mouse) => action(*mouse).is_some(),
        Event::FocusGained
        | Event::FocusLost
        | Event::Key(_)
        | Event::Paste(_)
        | Event::Resize(..) => true,
    }
}

/// The messages `gesture` on `target` gives, in order (ui.md §6): a click focuses or goes, an
/// Alt+click opens in a new pane and a right-click opens the context menu. A click on a pane's
/// part gives that pane the keys first, and a right-click on a block that is not selected makes
/// it the block the menu is for.
fn messages(target: Target, gesture: Gesture) -> Vec<Message> {
    let pane = |at: usize| {
        vec![
            Message::Sidebar(SidebarInput::Leave),
            Message::Pane(PaneChange::Focus(at)),
        ]
    };
    let mut messages = match target {
        Target::Entry { at, .. } => vec![Message::Sidebar(SidebarInput::Focus(at))],
        Target::Header { pane: at, .. } | Target::Block { pane: at, .. } => pane(at),
        Target::Text(_) => Vec::new(),
    };
    messages.extend(match (target, gesture) {
        (Target::Entry { fold, .. }, Gesture::Click) => vec![Message::Sidebar(match fold {
            Some(Fold::Expanded) => SidebarInput::Collapse,
            Some(Fold::Collapsed) => SidebarInput::Expand,
            Some(Fold::Leaf) | None => SidebarInput::Choose,
        })],
        (Target::Entry { .. }, Gesture::AltClick) => vec![Message::Pane(PaneChange::Open)],
        (Target::Entry { .. }, Gesture::RightClick) => vec![Message::OpenContextMenu],
        (Target::Header { crumb, .. }, Gesture::Click) => {
            crumb.map(Message::GoToCrumb).into_iter().collect()
        }
        (Target::Header { crumb, .. }, Gesture::AltClick) => crumb
            .map(|depth| Message::Pane(PaneChange::OpenCrumb(depth)))
            .into_iter()
            .collect(),
        (Target::Header { crumb, .. }, Gesture::RightClick) => vec![Message::OpenPaneMenu(crumb)],
        (Target::Block { id, fold, .. }, Gesture::Click) => [Message::Focus(id)]
            .into_iter()
            .chain(fold.then_some(Message::ToggleFold))
            .collect(),
        (Target::Block { id, .. }, Gesture::AltClick) => {
            vec![Message::Focus(id), Message::Pane(PaneChange::Open)]
        }
        (Target::Block { selected: true, .. }, Gesture::RightClick) => {
            vec![Message::OpenContextMenu]
        }
        (Target::Block { id, .. }, Gesture::RightClick) => vec![
            Message::Select(SelectionChange::Clear),
            Message::Focus(id),
            Message::OpenContextMenu,
        ],
        (Target::Text(offset), _) => vec![Message::Text(TextEdit::Move {
            motion: Motion::To(offset),
            select: false,
        })],
    });
    messages
}

impl Tui {
    /// Handles a mouse event at time `now`: a click, Alt+click or right-click on what the last
    /// frame drew there, or the wheel over a pane or the shortcuts panel (ui.md §6). The mouse
    /// does nothing while a popup or prompt takes the keys.
    pub(super) fn handle_mouse(&mut self, mouse: MouseEvent, now: Duration) {
        if self.overlay().is_some() || self.state.command_menu().is_some() {
            return;
        }
        let position = Position::new(mouse.column, mouse.row);
        let Some(spot) = self.drawn.spot(position) else {
            return;
        };
        let gesture = match action(mouse) {
            Some(Action::Press(gesture)) => gesture,
            Some(Action::Wheel(lines)) => return self.wheel(spot, lines),
            None => return,
        };
        // A click acts where the panes show their focus again.
        self.wheel.clear();
        let Some(target) = self.target(spot, gesture) else {
            return;
        };
        if gesture == Gesture::RightClick {
            self.menu_anchor = Some((position.x, position.y));
        }
        for message in messages(target, gesture) {
            self.send(message, now);
        }
    }

    /// Scrolls what is at `spot` by `lines` screen lines: a pane's outline, which then stays
    /// where it is scrolled to until a key or a click, or the shortcuts panel (D-55).
    fn wheel(&mut self, spot: Spot, lines: isize) {
        match spot {
            Spot::Header { pane, .. } | Spot::Outline { pane, .. } => {
                if let Some(pane) = self.drawn.panes.get(pane) {
                    *self.wheel.entry(pane.id).or_default() += lines;
                }
            }
            Spot::Shortcuts => {
                self.shortcuts_scroll = self.shortcuts_scroll.saturating_add_signed(lines);
            }
            Spot::Sidebar { .. } => {}
        }
    }

    /// What `gesture` at `spot` acts on, if anything is drawn there.
    fn target(&self, spot: Spot, gesture: Gesture) -> Option<Target> {
        match spot {
            Spot::Sidebar { line, column } => {
                let view = self.state.sidebar()?;
                let (at, fold) =
                    widgets::SidebarPanel::new(&view, self.icons).entry_at(line, column)?;
                Some(Target::Entry { at, fold })
            }
            Spot::Header { pane, column } => {
                let drawn = self.drawn.panes.get(pane)?;
                let view = self.state.panes().into_iter().nth(drawn.at)?;
                let width = drawn.width.saturating_sub(widgets::GUTTER_WIDTH);
                let crumb = column
                    .filter(|_| matches!(view.location, crate::app::Location::Outline(_)))
                    .and_then(|column| {
                        widgets::crumb_at(
                            &view.breadcrumbs,
                            usize::from(width),
                            self.icons,
                            usize::from(column),
                        )
                    });
                Some(Target::Header {
                    pane: drawn.at,
                    crumb,
                })
            }
            Spot::Outline { pane, line, column } => {
                let drawn = self.drawn.panes.get(pane)?;
                let view = self.state.panes().into_iter().nth(drawn.at)?;
                let scroll = self.scrolls.get(&view.id).copied().unwrap_or_default();
                let editor = self.state.editor().filter(|_| view.focused);
                let shown = view.rows.get(scroll.row..).unwrap_or_default();
                let (index, fold) = widgets::OutlineView::new(shown, (self.clock)().date())
                    .editor(editor.as_ref())
                    .wrap(self.wrap)
                    .icons(self.icons)
                    .offset(scroll.line)
                    .hit(drawn.width, line, column)?;
                let row = shown.get(index)?;
                if let Some(editor) = editor.filter(|editor| editor.id == row.id)
                    && gesture == Gesture::Click
                {
                    let width = drawn.width.saturating_sub(widgets::GUTTER_WIDTH);
                    let column = column.saturating_sub(widgets::GUTTER_WIDTH);
                    return Some(Target::Text(widgets::editor_offset(
                        row,
                        &editor,
                        usize::from(width),
                        self.icons,
                        usize::from(column),
                    )));
                }
                Some(Target::Block {
                    pane: drawn.at,
                    id: row.id,
                    fold,
                    selected: row.selected,
                })
            }
            Spot::Shortcuts => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pane(at: usize, shown: Rect, skipped: u16, width: u16) -> DrawnPane {
        DrawnPane {
            at,
            id: crate::app::State::new(crate::document::Document::default(), None).panes()[0].id,
            shown,
            skipped,
            width,
        }
    }

    // FR-KEY-05
    #[test]
    fn a_position_names_what_the_last_frame_drew_there() {
        let drawn = Drawn {
            sidebar: Some((Rect::new(0, 0, 10, 8), 3)),
            panes: vec![
                pane(1, Rect::new(10, 0, 5, 8), 7, 11),
                pane(2, Rect::new(15, 0, 12, 8), 0, 12),
            ],
            shortcuts: Some(Rect::new(24, 0, 3, 8)),
        };
        let spot = |x, y| drawn.spot(Position::new(x, y));
        assert_eq!(spot(3, 2), Some(Spot::Sidebar { line: 5, column: 1 }));
        assert_eq!(spot(1, 0), Some(Spot::Sidebar { line: 3, column: 0 }));
        assert_eq!(spot(9, 0), None, "the sidebar's rule");
        assert_eq!(
            spot(10, 0),
            Some(Spot::Header {
                pane: 0,
                column: Some(5)
            })
        );
        assert_eq!(
            spot(13, 1),
            Some(Spot::Header {
                pane: 0,
                column: None
            })
        );
        assert_eq!(spot(14, 4), None, "the pane's rule");
        assert_eq!(
            spot(15, 0),
            Some(Spot::Header {
                pane: 1,
                column: None
            }),
            "the gutter"
        );
        assert_eq!(
            spot(17, 0),
            Some(Spot::Header {
                pane: 1,
                column: Some(0)
            })
        );
        assert_eq!(
            spot(16, 7),
            Some(Spot::Outline {
                pane: 1,
                line: 5,
                column: 1
            })
        );
        assert_eq!(spot(24, 3), Some(Spot::Shortcuts));
        assert_eq!(spot(27, 3), None, "nothing drawn");
        assert_eq!(Drawn::default().spot(Position::new(0, 0)), None);
        let below = Drawn {
            sidebar: Some((Rect::new(2, 1, 10, 8), 0)),
            panes: vec![pane(0, Rect::new(12, 1, 10, 8), 0, 10)],
            shortcuts: None,
        };
        let spot = |x, y| below.spot(Position::new(x, y));
        assert_eq!(spot(5, 4), Some(Spot::Sidebar { line: 3, column: 1 }));
        assert_eq!(
            spot(15, 4),
            Some(Spot::Outline {
                pane: 0,
                line: 1,
                column: 3
            }),
            "areas that start below and right of the screen's corner"
        );
    }

    // FR-KEY-05, ui.md §6
    #[test]
    fn only_presses_and_the_wheel_ask_for_anything_so_other_mouse_events_draw_nothing() {
        let mouse = |kind, modifiers| {
            Event::Mouse(MouseEvent {
                kind,
                column: 3,
                row: 4,
                modifiers,
            })
        };
        let acts = |kind| acts_on(&mouse(kind, KeyModifiers::NONE));
        assert!(acts(MouseEventKind::Down(MouseButton::Left)));
        assert!(acts_on(&mouse(
            MouseEventKind::Down(MouseButton::Left),
            KeyModifiers::ALT
        )));
        assert!(acts(MouseEventKind::Down(MouseButton::Right)));
        assert!(acts(MouseEventKind::ScrollDown));
        assert!(acts(MouseEventKind::ScrollUp));
        for kind in [
            MouseEventKind::Moved,
            MouseEventKind::Drag(MouseButton::Left),
            MouseEventKind::Up(MouseButton::Left),
            MouseEventKind::Down(MouseButton::Middle),
            MouseEventKind::ScrollLeft,
            MouseEventKind::ScrollRight,
        ] {
            assert!(!acts(kind), "{kind:?}");
        }
        for event in [
            Event::FocusGained,
            Event::FocusLost,
            Event::Key(crossterm::event::KeyEvent::from(
                crossterm::event::KeyCode::Esc,
            )),
            Event::Paste("a".to_owned()),
            Event::Resize(80, 24),
        ] {
            assert!(acts_on(&event), "{event:?}");
        }
    }

    #[track_caller]
    fn check(target: Target, gesture: Gesture, expected: &[Message]) {
        assert_eq!(
            messages(target, gesture),
            expected,
            "{gesture:?} on {target:?}"
        );
    }

    // FR-KEY-05, FR-KEY-06, ui.md §6
    #[test]
    fn a_sidebar_entry_is_focused_then_gone_to_folded_opened_or_given_a_menu() {
        let focus = Message::Sidebar(SidebarInput::Focus(4));
        let entry = |fold| Target::Entry { at: 4, fold };
        let then = |input| [focus.clone(), Message::Sidebar(input)];
        check(entry(None), Gesture::Click, &then(SidebarInput::Choose));
        check(
            entry(Some(Fold::Leaf)),
            Gesture::Click,
            &then(SidebarInput::Choose),
        );
        check(
            entry(Some(Fold::Expanded)),
            Gesture::Click,
            &then(SidebarInput::Collapse),
        );
        check(
            entry(Some(Fold::Collapsed)),
            Gesture::Click,
            &then(SidebarInput::Expand),
        );
        check(
            entry(None),
            Gesture::AltClick,
            &[focus.clone(), Message::Pane(PaneChange::Open)],
        );
        check(
            entry(None),
            Gesture::RightClick,
            &[focus, Message::OpenContextMenu],
        );
    }

    /// The messages that give the pane at 2 the keys, then `rest`.
    fn in_pane(rest: &[Message]) -> Vec<Message> {
        [
            Message::Sidebar(SidebarInput::Leave),
            Message::Pane(PaneChange::Focus(2)),
        ]
        .into_iter()
        .chain(rest.iter().cloned())
        .collect()
    }

    // FR-KEY-05, FR-KEY-06, ui.md §6
    #[test]
    fn a_header_focuses_its_pane_then_goes_to_opens_or_gives_a_menu_on_a_crumb() {
        let header = |crumb| Target::Header { pane: 2, crumb };
        check(header(None), Gesture::Click, &in_pane(&[]));
        check(
            header(Some(1)),
            Gesture::Click,
            &in_pane(&[Message::GoToCrumb(1)]),
        );
        check(header(None), Gesture::AltClick, &in_pane(&[]));
        check(
            header(Some(0)),
            Gesture::AltClick,
            &in_pane(&[Message::Pane(PaneChange::OpenCrumb(0))]),
        );
        check(
            header(None),
            Gesture::RightClick,
            &in_pane(&[Message::OpenPaneMenu(None)]),
        );
        check(
            header(Some(2)),
            Gesture::RightClick,
            &in_pane(&[Message::OpenPaneMenu(Some(2))]),
        );
    }

    // FR-KEY-05, FR-KEY-06, ui.md §6
    #[test]
    fn a_block_is_focused_then_folded_opened_or_given_a_menu_and_text_takes_the_caret() {
        let id = ItemId::default();
        let block = |fold, selected| Target::Block {
            pane: 2,
            id,
            fold,
            selected,
        };
        check(
            block(false, false),
            Gesture::Click,
            &in_pane(&[Message::Focus(id)]),
        );
        check(
            block(true, false),
            Gesture::Click,
            &in_pane(&[Message::Focus(id), Message::ToggleFold]),
        );
        check(
            block(true, true),
            Gesture::AltClick,
            &in_pane(&[Message::Focus(id), Message::Pane(PaneChange::Open)]),
        );
        check(
            block(false, true),
            Gesture::RightClick,
            &in_pane(&[Message::OpenContextMenu]),
        );
        check(
            block(false, false),
            Gesture::RightClick,
            &in_pane(&[
                Message::Select(SelectionChange::Clear),
                Message::Focus(id),
                Message::OpenContextMenu,
            ]),
        );
        check(
            Target::Text(3),
            Gesture::Click,
            &[Message::Text(TextEdit::Move {
                motion: Motion::To(3),
                select: false,
            })],
        );
    }
}
