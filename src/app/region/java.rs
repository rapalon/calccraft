//! The formats Java's locale classes give, turned into `Formats`. The Android app reads them
//! through JNI (`android::region_source`, `docs/regional-formats.md`, "Android"); this module
//! is pure, so the host tests cover everything but the calls themselves: how the separators
//! and the grouping are read off a formatted number, Java's first day of the week, the
//! calendar's names, and a formatter for the Java date patterns
//! `getBestDateTimePattern` returns (`d. MMMM y`, `EEEE, d MMMM y 'г'.`).
#![cfg_attr(not(target_os = "android"), allow(dead_code))]

use chrono::{Datelike, NaiveDate, Weekday};

use super::{DateFormats, Formats};
use crate::locale::NumberFormat;

/// The number the separators and the grouping are read from: thirteen integer digits show a
/// secondary group size (India's `12,34,56,78,90,123.5`), and the fraction the decimal
/// separator.
pub(crate) const SAMPLE: f64 = 1234567890123.5;

/// The skeletons `DateFormat.getBestDateTimePattern` turns into the region's patterns: the
/// date pickers' date, the add/subtract result's long date, and the calendar's month and year.
pub(crate) const PICKER_SKELETON: &str = "yMMMMd";
pub(crate) const LONG_SKELETON: &str = "yMMMMEEEEd";
pub(crate) const YEAR_MONTH_SKELETON: &str = "yMMMM";

/// ICU `DateFormatSymbols`' contexts and widths (`getMonths(context, width)`,
/// `getWeekdays(context, width)`).
pub(crate) const FORMAT: i32 = 0;
pub(crate) const STANDALONE: i32 = 1;
pub(crate) const ABBREVIATED: i32 = 0;
pub(crate) const WIDE: i32 = 1;
pub(crate) const SHORT: i32 = 3;

/// What Java says about a region, as `android::region_source` reads it.
pub(crate) struct Source {
    /// `Locale.toLanguageTag()`: the region's tag, with the phone's extensions
    /// (`en-US-u-fw-mon` for a week starting on Monday).
    pub tag: String,
    /// `NumberFormat.getInstance(locale).format(SAMPLE)`.
    pub number: String,
    /// `Calendar.getInstance(locale).getFirstDayOfWeek()`: 1 is Sunday, 7 Saturday.
    pub first_day: i32,
    /// `DateFormatSymbols.getMonths(FORMAT, WIDE)`: the names a date is written with
    /// (`октября`).
    pub months: Vec<String>,
    /// `getMonths(STANDALONE, WIDE)`: the names on their own (`октябрь`).
    pub months_standalone: Vec<String>,
    /// `getMonths(FORMAT, ABBREVIATED)` and `getMonths(STANDALONE, ABBREVIATED)`.
    pub short_months: Vec<String>,
    pub short_months_standalone: Vec<String>,
    /// `getWeekdays(FORMAT, WIDE)`: 8 entries, the first empty, then Sunday.
    pub weekdays: Vec<String>,
    /// `getWeekdays(FORMAT, ABBREVIATED)`.
    pub short_weekdays: Vec<String>,
    /// `getWeekdays(STANDALONE, SHORT)` (`Su`, `Mo`), or the abbreviated names where ICU has
    /// no short ones.
    pub shortest_weekdays: Vec<String>,
    /// The patterns of the three skeletons.
    pub picker: String,
    pub long: String,
    pub year_month: String,
}

/// The tag with the Gregorian calendar and Latin digits, as the web version asks `Intl`: the
/// date mode counts Gregorian years, and the engine writes Latin digits. A `ca` or `nu`
/// keyword the tag has already (Android's regional preferences add theirs, like `fw`) is
/// replaced, the other keywords stay.
pub(crate) fn gregorian(tag: &str) -> String {
    let parts: Vec<&str> = tag.split('-').collect();
    let Some(u) = parts.iter().position(|part| *part == "u") else {
        return format!("{tag}-u-ca-gregory-nu-latn");
    };
    let mut out: Vec<&str> = parts[..=u].to_vec();
    let mut i = u + 1;
    let mut skip = false;
    // The extension's keywords (two letters, each followed by its values) up to the next
    // singleton, which starts another extension.
    while i < parts.len() && parts[i].len() > 1 {
        if parts[i].len() == 2 {
            skip = matches!(parts[i], "ca" | "nu");
        }
        if !skip {
            out.push(parts[i]);
        }
        i += 1;
    }
    out.extend(["ca", "gregory", "nu", "latn"]);
    out.extend(&parts[i..]);
    out.join("-")
}

/// The decimal and group separators and the digit grouping of a region, read off `SAMPLE` as
/// Java wrote it: `1,234,567,890,123.5` groups by 3 (`3;0`), India's `12,34,56,78,90,123.5` by
/// 3 and then 2 (`3;2;0`), and `1234567890123,5` doesn't group. The digits are whatever
/// `char::is_numeric` says, so a region's own digits work too.
pub(crate) fn number_format(sample: &str) -> Option<NumberFormat> {
    // Runs of digits and the separators between them.
    let mut digit_runs: Vec<usize> = Vec::new();
    let mut separators: Vec<String> = Vec::new();
    let mut digits = 0;
    let mut separator = String::new();
    for c in sample.chars() {
        if c.is_numeric() {
            if digits == 0 && !digit_runs.is_empty() {
                // The characters between two runs of digits; anything before the first
                // digits (a direction mark) is dropped.
                separators.push(std::mem::take(&mut separator));
            }
            separator.clear();
            digits += 1;
        } else {
            if digits > 0 {
                digit_runs.push(digits);
                digits = 0;
            }
            separator.push(c);
        }
    }
    if digits > 0 {
        digit_runs.push(digits);
    }
    // The fraction `5` follows the decimal separator; the runs before it are the integer's
    // groups, the leftmost one incomplete.
    let (fraction, integer_runs) = digit_runs.split_last()?;
    if *fraction != 1 || separators.len() != integer_runs.len() {
        return None;
    }
    let decimal = separators.last()?;
    let group = separators.first().filter(|_| separators.len() > 1);
    let mut sizes: Vec<usize> = integer_runs.to_vec();
    sizes.reverse();
    sizes.pop();
    let grouping = match sizes.as_slice() {
        [first, second, ..] if first != second => format!("{first};{second};0"),
        [first, ..] => format!("{first};0"),
        [] => String::new(),
    };
    Some(NumberFormat::new(
        decimal,
        group.map_or("", String::as_str),
        &grouping,
    ))
}

/// Java's first day of the week (`Calendar.SUNDAY` is 1, `SATURDAY` 7) as chrono's.
pub(crate) fn first_day(java: i32) -> Weekday {
    match java {
        2 => Weekday::Mon,
        3 => Weekday::Tue,
        4 => Weekday::Wed,
        5 => Weekday::Thu,
        6 => Weekday::Fri,
        7 => Weekday::Sat,
        _ => Weekday::Sun,
    }
}

/// The region of a tag (`DE` for `de-DE`, `IN` for `hi-IN-u-fw-mon`), for the converters'
/// default units. The keys of an extension (`fw`) are two letters too, so the search stops at
/// the singleton that starts one.
pub(crate) fn country(tag: &str) -> Option<String> {
    tag.split(['-', '_'])
        .skip(1)
        .take_while(|part| part.len() > 1)
        .find(|part| {
            (part.len() == 2 && part.chars().all(|c| c.is_ascii_alphabetic()))
                || (part.len() == 3 && part.chars().all(|c| c.is_ascii_digit()))
        })
        .map(str::to_ascii_uppercase)
}

/// The names a Java date pattern needs, Sunday first for the weekdays.
pub(super) struct Names {
    months: [String; 12],
    months_standalone: [String; 12],
    short_months: [String; 12],
    short_months_standalone: [String; 12],
    weekdays: [String; 7],
    short_weekdays: [String; 7],
}

/// A region's date formats: Java's patterns for the three skeletons and the names they use.
pub(super) struct Dates {
    names: Names,
    picker: String,
    long: String,
    year_month: String,
}

impl Dates {
    /// The date pickers' date (`15. Oktober 2026`).
    pub fn picker(&self, date: NaiveDate) -> String {
        format(&self.picker, date, &self.names)
    }

    /// The long date (`Donnerstag, 15. Oktober 2026`).
    pub fn long(&self, date: NaiveDate) -> String {
        format(&self.long, date, &self.names)
    }

    /// The calendar's title (`Oktober 2026`, `октябрь 2026 г.`).
    pub fn year_month(&self, date: NaiveDate) -> String {
        format(&self.year_month, date, &self.names)
    }
}

/// A date in a Java date pattern (`SimpleDateFormat`'s letters, as `getBestDateTimePattern`
/// writes them): `y` (two digits for `yy`), `M` and `L` (the number, two digits, the
/// abbreviated or the wide name, in the format or the standalone context), `d`, `E` and `c`
/// (the abbreviated or the wide weekday), and quoted text (`''` is an apostrophe). Any other
/// letter is left out, with the spaces around it: `G`, the era, which th-TH's Gregorian
/// pattern has (`d MMMM G y`). Unlike `SimpleDateFormat` it never throws.
pub(super) fn format(pattern: &str, date: NaiveDate, names: &Names) -> String {
    let chars: Vec<char> = pattern.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\'' {
            if chars.get(i + 1) == Some(&'\'') {
                out.push('\'');
                i += 2;
                continue;
            }
            i += 1;
            while i < chars.len() {
                if chars[i] == '\'' {
                    if chars.get(i + 1) == Some(&'\'') {
                        out.push('\'');
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                out.push(chars[i]);
                i += 1;
            }
            continue;
        }
        if c.is_ascii_alphabetic() {
            let count = chars[i..].iter().take_while(|&&d| d == c).count();
            field(&mut out, c, count, date, names);
            i += count;
            continue;
        }
        out.push(c);
        i += 1;
    }
    // A left-out field leaves two spaces; a leading or trailing one stays otherwise.
    let mut text = String::with_capacity(out.len());
    for c in out.chars() {
        if c == ' ' && text.ends_with(' ') {
            continue;
        }
        text.push(c);
    }
    text.trim_matches(' ').to_string()
}

/// One field of a pattern: `letter` repeated `count` times.
fn field(out: &mut String, letter: char, count: usize, date: NaiveDate, names: &Names) {
    let month = date.month0() as usize;
    let weekday = date.weekday().num_days_from_sunday() as usize;
    match letter {
        'y' if count == 2 => out.push_str(&format!("{:02}", date.year().rem_euclid(100))),
        'y' => out.push_str(&format!("{:0count$}", date.year())),
        'M' | 'L' => {
            let standalone = letter == 'L';
            match count {
                1 => out.push_str(&date.month().to_string()),
                2 => out.push_str(&format!("{:02}", date.month())),
                3 if standalone => out.push_str(&names.short_months_standalone[month]),
                3 => out.push_str(&names.short_months[month]),
                _ if standalone => out.push_str(&names.months_standalone[month]),
                _ => out.push_str(&names.months[month]),
            }
        }
        'd' if count == 1 => out.push_str(&date.day().to_string()),
        'd' => out.push_str(&format!("{:02}", date.day())),
        'E' | 'c' | 'e' if count <= 3 => out.push_str(&names.short_weekdays[weekday]),
        'E' | 'c' | 'e' => out.push_str(&names.weekdays[weekday]),
        _ => {}
    }
}

/// Twelve month names, or `None` where Java gave another number.
fn twelve(names: Vec<String>) -> Option<[String; 12]> {
    names.try_into().ok()
}

/// Seven weekday names, Sunday first, from Java's array of 8 (the first entry is empty).
fn seven(mut names: Vec<String>) -> Option<[String; 7]> {
    if names.len() == 8 {
        names.remove(0);
    }
    names.try_into().ok()
}

/// A calendar header's name from a short weekday name: without a closing period (`So.`),
/// at most two characters, the first in uppercase (`Пн`, `Lu`, as Windows' shortest names).
fn shortest(name: &str) -> String {
    let mut chars = name.trim_end_matches('.').chars().take(2);
    let mut out: String = chars
        .next()
        .map(char::to_uppercase)
        .into_iter()
        .flatten()
        .collect();
    out.extend(chars);
    out
}

/// A region's formats from what Java said, or `None` where a name list has the wrong length
/// or the number can't be read: the app then uses en-US.
pub(super) fn formats(source: Source) -> Option<Formats> {
    let numbers = number_format(&source.number)?;
    // Java has no list separator: Windows' is `;` where the decimal separator is a comma.
    let list = if numbers.decimal == "," { ";" } else { "," };
    let names = Names {
        months: twelve(source.months)?,
        months_standalone: twelve(source.months_standalone)?,
        short_months: twelve(source.short_months)?,
        short_months_standalone: twelve(source.short_months_standalone)?,
        weekdays: seven(source.weekdays)?,
        short_weekdays: seven(source.short_weekdays)?,
    };
    let months = names
        .short_months_standalone
        .clone()
        .map(|name| name.trim_end_matches('.').to_string());
    let weekdays = seven(source.shortest_weekdays)?.map(|name| shortest(&name));
    Some(Formats {
        tag: source.tag,
        numbers,
        list_separator: format!("{list} "),
        first_day: first_day(source.first_day),
        weekdays,
        months,
        dates: DateFormats::Java(Box::new(Dates {
            names,
            picker: source.picker,
            long: source.long,
            year_month: source.year_month,
        })),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list<const N: usize>(names: &[&str; N]) -> [String; N] {
        names.map(str::to_string)
    }

    fn english() -> Names {
        let months = list(&[
            "January",
            "February",
            "March",
            "April",
            "May",
            "June",
            "July",
            "August",
            "September",
            "October",
            "November",
            "December",
        ]);
        let short_months = list(&[
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ]);
        Names {
            months_standalone: months.clone(),
            months,
            short_months_standalone: short_months.clone(),
            short_months,
            weekdays: list(&[
                "Sunday",
                "Monday",
                "Tuesday",
                "Wednesday",
                "Thursday",
                "Friday",
                "Saturday",
            ]),
            short_weekdays: list(&["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]),
        }
    }

    /// What Android's ICU gives for de-DE, with the 8-entry weekday arrays.
    fn german() -> Source {
        let months = [
            "Januar",
            "Februar",
            "März",
            "April",
            "Mai",
            "Juni",
            "Juli",
            "August",
            "September",
            "Oktober",
            "November",
            "Dezember",
        ];
        let short = [
            "Jan.", "Feb.", "März", "Apr.", "Mai", "Juni", "Juli", "Aug.", "Sept.", "Okt.", "Nov.",
            "Dez.",
        ];
        let short_standalone = [
            "Jan", "Feb", "Mär", "Apr", "Mai", "Jun", "Jul", "Aug", "Sep", "Okt", "Nov", "Dez",
        ];
        let strings = |names: &[&str]| names.iter().map(|name| name.to_string()).collect();
        Source {
            tag: "de-DE".into(),
            number: "1.234.567.890.123,5".into(),
            first_day: 2,
            months: strings(&months),
            months_standalone: strings(&months),
            short_months: strings(&short),
            short_months_standalone: strings(&short_standalone),
            weekdays: strings(&[
                "",
                "Sonntag",
                "Montag",
                "Dienstag",
                "Mittwoch",
                "Donnerstag",
                "Freitag",
                "Samstag",
            ]),
            short_weekdays: strings(&["", "So.", "Mo.", "Di.", "Mi.", "Do.", "Fr.", "Sa."]),
            shortest_weekdays: strings(&["", "So.", "Mo.", "Di.", "Mi.", "Do.", "Fr.", "Sa."]),
            picker: "d. MMMM y".into(),
            long: "EEEE, d. MMMM y".into(),
            year_month: "MMMM y".into(),
        }
    }

    /// What Android's ICU gives for ru-RU: the format names are genitive (`октября`), the
    /// standalone ones nominative (`октябрь`), the short weekday names lowercase.
    fn russian() -> Source {
        let strings = |names: &[&str]| names.iter().map(|name| name.to_string()).collect();
        Source {
            tag: "ru-RU".into(),
            number: "1\u{a0}234\u{a0}567\u{a0}890\u{a0}123,5".into(),
            first_day: 2,
            months: strings(&[
                "января",
                "февраля",
                "марта",
                "апреля",
                "мая",
                "июня",
                "июля",
                "августа",
                "сентября",
                "октября",
                "ноября",
                "декабря",
            ]),
            months_standalone: strings(&[
                "январь",
                "февраль",
                "март",
                "апрель",
                "май",
                "июнь",
                "июль",
                "август",
                "сентябрь",
                "октябрь",
                "ноябрь",
                "декабрь",
            ]),
            short_months: strings(&[
                "янв.",
                "февр.",
                "мар.",
                "апр.",
                "мая",
                "июн.",
                "июл.",
                "авг.",
                "сент.",
                "окт.",
                "нояб.",
                "дек.",
            ]),
            short_months_standalone: strings(&[
                "янв.",
                "февр.",
                "март",
                "апр.",
                "май",
                "июнь",
                "июль",
                "авг.",
                "сент.",
                "окт.",
                "нояб.",
                "дек.",
            ]),
            weekdays: strings(&[
                "",
                "воскресенье",
                "понедельник",
                "вторник",
                "среда",
                "четверг",
                "пятница",
                "суббота",
            ]),
            short_weekdays: strings(&["", "вс", "пн", "вт", "ср", "чт", "пт", "сб"]),
            shortest_weekdays: strings(&["", "вс", "пн", "вт", "ср", "чт", "пт", "сб"]),
            picker: "d MMMM y 'г'.".into(),
            long: "EEEE, d MMMM y 'г'.".into(),
            year_month: "LLLL y 'г'.".into(),
        }
    }

    fn october_15() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 15).unwrap()
    }

    #[test]
    fn separators_and_grouping_are_read_off_the_sample() {
        let en = number_format("1,234,567,890,123.5").unwrap();
        assert_eq!(en, NumberFormat::default());
        let de = number_format("1.234.567.890.123,5").unwrap();
        assert_eq!(de.localize("1,234,567.5"), "1.234.567,5");
        assert_eq!(de.grouping, [3, 0]);
        let india = number_format("12,34,56,78,90,123.5").unwrap();
        assert_eq!(india.grouping, [3, 2, 0]);
        assert_eq!(india.localize("123,456,789.25"), "12,34,56,789.25");
        let ru = number_format("1\u{a0}234\u{a0}567\u{a0}890\u{a0}123,5").unwrap();
        assert_eq!(ru.localize("1,234,567.5"), "1\u{a0}234\u{a0}567,5");
        let swiss = number_format("1’234’567’890’123.5").unwrap();
        assert_eq!(swiss.localize("1,234,567.5"), "1’234’567.5");
        // No grouping at all.
        let plain = number_format("1234567890123,5").unwrap();
        assert_eq!(plain.group, "");
        assert_eq!(plain.localize("1,234,567.5"), "1234567,5");
        // A region's own digits (Arabic-Indic) are digits too.
        let arabic = number_format("١٬٢٣٤٬٥٦٧٬٨٩٠٬١٢٣٫٥").unwrap();
        assert_eq!(arabic.decimal, "٫");
        assert_eq!(arabic.group, "٬");
        // Text that isn't the sample.
        assert!(number_format("").is_none());
        assert!(number_format("1234567890123").is_none());
        assert!(number_format("NaN").is_none());
    }

    #[test]
    fn first_days() {
        assert_eq!(first_day(1), Weekday::Sun);
        assert_eq!(first_day(2), Weekday::Mon);
        assert_eq!(first_day(7), Weekday::Sat);
        assert_eq!(first_day(0), Weekday::Sun);
    }

    #[test]
    fn gregorian_tags() {
        assert_eq!(gregorian("de-DE"), "de-DE-u-ca-gregory-nu-latn");
        assert_eq!(
            gregorian("en-US-u-fw-mon"),
            "en-US-u-fw-mon-ca-gregory-nu-latn"
        );
        assert_eq!(
            gregorian("th-TH-u-ca-buddhist-fw-mon-nu-thai"),
            "th-TH-u-fw-mon-ca-gregory-nu-latn"
        );
        assert_eq!(
            gregorian("de-DE-u-fw-mon-x-private"),
            "de-DE-u-fw-mon-ca-gregory-nu-latn-x-private"
        );
    }

    #[test]
    fn countries() {
        assert_eq!(country("de-DE").as_deref(), Some("DE"));
        assert_eq!(country("de_CH").as_deref(), Some("CH"));
        assert_eq!(country("sr-Latn-RS").as_deref(), Some("RS"));
        assert_eq!(country("hi-IN-u-fw-mon").as_deref(), Some("IN"));
        assert_eq!(country("es-419").as_deref(), Some("419"));
        assert_eq!(country("en-u-fw-mon"), None);
        assert_eq!(country("de"), None);
    }

    #[test]
    fn java_patterns() {
        let date = october_15();
        let names = english();
        assert_eq!(format("MMMM d, y", date, &names), "October 15, 2026");
        assert_eq!(
            format("EEEE, MMMM d, y", date, &names),
            "Thursday, October 15, 2026"
        );
        assert_eq!(format("MMMM y", date, &names), "October 2026");
        assert_eq!(format("dd.MM.yyyy", date, &names), "15.10.2026");
        assert_eq!(format("d/M/yy", date, &names), "15/10/26");
        assert_eq!(format("EEE, MMM d", date, &names), "Thu, Oct 15");
        assert_eq!(
            format("y年M月d日EEEE", date, &names),
            "2026年10月15日Thursday"
        );
        // Quoted text, and the apostrophe.
        assert_eq!(
            format("d 'de' MMMM 'de' y", date, &names),
            "15 de October de 2026"
        );
        assert_eq!(format("d''M", date, &names), "15'10");
        assert_eq!(format("'it''s' d", date, &names), "it's 15");
        // The era of th-TH's Gregorian pattern is left out, with its space.
        assert_eq!(format("d MMMM G y", date, &names), "15 October 2026");
        // An unclosed quote takes the rest.
        assert_eq!(format("d 'MMMM", date, &names), "15 MMMM");
    }

    #[test]
    fn german_formats() {
        let date = october_15();
        let de = formats(german()).unwrap();
        assert_eq!(de.tag, "de-DE");
        assert_eq!(de.numbers.localize("1,234,567.5"), "1.234.567,5");
        assert_eq!(de.list_separator, "; ");
        assert_eq!(de.first_day, Weekday::Mon);
        assert_eq!(de.weekdays[0], "So");
        assert_eq!(de.weekdays[1], "Mo");
        assert_eq!(de.months[0], "Jan");
        assert_eq!(de.months[2], "Mär");
        assert_eq!(de.date(date), "15. Oktober 2026");
        assert_eq!(de.long_date(date), "Donnerstag, 15. Oktober 2026");
        assert_eq!(de.year_month(date), "Oktober 2026");
        assert_eq!(de.example(date), "1.234.567,89 · 15. Oktober 2026");
    }

    #[test]
    fn russian_formats() {
        let date = october_15();
        let ru = formats(russian()).unwrap();
        assert_eq!(ru.numbers.localize("1,234,567.5"), "1\u{a0}234\u{a0}567,5");
        assert_eq!(ru.list_separator, "; ");
        assert_eq!(ru.first_day, Weekday::Mon);
        // Windows' shortest names start with a capital letter.
        assert_eq!(
            ru.weekdays,
            list(&["Вс", "Пн", "Вт", "Ср", "Чт", "Пт", "Сб"])
        );
        assert_eq!(ru.months[0], "янв");
        assert_eq!(ru.months[9], "окт");
        assert_eq!(ru.date(date), "15 октября 2026 г.");
        assert_eq!(ru.long_date(date), "четверг, 15 октября 2026 г.");
        // The title's month stands alone: the nominative.
        assert_eq!(ru.year_month(date), "октябрь 2026 г.");
    }

    #[test]
    fn english_source_matches_the_english_formats() {
        let date = october_15();
        let names = english();
        let strings = |names: &[String]| names.to_vec();
        let mut weekdays = vec![String::new()];
        weekdays.extend(names.weekdays.iter().cloned());
        let mut short_weekdays = vec![String::new()];
        short_weekdays.extend(names.short_weekdays.iter().cloned());
        let us = formats(Source {
            tag: "en-US".into(),
            number: "1,234,567,890,123.5".into(),
            first_day: 1,
            months: strings(&names.months),
            months_standalone: strings(&names.months_standalone),
            short_months: strings(&names.short_months),
            short_months_standalone: strings(&names.short_months_standalone),
            weekdays,
            short_weekdays: short_weekdays.clone(),
            shortest_weekdays: short_weekdays,
            picker: "MMMM d, y".into(),
            long: "EEEE, MMMM d, y".into(),
            year_month: "MMMM y".into(),
        })
        .unwrap();
        let english = Formats::english();
        assert_eq!(us.numbers, english.numbers);
        assert_eq!(us.list_separator, english.list_separator);
        assert_eq!(us.first_day, english.first_day);
        assert_eq!(us.weekdays, english.weekdays);
        assert_eq!(us.months, english.months);
        assert_eq!(us.date(date), english.date(date));
        assert_eq!(us.long_date(date), english.long_date(date));
        assert_eq!(us.year_month(date), english.year_month(date));
    }

    /// A name list of the wrong length, or a number that isn't the sample, gives `None`
    /// (and the app en-US), never a panic.
    #[test]
    fn odd_sources_give_none() {
        let mut source = german();
        source.months.pop();
        assert!(formats(source).is_none());
        let mut source = german();
        source.weekdays = vec!["Sonntag".into()];
        assert!(formats(source).is_none());
        let mut source = german();
        source.number = "?".into();
        assert!(formats(source).is_none());
        // Seven weekday names without the empty first entry are fine.
        let mut source = german();
        source.shortest_weekdays.remove(0);
        assert_eq!(formats(source).unwrap().weekdays[0], "So");
    }
}
