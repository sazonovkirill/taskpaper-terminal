//! The clipboard that copying and pasting blocks use: the system clipboard when a display server
//! can be reached, else the terminal's clipboard through OSC 52 for copying, which works over SSH,
//! and the text copied last for pasting ([editing.md §4](../../../../docs/specs/editing.md#4-structural-operations-fr-ops)).
//! Text pasted into the terminal reaches the app as a bracketed paste instead.

use std::{fmt, io::Write};

use crossterm::{clipboard::CopyToClipboard, execute};

/// The system clipboard of the desktop the terminal runs on.
pub trait SystemClipboard {
    /// Puts `text` on the clipboard; returns whether that worked.
    fn set(&mut self, text: &str) -> bool;
    /// The text on the clipboard, or `None` when it holds none or cannot be read.
    fn get(&mut self) -> Option<String>;
}

/// Copies text and gives it back for pasting, through the system clipboard when there is one.
pub struct Clipboard {
    system: Option<Box<dyn SystemClipboard>>,
    /// Where OSC 52 goes: the terminal.
    terminal: Box<dyn Write>,
    /// The text copied last.
    copied: Option<String>,
}

impl fmt::Debug for Clipboard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Clipboard")
            .field("system", &self.system.is_some())
            .field("copied", &self.copied)
            .finish_non_exhaustive()
    }
}

impl Clipboard {
    /// A clipboard on `system`, or without a system clipboard when it is `None`, that writes
    /// OSC 52 to `terminal`.
    #[must_use]
    pub fn new(system: Option<Box<dyn SystemClipboard>>, terminal: impl Write + 'static) -> Self {
        Self {
            system,
            terminal: Box::new(terminal),
            copied: None,
        }
    }

    /// Puts `text` on the system clipboard, or when that fails, asks the terminal to put it on
    /// its clipboard with OSC 52, which terminals that do not support it ignore.
    pub fn copy(&mut self, text: &str) {
        self.copied = Some(text.to_owned());
        if self.system.as_mut().is_some_and(|system| system.set(text)) {
            return;
        }
        self.system = None;
        // The terminal is the screen: when it cannot be written, there is nowhere to say so.
        let _ = execute!(self.terminal, CopyToClipboard::to_clipboard_from(text));
    }

    /// The text on the system clipboard, or without one, the text copied last; `None` when
    /// there is none.
    pub fn paste(&mut self) -> Option<String> {
        match &mut self.system {
            Some(system) => system.get(),
            None => self.copied.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use super::*;

    /// What was written, shared with the test.
    #[derive(Clone, Default)]
    struct Output(Rc<RefCell<Vec<u8>>>);

    impl Write for Output {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.borrow_mut().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl Output {
        fn text(&self) -> String {
            String::from_utf8(self.0.borrow().clone()).unwrap()
        }
    }

    /// A system clipboard holding `text`, which refuses new text when `broken`.
    struct Fake {
        text: Rc<RefCell<Option<String>>>,
        broken: bool,
    }

    impl SystemClipboard for Fake {
        fn set(&mut self, text: &str) -> bool {
            if !self.broken {
                *self.text.borrow_mut() = Some(text.to_owned());
            }
            !self.broken
        }

        fn get(&mut self) -> Option<String> {
            self.text.borrow().clone()
        }
    }

    // FR-OPS-06
    #[test]
    fn without_a_system_clipboard_copying_writes_osc_52_and_pasting_gives_the_copied_text() {
        let output = Output::default();
        let mut clipboard = Clipboard::new(None, output.clone());
        assert_eq!(clipboard.paste(), None);
        clipboard.copy("- a\n");
        assert_eq!(output.text(), "\x1b]52;c;LSBhCg==\x1b\\");
        assert_eq!(clipboard.paste().as_deref(), Some("- a\n"));
    }

    // FR-OPS-06
    #[test]
    fn the_system_clipboard_is_used_when_it_works() {
        let output = Output::default();
        let text = Rc::new(RefCell::new(Some("elsewhere".to_owned())));
        let system = Fake {
            text: Rc::clone(&text),
            broken: false,
        };
        let mut clipboard = Clipboard::new(Some(Box::new(system)), output.clone());
        assert_eq!(clipboard.paste().as_deref(), Some("elsewhere"));
        clipboard.copy("x");
        assert_eq!(text.borrow().as_deref(), Some("x"));
        assert_eq!(output.text(), "", "no OSC 52");
        *text.borrow_mut() = None;
        assert_eq!(
            clipboard.paste(),
            None,
            "what the system clipboard holds, not the copy"
        );
    }

    // FR-OPS-06
    #[test]
    fn a_failing_system_clipboard_falls_back_to_osc_52() {
        let output = Output::default();
        let system = Fake {
            text: Rc::default(),
            broken: true,
        };
        let mut clipboard = Clipboard::new(Some(Box::new(system)), output.clone());
        clipboard.copy("x");
        assert_eq!(output.text(), "\x1b]52;c;eA==\x1b\\");
        assert!(format!("{clipboard:?}").contains("system: false"));
    }

    // FR-OPS-06
    #[test]
    fn pasting_after_a_failed_system_copy_uses_the_local_copy_instead_of_stale_system_text() {
        let output = Output::default();
        let system = Fake {
            text: Rc::new(RefCell::new(Some("old text".to_owned()))),
            broken: true,
        };
        let mut clipboard = Clipboard::new(Some(Box::new(system)), output.clone());
        clipboard.copy("new text");
        assert_eq!(clipboard.paste().as_deref(), Some("new text"));
        clipboard.copy("next text");
        assert_eq!(clipboard.paste().as_deref(), Some("next text"));
        assert_eq!(
            output.text(),
            "\x1b]52;c;bmV3IHRleHQ=\x1b\\\x1b]52;c;bmV4dCB0ZXh0\x1b\\"
        );
    }
}
