//! Date calculation logic: the Windows calculator's "Date calculation" mode,
//! independent of the user interface.
//!
//! Ported from `DateCalculationEngine` and `DateCalculatorViewModel` in the
//! Windows Calculator source. Month and year steps clamp to the end of the
//! month (January 31 + 1 month = February 28), like the Windows calendar.

use chrono::{Datelike, Days, Months, NaiveDate, Weekday};

#[cfg(test)]
mod ms_cases;

/// Earliest supported date: the start of the Windows `FILETIME` epoch.
pub const MIN_DATE: NaiveDate = NaiveDate::from_ymd_opt(1601, 1, 1).unwrap();
/// Latest supported date.
pub const MAX_DATE: NaiveDate = NaiveDate::from_ymd_opt(9999, 12, 31).unwrap();
/// Largest value of the years, months and days offsets in "Add or subtract days".
pub const MAX_OFFSET: u32 = 999;

const DAYS_IN_WEEK: u32 = 7;

/// A difference between two dates, broken down from the largest unit down.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Difference {
    pub years: u32,
    pub months: u32,
    pub weeks: u32,
    pub days: u32,
}

/// Breaks the distance between two dates into years, months, weeks and days.
///
/// The order of the dates doesn't matter. Each unit takes as many whole steps
/// as fit before the later date, starting from where the previous unit ended:
/// first years, then months, then weeks, and the rest is days.
pub fn difference(a: NaiveDate, b: NaiveDate) -> Difference {
    let (start, end) = if a <= b { (a, b) } else { (b, a) };

    let years = max_steps(start, end, end.year() - start.year(), add_years);
    let pivot = add_years(start, years).unwrap_or(start);

    let month_estimate =
        (end.year() - pivot.year()) * 12 + end.month() as i32 - pivot.month() as i32;
    let months = max_steps(pivot, end, month_estimate, add_months);
    let pivot = add_months(pivot, months).unwrap_or(pivot);

    let rest = days_between(pivot, end);
    Difference {
        years,
        months,
        weeks: rest / DAYS_IN_WEEK,
        days: rest % DAYS_IN_WEEK,
    }
}

/// Number of days between two dates, in either order.
pub fn days_between(a: NaiveDate, b: NaiveDate) -> u32 {
    b.signed_duration_since(a).num_days().unsigned_abs() as u32
}

/// The largest `n` with `step(start, n) <= end`, searching down from `estimate`.
/// `estimate` is never too small: it counts calendar years or months, and the
/// clamped step can only land later in the month than `end`, not in a later one.
fn max_steps(
    start: NaiveDate,
    end: NaiveDate,
    estimate: i32,
    step: fn(NaiveDate, u32) -> Option<NaiveDate>,
) -> u32 {
    let mut n = estimate.max(0) as u32;
    while n > 0 && step(start, n).is_none_or(|date| date > end) {
        n -= 1;
    }
    n
}

fn add_years(date: NaiveDate, years: u32) -> Option<NaiveDate> {
    date.checked_add_months(Months::new(years.checked_mul(12)?))
}

fn add_months(date: NaiveDate, months: u32) -> Option<NaiveDate> {
    date.checked_add_months(Months::new(months))
}

/// Adds an offset to a date: years first, then months, then days.
/// Returns `None` ("Date out of Bound") past `MAX_DATE`.
pub fn add(date: NaiveDate, years: u32, months: u32, days: u32) -> Option<NaiveDate> {
    let date = add_years(date, years)?;
    let date = add_months(date, months)?;
    let date = date.checked_add_days(Days::new(days.into()))?;
    (date <= MAX_DATE).then_some(date)
}

/// Subtracts an offset from a date. Windows goes the other way round here:
/// days first, then months, then years. Returns `None` before `MIN_DATE`.
pub fn subtract(date: NaiveDate, years: u32, months: u32, days: u32) -> Option<NaiveDate> {
    let date = date.checked_sub_days(Days::new(days.into()))?;
    let date = date.checked_sub_months(Months::new(months))?;
    let date = date.checked_sub_months(Months::new(years.checked_mul(12)?))?;
    (date >= MIN_DATE).then_some(date)
}

// ------------------------------------------------------------- formatting

/// The words of the "Difference" result, in the interface's language. The UI takes
/// them from the Windows Calculator's translations; `ENGLISH` is the en-US text.
#[derive(Clone, Copy, Debug)]
pub struct Words<'a> {
    pub same_dates: &'a str,
    /// Each unit's singular and plural (`Date_Year` and `Date_Years`, …).
    pub year: [&'a str; 2],
    pub month: [&'a str; 2],
    pub week: [&'a str; 2],
    pub day: [&'a str; 2],
    /// Joins the units: Windows uses the region's list separator and a space.
    pub separator: &'a str,
}

pub const ENGLISH: Words<'static> = Words {
    same_dates: "Same dates",
    year: ["year", "years"],
    month: ["month", "months"],
    week: ["week", "weeks"],
    day: ["day", "days"],
    separator: ", ",
};

/// The "Difference" result: the main line and, for 7 days or more, a second
/// line with the total number of days.
pub fn describe_difference(a: NaiveDate, b: NaiveDate, words: &Words) -> (String, Option<String>) {
    let total = days_between(a, b);
    if total == 0 {
        return (words.same_dates.to_string(), None);
    }
    let diff = difference(a, b);
    let in_days = count(total, words.day);
    if diff.years == 0 && diff.months == 0 && diff.weeks == 0 {
        return (in_days, None);
    }
    let parts = [
        (diff.years, words.year),
        (diff.months, words.month),
        (diff.weeks, words.week),
        (diff.days, words.day),
    ];
    let text = parts
        .iter()
        .filter(|(value, _)| *value > 0)
        .map(|&(value, unit)| count(value, unit))
        .collect::<Vec<_>>()
        .join(words.separator);
    (text, Some(in_days))
}

/// `1 day`, `5 days`: singular only for exactly one, as in Windows (no plural rules).
fn count(value: u32, [one, many]: [&str; 2]) -> String {
    format!("{value} {}", if value == 1 { one } else { many })
}

/// Date as the date pickers show it: `October 15, 2026`.
pub fn format_date(date: NaiveDate) -> String {
    date.format("%B %-d, %Y").to_string()
}

/// Date as the "Add or subtract days" result shows it: `Thursday, October 15, 2026`.
pub fn format_long_date(date: NaiveDate) -> String {
    date.format("%A, %B %-d, %Y").to_string()
}

// -------------------------------------------------------------- calendar

/// First cell of a month page in the calendar: the first day of the week (`week_start`,
/// a regional setting) on or before the 1st. The page always shows 6 weeks (42 days)
/// from there.
pub fn month_page_start(year: i32, month: u32, week_start: Weekday) -> NaiveDate {
    let first = NaiveDate::from_ymd_opt(year, month, 1).expect("valid month");
    let days_back =
        (first.weekday().num_days_from_monday() + 7 - week_start.num_days_from_monday()) % 7;
    first - Days::new(days_back.into())
}

/// Keeps a date inside the supported range.
pub fn clamp(date: NaiveDate) -> NaiveDate {
    date.clamp(MIN_DATE, MAX_DATE)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(text: &str) -> NaiveDate {
        text.parse().unwrap()
    }

    fn describe(a: &str, b: &str) -> (String, Option<String>) {
        describe_difference(date(a), date(b), &ENGLISH)
    }

    fn diff(years: u32, months: u32, weeks: u32, days: u32) -> Difference {
        Difference {
            years,
            months,
            weeks,
            days,
        }
    }

    #[test]
    fn clamp_keeps_dates_inside_the_supported_range() {
        let inside = NaiveDate::from_ymd_opt(2026, 10, 17).unwrap();
        assert_eq!(clamp(inside), inside);
        assert_eq!(clamp(MIN_DATE), MIN_DATE);
        assert_eq!(clamp(MAX_DATE), MAX_DATE);
        assert_eq!(clamp(MIN_DATE.pred_opt().unwrap()), MIN_DATE);
        assert_eq!(clamp(MAX_DATE.succ_opt().unwrap()), MAX_DATE);
    }

    #[test]
    fn same_dates() {
        assert_eq!(
            describe("2026-10-15", "2026-10-15"),
            ("Same dates".into(), None)
        );
    }

    #[test]
    fn under_a_week_is_just_days() {
        assert_eq!(describe("2026-10-15", "2026-10-16"), ("1 day".into(), None));
        assert_eq!(
            describe("2026-10-15", "2026-10-20"),
            ("5 days".into(), None)
        );
    }

    #[test]
    fn a_week_or_more_adds_total_days() {
        assert_eq!(
            describe("2026-10-15", "2026-10-22"),
            ("1 week".into(), Some("7 days".into()))
        );
        assert_eq!(
            describe("2026-10-15", "2026-10-25"),
            ("1 week, 3 days".into(), Some("10 days".into()))
        );
        // 31 days in a leap February: one whole month, then 2 days, no weeks.
        assert_eq!(
            describe("2024-02-01", "2024-03-03"),
            ("1 month, 2 days".into(), Some("31 days".into()))
        );
    }

    #[test]
    fn reference_screenshot() {
        assert_eq!(
            describe("2022-06-24", "2026-10-28"),
            ("4 years, 4 months, 4 days".into(), Some("1587 days".into()))
        );
    }

    #[test]
    fn singular_units_and_zero_units_omitted() {
        assert_eq!(
            describe("2025-01-01", "2026-02-09"),
            (
                "1 year, 1 month, 1 week, 1 day".into(),
                Some("404 days".into())
            )
        );
        assert_eq!(describe("2026-01-15", "2026-11-15").0, "10 months");
    }

    #[test]
    fn direction_does_not_matter() {
        assert_eq!(
            describe("2026-10-28", "2022-06-24"),
            describe("2022-06-24", "2026-10-28")
        );
    }

    #[test]
    fn month_end_clamping() {
        // January 31 + 1 month = February 28, which is exactly the end date.
        assert_eq!(
            difference(date("2026-01-31"), date("2026-02-28")),
            diff(0, 1, 0, 0)
        );
        // February 29 + 1 year = February 28 of the next year.
        assert_eq!(
            difference(date("2024-02-29"), date("2025-02-28")),
            diff(1, 0, 0, 0)
        );
        // Months continue from the clamped pivot: Feb 28 2025 + 1 month = Mar 28.
        assert_eq!(
            difference(date("2024-02-29"), date("2025-03-29")),
            diff(1, 1, 0, 1)
        );
        assert_eq!(
            difference(date("2026-01-31"), date("2026-03-01")),
            diff(0, 1, 0, 1)
        );
    }

    #[test]
    fn whole_supported_range() {
        // 1601-01-01 + 8398 years + 11 months = 9999-12-01, then 30 days.
        assert_eq!(difference(MIN_DATE, MAX_DATE), diff(8398, 11, 4, 2));
        // 8399 years with 2036 leap years, minus one because both ends are dates.
        assert_eq!(days_between(MIN_DATE, MAX_DATE), 8399 * 365 + 2036 - 1);
    }

    #[test]
    fn add_goes_years_months_days() {
        assert_eq!(add(date("2026-01-31"), 0, 1, 0), Some(date("2026-02-28")));
        // Months first (Feb 28), then a day: March 1, not February 28.
        assert_eq!(add(date("2026-01-30"), 0, 1, 1), Some(date("2026-03-01")));
        // Years first (Feb 28 2025), then a month: March 28, not March 29.
        assert_eq!(add(date("2024-02-29"), 1, 1, 0), Some(date("2025-03-28")));
        assert_eq!(add(date("2026-10-15"), 0, 0, 0), Some(date("2026-10-15")));
    }

    #[test]
    fn subtract_goes_days_months_years() {
        // Days first (March 30), then a month: February 28.
        assert_eq!(
            subtract(date("2026-03-31"), 0, 1, 1),
            Some(date("2026-02-28"))
        );
        assert_eq!(
            subtract(date("2026-10-15"), 1, 2, 3),
            Some(date("2025-08-12"))
        );
    }

    #[test]
    fn out_of_bound() {
        assert_eq!(subtract(MIN_DATE, 0, 0, 1), None);
        assert_eq!(subtract(date("2026-10-15"), 999, 999, 999), None);
        assert_eq!(subtract(MIN_DATE, 0, 0, 0), Some(MIN_DATE));
        assert_eq!(add(MAX_DATE, 0, 0, 1), None);
        assert_eq!(add(date("9500-01-01"), 999, 0, 0), None);
        assert_eq!(add(MAX_DATE, 0, 0, 0), Some(MAX_DATE));
    }

    #[test]
    fn formats() {
        assert_eq!(format_date(date("2026-10-15")), "October 15, 2026");
        assert_eq!(format_date(date("2022-06-04")), "June 4, 2022");
        assert_eq!(
            format_long_date(date("2026-10-15")),
            "Thursday, October 15, 2026"
        );
    }

    #[test]
    fn calendar_page() {
        // October 2026 starts on a Thursday; the page starts on Sunday, September 27.
        assert_eq!(month_page_start(2026, 10, Weekday::Sun), date("2026-09-27"));
        // A month starting on Sunday starts its own page.
        assert_eq!(month_page_start(2026, 2, Weekday::Sun), date("2026-02-01"));
        // Regions whose weeks start on another day.
        assert_eq!(month_page_start(2026, 10, Weekday::Mon), date("2026-09-28"));
        assert_eq!(month_page_start(2026, 6, Weekday::Mon), date("2026-06-01"));
        assert_eq!(month_page_start(2026, 2, Weekday::Sat), date("2026-01-31"));
    }
}
