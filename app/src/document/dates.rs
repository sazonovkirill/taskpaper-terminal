//! The date expressions users type, such as `tomorrow`, `next fri` or `+3d`, resolved against a
//! given day ([search.md §4](../../../docs/specs/search.md#4-dates)), and the words that name a
//! date relative to a given day, such as `tomorrow` or `in 15 days` (D-46). Hides their grammar.

use std::vec;

use jiff::{
    Span, ToSpan,
    civil::{Date, ISOWeekDate, Weekday},
};
use thiserror::Error;

use super::metadata::{self, DateTag};

/// Why a text is not a date. Offsets are byte offsets into the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum DateError {
    /// The text is blank.
    #[error("no date entered")]
    Empty,
    /// The text does not follow the date grammar from `offset` on.
    #[error("not a date from position {offset}")]
    Unexpected {
        /// Where the first part that does not fit starts; the text's length when more was expected.
        offset: usize,
    },
    /// A written date names a day the calendar does not have, such as `02-30`.
    #[error("no such day at position {offset}")]
    NoSuchDay {
        /// Where the written date starts.
        offset: usize,
    },
    /// The date falls outside 0000-01-01 to 9999-12-31, the dates `YYYY-MM-DD` writes (D-6).
    #[error("the date at position {offset} is out of range")]
    OutOfRange {
        /// Where the part that left the range starts.
        offset: usize,
    },
}

/// Resolves `text` to a date relative to `today` for the `tag` date: a month and day already
/// past this year fall in the next year only for `@due`.
///
/// # Errors
///
/// A [`DateError`] saying why `text` is not a date.
pub fn parse(text: &str, today: Date, tag: DateTag) -> Result<Date, DateError> {
    let tokens = lex(text)?;
    if tokens.is_empty() {
        return Err(DateError::Empty);
    }
    Parser {
        tokens: tokens.into_iter(),
        end: text.len(),
        today,
        tag,
    }
    .expression()
}

/// `date` named relative to `today`: `today`, `tomorrow` or `yesterday`; the weekday, such as
/// `Tue`, 2 to 6 days ahead; `next Tue` 7 to 13 days ahead; `in 15 days` further ahead; and
/// `3 days ago` further back (D-46).
#[must_use]
pub fn relative(date: Date, today: Date) -> String {
    let days = (date - today).get_days();
    match days {
        0 => "today".to_owned(),
        1 => "tomorrow".to_owned(),
        -1 => "yesterday".to_owned(),
        2..=6 => date.strftime("%a").to_string(),
        7..=13 => format!("next {}", date.strftime("%a")),
        14.. => format!("in {days} days"),
        ..=-2 => format!("{} days ago", days.unsigned_abs()),
    }
}

/// The heading of a special view's group of entries for the day `date` on `today`: the day named
/// as [`relative`] names it, capitalized, then ` · ` and the date, such as
/// `3 days ago · Sun 4 Oct` (D-80, D-82).
#[must_use]
pub fn day_heading(date: Date, today: Date) -> String {
    let name = relative(date, today);
    let mut letters = name.chars();
    let capitalized: String = letters
        .next()
        .map(|first| first.to_uppercase().chain(letters).collect())
        .unwrap_or_default();
    format!("{capitalized} · {}", date.strftime("%a %-d %b"))
}

/// One part of a date expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind<'a> {
    /// A run of letters, such as `fri` or `weeks`.
    Word(&'a str),
    /// A run of ASCII digits.
    Number(&'a str),
    /// `+` or `-`.
    Sign(Sign),
    /// Digit runs joined by two `-`, such as `2026-10-05`.
    FullDate(&'a str),
    /// The digits of a month and a day joined by `-` or `/`.
    MonthDay(&'a str, &'a str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sign {
    Plus,
    Minus,
}

/// A part of the expression and the byte offset where it starts.
#[derive(Debug, Clone, Copy)]
struct Token<'a> {
    kind: Kind<'a>,
    offset: usize,
}

/// Splits `text` into tokens, ignoring whitespace between them.
fn lex(text: &str) -> Result<Vec<Token<'_>>, DateError> {
    let mut tokens = Vec::new();
    let mut rest = text.trim_start();
    while let Some(first) = rest.chars().next() {
        let offset = text.len() - rest.len();
        let (kind, after) = if first.is_alphabetic() {
            let (word, after) = split_run(rest, char::is_alphabetic);
            (Kind::Word(word), after)
        } else if first.is_ascii_digit() {
            numeric(rest)
        } else if let Some(after) = rest.strip_prefix('+') {
            (Kind::Sign(Sign::Plus), after)
        } else if let Some(after) = rest.strip_prefix('-') {
            (Kind::Sign(Sign::Minus), after)
        } else {
            return Err(DateError::Unexpected { offset });
        };
        tokens.push(Token { kind, offset });
        rest = after.trim_start();
    }
    Ok(tokens)
}

/// The leading run of `text` whose characters satisfy `belongs`, and the rest.
fn split_run(text: &str, belongs: impl Fn(char) -> bool) -> (&str, &str) {
    text.split_at(text.find(|c| !belongs(c)).unwrap_or(text.len()))
}

/// The number or written date that `text` starts with, and the rest.
fn numeric(text: &str) -> (Kind<'_>, &str) {
    let (first, rest) = split_run(text, |c| c.is_ascii_digit());
    let Some((separator, second, rest)) = digits_after(rest, &['-', '/']) else {
        return (Kind::Number(first), rest);
    };
    if separator == '-'
        && let Some((_, _, rest)) = digits_after(rest, &['-'])
    {
        let (full, _) = text.split_at(text.len() - rest.len());
        return (Kind::FullDate(full), rest);
    }
    (Kind::MonthDay(first, second), rest)
}

/// When `text` starts with one of `separators` followed by digits: the separator, the digits and
/// the rest.
fn digits_after<'a>(text: &'a str, separators: &[char]) -> Option<(char, &'a str, &'a str)> {
    let mut chars = text.chars();
    let separator = chars.next().filter(|c| separators.contains(c))?;
    let (digits, rest) = split_run(chars.as_str(), |c| c.is_ascii_digit());
    (!digits.is_empty()).then_some((separator, digits, rest))
}

/// Reads `base? offset*` from the tokens, where a missing base means today.
struct Parser<'a> {
    tokens: vec::IntoIter<Token<'a>>,
    /// The text's length, reported as the offset when a token is missing.
    end: usize,
    today: Date,
    tag: DateTag,
}

impl<'a> Parser<'a> {
    fn expression(mut self) -> Result<Date, DateError> {
        let mut date = self.today;
        let mut at_start = true;
        while let Some(token) = self.tokens.next() {
            let offset = token.offset;
            date = match token.kind {
                Kind::Sign(sign) => {
                    let number = self.next()?;
                    let Kind::Number(digits) = number.kind else {
                        return Err(DateError::Unexpected {
                            offset: number.offset,
                        });
                    };
                    self.shift(date, sign, digits, offset)?
                }
                Kind::Number(digits) => self.shift(date, Sign::Plus, digits, offset)?,
                _ if !at_start => return Err(DateError::Unexpected { offset }),
                Kind::FullDate(text) if !text.split('-').map(str::len).eq([4, 2, 2]) => {
                    return Err(DateError::Unexpected { offset });
                }
                Kind::FullDate(text) => {
                    text.parse().map_err(|_| DateError::NoSuchDay { offset })?
                }
                Kind::MonthDay(month, day) => self.month_day(month, day, offset)?,
                Kind::Word(word) => self.named(&word.to_ascii_lowercase(), offset)?,
            };
            if !metadata::writable(date) {
                return Err(DateError::OutOfRange { offset });
            }
            at_start = false;
        }
        Ok(date)
    }

    /// The next token; missing when the text ends early.
    fn next(&mut self) -> Result<Token<'a>, DateError> {
        self.tokens
            .next()
            .ok_or(DateError::Unexpected { offset: self.end })
    }

    /// The next token as a lower-case word, with its offset.
    fn word(&mut self) -> Result<(String, usize), DateError> {
        let token = self.next()?;
        let Kind::Word(word) = token.kind else {
            return Err(DateError::Unexpected {
                offset: token.offset,
            });
        };
        Ok((word.to_ascii_lowercase(), token.offset))
    }

    /// `date` moved by `digits` units in the direction of `sign`, reading the unit; `offset` is
    /// where the move starts.
    fn shift(
        &mut self,
        date: Date,
        sign: Sign,
        digits: &str,
        offset: usize,
    ) -> Result<Date, DateError> {
        let (unit, unit_offset) = self.word()?;
        let out_of_range = DateError::OutOfRange { offset };
        let amount: i64 = digits.parse().map_err(|_| out_of_range)?;
        let amount = match sign {
            Sign::Plus => amount,
            Sign::Minus => -amount,
        };
        // `m` is months, not TaskPaper's minutes, because dates have no time (D-6).
        let span = match unit.as_str() {
            "d" | "day" | "days" => Span::new().try_days(amount),
            "w" | "week" | "weeks" => Span::new().try_weeks(amount),
            "m" | "month" | "months" => Span::new().try_months(amount),
            "y" | "year" | "years" => Span::new().try_years(amount),
            _ => {
                return Err(DateError::Unexpected {
                    offset: unit_offset,
                });
            }
        };
        span.and_then(|span| date.checked_add(span))
            .map_err(|_| out_of_range)
    }

    /// The date that the lower-case `word` at `offset` starts, reading the words it needs.
    fn named(&mut self, word: &str, offset: usize) -> Result<Date, DateError> {
        let out_of_range = DateError::OutOfRange { offset };
        match word {
            "today" => Ok(self.today),
            "tomorrow" => self.today.tomorrow().map_err(|_| out_of_range),
            "yesterday" => self.today.yesterday().map_err(|_| out_of_range),
            "this" | "next" | "last" => {
                // The next such day is the first after today, so `next` skips one more.
                let (step, nth) = match word {
                    "next" => (1, Some(2)),
                    "last" => (-1, Some(-1)),
                    _ => (0, None),
                };
                let (target, target_offset) = self.word()?;
                match (weekday(&target), nth) {
                    (Some(weekday), Some(nth)) => self
                        .today
                        .nth_weekday(nth, weekday)
                        .map_err(|_| out_of_range),
                    _ => self.period_start(&target, target_offset, step, offset),
                }
            }
            _ => {
                let weekday = weekday(word).ok_or(DateError::Unexpected { offset })?;
                self.today.nth_weekday(1, weekday).map_err(|_| out_of_range)
            }
        }
    }
    /// The first day of the lower-case `period` at `period_offset`, `step` periods from the one
    /// holding today; `offset` is where the expression naming it starts. Weeks are ISO weeks.
    fn period_start(
        &self,
        period: &str,
        period_offset: usize,
        step: i64,
        offset: usize,
    ) -> Result<Date, DateError> {
        let (start, span) = match period {
            "week" => (
                self.today
                    .iso_week_date()
                    .first_of_week()
                    .map(ISOWeekDate::date),
                step.weeks(),
            ),
            "month" => (Ok(self.today.first_of_month()), step.months()),
            "year" => (Ok(self.today.first_of_year()), step.years()),
            _ => {
                return Err(DateError::Unexpected {
                    offset: period_offset,
                });
            }
        };
        start
            .and_then(|start| start.checked_add(span))
            .map_err(|_| DateError::OutOfRange { offset })
    }

    /// The day `month`-`day` written at `offset`, in this year or, for a due date already past,
    /// in the next.
    fn month_day(&self, month: &str, day: &str, offset: usize) -> Result<Date, DateError> {
        let no_such_day = DateError::NoSuchDay { offset };
        let (Ok(month), Ok(day)) = (month.parse(), day.parse()) else {
            return Err(no_such_day);
        };
        let on = |year| Date::new(year, month, day).map_err(|_| no_such_day);
        let this_year = on(self.today.year())?;
        if self.tag == DateTag::Due && this_year < self.today {
            let next_year = self
                .today
                .first_of_year()
                .checked_add(1.year())
                .map_err(|_| DateError::OutOfRange { offset })?;
            return on(next_year.year());
        }
        Ok(this_year)
    }
}

/// Full weekday names; the first three letters of each are its abbreviation.
const WEEKDAYS: [(&str, Weekday); 7] = [
    ("monday", Weekday::Monday),
    ("tuesday", Weekday::Tuesday),
    ("wednesday", Weekday::Wednesday),
    ("thursday", Weekday::Thursday),
    ("friday", Weekday::Friday),
    ("saturday", Weekday::Saturday),
    ("sunday", Weekday::Sunday),
];

/// The weekday the lower-case `word` names in full or abbreviated.
fn weekday(word: &str) -> Option<Weekday> {
    WEEKDAYS
        .into_iter()
        .find(|(name, _)| *name == word || name.get(..3) == Some(word))
        .map(|(_, weekday)| weekday)
}

#[cfg(test)]
mod tests {
    use jiff::civil::date;

    use super::*;

    /// A Wednesday in ISO week 41, whose Monday is 2026-10-05.
    const TODAY: Date = date(2026, 10, 7);

    #[track_caller]
    fn check_on(today: Date, text: &str, expected: Date) {
        for tag in [DateTag::Start, DateTag::Due] {
            assert_eq!(parse(text, today, tag), Ok(expected), "{text:?} as {tag:?}");
        }
    }

    #[track_caller]
    fn check(text: &str, expected: Date) {
        check_on(TODAY, text, expected);
    }

    #[track_caller]
    fn fails_on(today: Date, text: &str, expected: DateError) {
        for tag in [DateTag::Start, DateTag::Due] {
            assert_eq!(
                parse(text, today, tag),
                Err(expected),
                "{text:?} as {tag:?}"
            );
        }
    }

    #[track_caller]
    fn fails(text: &str, expected: DateError) {
        fails_on(TODAY, text, expected);
    }

    #[test]
    fn reads_full_dates() {
        check("2026-10-05", date(2026, 10, 5));
        check("1999-02-03", date(1999, 2, 3));
        check("2028-02-29", date(2028, 2, 29));
        check("  2030-12-31  ", date(2030, 12, 31));
    }

    #[test]
    fn rejects_full_dates_the_calendar_lacks_or_not_written_in_full() {
        fails("2026-02-30", DateError::NoSuchDay { offset: 0 });
        fails("2026-13-01", DateError::NoSuchDay { offset: 0 });
        fails(" 2027-02-29", DateError::NoSuchDay { offset: 1 });
        // search.md §4: a full date is `YYYY-MM-DD`; other digit counts are not the form, whether
        // or not the day they mean exists.
        fails("2026-1-5", DateError::Unexpected { offset: 0 });
        fails("2026-12-2", DateError::Unexpected { offset: 0 });
        fails("226-10-05", DateError::Unexpected { offset: 0 });
        fails(" 20260-10-05", DateError::Unexpected { offset: 1 });
        fails("2026-10-005", DateError::Unexpected { offset: 0 });
    }

    #[test]
    fn reads_month_and_day_in_this_year() {
        check("12-25", date(2026, 12, 25));
        check("12/25", date(2026, 12, 25));
        check("10-07", TODAY);
        check("10/8", date(2026, 10, 8));
    }

    #[test]
    fn a_past_month_and_day_is_next_year_only_for_due() {
        for text in ["3/7", "03-07"] {
            assert_eq!(parse(text, TODAY, DateTag::Start), Ok(date(2026, 3, 7)));
            assert_eq!(parse(text, TODAY, DateTag::Due), Ok(date(2027, 3, 7)));
        }
        assert_eq!(parse("10-06", TODAY, DateTag::Due), Ok(date(2027, 10, 6)));
        assert_eq!(parse("10-07", TODAY, DateTag::Due), Ok(TODAY));
    }

    #[test]
    fn rejects_month_and_day_the_calendar_lacks() {
        fails("02-30", DateError::NoSuchDay { offset: 0 });
        fails("13/01", DateError::NoSuchDay { offset: 0 });
        fails("00-10", DateError::NoSuchDay { offset: 0 });
        fails("300-1", DateError::NoSuchDay { offset: 0 });
        fails("10 /1", DateError::Unexpected { offset: 3 });
        fails("02-29", DateError::NoSuchDay { offset: 0 });
        check_on(date(2028, 1, 10), "02-29", date(2028, 2, 29));
        assert_eq!(
            parse("02-29", date(2028, 3, 1), DateTag::Due),
            Err(DateError::NoSuchDay { offset: 0 }),
            "2029 has no February 29"
        );
    }

    #[test]
    fn reads_today_tomorrow_and_yesterday_in_any_case() {
        check("today", TODAY);
        check("tomorrow", date(2026, 10, 8));
        check("yesterday", date(2026, 10, 6));
        check("ToDay", TODAY);
        check("TOMORROW", date(2026, 10, 8));
        check_on(date(2026, 12, 31), "tomorrow", date(2027, 1, 1));
        check_on(date(2027, 1, 1), "yesterday", date(2026, 12, 31));
    }

    #[test]
    fn a_weekday_is_the_next_such_day_after_today() {
        for (names, expected) in [
            (["mon", "monday"], date(2026, 10, 12)),
            (["tue", "tuesday"], date(2026, 10, 13)),
            (["wed", "wednesday"], date(2026, 10, 14)),
            (["thu", "thursday"], date(2026, 10, 8)),
            (["fri", "friday"], date(2026, 10, 9)),
            (["sat", "saturday"], date(2026, 10, 10)),
            (["sun", "sunday"], date(2026, 10, 11)),
        ] {
            for name in names {
                check(name, expected);
                check(&name.to_uppercase(), expected);
            }
        }
        check_on(date(2026, 12, 30), "fri", date(2027, 1, 1));
    }

    #[test]
    fn next_and_last_weekday_step_a_week_further_or_back() {
        check("next fri", date(2026, 10, 16));
        check("Next Wednesday", date(2026, 10, 21));
        check("last fri", date(2026, 10, 2));
        check("last wed", date(2026, 9, 30));
        check("last thu", date(2026, 10, 1));
        check_on(date(2027, 1, 1), "last fri", date(2026, 12, 25));
    }

    #[test]
    fn rejects_other_weekday_spellings() {
        fails("fr", DateError::Unexpected { offset: 0 });
        fails("fridays", DateError::Unexpected { offset: 0 });
        fails("this fri", DateError::Unexpected { offset: 5 });
        fails("next", DateError::Unexpected { offset: 4 });
        fails("last soon", DateError::Unexpected { offset: 5 });
        fails("next 3", DateError::Unexpected { offset: 5 });
    }

    #[test]
    fn this_next_and_last_period_are_its_first_day() {
        check("this week", date(2026, 10, 5));
        check("next week", date(2026, 10, 12));
        check("last week", date(2026, 9, 28));
        check("this month", date(2026, 10, 1));
        check("next month", date(2026, 11, 1));
        check("last month", date(2026, 9, 1));
        check("this year", date(2026, 1, 1));
        check("next year", date(2027, 1, 1));
        check("LAST YEAR", date(2025, 1, 1));
    }

    #[test]
    fn weeks_are_iso_weeks_starting_on_monday() {
        check_on(date(2026, 10, 5), "this week", date(2026, 10, 5));
        check_on(date(2026, 10, 11), "this week", date(2026, 10, 5));
        check_on(date(2026, 10, 12), "last week", date(2026, 10, 5));
        check_on(date(2027, 1, 1), "this week", date(2026, 12, 28));
        check_on(date(2027, 1, 1), "next week", date(2027, 1, 4));
        check_on(date(2026, 12, 15), "next month", date(2027, 1, 1));
        check_on(date(2027, 1, 15), "last month", date(2026, 12, 1));
    }

    #[test]
    fn offsets_apply_to_today() {
        check("+3d", date(2026, 10, 10));
        check("3d", date(2026, 10, 10));
        check("-1d", date(2026, 10, 6));
        check("+1w", date(2026, 10, 14));
        check("2 weeks", date(2026, 10, 21));
        check("-1 month", date(2026, 9, 7));
        check("+1m", date(2026, 11, 7));
        check("+1y", date(2027, 10, 7));
        check("1 day", date(2026, 10, 8));
        check("2 days", date(2026, 10, 9));
        check("1 week", date(2026, 10, 14));
        check("1 months", date(2026, 11, 7));
        check("1 year", date(2027, 10, 7));
        check("3 years", date(2029, 10, 7));
        check("- 1 W", date(2026, 9, 30));
        check("+ 3 D", date(2026, 10, 10));
        check("0d", TODAY);
    }

    #[test]
    fn offsets_apply_to_a_preceding_date_in_order() {
        check("fri +1w", date(2026, 10, 16));
        check("today+1d", date(2026, 10, 8));
        check("next week -1d", date(2026, 10, 11));
        check("+1w +2d", date(2026, 10, 16));
        check("2026-01-31 +1m -1d", date(2026, 2, 27));
        check("12/25 +1y", date(2027, 12, 25));
    }

    #[test]
    fn month_and_year_offsets_clamp_to_the_end_of_the_month() {
        check("2026-01-31 +1m", date(2026, 2, 28));
        check("2028-01-31 +1m", date(2028, 2, 29));
        check("2026-03-31 -1m", date(2026, 2, 28));
        check("2028-02-29 +1y", date(2029, 2, 28));
        check("2026-12-31 +1d", date(2027, 1, 1));
        check("2026-12-31 +1w", date(2027, 1, 7));
    }

    #[test]
    fn rejects_incomplete_or_misplaced_offsets() {
        fails("+3", DateError::Unexpected { offset: 2 });
        fails("+d", DateError::Unexpected { offset: 1 });
        fails("3 parsecs", DateError::Unexpected { offset: 2 });
        fails("3 3d", DateError::Unexpected { offset: 2 });
        fails("+ +3d", DateError::Unexpected { offset: 2 });
        fails("+1w fri", DateError::Unexpected { offset: 4 });
        fails("today tomorrow", DateError::Unexpected { offset: 6 });
        fails("2026-10-05-", DateError::Unexpected { offset: 11 });
    }

    #[test]
    fn rejects_blank_text() {
        fails("", DateError::Empty);
        fails(" \t ", DateError::Empty);
    }

    #[test]
    fn rejects_unknown_words_and_symbols() {
        fails("soon", DateError::Unexpected { offset: 0 });
        fails("fri ?", DateError::Unexpected { offset: 4 });
        fails("día", DateError::Unexpected { offset: 0 });
        fails("tomorrow é", DateError::Unexpected { offset: 9 });
        fails("@due", DateError::Unexpected { offset: 0 });
    }

    #[test]
    fn rejects_dates_out_of_range() {
        fails(
            "+99999999999999999999d",
            DateError::OutOfRange { offset: 0 },
        );
        fails("today +20000y", DateError::OutOfRange { offset: 6 });
        fails("9999-12-31 +1d", DateError::OutOfRange { offset: 11 });
        fails_on(Date::MAX, "tomorrow", DateError::OutOfRange { offset: 0 });
        fails_on(Date::MIN, "yesterday", DateError::OutOfRange { offset: 0 });
        // D-6: dates are stored as YYYY-MM-DD, which has no room for a year before 0.
        check("0000-01-01", date(0, 1, 1));
        fails_on(
            date(0, 1, 1),
            "yesterday",
            DateError::OutOfRange { offset: 0 },
        );
        fails("0000-01-01 -1d +1d", DateError::OutOfRange { offset: 11 });
        fails("-2027y", DateError::OutOfRange { offset: 0 });
        fails_on(Date::MAX, "fri", DateError::OutOfRange { offset: 0 });
        fails_on(Date::MAX, "next fri", DateError::OutOfRange { offset: 0 });
        fails_on(Date::MIN, "last fri", DateError::OutOfRange { offset: 0 });
        fails_on(Date::MAX, " next year", DateError::OutOfRange { offset: 1 });
        fails_on(Date::MAX, "next week", DateError::OutOfRange { offset: 0 });
        fails_on(Date::MIN, "last month", DateError::OutOfRange { offset: 0 });
        assert_eq!(
            parse("12-25", Date::MAX, DateTag::Due),
            Err(DateError::OutOfRange { offset: 0 })
        );
    }

    // D-46
    #[test]
    fn relative_names_today_and_its_neighbors_in_words() {
        assert_eq!(relative(TODAY, TODAY), "today");
        assert_eq!(relative(date(2026, 10, 8), TODAY), "tomorrow");
        assert_eq!(relative(date(2026, 10, 6), TODAY), "yesterday");
    }

    // D-46
    #[test]
    fn relative_names_the_weekday_of_the_coming_week_then_the_next() {
        let names: Vec<String> = (9..=20)
            .map(|day| relative(date(2026, 10, day), TODAY))
            .collect();
        assert_eq!(
            names,
            [
                "Fri", "Sat", "Sun", "Mon", "Tue", "next Wed", "next Thu", "next Fri", "next Sat",
                "next Sun", "next Mon", "next Tue",
            ]
        );
    }

    // D-46
    #[test]
    fn relative_counts_days_further_ahead_and_back() {
        assert_eq!(relative(date(2026, 10, 21), TODAY), "in 14 days");
        assert_eq!(relative(date(2026, 10, 22), TODAY), "in 15 days");
        assert_eq!(relative(date(2027, 10, 7), TODAY), "in 365 days");
        assert_eq!(relative(date(2026, 10, 5), TODAY), "2 days ago");
        assert_eq!(relative(date(2025, 10, 7), TODAY), "365 days ago");
        assert_eq!(relative(Date::MAX, Date::MIN), "in 7304483 days");
        assert_eq!(relative(Date::MIN, Date::MAX), "7304483 days ago");
    }

    // D-80, D-82
    #[test]
    fn a_day_heading_names_the_day_capitalized_then_the_date() {
        assert_eq!(day_heading(TODAY, TODAY), "Today · Wed 7 Oct");
        assert_eq!(
            day_heading(date(2026, 10, 6), TODAY),
            "Yesterday · Tue 6 Oct"
        );
        assert_eq!(
            day_heading(date(2026, 10, 4), TODAY),
            "3 days ago · Sun 4 Oct"
        );
        assert_eq!(
            day_heading(date(2026, 10, 8), TODAY),
            "Tomorrow · Thu 8 Oct"
        );
        assert_eq!(day_heading(date(2026, 10, 9), TODAY), "Fri · Fri 9 Oct");
        assert_eq!(
            day_heading(date(2026, 10, 14), TODAY),
            "Next Wed · Wed 14 Oct"
        );
        assert_eq!(
            day_heading(date(2025, 12, 25), TODAY),
            "286 days ago · Thu 25 Dec"
        );
    }

    #[test]
    fn errors_say_where_the_text_stops_being_a_date() {
        assert_eq!(
            DateError::Unexpected { offset: 4 }.to_string(),
            "not a date from position 4"
        );
        assert_eq!(DateError::Empty.to_string(), "no date entered");
    }
}
