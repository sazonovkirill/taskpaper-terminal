//! Animation: the effects drawn over a rendered frame to show where focus landed, what a command
//! changed and that an intent was refused, and to ease what changes in place, the spinner of a
//! wait, and how much the `animations` setting lets them do
//! ([ui.md §9](../../../docs/specs/ui.md#9-animation-fr-ui-11)). Hides tachyonfx, the triggers and
//! every duration.

use std::{
    collections::{HashMap, HashSet},
    ffi::OsStr,
    mem,
    time::Duration,
};

use ratatui::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::{Color, Modifier},
};
use tachyonfx::{ColorSpace, Effect, EffectManager, EffectTimer, Interpolation, fx};

use super::{
    layout::Scroll,
    theme::{ColorSupport, IconSet, Theme},
};
use crate::{
    app::{
        Animations, Disk, Location, Menu, Message, Mode, Notice, PaneId, Row, Rows, SaveAs, State,
    },
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

/// How long a block's text takes to ease into or out of the style of a done block (UI-ANIM-04).
const EASE: Duration = Duration::from_millis(200);

/// How far toward the background dimmed text stands while it eases, as most terminals draw it.
const DIM_SHADE: f32 = 0.5;

/// How long a pane body takes to fade in when its root changes, and rows when a block is
/// expanded (UI-ANIM-05, UI-ANIM-06).
const FADE_IN: Duration = Duration::from_millis(120);

/// How long a status message takes to fade out once its time is up (UI-ANIM-07).
const FADE_OUT: Duration = Duration::from_millis(300);

/// How long the crossfade to a previewed theme lasts (UI-ANIM-09).
const CROSSFADE: Duration = Duration::from_millis(150);

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
    /// Text easing into or out of the style of a done block (UI-ANIM-04).
    Ease,
    /// The body of a pane whose root changed fading in (UI-ANIM-05).
    Root,
    /// The last crumb of a pane whose root changed (UI-ANIM-05).
    Crumb,
    /// The rows an expanded block shows fading in (UI-ANIM-06).
    Reveal,
}

/// The kinds, each of which can run in every pane.
const KINDS: [Kind; 7] = [
    Kind::Landing,
    Kind::Change,
    Kind::Refusal,
    Kind::Ease,
    Kind::Root,
    Kind::Crumb,
    Kind::Reveal,
];

/// What an effect is keyed by: its pane, always there, as `EffectManager` asks for keys with a
/// default, and its kind.
type Key = (Option<PaneId>, Kind);

/// What the panes and the status line showed before an input, which tells after it whether
/// focus jumped, what changed in place and what appeared or went.
#[derive(Debug)]
pub(super) struct Before {
    pane: Option<PaneId>,
    location: Location,
    focus: Option<ItemId>,
    rows: Rows,
    /// Each pane, what it shows and its rows.
    panes: Vec<(PaneId, Location, Rows)>,
    notice: Option<Notice>,
}

impl Before {
    /// What `state`'s panes and status line show now.
    pub(super) fn of(state: &State) -> Self {
        let panes = state.panes();
        Self {
            pane: panes.iter().find(|pane| pane.focused).map(|pane| pane.id),
            location: state.location(),
            focus: state.focus(),
            rows: state.rows(),
            panes: panes
                .into_iter()
                .map(|pane| (pane.id, pane.location, pane.rows))
                .collect(),
            notice: state.notice().cloned(),
        }
    }

    /// The blocks of the pane with the keys at the same place in its rows here and in `after`'s,
    /// whose text recedes in one and not the other, as a done block and those inside it do
    /// (UI-ANIM-04). Completing keeps every row in place, or drops the block with those inside
    /// it when completed blocks are hidden, which has no effect to show.
    fn receded_in(&self, after: &Self) -> Vec<ItemId> {
        let recedes = |row: &Row| row.done || row.inside_done;
        // A row in a chunk both share is the same in both, so only the others are compared.
        after
            .rows
            .apart_from(&self.rows)
            .filter_map(|(at, now)| Some((self.rows.get(at)?, now)))
            .filter(|(was, now)| was.id == now.id && recedes(was) != recedes(now))
            .map(|(_, now)| now.id)
            .collect()
    }

    /// The panes `after` shows that were not here or show another place now (UI-ANIM-05).
    fn rooted_in(&self, after: &Self) -> Vec<PaneId> {
        after
            .panes
            .iter()
            .filter(|&&(id, location, _)| {
                !self
                    .panes
                    .iter()
                    .any(|&(was, before, _)| was == id && before == location)
            })
            .map(|&(id, ..)| id)
            .collect()
    }

    /// The rows of the pane with the keys in `after` that it did not show here in the same
    /// place, as after an expand, which may give the keys to the pane it expands in, or `None`
    /// when it shows the same rows (UI-ANIM-06).
    fn revealed_in(&self, after: &Self) -> Option<Revealed> {
        let before = self
            .panes
            .iter()
            .find(|&&(id, location, _)| Some(id) == after.pane && location == after.location)
            .map(|(.., rows)| rows.clone())
            .unwrap_or_default();
        after
            .rows
            .apart_from(&before)
            .next()
            .is_some()
            .then(|| Revealed {
                before,
                after: after.rows.clone(),
            })
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

/// The rows of a pane after an expand that it did not show before, worked out only for the rows
/// a frame shows, since an expand may show most of a large outline (UI-ANIM-06, N-PERF-02).
#[derive(Debug)]
struct Revealed {
    before: Rows,
    after: Rows,
}

impl Revealed {
    /// The rows revealed among those `panes` show.
    fn among(&self, panes: &[DrawnPane]) -> HashSet<ItemId> {
        let mut shown: Vec<ItemId> = panes
            .iter()
            .flat_map(|pane| pane.rows.iter().map(|&(id, _)| id))
            .collect();
        shown.sort_unstable();
        // Rows built from one another share the chunks an expand left alone, which hold no row
        // it revealed, so those are not gone through; of the rest, only the few shown are kept
        // to look for before.
        let mut changed: Vec<ItemId> = self
            .after
            .apart_from(&self.before)
            .map(|(_, row)| row.id)
            .filter(|id| shown.binary_search(id).is_ok())
            .collect();
        changed.sort_unstable();
        let before: Vec<ItemId> = self
            .before
            .apart_from(&self.after)
            .map(|(_, row)| row.id)
            .filter(|id| changed.binary_search(id).is_ok())
            .collect();
        changed
            .into_iter()
            .filter(|id| !before.contains(id))
            .collect()
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
    /// Where its last crumb, or the name of its special view, is on screen.
    pub crumb: Rect,
}

/// What the latest inputs asked to show once the next frame places it.
#[derive(Debug, Default)]
struct Pending {
    landing: bool,
    refused: bool,
    changed: Vec<ItemId>,
    /// The blocks whose text eases into or out of the done style.
    receded: Vec<ItemId>,
    /// The panes whose root changed.
    rooted: Vec<PaneId>,
    /// The rows an expanded block shows.
    revealed: Option<Revealed>,
}

impl Pending {
    fn is_empty(&self) -> bool {
        !self.landing
            && !self.refused
            && self.changed.is_empty()
            && self.receded.is_empty()
            && self.rooted.is_empty()
            && self.revealed.is_none()
    }
}

/// What a message may do that the effects show, told before the core takes it.
#[derive(Debug, Clone, Copy)]
pub(super) struct Intent {
    /// It brings blocks in or moves them where the user may not be looking: pasting,
    /// duplicating, Move to, undo, redo or a reload (UI-ANIM-02).
    brings: bool,
    /// It expands blocks (UI-ANIM-06).
    expands: bool,
}

impl Intent {
    /// What `message` may do.
    pub(super) fn of(message: &Message) -> Self {
        Self {
            brings: matches!(
                message,
                Message::Pasted { .. }
                    | Message::Duplicate
                    | Message::Menu(Menu::MoveTo, _)
                    | Message::Undo
                    | Message::Redo
                    | Message::FileChecked { .. }
                    | Message::ConflictCopied { .. }
            ),
            expands: matches!(
                message,
                Message::Expand
                    | Message::ToggleFold
                    | Message::ToggleSubtree
                    | Message::ExpandLevel
                    | Message::ExpandAll
            ),
        }
    }
}

/// A status message fading out, kept to be drawn while it fades (UI-ANIM-07).
struct Fading {
    notice: Notice,
    /// The fade, once a frame has drawn the message.
    effect: Option<Effect>,
}

/// The effects over the panes and the spinner, as the `animations` setting allows them.
pub(super) struct Animator {
    setting: Animations,
    manager: EffectManager<Key>,
    /// What each pane with effects showed when they started.
    running: HashMap<PaneId, Shown>,
    /// What the input being handled asked to show.
    fresh: Pending,
    /// Whether the input being handled expanded blocks.
    expanded: bool,
    /// What the latest input asked to show, which the next frame starts.
    pending: Pending,
    /// The panes the frame being drawn put on screen.
    frame: Vec<DrawnPane>,
    /// The panes of the latest frame as its effects drew them, which the text of a block
    /// completed or uncompleted eases from; kept only while colors fade (UI-ANIM-04).
    last: Buffer,
    /// The status message fading out.
    fading: Option<Fading>,
    /// The crossfade to a previewed theme.
    crossfade: Option<Effect>,
    /// The latest frame while a theme is previewed, in its theme, for a crossfade from it.
    previewed: Option<(Theme, Buffer)>,
    /// The time, since the frontend started.
    now: Duration,
    /// When the latest frame was drawn.
    drawn_at: Duration,
    /// How much time the frame being drawn moves effects on.
    elapsed: Duration,
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
            expanded: false,
            pending: Pending::default(),
            frame: Vec::new(),
            last: Buffer::empty(Rect::ZERO),
            fading: None,
            crossfade: None,
            previewed: None,
            now: Duration::ZERO,
            drawn_at: Duration::ZERO,
            elapsed: Duration::ZERO,
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
        if setting != Animations::Full {
            self.fading = None;
            self.crossfade = None;
            self.previewed = None;
        }
    }

    /// Lets time pass to `now`, since the frontend started.
    pub(super) fn at(&mut self, now: Duration) {
        self.now = self.now.max(now);
    }

    /// Takes in what the core reports after it handled a message that may do what `intent`
    /// says: the blocks it changed, which are highlighted when the command changed blocks other
    /// than just the focused one, or brought blocks in from the clipboard, a duplicate, Move to,
    /// the history or the disk; whether it refused the intent; and whether it expanded blocks
    /// (UI-ANIM-02, UI-ANIM-03, UI-ANIM-06).
    pub(super) fn observe(&mut self, intent: Intent, state: &State) {
        self.fresh.refused |= state.refused();
        self.expanded |= intent.expands;
        let changed = state.changed();
        let focus = state.focus();
        let elsewhere = changed.iter().any(|&id| Some(id) != focus);
        if elsewhere || (intent.brings && state.mode() == Mode::Navigate) {
            self.fresh.changed.extend_from_slice(changed);
        }
    }

    /// Ends the input that started at `before`, whose effects the next frame starts: a landing
    /// flash when focus jumped (UI-ANIM-01), and with every animation the ease of blocks
    /// completed or uncompleted, the fade-in of panes whose root changed and of rows an expand
    /// showed, and the fade-out of a status message whose time is up (UI-ANIM-04–07). The event
    /// loop draws a frame after every input, so what an earlier input asked for has started or
    /// been left behind by then.
    pub(super) fn settle(&mut self, before: &Before, state: &State) {
        let mut fresh = mem::take(&mut self.fresh);
        let expanded = mem::take(&mut self.expanded);
        if self.setting == Animations::Off {
            return;
        }
        let after = Before::of(state);
        fresh.landing = before.jumped_to(&after);
        if self.setting == Animations::Full {
            fresh.receded = before.receded_in(&after);
            fresh.rooted = before.rooted_in(&after);
            if expanded {
                fresh.revealed = before.revealed_in(&after);
            }
            if after.notice.is_some() {
                self.fading = None;
            } else if let Some(notice) = before.notice.clone() {
                self.fading = Some(Fading {
                    notice,
                    effect: None,
                });
            }
        }
        self.pending = fresh;
    }

    /// Whether an effect is running or waits to start, which asks for frames.
    fn animating(&self) -> bool {
        self.manager.is_running()
            || !self.pending.is_empty()
            || self.fading.is_some()
            || self.crossfade.is_some()
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
        self.elapsed = elapsed;
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
        if let Some(effect) = self
            .fading
            .as_mut()
            .and_then(|fading| fading.effect.as_mut())
        {
            effect.process(elapsed, &mut Buffer::empty(Rect::ZERO), Rect::ZERO);
            if effect.done() {
                self.fading = None;
            }
        }
        if colors != ColorSupport::None && self.setting != Animations::Off {
            let revealed = pending
                .revealed
                .as_ref()
                .map(|revealed| revealed.among(&panes))
                .unwrap_or_default();
            for pane in &panes {
                self.start(pane, &pending, &revealed, theme);
            }
        }
        // Drops effects that new ones replaced before they are drawn.
        self.manager
            .process_effects(Duration::ZERO, &mut Buffer::empty(Rect::ZERO), Rect::ZERO);
        let area = buf.area;
        self.manager.process_effects(Duration::ZERO, buf, area);
        if self.fades(theme) {
            self.last.area = area;
            self.last.content.clone_from(&buf.content);
        } else {
            self.last = Buffer::empty(Rect::ZERO);
        }
    }

    /// Starts over `pane` the effects `pending` asks for there, `revealed` being the rows it
    /// reveals among those the frame shows.
    fn start(
        &mut self,
        pane: &DrawnPane,
        pending: &Pending,
        revealed: &HashSet<ItemId>,
        theme: &Theme,
    ) {
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
        let mut started = Vec::new();
        for (kind, asked, color, duration, wanted) in effects {
            let lines = lines(wanted);
            if !asked || lines.is_empty() {
                continue;
            }
            let fades = self.setting == Animations::Full && rgb(color) && rgb(theme.background);
            started.push((kind, flash(lines, color, theme.background, duration, fades)));
        }
        if self.fades(theme) {
            started.extend(fades_over(pane, pending, revealed, theme, &self.last));
        }
        for (kind, effect) in started {
            self.manager
                .add_unique_effect((Some(pane.id), kind), effect);
            self.running.insert(pane.id, pane.shown.clone());
        }
    }

    /// Whether colors fade in `theme`: only with every animation, between RGB colors (ui.md
    /// §9.1).
    fn fades(&self, theme: &Theme) -> bool {
        self.setting == Animations::Full && rgb(theme.text) && rgb(theme.background)
    }

    /// The status message fading out, which the status line draws in `theme` until it has faded
    /// (UI-ANIM-07); a message vanishes at once when colors do not fade.
    pub(super) fn fading_notice(&self, theme: &Theme) -> Option<&Notice> {
        self.fading
            .as_ref()
            .filter(|_| self.fades(theme))
            .map(|fading| &fading.notice)
    }

    /// Fades out on `buf` the status message fading out, which the frame being drawn put at
    /// `area` in `theme`'s colors; one it did not draw is gone (UI-ANIM-07).
    pub(super) fn fade_status(&mut self, buf: &mut Buffer, area: Rect, theme: &Theme) {
        if area.is_empty() {
            self.fading = None;
        } else if let Some(fading) = &mut self.fading {
            fading
                .effect
                .get_or_insert_with(|| fade_out(theme.text, theme.background))
                .process(Duration::ZERO, buf, area);
        }
    }

    /// Ends the frame drawn on `buf` in `theme`'s colors: crossfades it from the frame before
    /// when that one previewed another theme and both themes fade, and keeps it for a crossfade
    /// from it while `previewing` says a theme is previewed and its colors fade (UI-ANIM-09).
    pub(super) fn finish(&mut self, buf: &mut Buffer, theme: Theme, previewing: bool) {
        let mut elapsed = self.elapsed;
        if let Some((before, frame)) = self.previewed.take()
            && before != theme
            && frame.area == buf.area
            && self.fades(&theme)
        {
            self.crossfade = Some(crossfade(frame));
            elapsed = Duration::ZERO;
        }
        if let Some(effect) = &mut self.crossfade {
            effect.process(elapsed, buf, buf.area);
            if effect.done() {
                self.crossfade = None;
            }
        }
        self.previewed = (previewing && self.fades(&theme)).then(|| (theme, buf.clone()));
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

/// The fades `pending` asks for over `pane` in `theme`'s colors: the text of blocks completed
/// or uncompleted easing from how the frame `last` drew it, the body of a pane whose root changed
/// fading in from the top down with a flash of its last crumb, and the rows an expand showed,
/// `revealed`, fading in (UI-ANIM-04–06).
fn fades_over(
    pane: &DrawnPane,
    pending: &Pending,
    revealed: &HashSet<ItemId>,
    theme: &Theme,
    last: &Buffer,
) -> Vec<(Kind, Effect)> {
    let mut fades = Vec::new();
    let eased: Vec<Rect> = pane
        .rows
        .iter()
        .filter(|(id, _)| pending.receded.contains(id))
        .map(|&(_, lines)| lines)
        .collect();
    if !eased.is_empty() {
        fades.push((Kind::Ease, ease(&eased, last, theme.text, theme.background)));
    }
    if pending.rooted.contains(&pane.id) {
        let body = vec![pane.shown.area];
        fades.push((
            Kind::Root,
            fade_in(body, true, theme.text, theme.background),
        ));
        let crumb = vec![pane.crumb];
        fades.push((
            Kind::Crumb,
            flash(crumb, theme.accent, theme.background, FLASH, true),
        ));
    }
    let revealed: Vec<Rect> = pane
        .rows
        .iter()
        .filter(|(id, _)| revealed.contains(id))
        .map(|&(_, lines)| lines)
        .collect();
    if !revealed.is_empty() {
        fades.push((
            Kind::Reveal,
            fade_in(revealed, false, theme.text, theme.background),
        ));
    }
    fades
}

/// Whether `color` is an RGB color, which fades can pass through (ui.md §9.1).
fn rgb(color: Color) -> bool {
    matches!(color, Color::Rgb(..))
}

/// `color`, or `default` for the terminal's own color, which the theme names.
fn or_default(color: Color, default: Color) -> Color {
    if color == Color::Reset {
        default
    } else {
        color
    }
}

/// Text over `lines` easing for [`EASE`] from the color and dimming `last` drew it in to those
/// it has, in each cell where they differ, so that what keeps its look, such as the focus
/// marker and what is dimmed either way, is left as it is: dimmed text stands [`DIM_SHADE`] of
/// the way toward its background until the terminal's own dim takes over at the end; `text` and
/// `background` stand for the terminal's own colors (UI-ANIM-04).
fn ease(lines: &[Rect], last: &Buffer, text: Color, background: Color) -> Effect {
    let was: HashMap<Position, (Color, bool)> = lines
        .iter()
        .flat_map(|line| line.positions())
        .filter_map(|position| {
            let cell = last.cell(position)?;
            let dim = cell.modifier.contains(Modifier::DIM);
            Some((position, (or_default(cell.fg, text), dim)))
        })
        .collect();
    let timer = EffectTimer::new(EASE, Interpolation::QuadOut);
    fx::effect_fn(was, timer, move |was, context, cells| {
        let alpha = context.alpha();
        for (position, cell) in cells {
            let Some(&(was_fg, was_dim)) = was.get(&position) else {
                continue;
            };
            let (fg, bg) = (or_default(cell.fg, text), or_default(cell.bg, background));
            let dim = cell.modifier.contains(Modifier::DIM);
            if (was_fg, was_dim) == (fg, dim) || !(rgb(was_fg) && rgb(fg) && rgb(bg)) {
                continue;
            }
            let shown = |color: Color, dim: bool| {
                if dim {
                    ColorSpace::Rgb.lerp(&color, &bg, DIM_SHADE)
                } else {
                    color
                }
            };
            cell.modifier.remove(Modifier::DIM);
            cell.fg = ColorSpace::Rgb.lerp(&shown(was_fg, was_dim), &shown(fg, dim), alpha);
        }
    })
}

/// `lines` fading in from `background` for [`FADE_IN`], all together or, when `sweeps`, from
/// the top line down; `text` stands for the terminal's own text color (UI-ANIM-05, UI-ANIM-06).
fn fade_in(lines: Vec<Rect>, sweeps: bool, text: Color, background: Color) -> Effect {
    let top = lines
        .iter()
        .map(|line| line.top())
        .min()
        .unwrap_or_default();
    let bottom = lines
        .iter()
        .map(|line| line.bottom())
        .max()
        .unwrap_or_default();
    let last = f32::from(bottom.saturating_sub(top).saturating_sub(1).max(1));
    let timer = EffectTimer::new(FADE_IN, Interpolation::QuadOut);
    fx::effect_fn(lines, timer, move |lines, context, cells| {
        let alpha = context.alpha();
        for (position, cell) in cells {
            if !lines.iter().any(|line| line.contains(position)) {
                continue;
            }
            // Each line starts as the one above it is part way in, the last one half way through.
            let shown = if sweeps {
                let row = f32::from(position.y.saturating_sub(top)) / last;
                (alpha * 2.0 - row).clamp(0.0, 1.0)
            } else {
                alpha
            };
            let (fg, bg) = (or_default(cell.fg, text), or_default(cell.bg, background));
            // A blank's text color shows nowhere, so it is left as it is, not sent each frame.
            if rgb(fg) && cell.symbol() != " " {
                cell.fg = ColorSpace::Rgb.lerp(&background, &fg, shown);
            }
            if rgb(bg) {
                cell.bg = ColorSpace::Rgb.lerp(&background, &bg, shown);
            }
        }
    })
}

/// The text of the area it is drawn over fading out into its background for [`FADE_OUT`];
/// `text` and `background` stand for the terminal's own colors (UI-ANIM-07).
fn fade_out(text: Color, background: Color) -> Effect {
    let timer = EffectTimer::new(FADE_OUT, Interpolation::QuadOut);
    fx::effect_fn((), timer, move |(), context, cells| {
        let alpha = context.alpha();
        for (_, cell) in cells {
            let (fg, bg) = (or_default(cell.fg, text), or_default(cell.bg, background));
            if rgb(fg) && rgb(bg) {
                cell.fg = ColorSpace::Rgb.lerp(&fg, &bg, alpha);
            }
        }
    })
}

/// Every cell's colors passing for [`CROSSFADE`] from those `before` has in its place to its
/// own, where both are RGB (UI-ANIM-09).
fn crossfade(before: Buffer) -> Effect {
    let timer = EffectTimer::new(CROSSFADE, Interpolation::QuadOut);
    fx::effect_fn(before, timer, |before, context, cells| {
        let alpha = context.alpha();
        // The frame before covers the same area, so it has every cell.
        let cells =
            cells.filter_map(|(position, cell)| before.cell(position).map(|was| (was, cell)));
        for (was, cell) in cells {
            if rgb(was.fg) && rgb(cell.fg) {
                cell.fg = ColorSpace::Rgb.lerp(&was.fg, &cell.fg, alpha);
            }
            if rgb(was.bg) && rgb(cell.bg) {
                cell.bg = ColorSpace::Rgb.lerp(&was.bg, &cell.bg, alpha);
            }
        }
    })
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
    use crate::ui::theme::BuiltIn;
    use ratatui::style::Style;

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

    /// A buffer one line tall holding `cells`, each an `x` in its colors with its modifier.
    fn line_of(cells: &[(Color, Color, Modifier)]) -> Buffer {
        let width = u16::try_from(cells.len()).unwrap();
        let mut buf = Buffer::empty(Rect::new(0, 0, width, 1));
        for (x, &(fg, bg, modifier)) in (0..).zip(cells) {
            buf[(x, 0)]
                .set_symbol("x")
                .set_style(Style::new().fg(fg).bg(bg).add_modifier(modifier));
        }
        buf
    }

    /// The colors and modifier of each cell of `buf`'s first line after `effect` has run over it
    /// for `millis`.
    fn after(mut effect: Effect, mut buf: Buffer, millis: u64) -> Vec<(Color, Color, Modifier)> {
        let area = buf.area;
        effect.process(Duration::from_millis(millis), &mut buf, area);
        (0..area.width)
            .map(|x| {
                let cell = &buf[(x, 0)];
                (cell.fg, cell.bg, cell.modifier)
            })
            .collect()
    }

    const BLACK: Color = Color::Rgb(0, 0, 0);

    fn grey(level: u8) -> Color {
        Color::Rgb(level, level, level)
    }

    // UI-ANIM-04
    #[test]
    fn the_ease_moves_what_changed_from_how_it_was_drawn_dimmed_rgb_text_up_to_half_way() {
        let accent = Color::Rgb(0, 0, 200);
        let was = [
            (grey(200), BLACK, Modifier::empty()),
            (Color::Blue, BLACK, Modifier::empty()),
            (Color::Reset, Color::Reset, Modifier::empty()),
            (grey(160), BLACK, Modifier::empty()),
            (grey(200), BLACK, Modifier::DIM),
            (accent, BLACK, Modifier::empty()),
        ];
        let now = [
            (grey(200), BLACK, Modifier::DIM),
            (Color::Blue, BLACK, Modifier::DIM),
            (Color::Reset, Color::Reset, Modifier::DIM),
            (grey(200), BLACK, Modifier::DIM),
            (grey(200), BLACK, Modifier::DIM),
            (accent, BLACK, Modifier::empty()),
        ];
        let line = [Rect::new(0, 0, 6, 1)];
        let eased = |was: &[_], now: &[_]| {
            after(
                ease(&line, &line_of(was), grey(200), BLACK),
                line_of(now),
                100,
            )
        };
        assert_eq!(
            eased(&was, &now),
            [
                (grey(125), BLACK, Modifier::empty()),
                (Color::Blue, BLACK, Modifier::DIM),
                (grey(125), Color::Reset, Modifier::empty()),
                (grey(115), BLACK, Modifier::empty()),
                (grey(200), BLACK, Modifier::DIM),
                (accent, BLACK, Modifier::empty()),
            ],
            "three quarters of the way to a half shade, from the color drawn before, an ANSI \
             color, text dimmed either way and the focus marker left as they are"
        );
        assert_eq!(
            eased(&now[..1], &was[..1])[0],
            (grey(175), BLACK, Modifier::empty()),
            "and back from it"
        );
        assert_eq!(
            after(
                ease(&line, &Buffer::empty(Rect::ZERO), grey(200), BLACK),
                line_of(&now),
                100
            ),
            now,
            "with no frame before, nothing eases"
        );
    }

    // UI-ANIM-05, UI-ANIM-06
    #[test]
    fn a_fade_in_brings_text_and_background_in_from_the_background_from_the_top_down() {
        let column = |sweeps: bool| {
            let mut buf = Buffer::empty(Rect::new(0, 0, 2, 3));
            for y in 0..3 {
                buf[(0, y)]
                    .set_symbol("x")
                    .set_fg(grey(200))
                    .set_bg(grey(100));
                buf[(1, y)].set_fg(grey(200));
            }
            let mut effect = fade_in(vec![buf.area], sweeps, grey(200), BLACK);
            let area = buf.area;
            effect.process(Duration::from_millis(30), &mut buf, area);
            (0..3)
                .map(|y| (buf[(0, y)].fg, buf[(0, y)].bg, buf[(1, y)].fg))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            column(true),
            [
                (grey(175), grey(87), grey(200)),
                (grey(75), grey(37), grey(200)),
                (BLACK, BLACK, grey(200)),
            ],
            "the top line furthest in, a blank's text color untouched"
        );
        assert_eq!(column(false), [(grey(87), grey(43), grey(200)); 3]);
    }

    // UI-ANIM-07
    #[test]
    fn a_fade_out_moves_rgb_text_to_its_background() {
        let cells = [
            (grey(200), BLACK, Modifier::empty()),
            (Color::Blue, BLACK, Modifier::empty()),
        ];
        assert_eq!(
            after(fade_out(grey(200), BLACK), line_of(&cells), 150),
            [
                (grey(50), BLACK, Modifier::empty()),
                (Color::Blue, BLACK, Modifier::empty())
            ]
        );
    }

    // UI-ANIM-09
    #[test]
    fn a_crossfade_passes_from_the_colors_before_where_both_are_rgb() {
        let before = line_of(&[
            (BLACK, BLACK, Modifier::empty()),
            (Color::Blue, BLACK, Modifier::empty()),
        ]);
        let now = line_of(&[
            (grey(200), grey(100), Modifier::empty()),
            (grey(200), Color::Blue, Modifier::empty()),
        ]);
        assert_eq!(
            after(crossfade(before), now, 75),
            [
                (grey(150), grey(75), Modifier::empty()),
                (grey(200), Color::Blue, Modifier::empty())
            ]
        );
    }

    // ui.md §9.3
    #[test]
    fn less_than_every_animation_ends_the_fades_of_the_status_line_and_the_theme() {
        let mut animator = Animator::new(Animations::Full);
        animator.fading = Some(Fading {
            notice: Notice::NoInbox,
            effect: None,
        });
        animator.crossfade = Some(crossfade(Buffer::empty(Rect::ZERO)));
        animator.previewed = Some((
            BuiltIn::DEFAULT.colors(ColorSupport::None),
            Buffer::empty(Rect::ZERO),
        ));
        animator.set(Animations::Reduced);
        assert!(animator.fading.is_none() && animator.crossfade.is_none());
        assert!(animator.previewed.is_none());
        assert_eq!(animator.deadline(), None);
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
