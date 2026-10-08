//! Animation: the effects drawn over a rendered frame to show where focus landed, what a command
//! changed and that an intent was refused, the spinner of a wait, and how much the `animations`
//! setting lets them do ([ui.md §9](../../../docs/specs/ui.md#9-animation-fr-ui-11)). Hides
//! tachyonfx, the triggers and every duration.

use std::{collections::HashMap, ffi::OsStr, mem, time::Duration};

use ratatui::{buffer::Buffer, layout::Rect, style::Color};
use tachyonfx::{ColorSpace, Effect, EffectManager, EffectTimer, Interpolation, fx};

use super::{
    layout::Scroll,
    theme::{ColorSupport, IconSet, Theme},
};
use crate::{
    app::{Animations, Disk, Location, Menu, Message, Mode, PaneId, Rows, SaveAs, State},
    document::ItemId,
};

/// The `animations` setting's values as the settings file names them, in the settings window's
/// order (ui.md §9.3).
pub(super) const NAMES: [(Animations, &str); 3] = [
    (Animations::Full, "full"),
    (Animations::Reduced, "reduced"),
    (Animations::Off, "off"),
];

/// The setting a session runs with when the settings file does not set it: full, or reduced over
/// SSH, where every frame crosses the network, as `ssh_connection` and `ssh_tty`, the values of
/// `SSH_CONNECTION` and `SSH_TTY`, tell when either is set (ui.md §9.3).
#[must_use]
pub fn default_for(ssh_connection: Option<&OsStr>, ssh_tty: Option<&OsStr>) -> Animations {
    if ssh_connection.is_some() || ssh_tty.is_some() {
        Animations::Reduced
    } else {
        Animations::Full
    }
}

/// How long a frame lasts while an effect runs: 60 frames a second (ui.md §9.1).
const FRAME: Duration = Duration::from_micros(16_667);

/// How long the landing flash and the change highlight last (UI-ANIM-01, UI-ANIM-02).
const FLASH: Duration = Duration::from_millis(250);

/// How long the refusal flash lasts (UI-ANIM-03).
const REFUSAL: Duration = Duration::from_millis(150);

/// How much of its color a flash mixes into a background in RGB at its start: a faint tint.
const TINT: f32 = 0.3;

/// How long a wait lasts before the spinner shows (UI-ANIM-08).
const PATIENCE: Duration = Duration::from_millis(300);

/// How long each frame of the spinner shows.
const SPIN: Duration = Duration::from_millis(80);

/// The frames of the spinner, and with an ASCII locale (ui.md §3).
const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
const ASCII_SPINNER: [&str; 4] = ["|", "/", "-", "\\"];

/// What an effect over a pane shows; a new one replaces a running one of the same kind there.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    /// Where focus landed (UI-ANIM-01).
    #[default]
    Landing,
    /// What a command changed (UI-ANIM-02).
    Change,
    /// That an intent was refused (UI-ANIM-03).
    Refusal,
}

/// The kinds, each of which can run in every pane.
const KINDS: [Kind; 3] = [Kind::Landing, Kind::Change, Kind::Refusal];

/// What an effect is keyed by: its pane, always there, as `EffectManager` asks for keys with a
/// default, and its kind.
type Key = (Option<PaneId>, Kind);

/// What the pane with the keys showed before an input, which tells after it whether focus
/// jumped.
#[derive(Debug)]
pub(super) struct Before {
    pane: Option<PaneId>,
    location: Location,
    focus: Option<ItemId>,
    rows: Rows,
}

impl Before {
    /// What `state`'s pane with the keys shows now.
    pub(super) fn of(state: &State) -> Self {
        Self {
            pane: state
                .panes()
                .iter()
                .find(|pane| pane.focused)
                .map(|pane| pane.id),
            location: state.location(),
            focus: state.focus(),
            rows: state.rows(),
        }
    }

    /// Whether focus jumped from here to `after`: it moved, other than to a block next to the
    /// one it left, in the rows before or after the move, in the same pane and place (UI-ANIM-01).
    fn jumped_to(&self, after: &Self) -> bool {
        let Some(focus) = after.focus else {
            return false;
        };
        if self.pane != after.pane || self.location != after.location {
            return true;
        }
        let Some(left) = self.focus.filter(|&left| left != focus) else {
            return false;
        };
        let next_to = |rows: &Rows| {
            let at = |id| rows.iter().position(|row| row.id == id);
            at(left)
                .zip(at(focus))
                .is_some_and(|(a, b)| a.abs_diff(b) == 1)
        };
        !(next_to(&self.rows) || next_to(&after.rows))
    }
}

/// What a pane showed at a draw: the effects running over it end once it shows anything else.
#[derive(Debug, Clone)]
pub(super) struct Shown {
    /// Where its outline is on screen.
    pub area: Rect,
    /// How far its outline is scrolled.
    pub scroll: Scroll,
    /// Its focused block.
    pub focus: Option<ItemId>,
    /// Its rows.
    pub rows: Rows,
}

impl Shown {
    /// Whether `other` shows the same.
    fn same(&self, other: &Self) -> bool {
        self.area == other.area
            && self.scroll == other.scroll
            && self.focus == other.focus
            && self.rows == other.rows
    }
}

/// A pane as a draw put it on screen, for the effects over it.
#[derive(Debug)]
pub(super) struct DrawnPane {
    /// The pane.
    pub id: PaneId,
    /// Whether it has the keys.
    pub focused: bool,
    /// What it shows.
    pub shown: Shown,
    /// The screen lines of each block on screen, in order.
    pub rows: Vec<(ItemId, Rect)>,
}

/// What the latest inputs asked to show once the next frame places it.
#[derive(Debug, Default)]
struct Pending {
    landing: bool,
    refused: bool,
    changed: Vec<ItemId>,
}

impl Pending {
    fn is_empty(&self) -> bool {
        !self.landing && !self.refused && self.changed.is_empty()
    }
}

/// The effects over the panes and the spinner, as the `animations` setting allows them.
pub(super) struct Animator {
    setting: Animations,
    manager: EffectManager<Key>,
    /// What each pane with effects showed when they started.
    running: HashMap<PaneId, Shown>,
    /// What the input being handled asked to show.
    fresh: Pending,
    /// What the latest input asked to show, which the next frame starts.
    pending: Pending,
    /// The panes the frame being drawn put on screen.
    frame: Vec<DrawnPane>,
    /// The time, since the frontend started.
    now: Duration,
    /// When the latest frame was drawn.
    drawn_at: Duration,
    /// How many disk operations the user waits on, and since when.
    waits: usize,
    waiting_since: Duration,
}

impl std::fmt::Debug for Animator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Animator")
            .field("setting", &self.setting)
            .field("running", &self.running.keys())
            .field("waits", &self.waits)
            .finish_non_exhaustive()
    }
}

impl Animator {
    /// Effects as `setting` allows them, at time zero.
    pub(super) fn new(setting: Animations) -> Self {
        Self {
            setting,
            manager: EffectManager::default(),
            running: HashMap::new(),
            fresh: Pending::default(),
            pending: Pending::default(),
            frame: Vec::new(),
            now: Duration::ZERO,
            drawn_at: Duration::ZERO,
            waits: 0,
            waiting_since: Duration::ZERO,
        }
    }

    /// The setting effects follow.
    pub(super) fn setting(&self) -> Animations {
        self.setting
    }

    /// Follows `setting` from now on; effects it does not allow end at once.
    pub(super) fn set(&mut self, setting: Animations) {
        self.setting = setting;
        if setting == Animations::Off {
            self.cancel_all();
            self.pending = Pending::default();
        }
    }

    /// Lets time pass to `now`, since the frontend started.
    pub(super) fn at(&mut self, now: Duration) {
        self.now = self.now.max(now);
    }

    /// Takes in what the core reports after it handled `message`: the blocks it changed, which
    /// are highlighted when the command changed blocks other than just the focused one, or brought
    /// blocks in from the clipboard, a duplicate, Move to, the history or the disk; and whether it
    /// refused the intent (UI-ANIM-02, UI-ANIM-03).
    pub(super) fn observe(&mut self, brought: bool, state: &State) {
        self.fresh.refused |= state.refused();
        let changed = state.changed();
        let focus = state.focus();
        let elsewhere = changed.iter().any(|&id| Some(id) != focus);
        if elsewhere || (brought && state.mode() == Mode::Navigate) {
            self.fresh.changed.extend_from_slice(changed);
        }
    }

    /// Ends the input that started at `before`, whose effects the next frame starts, with a
    /// landing flash when focus jumped (UI-ANIM-01). The event loop draws a frame after every
    /// input, so what an earlier input asked for has started or been left behind by then.
    pub(super) fn settle(&mut self, before: &Before, state: &State) {
        let mut fresh = mem::take(&mut self.fresh);
        if self.setting == Animations::Off {
            return;
        }
        fresh.landing = before.jumped_to(&Before::of(state));
        self.pending = fresh;
    }

    /// Whether an effect is running or waits to start, which asks for frames.
    fn animating(&self) -> bool {
        self.manager.is_running() || !self.pending.is_empty()
    }

    /// Notes where the frame being drawn put `pane`.
    pub(super) fn drawn(&mut self, pane: DrawnPane) {
        self.frame.push(pane);
    }

    /// Draws the effects over the panes of the frame being drawn on `buf` in `theme`'s colors, as
    /// far as `colors` allows: those of panes that show anything else end, time moves on to now,
    /// and what the inputs since the last frame asked for starts (ui.md §9.1).
    pub(super) fn process(&mut self, buf: &mut Buffer, theme: &Theme, colors: ColorSupport) {
        let panes = mem::take(&mut self.frame);
        let pending = mem::take(&mut self.pending);
        let elapsed = self.now.saturating_sub(self.drawn_at);
        self.drawn_at = self.now;
        let ended: Vec<PaneId> = self
            .running
            .iter()
            .filter(|&(id, shown)| {
                !panes
                    .iter()
                    .any(|pane| pane.id == *id && pane.shown.same(shown))
            })
            .map(|(&id, _)| id)
            .collect();
        for id in ended {
            self.cancel(id);
        }
        // Time moves on off screen, so that effects ended or done by now are never drawn.
        self.manager
            .process_effects(elapsed, &mut Buffer::empty(Rect::ZERO), Rect::ZERO);
        if !self.manager.is_running() {
            self.running.clear();
        }
        if colors != ColorSupport::None && self.setting != Animations::Off {
            for pane in &panes {
                self.start(pane, &pending, theme);
            }
        }
        // Drops effects that new ones replaced before they are drawn.
        self.manager
            .process_effects(Duration::ZERO, &mut Buffer::empty(Rect::ZERO), Rect::ZERO);
        let area = buf.area;
        self.manager.process_effects(Duration::ZERO, buf, area);
    }

    /// Starts over `pane` the effects `pending` asks for there.
    fn start(&mut self, pane: &DrawnPane, pending: &Pending, theme: &Theme) {
        let lines = |wanted: &dyn Fn(ItemId) -> bool| -> Vec<Rect> {
            pane.rows
                .iter()
                .filter(|&&(id, _)| wanted(id))
                .map(|&(_, lines)| lines)
                .collect()
        };
        let focus = pane.shown.focus.filter(|_| pane.focused);
        let at_focus = |id| Some(id) == focus;
        let effects = [
            (
                Kind::Landing,
                pending.landing,
                theme.accent,
                FLASH,
                &at_focus as &dyn Fn(_) -> _,
            ),
            (
                Kind::Refusal,
                pending.refused,
                theme.error,
                REFUSAL,
                &at_focus,
            ),
            (
                Kind::Change,
                !pending.changed.is_empty(),
                theme.accent,
                FLASH,
                &|id| pending.changed.contains(&id),
            ),
        ];
        for (kind, asked, color, duration, wanted) in effects {
            let lines = lines(wanted);
            if !asked || lines.is_empty() {
                continue;
            }
            let fades = self.setting == Animations::Full && rgb(color) && rgb(theme.background);
            let effect = flash(lines, color, theme.background, duration, fades);
            self.manager
                .add_unique_effect((Some(pane.id), kind), effect);
            self.running.insert(pane.id, pane.shown.clone());
        }
    }

    /// Ends the effects running over the pane `id`.
    fn cancel(&mut self, id: PaneId) {
        for kind in KINDS {
            self.manager.cancel_unique_effect((Some(id), kind));
        }
        self.running.remove(&id);
    }

    /// Ends every effect.
    fn cancel_all(&mut self) {
        let ids: Vec<PaneId> = self.running.keys().copied().collect();
        for id in ids {
            self.cancel(id);
        }
        self.manager
            .process_effects(Duration::ZERO, &mut Buffer::empty(Rect::ZERO), Rect::ZERO);
    }

    /// Notes at time `now` that `disk` is asked for, which the user waits on when it is a write,
    /// whose result always comes back. Backups and checks run unwatched: neither reports every
    /// outcome, so a wait on them might never end.
    pub(super) fn submitted(&mut self, disk: &Disk, now: Duration) {
        if !matches!(
            disk,
            Disk::Save { .. } | Disk::SaveAs { .. } | Disk::ConflictCopy { .. }
        ) {
            return;
        }
        if self.waits == 0 {
            self.waiting_since = now;
        }
        self.waits += 1;
    }

    /// Notes that `message` came in, which ends a wait when it is the result of a disk operation.
    pub(super) fn received(&mut self, message: &Message) {
        let result = matches!(
            message,
            Message::Saved { .. }
                | Message::SaveFailed { .. }
                | Message::SaveRefused { .. }
                | Message::ConflictCopied { .. }
                | Message::ConflictCopyFailed { .. }
                | Message::SaveAs(
                    SaveAs::Saved { .. } | SaveAs::Failed { .. } | SaveAs::Refused { .. }
                )
        );
        if result {
            self.waits = self.waits.saturating_sub(1);
        }
    }

    /// The spinner the status line shows in `icons` while the user has waited longer than
    /// [`PATIENCE`] on the disk: turning, or a static `…` unless every animation runs
    /// (UI-ANIM-08).
    pub(super) fn spinner(&self, icons: IconSet) -> Option<&'static str> {
        let waited = self.waited()?;
        let frames: &[&str] = match icons {
            IconSet::Ascii => &ASCII_SPINNER,
            IconSet::Nerd | IconSet::Unicode => &SPINNER,
        };
        Some(match (self.setting, icons) {
            (Animations::Full, _) => {
                let turns = usize::try_from(waited.as_millis() / SPIN.as_millis()).unwrap_or(0);
                frames
                    .get(turns % frames.len())
                    .copied()
                    .unwrap_or_default()
            }
            (Animations::Reduced | Animations::Off, IconSet::Ascii) => "...",
            (Animations::Reduced | Animations::Off, IconSet::Nerd | IconSet::Unicode) => "…",
        })
    }

    /// How long the user has waited past [`PATIENCE`], if they have.
    fn waited(&self) -> Option<Duration> {
        if self.waits == 0 {
            return None;
        }
        self.now
            .checked_sub(self.waiting_since)?
            .checked_sub(PATIENCE)
    }

    /// When the screen next changes by itself: the next frame while an effect runs or waits to
    /// start, or the next turn of the spinner; `None` when nothing moves (N-PERF-03).
    pub(super) fn deadline(&self) -> Option<Duration> {
        let frame = self
            .animating()
            .then(|| self.drawn_at.saturating_add(FRAME));
        let spinner = (self.waits > 0).then(|| match self.waited() {
            None => Some(self.waiting_since.saturating_add(PATIENCE)),
            Some(waited) if self.setting == Animations::Full => {
                let turns = waited.as_millis() / SPIN.as_millis() + 1;
                let next = SPIN.saturating_mul(u32::try_from(turns).unwrap_or(u32::MAX));
                Some(self.waiting_since + PATIENCE + next)
            }
            Some(_) => None,
        });
        frame.into_iter().chain(spinner.flatten()).min()
    }
}

/// Whether `message` brings blocks in or moves them where the user may not be looking: pasting,
/// duplicating, Move to, undo, redo or a reload (UI-ANIM-02).
pub(super) fn brings_blocks(message: &Message) -> bool {
    matches!(
        message,
        Message::Pasted { .. }
            | Message::Duplicate
            | Message::Menu(Menu::MoveTo, _)
            | Message::Undo
            | Message::Redo
            | Message::FileChecked { .. }
            | Message::ConflictCopied { .. }
    )
}

/// Whether `color` is an RGB color, which fades can pass through (ui.md §9.1).
fn rgb(color: Color) -> bool {
    matches!(color, Color::Rgb(..))
}

/// A flash over `lines` in `color` for `duration`: a faint tint of each cell's background that
/// fades out with an ease-out curve when `fades`, else `color` itself, or the tint when both are
/// RGB, for the whole duration, reverting in one step; the terminal's own background stands for
/// `background` (ui.md §9.1).
fn flash(
    lines: Vec<Rect>,
    color: Color,
    background: Color,
    duration: Duration,
    fades: bool,
) -> Effect {
    let timer = EffectTimer::new(duration, Interpolation::QuadOut);
    fx::effect_fn(lines, timer, move |lines, context, cells| {
        let strength = if fades {
            TINT * (1.0 - context.alpha())
        } else {
            TINT
        };
        for (position, cell) in cells {
            if !lines.iter().any(|line| line.contains(position)) {
                continue;
            }
            let base = if cell.bg == Color::Reset {
                background
            } else {
                cell.bg
            };
            let tinted = if rgb(base) && rgb(color) {
                ColorSpace::Rgb.lerp(&base, &color, strength)
            } else {
                color
            };
            cell.set_bg(tinted);
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sessions_over_ssh_default_to_reduced() {
        let set = Some(OsStr::new("10.0.0.1 22 10.0.0.2 22"));
        assert_eq!(default_for(None, None), Animations::Full);
        assert_eq!(default_for(set, None), Animations::Reduced);
        assert_eq!(
            default_for(None, Some(OsStr::new("/dev/pts/1"))),
            Animations::Reduced
        );
    }

    #[test]
    fn the_debug_form_shows_the_setting_and_the_waits() {
        let debug = format!("{:?}", Animator::new(Animations::Reduced));
        assert_eq!(
            debug,
            "Animator { setting: Reduced, running: [], waits: 0, .. }"
        );
    }

    #[test]
    fn checks_and_backups_are_not_waited_on_since_they_may_never_report() {
        let mut animator = Animator::new(Animations::Full);
        let path = std::path::PathBuf::from("/outline.taskpaper");
        animator.submitted(&Disk::Check { path: path.clone() }, Duration::ZERO);
        animator.submitted(&Disk::Backup { path }, Duration::ZERO);
        animator.at(Duration::from_secs(1));
        assert_eq!(animator.spinner(IconSet::Unicode), None);
        assert_eq!(animator.deadline(), None);
    }

    /// The backgrounds of the first line of a 2×2 buffer, after `flash` over that line has run
    /// for `elapsed`: the left cell in the terminal's own background, the right one in blue.
    fn flashed(color: Color, background: Color, fades: bool, elapsed: Duration) -> [Color; 3] {
        let area = Rect::new(0, 0, 2, 2);
        let mut buf = Buffer::empty(area);
        buf[(1, 0)].set_bg(Color::Rgb(0, 0, 200));
        let line = vec![Rect::new(0, 0, 2, 1)];
        let mut effect = flash(line, color, background, FLASH, fades);
        effect.process(elapsed, &mut buf, area);
        [buf[(0, 0)].bg, buf[(1, 0)].bg, buf[(0, 1)].bg]
    }

    // UI-ANIM-01, ui.md §9.1
    #[test]
    fn a_flash_tints_its_lines_toward_its_color_and_fades_out_with_an_ease_out() {
        let grey = Color::Rgb(100, 100, 100);
        let black = Color::Rgb(0, 0, 0);
        assert_eq!(
            flashed(grey, black, true, Duration::ZERO),
            [
                Color::Rgb(29, 29, 29),
                Color::Rgb(29, 29, 170),
                Color::Reset
            ],
            "30% of the way, only over its lines"
        );
        assert_eq!(
            flashed(grey, black, true, Duration::from_millis(125)),
            [Color::Rgb(7, 7, 7), Color::Rgb(7, 7, 192), Color::Reset],
            "a quarter of that half way through"
        );
        assert_eq!(
            flashed(grey, black, false, Duration::from_millis(125)),
            [
                Color::Rgb(29, 29, 29),
                Color::Rgb(29, 29, 170),
                Color::Reset
            ],
            "in one step"
        );
        assert_eq!(
            flashed(Color::Blue, black, false, Duration::ZERO),
            [Color::Blue, Color::Blue, Color::Reset],
            "an ANSI color is shown as it is"
        );
        assert_eq!(
            flashed(grey, Color::Reset, false, Duration::ZERO),
            [grey, Color::Rgb(29, 29, 170), Color::Reset],
            "and so is any color over the terminal's own background"
        );
    }
}
