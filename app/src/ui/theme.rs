//! The icons the UI draws, each next to its one fallback ([ui.md §7](../../../docs/specs/ui.md#7-icons-fr-ui-09-fr-ui-10)).
//! Hides which glyph stands for what, so the two sets cannot drift.

/// Something the UI marks with an icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    /// A start date chip.
    Start,
    /// A due date chip that is not overdue.
    Due,
    /// An overdue due date chip.
    Overdue,
    /// A block with `@today`, after its text.
    Today,
    /// A bookmarked block, after its text.
    Bookmark,
}

/// An icon's Nerd Font glyph and the Unicode fallback drawn when icons are off.
struct Glyphs {
    nerd: &'static str,
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "drawn once the icons setting of step 6.3 exists")
    )]
    unicode: &'static str,
}

impl Icon {
    /// The glyph drawn for the icon: Nerd Fonts by default (D-20).
    #[must_use]
    pub fn glyph(self) -> &'static str {
        self.glyphs().nerd
    }

    /// The table: each Nerd Fonts v3 code point beside its fallback, every one a single cell.
    const fn glyphs(self) -> Glyphs {
        let (nerd, unicode) = match self {
            // nf-md-calendar_start
            Self::Start => ("\u{f166d}", "↦"),
            // nf-md-calendar_end
            Self::Due => ("\u{f166c}", "⇥"),
            // nf-md-calendar_alert
            Self::Overdue => ("\u{f0a31}", "!"),
            // nf-md-star
            Self::Today => ("\u{f04ce}", "★"),
            // nf-md-bookmark
            Self::Bookmark => ("\u{f00c0}", "⚑"),
        };
        Glyphs { nerd, unicode }
    }
}

#[cfg(test)]
mod tests {
    use ratatui::text::Span;

    use super::*;

    const ICONS: [Icon; 5] = [
        Icon::Start,
        Icon::Due,
        Icon::Overdue,
        Icon::Today,
        Icon::Bookmark,
    ];

    #[test]
    fn every_icon_and_its_fallback_is_one_cell_wide() {
        for icon in ICONS {
            let Glyphs { nerd, unicode } = icon.glyphs();
            assert_eq!(Span::raw(nerd).width(), 1, "{icon:?}");
            assert_eq!(Span::raw(unicode).width(), 1, "{icon:?} fallback");
        }
    }

    #[test]
    fn nerd_glyphs_come_from_the_nerd_fonts_v3_material_design_range() {
        for icon in ICONS {
            let code = icon.glyph().chars().next().map(u32::from);
            assert!(
                code.is_some_and(|code| (0xF0001..=0xF1AF0).contains(&code)),
                "{icon:?}"
            );
        }
    }

    #[test]
    fn every_icon_has_its_own_glyph_and_fallback() {
        for (at, icon) in ICONS.iter().enumerate() {
            for other in &ICONS[at + 1..] {
                assert_ne!(icon.glyph(), other.glyph());
                assert_ne!(icon.glyphs().unicode, other.glyphs().unicode);
            }
        }
    }
}
