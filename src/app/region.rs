//! Regional formats: numbers, the list separator, dates, month and weekday names, and the
//! first day of the week. As in the Windows Calculator they come from the user's locale
//! (Settings → Time & language → Language & region → Regional format), separately from the
//! display language of the strings, unless the Settings page picks another region. The
//! environment variable `CALCCRAFT_REGION` (for example `de-DE`) stands in for the
//! Windows setting. The web version reads them from the browser's `Intl`, the Android app
//! from Java's locale classes (`java`, through JNI in `android.rs`); Linux and macOS use en-US
//! (`docs/regional-formats.md`).

pub(super) mod java;

use std::sync::{Mutex, OnceLock, PoisonError, RwLock};

use chrono::{NaiveDate, Weekday};

use crate::dates;
use crate::locale::NumberFormat;

/// A region's formats.
pub(super) struct Formats {
    /// The region's locale name, like `de-CH`.
    pub tag: String,
    pub numbers: NumberFormat,
    /// Joins the units of a date difference: Windows uses the list separator and a space.
    pub list_separator: String,
    pub first_day: Weekday,
    /// The shortest weekday names, Sunday first (`Su`, `Mo`, …).
    pub weekdays: [String; 7],
    /// The abbreviated month names (`Jan`, `Feb`, …).
    pub months: [String; 12],
    /// Formats dates with Windows' `GetDateFormatEx`, or chrono in en-US without it.
    dates: DateFormats,
}

/// The formats in use, and every region loaded so far (by choice: `None` is the Windows
/// setting). They live as long as the app, since `formats` hands out `&'static`.
static CURRENT: RwLock<Option<&'static Formats>> = RwLock::new(None);
static LOADED: Mutex<Vec<(Option<String>, &'static Formats)>> = Mutex::new(Vec::new());

/// The formats in use: the Windows setting until `set_region` picks a region.
pub(super) fn formats() -> &'static Formats {
    if let Some(formats) = *CURRENT.read().unwrap_or_else(PoisonError::into_inner) {
        return formats;
    }
    set_region(None)
}

/// Switches to a region's formats: `choice` is a locale name from `regions()`, or `None`
/// for the Windows setting. Returns the formats now in use.
pub(super) fn set_region(choice: Option<&str>) -> &'static Formats {
    let formats = load(choice);
    *CURRENT.write().unwrap_or_else(PoisonError::into_inner) = Some(formats);
    formats
}

fn load(choice: Option<&str>) -> &'static Formats {
    let mut loaded = LOADED.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some((_, formats)) = loaded.iter().find(|(key, _)| key.as_deref() == choice) {
        return formats;
    }
    #[cfg(windows)]
    let formats = windows::load(choice).unwrap_or_else(Formats::english);
    #[cfg(target_arch = "wasm32")]
    let formats = browser(choice).unwrap_or_else(Formats::english);
    // A failed JNI call (`with_activity` logged it) or odd data leaves en-US, never a crash.
    #[cfg(target_os = "android")]
    let formats = super::android::region_source(choice)
        .and_then(java::formats)
        .unwrap_or_else(|| {
            log::warn!("Regional format {choice:?}: Java gave no formats, using en-US");
            Formats::english()
        });
    #[cfg(not(any(windows, target_arch = "wasm32", target_os = "android")))]
    let formats = Formats::english();
    let formats: &'static Formats = Box::leak(Box::new(formats));
    loaded.push((choice.map(str::to_string), formats));
    formats
}

enum DateFormats {
    /// chrono's en-US formats, the same as `dates::format_date`.
    English,
    #[cfg(windows)]
    Windows(windows::Locale),
    /// The browser's `Intl.DateTimeFormat` for a locale name.
    #[cfg(target_arch = "wasm32")]
    Web(String),
    /// Java's date patterns and names (the Android app), formatted here.
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    Java(Box<java::Dates>),
}

/// The formats of a region from the browser's `Intl` (`None`: the browser's language, or the
/// page address's `?region=de-DE`), as `windows::load` reads Windows'. Browsers have no list
/// separator: Windows' is `;` where the decimal separator is a comma, and `,` elsewhere.
#[cfg(target_arch = "wasm32")]
fn browser(choice: Option<&str>) -> Option<Formats> {
    use super::web;

    let tag = choice.map_or_else(web::region, str::to_string);
    let numbers = web::number_format(&tag)?;
    let list = if numbers.decimal == "," { ";" } else { "," };
    Some(Formats {
        first_day: web::first_day(&tag),
        weekdays: web::weekdays(&tag)?,
        months: web::months(&tag)?,
        list_separator: format!("{list} "),
        numbers,
        dates: DateFormats::Web(tag.clone()),
        tag,
    })
}

impl Formats {
    /// en-US, for when Windows can't tell.
    fn english() -> Self {
        let names = |list: &[&str]| list.iter().map(|name| name.to_string()).collect::<Vec<_>>();
        Self {
            tag: "en-US".into(),
            numbers: NumberFormat::default(),
            list_separator: dates::ENGLISH.separator.to_string(),
            first_day: Weekday::Sun,
            weekdays: names(&["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"])
                .try_into()
                .expect("7 days"),
            months: names(&[
                "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
            ])
            .try_into()
            .expect("12 months"),
            dates: DateFormats::English,
        }
    }

    /// A date as the date pickers show it: `October 15, 2026`, `15. Oktober 2026`.
    pub fn date(&self, date: NaiveDate) -> String {
        match &self.dates {
            DateFormats::English => dates::format_date(date),
            #[cfg(windows)]
            DateFormats::Windows(locale) => locale
                .format(date, 0, Some(&locale.picker_pattern))
                .unwrap_or_else(|| dates::format_date(date)),
            #[cfg(target_arch = "wasm32")]
            DateFormats::Web(tag) => super::web::format_date(
                tag,
                date,
                &[("year", "numeric"), ("month", "long"), ("day", "numeric")],
            )
            .unwrap_or_else(|| dates::format_date(date)),
            DateFormats::Java(dates) => dates.picker(date),
        }
    }

    /// A date as the add/subtract result shows it, the region's long date:
    /// `Thursday, October 15, 2026`.
    pub fn long_date(&self, date: NaiveDate) -> String {
        match &self.dates {
            DateFormats::English => dates::format_long_date(date),
            #[cfg(windows)]
            DateFormats::Windows(locale) => locale
                .format(date, windows::DATE_LONGDATE, None)
                .unwrap_or_else(|| dates::format_long_date(date)),
            #[cfg(target_arch = "wasm32")]
            DateFormats::Web(tag) => super::web::format_date(
                tag,
                date,
                &[
                    ("weekday", "long"),
                    ("year", "numeric"),
                    ("month", "long"),
                    ("day", "numeric"),
                ],
            )
            .unwrap_or_else(|| dates::format_long_date(date)),
            DateFormats::Java(dates) => dates.long(date),
        }
    }

    /// The month and year of the calendar's title: `October 2026`.
    pub fn year_month(&self, date: NaiveDate) -> String {
        match &self.dates {
            DateFormats::English => date.format("%B %Y").to_string(),
            #[cfg(windows)]
            DateFormats::Windows(locale) => locale
                .format(date, windows::DATE_YEARMONTH, None)
                .unwrap_or_else(|| date.format("%B %Y").to_string()),
            #[cfg(target_arch = "wasm32")]
            DateFormats::Web(tag) => {
                super::web::format_date(tag, date, &[("year", "numeric"), ("month", "long")])
                    .unwrap_or_else(|| date.format("%B %Y").to_string())
            }
            DateFormats::Java(dates) => dates.year_month(date),
        }
    }

    /// The region of these formats (`DE` for `de-DE`), for the converters' default units
    /// where the system has no setting of its own; `None` for a tag without one (`en`).
    pub fn country(&self) -> Option<String> {
        java::country(&self.tag)
    }

    /// A number and a date in these formats, for the Settings page:
    /// `1,234,567.89 · October 15, 2026`.
    pub fn example(&self, date: NaiveDate) -> String {
        format!(
            "{} · {}",
            self.numbers.localize("1,234,567.89"),
            self.date(date)
        )
    }
}

/// A regional format to choose from: its locale name and its name in its own language
/// (`de-CH`, `Deutsch (Schweiz)`).
pub(super) struct Region {
    pub tag: String,
    pub name: String,
}

/// The regional formats to choose from, sorted by name: Windows' regional formats whose
/// names and dates egui can draw and that use the Gregorian calendar; the translations'
/// regions in the web version and the Android app; none on Linux and macOS, where the
/// Regional format card is left out (`docs/regional-formats.md`).
pub(super) fn regions() -> &'static [Region] {
    static REGIONS: OnceLock<Vec<Region>> = OnceLock::new();
    REGIONS.get_or_init(|| {
        #[cfg(windows)]
        let tags = windows::regions();
        // Browsers and Android don't list their locales: the web version and the Android app
        // offer the regions of the translations.
        #[cfg(any(target_arch = "wasm32", target_os = "android"))]
        let tags: Vec<String> = super::strings::languages().map(str::to_string).collect();
        #[cfg(not(any(windows, target_arch = "wasm32", target_os = "android")))]
        let tags: Vec<String> = Vec::new();
        let mut regions: Vec<Region> = tags
            .into_iter()
            .map(|tag| Region {
                name: native_name(&tag),
                tag,
            })
            .collect();
        // Variants of a region (`ca-ES` and `ca-ES-valencia`) can get the same name; those
        // take Windows' display name, which tells them apart.
        #[cfg(windows)]
        {
            let names: Vec<String> = regions.iter().map(|region| region.name.clone()).collect();
            for region in &mut regions {
                if names.iter().filter(|name| **name == region.name).count() > 1
                    && let Some(name) = windows::display_name(&region.tag)
                {
                    region.name = name;
                }
            }
        }
        regions.sort_by_cached_key(|region| region.name.to_lowercase());
        regions
    })
}

/// A language's or region's name in its own language (`Deutsch (Deutschland)`), or the tag
/// where Windows doesn't know it.
pub(super) fn native_name(tag: &str) -> String {
    #[cfg(windows)]
    if let Some(name) = windows::native_name(tag) {
        return name;
    }
    #[cfg(target_arch = "wasm32")]
    if let Some(name) = super::web::display_name(tag) {
        return name;
    }
    #[cfg(target_os = "android")]
    if let Some(name) = super::android::display_name(tag) {
        return name;
    }
    tag.to_string()
}

#[cfg(windows)]
mod windows {
    use chrono::{Datelike, NaiveDate, Weekday};
    use windows_sys::Win32::Foundation::{LPARAM, SYSTEMTIME};
    pub(super) use windows_sys::Win32::Globalization::{DATE_LONGDATE, DATE_YEARMONTH};
    use windows_sys::Win32::Globalization::{
        DATE_USE_ALT_CALENDAR, EnumSystemLocalesEx, GetDateFormatEx, GetLocaleInfoEx,
        GetUserDefaultLocaleName, LOCALE_ICALENDARTYPE, LOCALE_IFIRSTDAYOFWEEK,
        LOCALE_IOPTIONALCALENDAR, LOCALE_RETURN_NUMBER, LOCALE_SABBREVMONTHNAME1, LOCALE_SDECIMAL,
        LOCALE_SGROUPING, LOCALE_SLIST, LOCALE_SLONGDATE, LOCALE_SNATIVECOUNTRYNAME,
        LOCALE_SNATIVEDISPLAYNAME, LOCALE_SNATIVELANGUAGENAME, LOCALE_SPECIFICDATA,
        LOCALE_SSCRIPTS, LOCALE_SSHORTESTDAYNAME1, LOCALE_STHOUSAND,
    };

    use super::{DateFormats, Formats};
    use crate::locale::{NumberFormat, without_weekday};

    /// A locale for the Windows date and locale functions.
    pub(super) struct Locale {
        /// The zero-terminated locale name, or `None` for the user's own locale, which
        /// includes the changes made to it in Windows Settings.
        name: Option<Vec<u16>>,
        /// `DATE_USE_ALT_CALENDAR` where the dates need the locale's alternate calendar
        /// to be Gregorian (see `load`), else 0.
        calendar: u32,
        /// The date pickers' pattern, zero-terminated.
        pub picker_pattern: Vec<u16>,
    }

    impl Locale {
        fn new(name: Option<Vec<u16>>) -> Self {
            Self {
                name,
                calendar: 0,
                picker_pattern: Vec::new(),
            }
        }

        fn named(name: &str) -> Self {
            Self::new(Some(wide(name)))
        }

        fn name(&self) -> *const u16 {
            self.name
                .as_ref()
                .map_or(std::ptr::null(), |name| name.as_ptr())
        }

        /// A locale setting as text (`GetLocaleInfoEx`).
        fn info(&self, setting: u32) -> Option<String> {
            let mut buffer = [0u16; 128];
            // SAFETY: the buffer and its length match; the name is null (the user's
            // locale) or zero-terminated.
            let length = unsafe {
                GetLocaleInfoEx(
                    self.name(),
                    setting,
                    buffer.as_mut_ptr(),
                    buffer.len() as i32,
                )
            };
            (length > 0).then(|| String::from_utf16_lossy(&buffer[..length as usize - 1]))
        }

        /// A numeric locale setting.
        fn number(&self, setting: u32) -> Option<u32> {
            let mut value = 0u32;
            // SAFETY: with LOCALE_RETURN_NUMBER the function writes one u32, which is two
            // UTF-16 units long.
            let length = unsafe {
                GetLocaleInfoEx(
                    self.name(),
                    setting | LOCALE_RETURN_NUMBER,
                    (&mut value as *mut u32).cast(),
                    2,
                )
            };
            (length > 0).then_some(value)
        }

        /// A date in a format (`GetDateFormatEx`): flags like `DATE_LONGDATE`, or a
        /// zero-terminated pattern.
        pub(super) fn format(
            &self,
            date: NaiveDate,
            flags: u32,
            pattern: Option<&[u16]>,
        ) -> Option<String> {
            let time = SYSTEMTIME {
                wYear: u16::try_from(date.year()).ok()?,
                wMonth: date.month() as u16,
                wDayOfWeek: date.weekday().num_days_from_sunday() as u16,
                wDay: date.day() as u16,
                wHour: 0,
                wMinute: 0,
                wSecond: 0,
                wMilliseconds: 0,
            };
            let pattern = pattern.map_or(std::ptr::null(), <[u16]>::as_ptr);
            let mut buffer = [0u16; 128];
            // SAFETY: the time is a valid date (1601–9999), the buffer and its length
            // match, and the name and the pattern are null or zero-terminated.
            let length = unsafe {
                GetDateFormatEx(
                    self.name(),
                    flags | self.calendar,
                    &time,
                    pattern,
                    buffer.as_mut_ptr(),
                    buffer.len() as i32,
                    std::ptr::null(),
                )
            };
            (length > 0).then(|| String::from_utf16_lossy(&buffer[..length as usize - 1]))
        }
    }

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(Some(0)).collect()
    }

    /// The user's locale name, like `de-CH`.
    fn user_locale() -> Option<String> {
        let mut buffer = [0u16; 85];
        // SAFETY: the buffer and its length match (LOCALE_NAME_MAX_LENGTH is 85).
        let length = unsafe { GetUserDefaultLocaleName(buffer.as_mut_ptr(), buffer.len() as i32) };
        (length > 1).then(|| String::from_utf16_lossy(&buffer[..length as usize - 1]))
    }

    /// The formats of a region (`None`: the Windows setting, or `CALCCRAFT_REGION`).
    pub(super) fn load(choice: Option<&str>) -> Option<Formats> {
        let (mut locale, tag) = match choice
            .map(str::to_string)
            .or_else(|| std::env::var("CALCCRAFT_REGION").ok())
        {
            Some(name) => (Locale::named(&name), name),
            None => (
                Locale::new(None),
                user_locale().unwrap_or_else(|| "en-US".into()),
            ),
        };
        let numbers = NumberFormat::new(
            &locale.info(LOCALE_SDECIMAL)?,
            &locale.info(LOCALE_STHOUSAND).unwrap_or_default(),
            &locale
                .info(LOCALE_SGROUPING)
                .unwrap_or_else(|| "3;0".into()),
        );
        let list = locale.info(LOCALE_SLIST).unwrap_or_else(|| ",".into());
        // LOCALE_IFIRSTDAYOFWEEK counts from Monday.
        let first_day = match locale.number(LOCALE_IFIRSTDAYOFWEEK).unwrap_or(6) {
            0 => Weekday::Mon,
            1 => Weekday::Tue,
            2 => Weekday::Wed,
            3 => Weekday::Thu,
            4 => Weekday::Fri,
            5 => Weekday::Sat,
            _ => Weekday::Sun,
        };
        // The date mode only knows the Gregorian calendar: the calendar's year and decade
        // views count chrono's years. Where the locale's calendar is another one (th-TH's
        // Thai Buddhist calendar, or the Japanese era picked in Windows Settings),
        // GetDateFormatEx writes its years (2569), so the dates take the alternate calendar
        // when that one is Gregorian, and are English otherwise. Windows can't be asked
        // for the Gregorian calendar directly: it ignores `th-TH-u-ca-gregory`.
        let gregorian = |setting| {
            locale
                .number(setting)
                .is_some_and(|calendar| GREGORIAN.contains(&calendar))
        };
        if !gregorian(LOCALE_ICALENDARTYPE) {
            if !gregorian(LOCALE_IOPTIONALCALENDAR) {
                return Some(Formats {
                    tag,
                    numbers,
                    list_separator: format!("{list} "),
                    first_day,
                    ..Formats::english()
                });
            }
            locale.calendar = DATE_USE_ALT_CALENDAR;
        }
        // The shortest day names start with Monday too.
        let monday_first: Vec<String> = (0..7)
            .map(|i| locale.info(LOCALE_SSHORTESTDAYNAME1 + i))
            .collect::<Option<_>>()?;
        let mut weekdays = monday_first;
        weekdays.rotate_right(1);
        let months: Vec<String> = (0..12)
            .map(|i| locale.info(LOCALE_SABBREVMONTHNAME1 + i))
            .collect::<Option<_>>()?;
        let long_date = locale
            .info(LOCALE_SLONGDATE)
            .unwrap_or_else(|| "dddd, MMMM d, yyyy".into());
        locale.picker_pattern = wide(&without_weekday(&long_date));
        Some(Formats {
            tag,
            numbers,
            list_separator: format!("{list} "),
            first_day,
            weekdays: weekdays.try_into().ok()?,
            months: months.try_into().ok()?,
            dates: DateFormats::Windows(locale),
        })
    }

    /// The language and the country in their own language (`català (Espanya)`). Windows'
    /// display name says less for some (`català (català)`, `euskara (euskara)`) and more for
    /// others (`español (España, alfabetización internacional)`), but a language name that
    /// has parentheses itself (`中文(简体)`) reads better as the display name (`中文(中国)`).
    pub(super) fn native_name(tag: &str) -> Option<String> {
        let locale = Locale::named(tag);
        let language = locale.info(LOCALE_SNATIVELANGUAGENAME)?;
        match locale.info(LOCALE_SNATIVECOUNTRYNAME) {
            Some(country) if !language.contains(['(', '（']) => {
                Some(format!("{language} ({country})"))
            }
            _ => display_name(tag),
        }
    }

    /// Windows' own name of a locale in its language (`Deutsch (Schweiz)`).
    pub(super) fn display_name(tag: &str) -> Option<String> {
        Locale::named(tag).info(LOCALE_SNATIVEDISPLAYNAME)
    }

    /// Scripts egui can draw: Segoe UI's, and the ones `strings::script_font` adds.
    const DRAWABLE_SCRIPTS: [&str; 15] = [
        "Latn", "Cyrl", "Grek", "Armn", "Geor", "Hani", "Hans", "Hant", "Hira", "Kana", "Jpan",
        "Hang", "Kore", "Thai", "Ethi",
    ];

    /// The Gregorian calendars of `LOCALE_ICALENDARTYPE` (CAL_GREGORIAN and its variants).
    /// The date mode only knows the Gregorian calendar.
    const GREGORIAN: [u32; 6] = [1, 2, 9, 10, 11, 12];

    /// Windows' regional formats that `regions()` offers.
    pub(super) fn regions() -> Vec<String> {
        unsafe extern "system" fn collect(name: *const u16, _flags: u32, list: LPARAM) -> i32 {
            // SAFETY: Windows passes a zero-terminated name, and `list` is the vector
            // that `regions` passes in, alive for the whole enumeration.
            unsafe {
                let length = (0..).take_while(|&i| *name.add(i) != 0).count();
                let names = &mut *(list as *mut Vec<String>);
                names.push(String::from_utf16_lossy(std::slice::from_raw_parts(
                    name, length,
                )));
            }
            1
        }
        let mut names: Vec<String> = Vec::new();
        // SAFETY: the callback only uses `names` while the call runs.
        unsafe {
            EnumSystemLocalesEx(
                Some(collect),
                LOCALE_SPECIFICDATA,
                &mut names as *mut Vec<String> as LPARAM,
                std::ptr::null(),
            );
        }
        names.retain(|name| {
            // Only regions (`de-CH`, not `de`), not the pseudo-locales, nor alternative sort
            // orders (`de-DE_phoneb`).
            if !name.contains('-') || name.starts_with("qps-") || name.contains('_') {
                return false;
            }
            let locale = Locale::named(name);
            let drawable = locale.info(LOCALE_SSCRIPTS).is_some_and(|scripts| {
                scripts
                    .split(';')
                    .filter(|script| !script.is_empty())
                    .all(|script| DRAWABLE_SCRIPTS.contains(&script))
            });
            let gregorian = locale
                .number(LOCALE_ICALENDARTYPE)
                .is_some_and(|calendar| GREGORIAN.contains(&calendar));
            drawable && gregorian
        });
        names
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn windows_formats() {
            let date = NaiveDate::from_ymd_opt(2026, 10, 15).unwrap();
            let us = load(Some("en-US")).unwrap();
            assert_eq!(us.numbers, NumberFormat::default());
            assert_eq!(us.date(date), "October 15, 2026");
            assert_eq!(us.long_date(date), "Thursday, October 15, 2026");
            assert_eq!(us.year_month(date), "October 2026");
            assert_eq!(us.example(date), "1,234,567.89 · October 15, 2026");

            let de = load(Some("de-DE")).unwrap();
            assert_eq!(de.tag, "de-DE");
            assert_eq!(de.numbers.localize("1,234,567.5"), "1.234.567,5");
            assert_eq!(de.date(date), "15. Oktober 2026");
            assert_eq!(de.first_day, Weekday::Mon);
            assert_eq!(de.list_separator, "; ");
        }

        /// The calendar's year and decade views count Gregorian years, so every date is
        /// Gregorian, also where the region's calendar isn't (Windows would write 2569 in
        /// th-TH and the Hijri year 1448 in ar-SA).
        #[test]
        fn other_calendars_show_gregorian_dates() {
            let date = NaiveDate::from_ymd_opt(2026, 10, 15).unwrap();
            // th-TH's alternate calendar is the Gregorian one: `วันที่ 15 ตุลาคม 2026`,
            // `วันพฤหัสบดีที่ 15 ตุลาคม 2026` (the picker without an apostrophe).
            let th = load(Some("th-TH")).unwrap();
            for text in [th.date(date), th.long_date(date), th.year_month(date)] {
                assert!(
                    text.ends_with("ตุลาคม 2026") && !text.contains('\''),
                    "{text}"
                );
            }
            // ar-SA has none: its dates are English, its other formats stay.
            let sa = load(Some("ar-SA")).unwrap();
            assert_eq!(sa.date(date), "October 15, 2026");
            assert_eq!(sa.long_date(date), "Thursday, October 15, 2026");
            assert_eq!(sa.year_month(date), "October 2026");
            assert_eq!(sa.months[0], "Jan");
            assert_eq!(sa.weekdays[0], "Su");
            assert_eq!(sa.tag, "ar-SA");
            assert_eq!(sa.list_separator, "; ");
        }

        #[test]
        fn region_list() {
            let regions = regions();
            for tag in ["en-US", "de-CH", "fr-CA", "ja-JP", "ru-RU", "zh-TW"] {
                assert!(regions.iter().any(|name| name.as_str() == tag), "{tag}");
            }
            // Right-to-left scripts, scripts that need shaping, other calendars.
            for tag in [
                "ar-SA",
                "he-IL",
                "hi-IN",
                "th-TH",
                "de-DE_phoneb",
                "de",
                "dua",
            ] {
                assert!(!regions.iter().any(|name| name.as_str() == tag), "{tag}");
            }
            assert_eq!(native_name("de-CH").as_deref(), Some("Deutsch (Schweiz)"));
            assert_eq!(native_name("ca-ES").as_deref(), Some("català (Espanya)"));
            assert_eq!(native_name("es-ES").as_deref(), Some("español (España)"));
            assert_eq!(native_name("zh-CN").as_deref(), Some("中文(中国)"));
        }
    }
}

/// The date difference's words in the interface language, joined with the region's
/// list separator.
pub(super) fn date_words() -> dates::Words<'static> {
    use super::strings::tr;
    dates::Words {
        same_dates: tr("Date_SameDates"),
        year: [tr("Date_Year"), tr("Date_Years")],
        month: [tr("Date_Month"), tr("Date_Months")],
        week: [tr("Date_Week"), tr("Date_Weeks")],
        day: [tr("Date_Day"), tr("Date_Days")],
        separator: &formats().list_separator,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_formats_match_the_date_module() {
        let english = Formats::english();
        let date = NaiveDate::from_ymd_opt(2026, 10, 15).unwrap();
        assert_eq!(english.date(date), "October 15, 2026");
        assert_eq!(english.long_date(date), "Thursday, October 15, 2026");
        assert_eq!(english.year_month(date), "October 2026");
        assert_eq!(english.weekdays[0], "Su");
        assert_eq!(english.example(date), "1,234,567.89 · October 15, 2026");
        assert_eq!(english.country().as_deref(), Some("US"));
    }
}
