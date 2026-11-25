//! Regional formats: how numbers look in the user's region (decimal separator, digit
//! grouping) and the date patterns derived from Windows' regional settings. No UI or
//! operating system code: the app reads the settings and passes them in.
//!
//! The engine and the converter format numbers the en-US way (`1,234,567.5`), as their
//! tests pin down. `NumberFormat` turns that text into the region's format at the
//! display, like Windows' engine does with the `sDecimal`, `sThousand` and `sGrouping`
//! settings, and turns pasted text back.

/// A region's number format (Windows' `LOCALE_SDECIMAL`, `LOCALE_STHOUSAND` and
/// `LOCALE_SGROUPING`).
#[derive(Clone, Debug, PartialEq)]
pub struct NumberFormat {
    pub decimal: String,
    pub group: String,
    /// Digits per group from the right; see `parse_grouping`.
    pub grouping: Vec<u32>,
}

impl Default for NumberFormat {
    /// en-US: `1,234,567.5`.
    fn default() -> Self {
        Self {
            decimal: ".".into(),
            group: ",".into(),
            grouping: vec![3, 0],
        }
    }
}

impl NumberFormat {
    /// A format from Windows' settings. Windows Settings allow separators of up to three
    /// characters, but the Windows Calculator keeps only the first one
    /// (`LocalizationSettings.Initialize`). An empty decimal separator falls back to `.`.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn new(decimal: &str, group: &str, grouping: &str) -> Self {
        let first = |text: &str| text.chars().next().map(String::from);
        Self {
            decimal: first(decimal).unwrap_or_else(|| ".".into()),
            group: first(group).unwrap_or_default(),
            grouping: parse_grouping(grouping),
        }
    }

    /// A number as the engine shows it in en-US (`-1,234,567.5`, `1.e+16`, `1,234.`)
    /// in this format: grouped again, with the region's decimal separator.
    pub fn localize(&self, en_us: &str) -> String {
        let plain: String = en_us.chars().filter(|&c| c != ',').collect();
        let (sign, unsigned) = match plain.strip_prefix('-') {
            Some(rest) => ("-", rest),
            None => ("", plain.as_str()),
        };
        let (mantissa, exponent) = match unsigned.find('e') {
            Some(at) => unsigned.split_at(at),
            None => (unsigned, ""),
        };
        let (int, fraction) = match mantissa.split_once('.') {
            Some((int, fraction)) => (int, Some(fraction)),
            None => (mantissa, None),
        };
        let mut text = format!("{sign}{}", group_digits(int, &self.group, &self.grouping));
        if let Some(fraction) = fraction {
            text.push_str(&self.decimal);
            text.push_str(fraction);
        }
        text.push_str(exponent);
        text
    }

    /// The expression line, which has no grouping: only the decimal separator changes.
    pub fn localize_expression(&self, en_us: &str) -> String {
        en_us.replace('.', &self.decimal)
    }

    /// Ungrouped en-US text (`1234.5`, what Ctrl+C copies) with the region's decimal
    /// separator: the same change as for the expression line.
    pub fn localize_plain(&self, en_us: &str) -> String {
        self.localize_expression(en_us)
    }

    /// Pasted text in this format, for the engine's en-US paste rules: without the group
    /// separators, and with `.` for the decimal separator. Windows maps the decimal
    /// separator first, so where the group separator is `.` it drops the decimal point
    /// too (`1,5` becomes `15` in German); removing the separators first keeps it.
    pub fn delocalize(&self, text: &str) -> String {
        let mut text = text.to_string();
        if !self.group.is_empty() && self.group != self.decimal {
            text = text.replace(&self.group, "");
        }
        if self.decimal != "." {
            text = text.replace(&self.decimal, ".");
        }
        text
    }
}

/// Windows' grouping string (`LOCALE_SGROUPING`) as a list, like the engine's
/// `DigitGroupingStringToGroupingVector`: `3;0` groups every 3 digits, `3;2;0` is 3 and
/// then every 2 (India), `3` groups only the first 3, and `0;0` doesn't group.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn parse_grouping(grouping: &str) -> Vec<u32> {
    grouping
        .split(';')
        .filter_map(|part| part.trim().parse().ok())
        .filter(|&size| size < 10)
        .collect()
}

/// Puts separators into a string of digits like Windows' `CCalcEngine::GroupDigits`:
/// from the right, a group per value of `grouping`. A 0 at the end repeats the previous
/// group, and without one the grouping stops after the last value.
fn group_digits(digits: &str, separator: &str, grouping: &[u32]) -> String {
    if separator.is_empty() || grouping.is_empty() {
        return digits.to_string();
    }
    let reversed_separator: String = separator.chars().rev().collect();
    let chars: Vec<char> = digits.chars().collect();
    let mut out = String::with_capacity(digits.len() * 2);
    let mut index = 0;
    let mut current = grouping[0];
    let mut size = 0;
    for (i, &c) in chars.iter().rev().enumerate() {
        out.push(c);
        size += 1;
        if current != 0 && size % current == 0 && i + 1 < chars.len() {
            out.push_str(&reversed_separator);
            size = 0;
            if index != grouping.len() {
                index += 1;
                current = 0;
                while index != grouping.len() {
                    if grouping[index] != 0 {
                        current = grouping[index];
                        break;
                    }
                    current = grouping[index - 1];
                    index += 1;
                }
            }
        }
    }
    out.chars().rev().collect()
}

/// The date pickers' pattern ("day month year", like `October 15, 2026` in en-US) from
/// the region's long date pattern (`LOCALE_SLONGDATE`, like `dddd, MMMM d, yyyy`):
/// the long date without the day of the week and the separators and words that go with it.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn without_weekday(pattern: &str) -> String {
    // Find `ddd` or `dddd` outside quoted text.
    let chars: Vec<char> = pattern.chars().collect();
    let mut quoted = false;
    let mut found = None;
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '\'' => quoted = !quoted,
            'd' if !quoted => {
                let run = chars[i..].iter().take_while(|&&c| c == 'd').count();
                if run >= 3 {
                    found = Some((i, i + run));
                    break;
                }
                i += run;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    let Some((start, end)) = found else {
        return pattern.to_string();
    };
    let quotes = quoted_texts(&chars);
    let mut after = skip_separators(&chars, &quotes, end);
    if let Some(next) = weekday_word(&chars, &quotes, after) {
        after = skip_separators(&chars, &quotes, next);
    }
    let mut before = start;
    if after == chars.len() {
        // The day of the week ends the pattern: drop the separators before it.
        before = skip_separators_back(&chars, &quotes, before);
    } else if before > 0 && chars[before - 1] == '\'' && chars[after] == '\'' {
        // Quoted text on both sides (th-TH's `'วัน'dddd'ที่'`) becomes one quote: the two
        // quotes left in a row would be an apostrophe.
        before -= 1;
        after += 1;
    }
    chars[..before].iter().chain(&chars[after..]).collect()
}

/// Separators between the day of the week and the rest of a long date: the comma and the
/// space, and the commas and word separators of other scripts: Arabic `،`, ideographic `、`,
/// Ethiopic `፡` `፣` `፥` (byn, ti, tig, wal), Mongolian `᠂` and Myanmar `၊`.
fn is_separator(c: char) -> bool {
    matches!(
        c,
        ',' | ' '
            | '\u{060C}'
            | '、'
            | '\u{1361}'
            | '\u{1363}'
            | '\u{1365}'
            | '\u{1802}'
            | '\u{104A}'
    )
}

/// Quoted text in a pattern: where its opening quote is, the index after its closing quote,
/// and the text, in which two quotes in a row are an apostrophe.
struct Quoted {
    start: usize,
    end: usize,
    text: String,
}

impl Quoted {
    /// Quoted separators, like se-FI's `dddd', 'MMMM`.
    fn is_separator(&self) -> bool {
        !self.text.is_empty() && self.text.chars().all(is_separator)
    }
}

/// The quoted texts of a pattern.
fn quoted_texts(chars: &[char]) -> Vec<Quoted> {
    let mut quotes = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '\'' {
            i += 1;
            continue;
        }
        let start = i;
        let mut text = String::new();
        i += 1;
        while i < chars.len() {
            if chars[i] == '\'' {
                if chars.get(i + 1) == Some(&'\'') {
                    text.push('\'');
                    i += 2;
                    continue;
                }
                i += 1;
                break;
            }
            text.push(chars[i]);
            i += 1;
        }
        quotes.push(Quoted {
            start,
            end: i,
            text,
        });
    }
    quotes
}

/// The index after the separators that start at `i`, quoted ones too.
fn skip_separators(chars: &[char], quotes: &[Quoted], mut i: usize) -> usize {
    loop {
        if i < chars.len() && is_separator(chars[i]) {
            i += 1;
        } else if let Some(quoted) = quotes.iter().find(|q| q.start == i && q.is_separator()) {
            i = quoted.end;
        } else {
            return i;
        }
    }
}

/// The index where the separators that end at `i` start, quoted ones too.
fn skip_separators_back(chars: &[char], quotes: &[Quoted], mut i: usize) -> usize {
    loop {
        if i > 0 && is_separator(chars[i - 1]) {
            i -= 1;
        } else if let Some(quoted) = quotes.iter().find(|q| q.end == i && q.is_separator()) {
            i = quoted.start;
        } else {
            return i;
        }
    }
}

/// Words that go with the day of the week: before the day of the month, like `den` in Danish
/// "onsdag den 15. oktober 2026", or after the day of the week, like Mongolian `гараг`
/// ("Лхагва гараг", Wednesday). CLDR's long dates, the same dates without the day of the
/// week, drop them: `d. MMMM y` for Danish, Low German (`de`) and Colognian (`dä`),
/// `d MMMM y` for Romansh (`ils`) and Lao (`ທີ`), `d 'de' MMMM y` for Interlingua (`le`),
/// `y 'оны' MMMM'ын' d` for Mongolian. Latin's `die` stays in CLDR's (`'die' d MMMM y G`), so
/// it isn't here.
const WEEKDAY_WORDS: [&str; 7] = ["den", "ils", "de", "dä", "le", "ທີ", "гараг"];

/// The index after a word of `WEEKDAY_WORDS` that starts at `i`, quoted or not (letters
/// outside the ASCII range need no quotes, like Lao `dddd ທີ d MMMM`).
fn weekday_word(chars: &[char], quotes: &[Quoted], i: usize) -> Option<usize> {
    let (word, next) = match quotes.iter().find(|q| q.start == i) {
        Some(quoted) => (quoted.text.clone(), quoted.end),
        None => {
            let length = chars[i..]
                .iter()
                .take_while(|&&c| !c.is_ascii() && !is_separator(c))
                .count();
            (chars[i..i + length].iter().collect(), i + length)
        }
    };
    WEEKDAY_WORDS.contains(&word.as_str()).then_some(next)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn german() -> NumberFormat {
        NumberFormat::new(",", ".", "3;0")
    }

    #[test]
    fn grouping_strings() {
        assert_eq!(parse_grouping("3;0"), [3, 0]);
        assert_eq!(parse_grouping("3;2;0"), [3, 2, 0]);
        assert_eq!(parse_grouping("3"), [3]);
        assert_eq!(parse_grouping("0;0"), [0, 0]);
        assert_eq!(parse_grouping(""), Vec::<u32>::new());
    }

    #[test]
    fn digits_group_like_windows() {
        assert_eq!(group_digits("1234567", ",", &[3, 0]), "1,234,567");
        assert_eq!(group_digits("123", ",", &[3, 0]), "123");
        assert_eq!(group_digits("123456789", ",", &[3, 2, 0]), "12,34,56,789");
        assert_eq!(group_digits("1234567", ",", &[3]), "1234,567");
        assert_eq!(group_digits("1234567", ",", &[0, 0]), "1234567");
        assert_eq!(group_digits("123456789", ",", &[5, 3, 2]), "1,234,56789");
        // Without a 0 at the end, the digits left after the last group stay together.
        assert_eq!(
            group_digits("9912345678901", ",", &[5, 3, 2]),
            "991,23,456,78901"
        );
        assert_eq!(
            group_digits("1234567", "\u{202F}", &[3, 0]),
            "1\u{202F}234\u{202F}567"
        );
    }

    #[test]
    fn display_numbers() {
        let en = NumberFormat::default();
        assert_eq!(en.localize("1,234,567.5"), "1,234,567.5");
        assert_eq!(en.localize("-1,234."), "-1,234.");
        let de = german();
        assert_eq!(de.localize("1,234,567.5"), "1.234.567,5");
        assert_eq!(de.localize("-1,234."), "-1.234,");
        assert_eq!(de.localize("0.0033333333333333"), "0,0033333333333333");
        assert_eq!(de.localize("1.e+16"), "1,e+16");
        assert_eq!(
            de.localize("-1.234567890123456e+18"),
            "-1,234567890123456e+18"
        );
        assert_eq!(de.localize("0"), "0");
        let india = NumberFormat::new(".", ",", "3;2;0");
        assert_eq!(india.localize("123,456,789.25"), "12,34,56,789.25");
        let switzerland = NumberFormat::new(".", "\u{2019}", "3;0");
        assert_eq!(
            switzerland.localize("1,234,567.5"),
            "1\u{2019}234\u{2019}567.5"
        );
    }

    /// Windows Settings take separators of up to three characters; the calculator uses
    /// only the first one.
    #[test]
    fn separators_are_one_character() {
        let custom = NumberFormat::new(",;", "_-", "3;0");
        assert_eq!(custom, NumberFormat::new(",", "_", "3;0"));
        assert_eq!(custom.localize("1,234,567.5"), "1_234_567,5");
        assert_eq!(custom.localize_expression("1234.5"), "1234,5");
        assert_eq!(custom.delocalize("1_234,5"), "1234.5");
        // An empty group separator still means no grouping.
        assert_eq!(NumberFormat::new(".", "", "3;0").localize("1,234"), "1234");
    }

    #[test]
    fn expression_and_copy() {
        let de = german();
        assert_eq!(
            de.localize_expression("1234567.5 + sqr(2.5)"),
            "1234567,5 + sqr(2,5)"
        );
        assert_eq!(de.localize_plain("1234.5"), "1234,5");
    }

    #[test]
    fn pasted_text() {
        let en = NumberFormat::default();
        assert_eq!(en.delocalize("1,234.5"), "1234.5");
        let de = german();
        assert_eq!(de.delocalize("1.234,5"), "1234.5");
        assert_eq!(de.delocalize("1,5 + 2"), "1.5 + 2");
        let france = NumberFormat::new(",", "\u{202F}", "3;0");
        assert_eq!(france.delocalize("1\u{202F}234,5"), "1234.5");
    }

    #[test]
    fn date_picker_patterns() {
        assert_eq!(without_weekday("dddd, MMMM d, yyyy"), "MMMM d, yyyy");
        assert_eq!(without_weekday("dddd, d. MMMM yyyy"), "d. MMMM yyyy");
        assert_eq!(without_weekday("dddd d MMMM yyyy"), "d MMMM yyyy");
        assert_eq!(
            without_weekday("dddd, d' de 'MMMM' de 'yyyy"),
            "d' de 'MMMM' de 'yyyy"
        );
        assert_eq!(without_weekday("d MMMM yyyy, dddd"), "d MMMM yyyy");
        // Text quoted on both sides of the day of the week joins into one quote: two quotes
        // in a row would be an apostrophe (th-TH, `วันที่ 15 ตุลาคม 2026`).
        assert_eq!(
            without_weekday("'วัน'dddd'ที่' d MMMM yyyy"),
            "'วันที่' d MMMM yyyy"
        );
        assert_eq!(without_weekday("yyyy'年'M'月'd'日'"), "yyyy'年'M'月'd'日'");
        // A quoted "ddd" is text, not the day of the week.
        assert_eq!(without_weekday("'dddd' d MMMM"), "'dddd' d MMMM");
        assert_eq!(without_weekday("d MMMM yyyy"), "d MMMM yyyy");
    }

    /// Windows' long dates (microsoft/globe's `WindowsRegionalSettingsPerLocale`) lose the
    /// words and separators that go with the day of the week, as CLDR's long dates (the same
    /// dates without the day of the week) show.
    #[test]
    fn date_picker_patterns_drop_weekday_words() {
        // mn-MN, se-FI, rm-CH and da-GL.
        assert_eq!(
            without_weekday("yyyy 'оны' MMMM'ын' d, dddd 'гараг'"),
            "yyyy 'оны' MMMM'ын' d"
        );
        assert_eq!(
            without_weekday("dddd', 'MMMM d'. b. 'yyyy"),
            "MMMM d'. b. 'yyyy"
        );
        assert_eq!(without_weekday("dddd, 'ils' d MMMM yyyy"), "d MMMM yyyy");
        assert_eq!(without_weekday("dddd 'den' d. MMMM yyyy"), "d. MMMM yyyy");
        // lo-LA, and the separators of ti-ER, mn-Mong-CN and my-MM.
        assert_eq!(without_weekday("dddd ທີ d MMMM gg yyyy"), "d MMMM gg yyyy");
        assert_eq!(
            without_weekday("dddd፣ dd MMMM መዓልቲ yyyy gg"),
            "dd MMMM መዓልቲ yyyy gg"
        );
        assert_eq!(
            without_weekday("yyyyᠣᠨ MMMM dᠡᠳᠦᠷ᠂ dddd"),
            "yyyyᠣᠨ MMMM dᠡᠳᠦᠷ"
        );
        assert_eq!(without_weekday("yyyy၊ MMMM d၊ dddd"), "yyyy၊ MMMM d");
        // Words that CLDR keeps without the day of the week (la-VA, nnh-CM, ps-PK).
        assert_eq!(
            without_weekday("dddd, 'die' d MMMM yyyy gg"),
            "'die' d MMMM yyyy gg"
        );
        assert_eq!(
            without_weekday("dddd , 'lyɛ'̌ʼ d 'na' MMMM, yyyy"),
            "'lyɛ'̌ʼ d 'na' MMMM, yyyy"
        );
        assert_eq!(without_weekday("dddd د yyyy د MMMM d"), "د yyyy د MMMM d");
        // Other text stays: "den's" is no weekday word, and nor is text after the day of the
        // week that isn't one. Quoted separators before a final weekday go.
        assert_eq!(
            without_weekday("dddd 'den''s' d MMMM yyyy"),
            "'den''s' d MMMM yyyy"
        );
        assert_eq!(without_weekday("MMMM d yyyy dddd 'AD'"), "MMMM d yyyy 'AD'");
        assert_eq!(without_weekday("MMMM d', 'dddd"), "MMMM d");
    }
}
