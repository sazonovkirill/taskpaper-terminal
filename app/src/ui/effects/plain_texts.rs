//! Working out every line's plain text on another thread once a document shows, since the first
//! finder that lists every line of a large file reads them all (N-PERF-02).

use std::thread::{self, JoinHandle};

use crate::document::{Document, Stamp};

/// Starts the work for each document shown: the one opened, and each reloaded in its place.
#[derive(Debug, Default)]
pub struct PlainTexts {
    /// The document shown last, and the thread working out its lines' plain text.
    shown: Option<(Stamp, JoinHandle<()>)>,
}

impl PlainTexts {
    /// Notes that `document` shows, and works out its lines' plain text on another thread unless
    /// it is the document shown last; lines changed since then work out their own when read.
    pub fn shown(&mut self, document: &Document) {
        if let Some((stamp, _)) = &mut self.shown
            && document.changed_since(stamp).is_some()
        {
            *stamp = document.stamp();
            return;
        }
        let copy = document.clone();
        let thread = thread::spawn(move || copy.work_out_plain_texts());
        self.shown = Some((document.stamp(), thread));
    }
}

#[cfg(test)]
mod tests {
    use std::thread::ThreadId;

    use super::*;
    use crate::document::format;

    fn working(plain_texts: &PlainTexts) -> Option<ThreadId> {
        plain_texts
            .shown
            .as_ref()
            .map(|(_, thread)| thread.thread().id())
    }

    // N-PERF-02
    #[test]
    fn each_document_shown_is_worked_out_once() {
        let document = format::parse("**a**");
        let mut plain_texts = PlainTexts::default();
        plain_texts.shown(&document);
        let first = working(&plain_texts).expect("a thread working out the document");
        plain_texts.shown(&document);
        assert_eq!(working(&plain_texts), Some(first), "the same document");
        let reloaded = format::parse("**a**");
        plain_texts.shown(&reloaded);
        let second = working(&plain_texts).expect("a thread working out the reloaded document");
        assert_ne!(second, first);
    }
}
