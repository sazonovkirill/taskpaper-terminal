//! A fuzzy finder over a list of candidates, such as Go to Symbol: the query typed into it, the
//! candidates it matches in ranked order, the highlighted one and what each key does to them
//! ([navigation.md §8](../../../docs/specs/navigation.md#8-go-to-symbol-and-go-to-command-fr-nav-05-fr-nav-06),
//! [keybindings.md §4](../../../docs/specs/keybindings.md#4-other-contexts)).

use std::{
    cmp::Reverse,
    collections::BTreeSet,
    fmt,
    ops::Deref,
    sync::{Arc, OnceLock},
};

use nucleo_matcher::{
    Config, Matcher, Utf32Str,
    pattern::{AtomKind, CaseMatching, Normalization, Pattern},
};

use super::{MenuInput, picker::Picked};

/// Something a finder can choose, as it lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate<T> {
    /// What choosing it gives.
    pub value: T,
    /// The text the query matches.
    pub text: Arc<str>,
    /// What a finder shows beside the text to tell candidates apart, such as a block's path.
    pub detail: Detail,
    /// Which group it ranks in: every candidate of a lower tier ranks above every candidate of a
    /// higher one.
    pub tier: u8,
}

/// What a finder shows beside a candidate's text: a text, or the path of the blocks above a
/// block, joined by ` › ` only once it is read, since a finder over a large outline shows few of
/// its candidates (N-PERF-02).
#[derive(Clone, Default)]
pub struct Detail {
    path: Arc<[Arc<str>]>,
    text: OnceLock<Arc<str>>,
}

impl Detail {
    /// The path of the blocks `path` names, from the top level down.
    pub(super) fn path(path: Arc<[Arc<str>]>) -> Self {
        Self {
            path,
            text: OnceLock::new(),
        }
    }
}

impl From<&str> for Detail {
    fn from(text: &str) -> Self {
        Self {
            path: Arc::default(),
            text: OnceLock::from(Arc::from(text)),
        }
    }
}

impl Deref for Detail {
    type Target = str;

    fn deref(&self) -> &str {
        self.text.get_or_init(|| self.path.join(" › ").into())
    }
}

impl fmt::Display for Detail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self)
    }
}

impl fmt::Debug for Detail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl PartialEq for Detail {
    fn eq(&self, other: &Self) -> bool {
        **self == **other
    }
}

impl Eq for Detail {}

/// An open finder as a frontend shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinderView<'a, T> {
    /// What was typed.
    pub query: &'a str,
    /// The candidates the query matches, best first.
    pub entries: Entries<'a, T>,
    /// The position of the highlighted entry in `entries`.
    pub selected: usize,
}

/// The candidates a query matches, best first, read one at a time, since a finder over a large
/// outline matches many more than show (N-PERF-02).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entries<'a, T> {
    candidates: &'a [Candidate<T>],
    /// The positions in `candidates` of those matched, best first.
    ranked: &'a [usize],
}

impl<'a, T> Entries<'a, T> {
    /// How many there are.
    #[must_use]
    pub fn len(&self) -> usize {
        self.ranked.len()
    }

    /// Whether there are none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ranked.is_empty()
    }

    /// The entry at `index`, if there is one.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&'a Candidate<T>> {
        self.candidates.get(*self.ranked.get(index)?)
    }

    /// Every entry in order.
    pub fn iter(&self) -> impl Iterator<Item = &'a Candidate<T>> + use<'a, T> {
        let candidates = self.candidates;
        self.ranked
            .iter()
            .filter_map(move |&index| candidates.get(index))
    }
}

#[cfg(test)]
impl<'a, T> Entries<'a, T> {
    /// The candidates at the positions `ranked` in `candidates`, in that order.
    pub(crate) fn new(candidates: &'a [Candidate<T>], ranked: &'a [usize]) -> Self {
        Self { candidates, ranked }
    }
}

#[cfg(test)]
impl<T> std::ops::Index<usize> for Entries<'_, T> {
    type Output = Candidate<T>;

    fn index(&self, index: usize) -> &Candidate<T> {
        self.get(index).expect("an entry at the index")
    }
}

/// An open finder: its candidates, the query, the matching candidates ranked and the highlighted
/// one.
#[derive(Debug, Clone)]
pub(super) struct Finder<T> {
    candidates: Vec<Candidate<T>>,
    query: String,
    /// The positions in `candidates` of those the query matches, best first.
    ranked: Vec<usize>,
    selected: usize,
}

impl<T: Clone> Finder<T> {
    /// A finder over `candidates` with an empty query, which matches them all, the first
    /// highlighted.
    pub(super) fn new(candidates: Vec<Candidate<T>>) -> Self {
        let mut finder = Self {
            candidates,
            query: String::new(),
            ranked: Vec::new(),
            selected: 0,
        };
        finder.rank();
        finder
    }

    /// The finder as a frontend shows it.
    pub(super) fn view(&self) -> FinderView<'_, T> {
        FinderView {
            query: &self.query,
            entries: Entries {
                candidates: &self.candidates,
                ranked: &self.ranked,
            },
            selected: self.selected,
        }
    }

    /// The value of the highlighted candidate, or `None` when the query matches none.
    pub(super) fn highlighted(&self) -> Option<&T> {
        self.ranked
            .get(self.selected)
            .and_then(|&index| self.candidates.get(index))
            .map(|candidate| &candidate.value)
    }

    /// Highlights the candidate whose value is `value`, when the query matches it.
    pub(super) fn highlight(&mut self, value: &T)
    where
        T: PartialEq,
    {
        if let Some(at) = self.ranked.iter().position(|&index| {
            self.candidates
                .get(index)
                .is_some_and(|candidate| candidate.value == *value)
        }) {
            self.selected = at;
        }
    }

    /// Ranks the candidates the query matches: by tier, then by how well their text matches,
    /// then in the order given. Each word of the query must match, fuzzily, ignoring case unless
    /// it has a capital (navigation.md §8); characters such as `!` or `^` match themselves.
    fn rank(&mut self) {
        let pattern = Pattern::new(
            &self.query,
            CaseMatching::Smart,
            Normalization::Smart,
            AtomKind::Fuzzy,
        );
        self.selected = 0;
        // An empty pattern scores every candidate alike, so only the tiers order them.
        if pattern.atoms.is_empty() {
            // A pass over the candidates for each tier, rather than a sort, since a finder over
            // every block of a large outline opens with it (N-PERF-02).
            let tiers: BTreeSet<u8> = self
                .candidates
                .iter()
                .map(|candidate| candidate.tier)
                .collect();
            let candidates = &self.candidates;
            self.ranked = tiers
                .into_iter()
                .flat_map(|tier| {
                    candidates
                        .iter()
                        .enumerate()
                        .filter(move |(_, candidate)| candidate.tier == tier)
                        .map(|(index, _)| index)
                })
                .collect();
            return;
        }
        let mut matcher = Matcher::new(Config::DEFAULT);
        let mut buffer = Vec::new();
        let mut scored: Vec<(usize, u8, u32)> = self
            .candidates
            .iter()
            .enumerate()
            .filter_map(|(index, candidate)| {
                let text = Utf32Str::new(&candidate.text, &mut buffer);
                let score = pattern.score(text, &mut matcher)?;
                Some((index, candidate.tier, score))
            })
            .collect();
        scored.sort_by_key(|&(_, tier, score)| (tier, Reverse(score)));
        self.ranked = scored.into_iter().map(|(index, _, _)| index).collect();
    }

    /// Applies `input`, and returns how it closed the finder, or `None` while it stays open.
    pub(super) fn input(&mut self, input: MenuInput) -> Option<Picked<T>> {
        match input {
            MenuInput::Type(c) => {
                self.query.push(c);
                self.rank();
            }
            MenuInput::Erase => {
                if self.query.pop().is_some() {
                    self.rank();
                }
            }
            MenuInput::Previous => self.selected = self.selected.saturating_sub(1),
            MenuInput::Next => {
                self.selected = (self.selected + 1).min(self.ranked.len().saturating_sub(1));
            }
            MenuInput::Choose => return self.highlighted().cloned().map(Picked::Chosen),
            MenuInput::Dismiss => return Some(Picked::Cancelled),
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A candidate with the value `value`, the text `text` and the tier `tier`.
    fn candidate(value: u8, text: &str, tier: u8) -> Candidate<u8> {
        Candidate {
            value,
            text: text.into(),
            detail: format!("path {value}").as_str().into(),
            tier,
        }
    }

    fn sample() -> Finder<u8> {
        Finder::new(vec![
            candidate(1, "Write the report", 2),
            candidate(2, "Reports", 1),
            candidate(3, "Buy milk", 2),
            candidate(4, "Read later", 0),
        ])
    }

    /// The values listed, best first, and the highlighted position.
    fn listed(finder: &Finder<u8>) -> (Vec<u8>, usize) {
        let view = finder.view();
        (
            view.entries
                .iter()
                .map(|candidate| candidate.value)
                .collect(),
            view.selected,
        )
    }

    fn typed(finder: &mut Finder<u8>, text: &str) {
        for c in text.chars() {
            assert_eq!(finder.input(MenuInput::Type(c)), None);
        }
    }

    // FR-NAV-05
    #[test]
    fn an_empty_query_lists_every_candidate_by_tier_then_in_order() {
        let finder = sample();
        assert_eq!(listed(&finder), (vec![4, 2, 1, 3], 0));
        assert_eq!(finder.view().query, "");
        assert_eq!(&*finder.view().entries[0].detail, "path 4");
    }

    // FR-NAV-05, N-PERF-02
    #[test]
    fn a_path_detail_reads_and_compares_as_its_blocks_joined() {
        let detail = Detail::path(["A", "b"].map(Arc::from).into());
        assert_eq!(&*detail, "A › b");
        assert_eq!(detail.to_string(), "A › b");
        assert_eq!(format!("{detail:?}"), r#""A › b""#);
        assert_eq!(detail, Detail::from("A › b"));
        assert_ne!(detail, Detail::from("A"));
        assert_eq!(&*Detail::default(), "");
    }

    // FR-NAV-05
    #[test]
    fn typing_keeps_the_fuzzy_matches_by_tier_then_score() {
        let mut finder = sample();
        typed(&mut finder, "rep");
        assert_eq!(listed(&finder).0, vec![2, 1]);
        assert_eq!(finder.view().query, "rep");
        let mut finder = sample();
        typed(&mut finder, "wrr");
        assert_eq!(listed(&finder).0, vec![1], "letters in order, not together");
        let mut finder = sample();
        typed(&mut finder, "milk buy");
        assert_eq!(listed(&finder).0, vec![3], "each word matches on its own");
        let mut finder = sample();
        typed(&mut finder, "zz");
        assert_eq!(listed(&finder), (vec![], 0));
        assert_eq!(finder.input(MenuInput::Choose), None, "nothing to choose");
    }

    #[test]
    fn a_better_match_ranks_first_within_a_tier() {
        let mut finder = Finder::new(vec![
            candidate(1, "a b c d e f g h", 0),
            candidate(2, "abc", 0),
        ]);
        typed(&mut finder, "abc");
        assert_eq!(listed(&finder).0, vec![2, 1]);
    }

    #[test]
    fn a_capital_in_the_query_matches_case() {
        let mut finder = sample();
        typed(&mut finder, "r");
        assert_eq!(listed(&finder).0, vec![4, 2, 1]);
        let mut finder = sample();
        typed(&mut finder, "R");
        assert_eq!(listed(&finder).0, vec![4, 2]);
    }

    #[test]
    fn operator_characters_match_themselves() {
        let mut finder = Finder::new(vec![
            candidate(1, "!important", 0),
            candidate(2, "plain", 0),
        ]);
        typed(&mut finder, "!imp");
        assert_eq!(listed(&finder).0, vec![1]);
    }

    #[test]
    fn arrows_move_within_the_matches_and_typing_highlights_the_first() {
        let mut finder = sample();
        assert_eq!(finder.input(MenuInput::Previous), None);
        assert_eq!(listed(&finder).1, 0, "the highlight stops at the first");
        for _ in 0..9 {
            assert_eq!(finder.input(MenuInput::Next), None);
        }
        assert_eq!(listed(&finder).1, 3, "the highlight stops at the last");
        finder.input(MenuInput::Previous);
        assert_eq!(finder.input(MenuInput::Choose), Some(Picked::Chosen(1)));
        typed(&mut finder, "e");
        assert_eq!(listed(&finder).1, 0);
    }

    #[test]
    fn erasing_widens_the_matches_and_nothing_to_erase_keeps_the_highlight() {
        let mut finder = sample();
        typed(&mut finder, "rep");
        finder.input(MenuInput::Next);
        assert_eq!(finder.input(MenuInput::Erase), None);
        assert_eq!(finder.view().query, "re");
        assert_eq!(listed(&finder), (vec![4, 2, 1], 0));
        let mut finder = sample();
        finder.input(MenuInput::Next);
        assert_eq!(finder.input(MenuInput::Erase), None);
        assert_eq!(listed(&finder).1, 1);
    }

    #[test]
    fn dismissing_cancels() {
        assert_eq!(sample().input(MenuInput::Dismiss), Some(Picked::Cancelled));
    }

    #[test]
    fn highlighting_a_value_highlights_its_match_and_ignores_values_not_matched() {
        let mut finder = sample();
        finder.highlight(&1);
        assert_eq!(listed(&finder), (vec![4, 2, 1, 3], 2));
        typed(&mut finder, "rep");
        finder.highlight(&3);
        assert_eq!(listed(&finder).1, 0, "3 does not match");
        finder.highlight(&1);
        assert_eq!(listed(&finder).1, 1);
    }
}
