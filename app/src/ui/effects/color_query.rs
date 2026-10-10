//! Asks the terminal which colors it draws with: its background, its text color and its 16 ANSI
//! colors, so that Terminal Dark can be drawn in them with truecolor (ui.md §3, D-128).

use std::{
    io::{Read, Write},
    os::fd::AsFd,
};

use rustix::termios::{self, OptionalActions, SpecialCodeIndex};

use crate::ui::theme::TerminalColors;

/// How many tenths of a second the terminal may stay silent in all before its answers are taken
/// as complete: a terminal that does not answer delays the start by this much.
const PATIENCE: u32 = 10;

/// How many bytes of answers are read at most, so that a terminal sending more cannot keep the
/// start waiting.
const LIMIT: usize = 4096;

/// Asks the terminal on `tty` for its colors and returns them, if it reported at least its
/// background and text colors. The questions end with one for the primary device attributes,
/// which every terminal answers after the others, so its answer tells that the others are in.
/// The answers are read before anything else reads the terminal, which would take them for
/// keys. `tty` is read without waiting for a line and without echo while asking, and set back
/// after.
pub fn query_terminal_colors<T: AsFd + Read + Write>(tty: &mut T) -> Option<TerminalColors> {
    let saved = termios::tcgetattr(tty.as_fd()).ok()?;
    let mut asking = saved.clone();
    asking.make_raw();
    asking.special_codes[SpecialCodeIndex::VMIN] = 0;
    asking.special_codes[SpecialCodeIndex::VTIME] = 1;
    termios::tcsetattr(tty.as_fd(), OptionalActions::Now, &asking).ok()?;
    let answers = exchange(tty);
    // The terminal is left as it was found; one that cannot be set back is gone.
    let _ = termios::tcsetattr(tty.as_fd(), OptionalActions::Now, &saved);
    parse(&answers?)
}

/// Writes the questions to `tty` and reads its answers, each read waiting a tenth of a second
/// at most.
fn exchange(tty: &mut (impl Write + Read)) -> Option<Vec<u8>> {
    tty.write_all(&questions()).ok()?;
    tty.flush().ok()?;
    let mut answers = Vec::new();
    let mut chunk = [0; 256];
    let mut silences = 0;
    while silences < PATIENCE && answers.len() < LIMIT && !answered(&answers) {
        // A terminal that cannot be read, as after it hung up, says no more than a silent one.
        match tty.read(&mut chunk) {
            Ok(0) | Err(_) => silences += 1,
            Ok(read) => answers.extend(chunk.iter().take(read)),
        }
    }
    Some(answers)
}

/// The questions: the text color (OSC 10), the background (OSC 11) and each of the 16 ANSI
/// colors (OSC 4), then the primary device attributes (DA1).
fn questions() -> Vec<u8> {
    let ansi = (0..16).flat_map(|index| format!("\x1b]4;{index};?\x1b\\").into_bytes());
    (*b"\x1b]10;?\x1b\\\x1b]11;?\x1b\\")
        .into_iter()
        .chain(ansi)
        .chain(*b"\x1b[c")
        .collect()
}

/// Whether `answers` hold the answer to the primary device attributes, `CSI ? … c`.
fn answered(answers: &[u8]) -> bool {
    answers
        .windows(3)
        .enumerate()
        .filter(|(_, window)| *window == b"\x1b[?")
        .any(|(start, _)| {
            answers
                .iter()
                .skip(start + 3)
                .find(|byte| !(byte.is_ascii_digit() || **byte == b';'))
                == Some(&b'c')
        })
}

/// The colors `answers` report, if they hold the background and the text color. Each answer is
/// `OSC code ; [index ;] rgb:R/G/B`, ended by BEL or ST, with one to four hex digits a channel.
fn parse(answers: &[u8]) -> Option<TerminalColors> {
    let mut background = None;
    let mut text = None;
    let mut ansi = [None; 16];
    for body in String::from_utf8_lossy(answers).split("\x1b]").skip(1) {
        let body = body.split(['\x07', '\x1b']).next().unwrap_or_default();
        match body.split(';').collect::<Vec<_>>().as_slice() {
            ["10", spec] => text = rgb(spec),
            ["11", spec] => background = rgb(spec),
            ["4", index, spec] => {
                if let Some(slot) = index.parse::<usize>().ok().and_then(|i| ansi.get_mut(i)) {
                    *slot = rgb(spec);
                }
            }
            _ => {}
        }
    }
    Some(TerminalColors::new(background?, text?, ansi))
}

/// The color `spec` names as `rgb:R/G/B`.
fn rgb(spec: &str) -> Option<[u8; 3]> {
    let mut channels = spec.strip_prefix("rgb:")?.split('/').map(channel);
    let color = [channels.next()??, channels.next()??, channels.next()??];
    channels.next().is_none().then_some(color)
}

/// A channel of one to four hex digits, scaled to 0–255.
fn channel(digits: &str) -> Option<u8> {
    if !(1..=4).contains(&digits.len()) {
        return None;
    }
    let value = u32::from_str_radix(digits, 16).ok()?;
    let most = (1 << (4 * digits.len())) - 1;
    u8::try_from((value * 255 + most / 2) / most).ok()
}

#[cfg(test)]
mod tests {
    use std::{fs::File, thread};

    use rustix::pty::{self, OpenptFlags};

    use super::*;

    const ST: &str = "\x1b\\";

    fn answer(code: &str, color: &str, end: &str) -> String {
        format!("\x1b]{code};rgb:{color}{end}")
    }

    /// A pseudo-terminal: the side a terminal emulator holds, and the side tpt reads.
    fn pseudo_terminal() -> (File, File) {
        let main = pty::openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY).unwrap();
        pty::grantpt(&main).unwrap();
        pty::unlockpt(&main).unwrap();
        let name = pty::ptsname(&main, Vec::new()).unwrap();
        let side = File::options()
            .read(true)
            .write(true)
            .open(name.to_str().unwrap())
            .unwrap();
        (File::from(main), side)
    }

    /// A terminal on `main` that reads the questions up to DA1 and writes `reply`, keeping `main`
    /// open until joined, as closing it hangs up the other side.
    fn emulator(mut main: File, reply: String) -> thread::JoinHandle<(Vec<u8>, File)> {
        thread::spawn(move || {
            let mut asked = Vec::new();
            let mut byte = [0];
            while !asked.ends_with(b"\x1b[c") {
                main.read_exact(&mut byte).unwrap();
                asked.push(byte[0]);
            }
            main.write_all(reply.as_bytes()).unwrap();
            (asked, main)
        })
    }

    #[test]
    fn a_terminal_that_answers_reports_its_colors() {
        let (main, mut side) = pseudo_terminal();
        let before = termios::tcgetattr(&side).unwrap();
        let reply = [
            answer("10", "eeee/dddd/cccc", ST),
            answer("11", "1111/2222/3333", "\x07"),
            answer("4;1", "ff/00/00", ST),
            answer("4;12", "0/8/f", ST),
            "\x1b[?62;22c".to_owned(),
        ]
        .concat();
        let terminal = emulator(main, reply);
        let colors = query_terminal_colors(&mut side);
        let mut ansi = [None; 16];
        ansi[1] = Some([255, 0, 0]);
        ansi[12] = Some([0, 136, 255]);
        assert_eq!(
            colors,
            Some(TerminalColors::new([17, 34, 51], [238, 221, 204], ansi))
        );
        let (asked, _main) = terminal.join().unwrap();
        assert_eq!(asked, questions());
        let after = termios::tcgetattr(&side).unwrap();
        assert_eq!(after.local_modes, before.local_modes, "set back");
    }

    #[test]
    fn a_terminal_without_device_attributes_still_reports_what_it_answered() {
        let (main, mut side) = pseudo_terminal();
        let reply = [
            answer("10", "ffff/ffff/ffff", ST),
            answer("11", "0000/0000/0000", ST),
        ]
        .concat();
        let terminal = emulator(main, reply);
        assert_eq!(
            query_terminal_colors(&mut side),
            Some(TerminalColors::new([0, 0, 0], [255, 255, 255], [None; 16]))
        );
        terminal.join().unwrap();
    }

    #[test]
    fn a_terminal_reporting_no_background_reports_nothing() {
        let (main, mut side) = pseudo_terminal();
        let reply = [answer("10", "ffff/ffff/ffff", ST), "\x1b[?1;2c".to_owned()].concat();
        let _terminal = emulator(main, reply);
        assert_eq!(query_terminal_colors(&mut side), None);
    }

    #[test]
    fn a_terminal_that_hangs_up_reports_nothing() {
        let (main, mut side) = pseudo_terminal();
        let terminal = emulator(main, String::new());
        thread::spawn(move || drop(terminal.join()));
        assert_eq!(query_terminal_colors(&mut side), None);
    }

    #[test]
    fn a_file_that_is_not_a_terminal_is_not_asked() {
        let mut file = tempfile::tempfile().unwrap();
        assert_eq!(query_terminal_colors(&mut file), None);
        assert_eq!(std::io::Seek::stream_position(&mut file).unwrap(), 0);
    }

    #[test]
    fn answers_end_with_the_device_attributes() {
        assert!(answered(b"\x1b]11;rgb:0/0/0\x07\x1b[?62;22c"));
        assert!(answered(b"\x1b[?c"));
        assert!(!answered(b"\x1b[?62;22"));
        assert!(!answered(b"\x1b[?62;2x c"));
        assert!(!answered(b"\x1b[62c"));
    }

    #[test]
    fn malformed_answers_report_nothing_of_their_color() {
        let text = answer("10", "ffff/ffff/ffff", ST);
        for background in [
            "rgba:0/0/0/0",
            "rgb:0/0",
            "rgb:0/0/0/0",
            "rgb:00000/0/0",
            "rgb:/0/0",
            "rgb:g/0/0",
        ] {
            let answers = format!("{text}\x1b]11;{background}{ST}");
            assert_eq!(parse(answers.as_bytes()), None, "{background}");
        }
        let answers = [
            text.clone(),
            answer("11", "0/0/0", ST),
            answer("4;16", "f/f/f", ST),
            answer("4;x", "f/f/f", ST),
            answer("12", "f/f/f", ST),
        ]
        .concat();
        assert_eq!(
            parse(answers.as_bytes()),
            Some(TerminalColors::new([0, 0, 0], [255, 255, 255], [None; 16]))
        );
    }

    #[test]
    fn channels_of_any_width_scale_to_a_byte() {
        let cases = [
            ("f", 255),
            ("8", 136),
            ("80", 128),
            ("7f", 127),
            ("800", 128),
            ("8000", 128),
        ];
        for (digits, value) in cases {
            assert_eq!(channel(digits), Some(value), "{digits}");
        }
        assert_eq!(channel("0"), Some(0));
    }
}
