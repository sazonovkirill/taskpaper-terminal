//! The rows a pane shows as one sequence kept in shared chunks, so that rows built again after a
//! change share every chunk the change left alone with the rows before it instead of copying a
//! large outline's rows for each keystroke (N-PERF-02).

use std::{
    collections::{HashMap, HashSet},
    hash::{Hash, Hasher},
    iter::FusedIterator,
    ops::Range,
    rc::Rc,
    slice,
};

use super::Row;
use crate::document::ItemId;

/// How many rows a chunk holds at least, apart from the last; a chunk holds fewer than twice as
/// many. Rebuilding rows copies no more than a few chunks' worth of the rows it keeps.
const CHUNK: usize = 256;

/// Visible rows in order, cheap to clone.
#[derive(Debug, Clone, Default)]
pub struct Rows {
    /// Each chunk with the index of its first row.
    chunks: Rc<[(usize, Chunk)]>,
    len: usize,
}

type Chunk = Rc<[Entry]>;

/// A row with the number of rows its block's subtree takes, its own included, and the open
/// tasks among them below it (D-67).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    row: Row,
    span: usize,
    open: usize,
}

impl Rows {
    /// How many rows there are.
    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether there are none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The row at `index`, if there is one.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&Row> {
        self.entry(index).map(|entry| &entry.row)
    }

    /// The first row.
    #[must_use]
    pub fn first(&self) -> Option<&Row> {
        self.get(0)
    }

    /// The last row.
    #[must_use]
    pub fn last(&self) -> Option<&Row> {
        self.get(self.len.checked_sub(1)?)
    }

    /// Every row in order.
    #[must_use]
    pub fn iter(&self) -> Iter<'_> {
        self.iter_from(0)
    }

    /// The rows from `index` on.
    #[must_use]
    pub fn iter_from(&self, index: usize) -> Iter<'_> {
        let at = self.chunk_of(index);
        let mut chunks = self.chunks.get(at..).unwrap_or_default().iter();
        let front = chunks.next().map_or(&[][..], |(start, chunk)| {
            chunk
                .get(index.saturating_sub(*start)..)
                .unwrap_or_default()
        });
        Iter {
            chunks,
            front: front.iter(),
            back: [].iter(),
            len: self.len.saturating_sub(index),
        }
    }

    /// The rows in the chunks `other` does not share, in order, each with its index: of rows
    /// built from `other` or it from them, those that may not be in `other` as they are here,
    /// found without going through the rest.
    pub fn apart_from<'a>(
        &'a self,
        other: &Self,
    ) -> impl Iterator<Item = (usize, &'a Row)> + use<'a> {
        let shared: HashSet<*const [Entry]> = other
            .chunks
            .iter()
            .map(|(_, chunk)| Rc::as_ptr(chunk))
            .collect();
        self.chunks
            .iter()
            .filter(move |(_, chunk)| !shared.contains(&Rc::as_ptr(chunk)))
            .flat_map(|(start, chunk)| {
                chunk
                    .iter()
                    .enumerate()
                    .map(move |(at, entry)| (start + at, &entry.row))
            })
    }

    /// The rows chunk by chunk, in order.
    pub(super) fn chunks(&self) -> impl Iterator<Item = RowChunk> + '_ {
        self.chunks
            .iter()
            .map(|(_, chunk)| RowChunk(Rc::clone(chunk)))
    }

    /// How many rows the subtree of the row at `index` takes, its own included.
    pub(super) fn span(&self, index: usize) -> Option<usize> {
        self.entry(index).map(|entry| entry.span)
    }

    fn entry(&self, index: usize) -> Option<&Entry> {
        let (start, chunk) = self.chunks.get(self.chunk_of(index))?;
        chunk.get(index.checked_sub(*start)?)
    }

    /// The position of the chunk holding the row at `index`.
    fn chunk_of(&self, index: usize) -> usize {
        self.chunks
            .partition_point(|&(start, _)| start <= index)
            .saturating_sub(1)
    }
}

#[cfg(test)]
impl Rows {
    /// The rows as a vector, for views of a slice of rows.
    pub(crate) fn to_vec(&self) -> Vec<Row> {
        self.iter().cloned().collect()
    }

    /// How many of its chunks are not shared with `other`.
    pub(crate) fn chunks_apart_from(&self, other: &Self) -> usize {
        self.chunks
            .iter()
            .filter(|(_, chunk)| !other.chunks.iter().any(|(_, kept)| Rc::ptr_eq(chunk, kept)))
            .count()
    }

    /// How many rows each chunk holds.
    pub(crate) fn chunk_sizes(&self) -> Vec<usize> {
        self.chunks.iter().map(|(_, chunk)| chunk.len()).collect()
    }

    /// Whether `a` and `b` are the same rows, not only equal ones.
    pub(crate) fn ptr_eq(a: &Self, b: &Self) -> bool {
        Rc::ptr_eq(&a.chunks, &b.chunks)
    }
}

#[cfg(test)]
impl std::ops::Index<usize> for Rows {
    type Output = Row;

    fn index(&self, index: usize) -> &Row {
        self.get(index).expect("a row at the index")
    }
}

impl PartialEq for Rows {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.chunks, &other.chunks)
            || self.len == other.len && self.entries().eq(other.entries())
    }
}

impl Eq for Rows {}

impl Rows {
    fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.chunks.iter().flat_map(|(_, chunk)| chunk.iter())
    }
}

/// A chunk of rows. Rows built again after a change share the chunks it left alone with the rows
/// before, and two chunks are equal only when they are the same one.
#[derive(Debug, Clone)]
pub(super) struct RowChunk(Chunk);

impl RowChunk {
    /// Its rows in order.
    pub(super) fn rows(&self) -> impl Iterator<Item = &Row> {
        self.0.iter().map(|entry| &entry.row)
    }
}

impl PartialEq for RowChunk {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for RowChunk {}

impl Hash for RowChunk {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Rc::as_ptr(&self.0).cast::<Entry>().hash(state);
    }
}

impl FromIterator<Row> for Rows {
    /// Rows that stand alone, each its own subtree, as the Today view lists them.
    fn from_iter<I: IntoIterator<Item = Row>>(rows: I) -> Self {
        let mut builder = Builder::new(None);
        for row in rows {
            let at = builder.open(row);
            builder.close(at);
        }
        builder.finish()
    }
}

impl<'a> IntoIterator for &'a Rows {
    type Item = &'a Row;
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Iter<'a> {
        self.iter()
    }
}

/// The rows of [`Rows`] in order, from either end.
#[derive(Debug, Clone)]
pub struct Iter<'a> {
    chunks: slice::Iter<'a, (usize, Chunk)>,
    front: slice::Iter<'a, Entry>,
    back: slice::Iter<'a, Entry>,
    len: usize,
}

impl<'a> Iterator for Iter<'a> {
    type Item = &'a Row;

    fn next(&mut self) -> Option<&'a Row> {
        loop {
            if let Some(entry) = self.front.next() {
                self.len -= 1;
                return Some(&entry.row);
            }
            if let Some((_, chunk)) = self.chunks.next() {
                self.front = chunk.iter();
            } else {
                let entry = self.back.next()?;
                self.len -= 1;
                return Some(&entry.row);
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl DoubleEndedIterator for Iter<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(entry) = self.back.next_back() {
                self.len -= 1;
                return Some(&entry.row);
            }
            if let Some((_, chunk)) = self.chunks.next_back() {
                self.back = chunk.iter();
            } else {
                let entry = self.front.next_back()?;
                self.len -= 1;
                return Some(&entry.row);
            }
        }
    }
}

impl ExactSizeIterator for Iter<'_> {}

impl FusedIterator for Iter<'_> {}

/// Builds rows in order from new rows and runs of the rows of `old`, which it shares rather
/// than copies where it can.
#[derive(Debug)]
pub(super) struct Builder<'a> {
    old: Option<&'a Rows>,
    /// The runs of old rows, each with how many new rows come before it.
    kept: Vec<(usize, Range<usize>)>,
    /// The new rows, in order.
    fresh: Vec<Entry>,
    len: usize,
    /// How many of the rows are open tasks.
    open: usize,
}

/// A row [`Builder::open`] added, whose subtree's rows follow until it is closed.
#[must_use]
#[derive(Debug, Clone, Copy)]
pub(super) struct Opened {
    fresh: usize,
    len: usize,
    open: usize,
}

impl<'a> Builder<'a> {
    /// A builder that may keep rows of `old`.
    pub(super) fn new(old: Option<&'a Rows>) -> Self {
        Self {
            old,
            kept: Vec::new(),
            fresh: Vec::new(),
            len: 0,
            open: 0,
        }
    }

    /// The rows it may keep.
    pub(super) fn old(&self) -> Option<&'a Rows> {
        self.old
    }

    /// Adds `row`, whose subtree's rows are the ones added until [`Builder::close`] is given what
    /// this returns.
    pub(super) fn open(&mut self, row: Row) -> Opened {
        self.open += usize::from(row.open_task());
        let opened = Opened {
            fresh: self.fresh.len(),
            len: self.len,
            open: self.open,
        };
        self.fresh.push(Entry {
            row,
            span: 1,
            open: 0,
        });
        self.len += 1;
        opened
    }

    /// Ends the subtree of the row `opened` stands for.
    pub(super) fn close(&mut self, opened: Opened) {
        if let Some(entry) = self.fresh.get_mut(opened.fresh) {
            entry.span = self.len - opened.len;
            entry.open = self.open - opened.open;
        }
    }

    /// Adds the old row at `index` with its subtree's rows, as they were; returns how many rows
    /// that is, or `None` without an old row there.
    pub(super) fn keep(&mut self, index: usize) -> Option<usize> {
        let entry = self.old?.entry(index)?;
        let span = entry.span;
        let rows = index..index + span;
        match self.kept.last_mut() {
            Some((before, old)) if *before == self.fresh.len() && old.end == rows.start => {
                old.end = rows.end;
            }
            _ => self.kept.push((self.fresh.len(), rows)),
        }
        self.len += span;
        self.open += usize::from(entry.row.open_task()) + entry.open;
        Some(span)
    }

    /// The rows built.
    pub(super) fn finish(self) -> Rows {
        let mut chunks = Chunks::default();
        let mut fresh = self.fresh.into_iter();
        let mut added = 0;
        for (before, range) in self.kept {
            chunks.extend(fresh.by_ref().take(before - added));
            added = before;
            if let Some(old) = self.old {
                chunks.keep(old, range);
            }
        }
        chunks.extend(fresh);
        chunks.finish()
    }
}

/// The rows of one block's children among the rows with nothing collapsed, gone through in order
/// while rows with blocks collapsed are built, so that a collapsed block's open tasks come from
/// its row there rather than from a walk over the document's lines.
#[derive(Debug)]
pub(super) struct Unfolded<'a> {
    rows: &'a Rows,
    /// The first of the children's rows.
    start: usize,
    /// The first row not yet passed.
    at: usize,
}

impl<'a> Unfolded<'a> {
    /// The children's rows among `rows`, from `start` on.
    pub(super) fn new(rows: &'a Rows, start: usize) -> Self {
        Self {
            rows,
            start,
            at: start,
        }
    }

    /// The rows of the children of the block `id`, the next of these children that is, passing
    /// those before it; `None` when it has no row.
    pub(super) fn below(&mut self, id: ItemId) -> Option<Self> {
        loop {
            let at = self.at;
            self.at += self.rows.span(at)?;
            if self.rows.get(at)?.id == id {
                return Some(Self::new(self.rows, at + 1));
            }
        }
    }

    /// How many open tasks these rows hold (D-67), or `None` when they are not the children of
    /// a row.
    pub(super) fn open_tasks(&self) -> Option<usize> {
        Some(self.rows.entry(self.start.checked_sub(1)?)?.open)
    }
}

/// The old rows of one block's children, gone through in order while its rows are built again,
/// so that the rows of children that did not change are kept.
#[derive(Debug)]
pub(super) struct OldSiblings<'a> {
    rows: &'a Rows,
    /// The first old row not yet kept or passed.
    at: usize,
    end: usize,
    /// Where looking ahead for a changed child's old row got to, with each row it passed.
    scanned: usize,
    seen: HashMap<ItemId, usize>,
}

impl<'a> OldSiblings<'a> {
    /// The children's rows among `rows`, which are `range`.
    pub(super) fn new(rows: &'a Rows, range: Range<usize>) -> Self {
        Self {
            rows,
            at: range.start,
            end: range.end,
            scanned: range.start,
            seen: HashMap::new(),
        }
    }

    /// Passes the rows of blocks in `changed`, which are built again rather than kept.
    pub(super) fn skip(&mut self, changed: &HashSet<ItemId>) {
        while let Some((row, span)) = self.next()
            && changed.contains(&row.id)
        {
            self.at += span;
        }
    }

    /// Keeps the next old rows in `rows` when they are those of the block `id` and its subtree,
    /// at the same depth and inside a done block or not alike, as `(depth, inside_done)` says;
    /// returns whether it did.
    pub(super) fn keep(
        &mut self,
        id: ItemId,
        (depth, inside_done): (usize, bool),
        rows: &mut Builder,
    ) -> bool {
        let Some((row, span)) = self.next().filter(|(row, _)| row.id == id) else {
            return false;
        };
        let same = row.depth == depth && row.inside_done == inside_done;
        let kept = same && rows.keep(self.at).is_some();
        self.at += span;
        kept
    }

    /// The old rows of the children of the block `id`, when it had a row among these.
    pub(super) fn children_of(&mut self, id: ItemId) -> Option<Range<usize>> {
        let mut found = self.seen.get(&id).copied();
        self.scanned = self.scanned.max(self.at);
        while found.is_none() && self.scanned < self.end {
            let row = self.rows.get(self.scanned)?;
            self.seen.insert(row.id, self.scanned);
            found = (row.id == id).then_some(self.scanned);
            self.scanned += self.rows.span(self.scanned)?;
        }
        let at = found?;
        Some(at + 1..at + self.rows.span(at)?)
    }

    /// The next old row with its span.
    fn next(&self) -> Option<(&'a Row, usize)> {
        if self.at >= self.end {
            return None;
        }
        Some((self.rows.get(self.at)?, self.rows.span(self.at)?))
    }
}

/// The chunks of rows being built: shared old chunks, and new ones of at least [`CHUNK`] rows
/// apart from the last.
#[derive(Debug, Default)]
struct Chunks {
    done: Vec<(usize, Chunk)>,
    len: usize,
    /// The rows of the next new chunk.
    next: Vec<Entry>,
}

impl Chunks {
    fn extend(&mut self, entries: impl IntoIterator<Item = Entry>) {
        for entry in entries {
            self.next.push(entry);
            if self.next.len() == CHUNK {
                self.seal();
            }
        }
    }

    /// Adds the rows `range` of `old`, sharing each of its chunks that the range covers and
    /// that is big enough to stand on its own.
    fn keep(&mut self, old: &Rows, range: Range<usize>) {
        let mut at = range.start;
        while at < range.end
            && let Some((start, chunk)) = old.chunks.get(old.chunk_of(at))
        {
            let from = at - start;
            let to = (range.end - start).min(chunk.len());
            if from == 0 && to == chunk.len() && chunk.len() >= CHUNK && self.next.is_empty() {
                self.done.push((self.len, Rc::clone(chunk)));
                self.len += chunk.len();
            } else {
                // Copied whole into the next chunk, so that sealing it never leaves a few rows
                // to copy the following shared chunk onto.
                self.next
                    .extend(chunk.get(from..to).unwrap_or_default().iter().cloned());
                if self.next.len() >= CHUNK {
                    self.seal();
                }
            }
            at = start + to;
        }
    }

    /// Ends the new chunk, split evenly into chunks of fewer than twice [`CHUNK`] rows.
    fn seal(&mut self) {
        let total = self.next.len();
        let pieces = (total / CHUNK).max(1);
        let mut rows = std::mem::take(&mut self.next).into_iter();
        for piece in 0..pieces {
            let size = total * (piece + 1) / pieces - total * piece / pieces;
            if size > 0 {
                self.done
                    .push((self.len, rows.by_ref().take(size).collect()));
                self.len += size;
            }
        }
    }

    fn finish(mut self) -> Rows {
        self.seal();
        Rows {
            chunks: self.done.into(),
            len: self.len,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

    use super::*;
    use crate::{app::State, document::format};

    /// The rows of an outline of `count` top-level tasks, built anew.
    fn tasks(count: usize) -> Rows {
        let mut text = String::new();
        for task in 0..count {
            writeln!(text, "- {task}").unwrap();
        }
        State::new(format::parse(&text), None).rows()
    }

    fn texts<'a>(rows: impl IntoIterator<Item = &'a Row>) -> Vec<String> {
        rows.into_iter().map(|row| row.text.to_string()).collect()
    }

    // N-PERF-02
    #[test]
    fn rows_read_like_a_slice_across_their_chunks() {
        let rows = tasks(CHUNK * 3 + 7);
        let all: Vec<String> = (0..rows.len()).map(|task| task.to_string()).collect();
        assert_eq!(rows.len(), CHUNK * 3 + 7);
        assert!(!rows.is_empty());
        assert_eq!(texts(&rows), all);
        assert_eq!(
            texts(rows.iter().rev()),
            all.iter().rev().cloned().collect::<Vec<_>>()
        );
        assert_eq!(texts(rows.iter_from(CHUNK + 1)), all[CHUNK + 1..]);
        assert_eq!(rows.iter_from(CHUNK + 1).len(), all.len() - CHUNK - 1);
        assert_eq!(rows.iter_from(rows.len()).next(), None);
        assert_eq!(rows.get(CHUNK).map(|row| &*row.text), Some(&*all[CHUNK]));
        assert_eq!(rows.get(rows.len()), None);
        assert_eq!(rows.first().map(|row| &*row.text), Some("0"));
        assert_eq!(
            rows.last().map(|row| &*row.text),
            all.last().map(String::as_str)
        );
        assert_eq!(rows.span(5), Some(1));
        assert!(Rows::default().is_empty());
        assert_eq!(Rows::default().last(), None);
    }

    // N-PERF-02
    #[test]
    fn iterating_from_both_ends_meets_in_the_middle() {
        let rows = tasks(CHUNK * 2 + 3);
        let mut iter = rows.iter();
        let mut seen = Vec::new();
        while let (Some(front), back) = (iter.next(), iter.next_back()) {
            seen.push(front.text.to_string());
            seen.extend(back.map(|row| row.text.to_string()));
            assert_eq!(iter.len(), rows.len() - seen.len());
        }
        seen.sort_by_key(|text| text.parse::<usize>().unwrap());
        assert_eq!(seen, texts(rows.iter()));
        let three = tasks(3);
        let mut short = three.iter();
        assert_eq!(short.next_back().map(|row| &*row.text), Some("2"));
        assert_eq!(short.len(), 2);
        assert_eq!(texts(short.by_ref()), ["0", "1"]);
        assert_eq!(short.next_back(), None);
    }

    // N-PERF-02
    #[test]
    fn rows_built_from_old_ones_share_the_chunks_they_keep() {
        let old = tasks(CHUNK * 8);
        let new = tasks(1).get(0).cloned().unwrap();
        let mut rows = Builder::new(Some(&old));
        assert_eq!(rows.keep(0), Some(1));
        assert_eq!(rows.keep(1), Some(1));
        let opened = rows.open(new.clone());
        rows.close(opened);
        for at in 2..old.len() {
            rows.keep(at);
        }
        assert_eq!(rows.keep(old.len()), None);
        let rows = rows.finish();
        let mut expected = texts(old.iter());
        expected.insert(2, "0".to_owned());
        assert_eq!(texts(rows.iter()), expected);
        assert_eq!(
            rows.chunks_apart_from(&old),
            1,
            "only the chunk with the new row"
        );
        assert_ne!(rows, old);
        assert_eq!(rows, rows.clone());
        assert_eq!(
            rows.iter().cloned().collect::<Rows>(),
            rows,
            "equal rows in other chunks"
        );
        assert_eq!(Builder::new(None).keep(0), None);
        let later: Rows = tasks(3).iter().skip(1).cloned().collect();
        assert_ne!(later, tasks(2), "as many rows, but others");
    }

    // N-PERF-02
    #[test]
    fn the_rows_apart_from_other_rows_are_those_of_the_chunks_they_do_not_share() {
        let old = tasks(CHUNK * 4);
        let ids = |rows: &Rows| rows.iter().map(|row| row.id).collect::<Vec<_>>();
        let apart = |rows: &Rows, other: &Rows| {
            rows.apart_from(other)
                .map(|(at, row)| {
                    assert_eq!(rows.get(at), Some(row));
                    row.id
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(apart(&old, &Rows::default()), ids(&old));
        assert_eq!(old.apart_from(&old.clone()).count(), 0);
        let mut builder = Builder::new(Some(&old));
        builder.keep(0);
        for at in 2..old.len() {
            builder.keep(at);
        }
        let rows = builder.finish();
        let gone = old.get(1).unwrap().id;
        let left = apart(&old, &rows);
        let apart = apart(&rows, &old);
        assert!(left.contains(&gone) && !apart.contains(&gone));
        assert!(apart.len() < CHUNK * 2, "only the chunk the row left");
        assert_eq!(left.len(), apart.len() + 1);
        assert!(apart.iter().all(|id| ids(&rows).contains(id)));
    }

    // N-PERF-02
    #[test]
    fn chunks_stay_small_however_often_rows_go_in_or_out_at_one_place() {
        let row = tasks(1).get(0).cloned().unwrap();
        let mut rows = tasks(CHUNK * 4);
        for round in 0..CHUNK * 3 {
            let old = rows.clone();
            let mut builder = Builder::new(Some(&old));
            builder.keep(0);
            if round % 3 == 2 {
                builder.keep(2);
            } else {
                builder.keep(1);
                let at = builder.open(row.clone());
                builder.close(at);
            }
            for at in 2..old.len() {
                builder.keep(at);
            }
            rows = builder.finish();
            let sizes = rows.chunk_sizes();
            let (last, others) = sizes.split_last().unwrap();
            assert!(
                others.iter().chain([last]).all(|&size| size < CHUNK * 2)
                    && others.iter().all(|&size| size >= CHUNK),
                "chunks of {sizes:?} rows"
            );
        }
    }

    // N-PERF-02
    #[test]
    fn opened_rows_span_the_rows_added_until_closed() {
        let row = tasks(1).get(0).cloned().unwrap();
        let mut rows = Builder::new(None);
        let outer = rows.open(row.clone());
        let inner = rows.open(row.clone());
        rows.close(inner);
        let last = rows.open(row);
        rows.close(last);
        rows.close(outer);
        let rows = rows.finish();
        assert_eq!(
            [0, 1, 2].map(|at| rows.span(at)),
            [Some(3), Some(1), Some(1)]
        );
    }
}
