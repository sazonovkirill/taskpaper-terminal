//! The icons the UI draws, each next to its one fallback ([ui.md §7](../../../docs/specs/ui.md#7-icons-fr-ui-09-fr-ui-10)).
//! Hides which glyph stands for what, so the two sets cannot drift.

/// Something the UI marks with an icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    /// A start date chip, and the command menu's start date entries.
    Start,
    /// A due date chip that is not overdue, and the command menu's due date entries.
    Due,
    /// An overdue due date chip.
    Overdue,
    /// A block with `@today`, after its text, the command menu's Toggle Today and the sidebar's
    /// Today entry.
    Today,
    /// A bookmarked block, after its text, the command menu's bookmark entries and the sidebar's
    /// bookmarks.
    Bookmark,
    /// The command menu's entry that makes a block a project.
    Project,
    /// The command menu's entry that makes a block a task.
    Task,
    /// The command menu's entries that make a block plain text.
    Note,
    /// The command menu's entry that makes a block a section.
    Section,
    /// The command menu's entry that completes or uncompletes a block.
    Complete,
    /// The command menu's color entries.
    Color,
    /// The command menu's entry that shows a block's details.
    Details,
    /// The command menu's entry that opens a block's link.
    Link,
    /// The command menu's entry that duplicates a block.
    Duplicate,
    /// The command menu's entry that deletes a block.
    Delete,
    /// A command menu entry without an icon of its own.
    Command,
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
            // nf-md-folder_outline
            Self::Project => ("\u{f0256}", "▸"),
            // nf-md-checkbox_blank_outline
            Self::Task => ("\u{f0131}", "☐"),
            // nf-md-text
            Self::Note => ("\u{f09a8}", "¶"),
            // nf-md-format_section
            Self::Section => ("\u{f069f}", "§"),
            // nf-md-check_circle_outline
            Self::Complete => ("\u{f05e1}", "✓"),
            // nf-md-palette
            Self::Color => ("\u{f03d8}", "●"),
            // nf-md-information_outline
            Self::Details => ("\u{f02fd}", "i"),
            // nf-md-link
            Self::Link => ("\u{f0337}", "↗"),
            // nf-md-content_copy
            Self::Duplicate => ("\u{f018f}", "⧉"),
            // nf-md-delete_outline
            Self::Delete => ("\u{f09e7}", "✕"),
            // nf-md-chevron_right
            Self::Command => ("\u{f0142}", "›"),
        };
        Glyphs { nerd, unicode }
    }
}

#[cfg(test)]
mod tests {
    use ratatui::text::Span;

    use super::*;

    const ICONS: [Icon; 16] = [
        Icon::Start,
        Icon::Due,
        Icon::Overdue,
        Icon::Today,
        Icon::Bookmark,
        Icon::Project,
        Icon::Task,
        Icon::Note,
        Icon::Section,
        Icon::Complete,
        Icon::Color,
        Icon::Details,
        Icon::Link,
        Icon::Duplicate,
        Icon::Delete,
        Icon::Command,
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
