# Animation in terminal UIs

Research snapshot of 2026-10-04: what animation a terminal app can do, how other TUIs use it, and what it costs. Our decisions are in [../specs/ui.md §9](../specs/ui.md#9-animation-fr-ui-11).

## What a terminal can animate

A terminal redraws in whole cells, so animation is limited to what can change per cell between frames:

- **Color**: foreground and background can be interpolated smoothly only between RGB (truecolor) values. With the 16 ANSI colors, or a theme that uses them so it follows the user's palette, there are no in-between colors: a transition is a jump. (inferred)
- **Characters**: revealing, dissolving or replacing glyphs cell by cell, and partial-block characters (`▏▎▍▌▋▊▉█`) for sub-cell bars.
- **Position**: content can move only in whole rows and columns. Sliding a block of text by one cell per frame reads as jerky rather than smooth. (inferred)
- **Text attributes**: bold, dim, reverse and underline can be toggled, which gives a one-step flash in any terminal.
- **The cursor**: the hardware cursor's blink is the terminal's own setting, not the app's.

## Tooling

### tachyonfx

[tachyonfx](https://github.com/junkdog/tachyonfx) ([docs](https://docs.rs/tachyonfx/latest/tachyonfx/)) is the effects and animation library for Ratatui. Version 0.25.2 (2026-09-06) depends on `ratatui-core` 0.1, the core crate of Ratatui 0.30, so it fits our stack.

- Effects are shaders applied to the already-rendered buffer after widgets draw: they change the colors, symbols or visibility of cells, never the app's state or layout.
- About 50 effects: color (`fade_from`, `fade_to`, `paint`, `hsl_shift`, `saturate`, `lighten`, `darken`), text and motion (`coalesce`, `dissolve`, `evolve`, `slide_in/out`, `sweep_in/out`, `explode`, `expand`, `stretch`), geometry (`translate`) and composition (`parallel`, `sequence`, `repeat`, `delay`, `ping_pong`, `prolong_end`, `never_complete`).
- Each effect has a duration and an `Interpolation` (easing curve). A `CellFilter` limits it to cells of a region, color or text.
- An `EffectManager` holds running effects, keyed so that starting an effect replaces the previous one with the same key. Each frame the app advances effects by the elapsed `Duration`; output depends only on that elapsed time, so effects are deterministic and testable with a fixed clock.
- Feature flags include `std-duration`, `sendable` and `dsl`; it also runs in WebAssembly and `no_std`.

### Flicker-free output

- Textual's author recommends overwriting instead of clearing, sending each frame in one write, and **synchronized output**, and targets 60 fps as the useful ceiling ([Textual blog](https://www.textualize.io/blog/7-things-ive-learned-building-a-modern-tui-framework)). Ratatui's diffing backend already overwrites only changed cells. (inferred)
- **Synchronized output** (DEC private mode 2026: `CSI ? 2026 h` begins a frame, `CSI ? 2026 l` ends it) makes the terminal show a frame only when it is complete, which prevents tearing while many cells change. Support is queried with `CSI ? 2026 $ p`. Supported by Alacritty, foot, Ghostty, iTerm2, kitty, WezTerm, Windows Terminal, Zellij and others; not yet by VTE (GNOME Terminal); unknown for Konsole and urxvt ([spec and support table](https://github.com/contour-terminal/vt-extensions/blob/master/synchronized-output.md)). Terminals that do not recognize it ignore it. crossterm exposes it as `BeginSynchronizedUpdate` / `EndSynchronizedUpdate`. Support inside tmux depends on the tmux version (unverified).

## How other TUIs use animation

- **Textual** (Python): animates scrolling so the reader keeps their place, and in 2025 added smoother mouse scrolling using pixel-precise pointer reporting in kitty, Ghostty and a few others ([Textual blog, 2025-02-16](https://textual.textualize.io/blog/2025/02/16/smoother-scrolling-in-the-terminal-mdash-a-feature-decades-in-the-making/)). Its author separates animation that helps orientation from gratuitous animation and plans to let users turn off the decorative kind.
- **Crush** (Charm): added a `reduce_animations` setting that replaces animated spinners with static ones ([commit](https://git.secluded.site/crush-mirror/commit/ba02a4ba923080aa11e39e4a90bf486001a7d4b3)).
- **Claude Code**: users have asked for its `prefersReducedMotion` setting to be switchable at runtime ([issue](https://claudeissues.com/issue/57237-expose-prefersreducedmotion-as-a-runtime-toggle-slash-command-or-config-entry)), showing that reduced motion is an expected option in terminal apps.
- **Common practice** in git and file-manager TUIs (lazygit, gitui, yazi) and editors (Helix): spinners for background work, otherwise instant redraws; no transitions on navigation. Neovim users add smooth scrolling and cursor-trail plugins. (unverified)

## Costs and risks

- **Latency**: an animation that delays a state or layout change makes the app feel slower, however short it is. Animations that only decorate the already-final state cost nothing in responsiveness. (inferred)
- **CPU and bandwidth**: an animation needs frames, typically 60 per second; each frame over SSH sends the changed cells. Long or full-screen effects cost bandwidth and battery, so effects should be short and small. (inferred)
- **Readability**: fading or dissolving text is briefly unreadable; repeated on every keystroke it becomes noise. (inferred)
- **Color depth**: smooth fades need truecolor; 16-color themes need a stepped fallback. (inferred)
- **Accessibility**: some users need reduced motion; apps offer a setting to reduce or disable animation.

## Takeaways for taskpaper-terminal

1. Animate only to help orientation and confirm results: where did focus land, what did that command change, what failed. Never animate plain movement or typing.
2. Change state, layout and focus at the keystroke; effects only decorate the final frame and are cut short by the next input.
3. Keep effects to 100–250 ms with ease-out curves, and draw frames only while one runs (idle CPU stays near zero).
4. Use tachyonfx in the UI layer, synchronized output for every frame, and a one-step fallback without truecolor.
5. Offer full, reduced and off.
