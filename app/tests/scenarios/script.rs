//! The scenario language: what a user does in one session of the app and what they should see
//! and what should be saved, one command per line (docs/quality/testing-strategy.md, Scenarios).

use termlens::{Chord, Key, Signal};

/// A parsed scenario file.
#[derive(Debug, PartialEq)]
pub struct Scenario {
    /// The first comment line: what the user does and expects.
    pub title: String,
    /// Why the scenario fails today, from a `# BUG:` line. Such a scenario is expected to fail.
    pub bug: Option<String>,
    pub size: (u16, u16),
    pub env: Vec<(String, String)>,
    pub config: Option<String>,
    pub file: Option<String>,
    /// Each step with the number of the line it starts on.
    pub steps: Vec<(usize, Step)>,
}

/// One thing the user does or checks.
#[derive(Debug, PartialEq)]
pub enum Step {
    Keys(Vec<Press>),
    Type(String),
    Paste(String),
    See(String),
    Hide(String),
    Status(String),
    Resize(u16, u16),
    Quit,
    Saved(String),
    /// Another program writes the file.
    Outside(String),
    /// Quits, unless the app has ended, and starts it again on the same file.
    Restart,
    Signal(Signal),
}

/// A key press as a terminal without the kitty keyboard protocol sends it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Press {
    Key(Key),
    Chord(Chord),
}

/// The default terminal: wide enough that the status line shows a temporary folder's path.
const SIZE: (u16, u16) = (100, 30);

/// Parses the text of a scenario file, or says on which line and why it cannot.
pub fn parse(text: &str) -> Result<Scenario, String> {
    let mut scenario = Scenario {
        title: String::new(),
        bug: None,
        size: SIZE,
        env: Vec::new(),
        config: None,
        file: None,
        steps: Vec::new(),
    };
    let mut lines = text
        .lines()
        .enumerate()
        .map(|(index, line)| (index + 1, line));
    while let Some((number, line)) = lines.next() {
        let at = |message: String| format!("line {number}: {message}");
        let line = line.trim_end();
        if let Some(comment) = line.strip_prefix('#') {
            let comment = comment.trim();
            if let Some(bug) = comment.strip_prefix("BUG:") {
                scenario.bug = Some(bug.trim().to_owned());
            } else if scenario.title.is_empty() {
                comment.clone_into(&mut scenario.title);
            }
            continue;
        }
        if line.is_empty() {
            continue;
        }
        let (command, rest) = line.split_once(' ').unwrap_or((line, ""));
        let step = match command {
            "size" if scenario.steps.is_empty() => {
                scenario.size = size(rest).map_err(at)?;
                continue;
            }
            "env" => {
                let (name, value) = rest
                    .split_once('=')
                    .ok_or_else(|| at(format!("expected NAME=VALUE, found {rest:?}")))?;
                scenario.env.push((name.to_owned(), value.to_owned()));
                continue;
            }
            "config" => {
                scenario.config = Some(block(rest, &mut lines).map_err(at)?);
                continue;
            }
            "file" => {
                scenario.file = Some(block(rest, &mut lines).map_err(at)?);
                continue;
            }
            "keys" => Step::Keys(keys(rest).map_err(at)?),
            "type" => Step::Type(rest.to_owned()),
            "paste" => Step::Paste(block(rest, &mut lines).map_err(at)?),
            "see" => Step::See(text_of(rest).map_err(at)?),
            "hide" => Step::Hide(text_of(rest).map_err(at)?),
            "status" => Step::Status(text_of(rest).map_err(at)?),
            "resize" => {
                let (width, height) = size(rest).map_err(at)?;
                Step::Resize(width, height)
            }
            "quit" if rest.is_empty() => Step::Quit,
            "saved" => Step::Saved(block(rest, &mut lines).map_err(at)?),
            "outside" => Step::Outside(block(rest, &mut lines).map_err(at)?),
            "restart" if rest.is_empty() => Step::Restart,
            "signal" => Step::Signal(match rest {
                "TERM" => Signal::Term,
                "HUP" => Signal::Hup,
                _ => return Err(at(format!("expected TERM or HUP, found {rest:?}"))),
            }),
            _ => return Err(at(format!("unknown command {line:?}"))),
        };
        scenario.steps.push((number, step));
    }
    if scenario.title.is_empty() {
        return Err("the first comment line, the title, is missing".to_owned());
    }
    Ok(scenario)
}

fn size(text: &str) -> Result<(u16, u16), String> {
    let parse = |part: Option<&str>| part.and_then(|part| part.parse::<u16>().ok());
    let mut parts = text.split_whitespace();
    match (parse(parts.next()), parse(parts.next()), parts.next()) {
        (Some(width), Some(height), None) if width > 0 && height > 0 => Ok((width, height)),
        _ => Err(format!("expected columns and rows, found {text:?}")),
    }
}

fn text_of(rest: &str) -> Result<String, String> {
    if rest.trim().is_empty() {
        Err("expected the text to look for".to_owned())
    } else {
        Ok(rest.to_owned())
    }
}

/// The lines after `<<MARK` up to the line `MARK`, each ended by a newline.
fn block<'a>(
    rest: &str,
    lines: &mut impl Iterator<Item = (usize, &'a str)>,
) -> Result<String, String> {
    let mark = rest
        .trim()
        .strip_prefix("<<")
        .filter(|mark| !mark.is_empty())
        .ok_or_else(|| format!("expected <<MARK, found {rest:?}"))?;
    let mut text = String::new();
    for (_, line) in lines.by_ref() {
        if line.trim_end() == mark {
            return Ok(text);
        }
        text.push_str(line);
        text.push('\n');
    }
    Err(format!("no line {mark:?} ends the block"))
}

fn keys(text: &str) -> Result<Vec<Press>, String> {
    let mut presses = Vec::new();
    for token in text.split_whitespace() {
        match token
            .strip_prefix('<')
            .and_then(|token| token.strip_suffix('>'))
        {
            Some(name) if !name.is_empty() => presses.push(named(name)?),
            _ => presses.extend(token.chars().map(|c| Press::Key(Key::Char(c)))),
        }
    }
    if presses.is_empty() {
        return Err("expected keys".to_owned());
    }
    Ok(presses)
}

/// A key written as `<Name>` or `<C-x>`, `<A-x>`, `<S-Up>`, `<A-S-Left>`.
fn named(name: &str) -> Result<Press, String> {
    let unknown = || format!("unknown key <{name}>");
    let mut parts: Vec<&str> = name.split('-').collect();
    // `<A-->` is Alt with the minus key: the last part is empty after the split.
    if parts.len() > 1 && parts.last() == Some(&"") {
        parts.pop();
        if let Some(last) = parts.last_mut() {
            *last = "-";
        }
    }
    let (base, modifiers) = parts.split_last().ok_or_else(unknown)?;
    let (mut ctrl, mut alt, mut shift) = (false, false, false);
    for modifier in modifiers {
        match *modifier {
            "C" => ctrl = true,
            "A" => alt = true,
            "S" => shift = true,
            _ => return Err(unknown()),
        }
    }
    let key = match *base {
        "Enter" => Key::Enter,
        "Esc" => Key::Esc,
        "Tab" if shift && !ctrl && !alt => return Ok(Press::Key(Key::BackTab)),
        "Tab" => Key::Tab,
        "BS" => Key::Backspace,
        "Del" => Key::Delete,
        "Up" => Key::Up,
        "Down" => Key::Down,
        "Left" => Key::Left,
        "Right" => Key::Right,
        "Home" => Key::Home,
        "End" => Key::End,
        "PgUp" => Key::PageUp,
        "PgDn" => Key::PageDown,
        "Space" => Key::Char(' '),
        "lt" => Key::Char('<'),
        function if function.starts_with('F') => {
            let number = function[1..].parse::<u8>().map_err(|_| unknown())?;
            if !(1..=12).contains(&number) {
                return Err(unknown());
            }
            Key::F(number)
        }
        _ => {
            let mut chars = base.chars();
            let (Some(c), None) = (chars.next(), chars.next()) else {
                return Err(unknown());
            };
            return match (ctrl, alt, shift) {
                (true, false, false) if c.is_ascii_alphabetic() || " @[\\]^_?".contains(c) => {
                    Ok(Press::Key(Key::Ctrl(c)))
                }
                (false, true, false) => Ok(Press::Key(Key::Alt(c))),
                _ => Err(format!(
                    "<{name}> is not a key a terminal without the kitty keyboard protocol sends"
                )),
            };
        }
    };
    if !(ctrl || alt || shift) {
        return Ok(Press::Key(key));
    }
    if matches!(
        key,
        Key::Char(_) | Key::Enter | Key::Esc | Key::Tab | Key::Backspace
    ) {
        return Err(format!(
            "<{name}> is not a key a terminal without the kitty keyboard protocol sends"
        ));
    }
    let mut chord = match (ctrl, alt) {
        (true, _) => key.ctrl(),
        (false, true) => key.alt(),
        (false, false) => key.shift(),
    };
    if alt {
        chord = chord.alt();
    }
    if shift {
        chord = chord.shift();
    }
    Ok(Press::Chord(chord))
}
