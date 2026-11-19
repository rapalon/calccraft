# Regional formats on every platform

CalcCraft follows the user's regional format, as the Windows Calculator does: the decimal and
group separators, the digit grouping, the list separator of a date difference, the first day of
the week, the calendar's weekday and month names, and the date pickers' and results' date
formats. The interface language is separate (`strings.rs`); the formats live in
`src/app/region.rs` (`Formats`, `regions`, `native_name`) and are applied by `locale.rs`.

This document says where each platform reads them, what it offers in Settings → Regional
format, and how the Android reading works. The Windows and web behaviors are the reference; the
tests named here pin them.

## What a region gives

| Part | Windows (`region::windows`) | Web (`region::browser`, `web.rs`) | Android (`region::java`, `android.rs`) |
| --- | --- | --- | --- |
| Tag | `GetUserDefaultLocaleName` | `?region=`, else the first language | `Locale.getDefault(FORMAT).toLanguageTag()` |
| Decimal, group, grouping | `LOCALE_SDECIMAL`, `STHOUSAND`, `SGROUPING` | `Intl.NumberFormat.formatToParts`, a 13-digit sample | `java.text.NumberFormat.format(1234567890123.5)`, parsed |
| List separator | `LOCALE_SLIST` | `;` with a decimal comma, else `,` | the same rule |
| First day of the week | `LOCALE_IFIRSTDAYOFWEEK` | `Intl.Locale.getWeekInfo`, else a country table | `java.util.Calendar.getInstance(locale).getFirstDayOfWeek()` |
| Shortest weekday names | `LOCALE_SSHORTESTDAYNAME` | short names cut to two letters | ICU `DateFormatSymbols` standalone short names, cut to two letters, first letter uppercase |
| Abbreviated month names | `LOCALE_SABBREVMONTHNAME` | `month: short` | ICU standalone abbreviated names, without a final period |
| Picker date | `LOCALE_SLONGDATE` without the weekday | `year, month: long, day` | `DateFormat.getBestDateTimePattern(locale, "yMMMMd")` |
| Long date | `DATE_LONGDATE` | `weekday, year, month, day` | `getBestDateTimePattern(locale, "yMMMMEEEEd")` |
| Month and year | `DATE_YEARMONTH` | `year, month: long` | `getBestDateTimePattern(locale, "yMMMM")` |
| Region list | `EnumSystemLocalesEx`, drawable scripts, Gregorian | the translations' regions | the translations' regions |
| Native name | `LOCALE_SNATIVELANGUAGENAME (SNATIVECOUNTRYNAME)` | `Intl.DisplayNames` | `Locale.getDisplayName(locale)` |

Linux and macOS still use en-US formats and offer no Regional format card ("Linux and macOS"
below says how they would read theirs).

Everywhere the date mode counts Gregorian years: a region whose calendar is another one
(Thai Buddhist, Hijri) gets the Gregorian calendar where the platform can be asked for it (the
`-u-ca-gregory` tag extension in the browser and on Android), and English dates on Windows,
which can't.

## Android

### Where it is read

`android::region_source(choice)` reads one region's raw data through JNI, once per region
(`region::load` keeps every region loaded): `choice` is a tag from the Settings list, or `None`
for the phone's own format locale, `Locale.getDefault(Locale.Category.FORMAT)`, which follows
the system region, Android 13's per-app language, and Android 14's regional preferences (the
first day of the week arrives as a `-u-fw-` extension of its tag, which ICU honors).

The locale the classes are asked with is the tag plus `-u-ca-gregory-nu-latn`
(`java::gregorian`): Gregorian dates and Latin digits, as in the web version. The classes:

- `java.text.NumberFormat.getInstance(locale).format(1234567890123.5)`: the separators and the
  grouping are read off the text (`java::number_format`), as the web version reads
  `formatToParts`. Thirteen integer digits show a secondary group (India's `12,34,56,78,90,123`).
- `java.util.Calendar.getInstance(locale).getFirstDayOfWeek()`: 1 is Sunday, 7 Saturday
  (`java::first_day`).
- `android.icu.text.DateFormatSymbols.getInstance(locale)`: the month names in the format and
  standalone contexts, wide and abbreviated (`MMMM`, `LLLL`, `MMM`, `LLL`), the weekday names
  wide and abbreviated (`EEEE`, `EEE`) and the standalone short ones (`Su`, `Mo`). ICU's arrays
  of weekdays have 8 entries, the first empty.
- `android.text.format.DateFormat.getBestDateTimePattern(locale, skeleton)` for the three
  skeletons above: Java date patterns such as `d. MMMM y`, `EEEE, d MMMM y 'г'.` and
  `LLLL y 'г'.`.
- `Locale.forLanguageTag(tag).getDisplayName(locale)` for the names of the Regional format and
  Language lists (`Deutsch (Deutschland)`, `русский (Россия)`); a name equal to the tag means
  the locale is unknown, and the tag is shown.

### What is pure, and tested on the host

`src/app/region/java.rs` has no JNI: it turns the raw strings (`java::Source`) into `Formats`,
so the host tests cover everything but the calls themselves, with the values Android gives for
en-US, de-DE, ru-RU, ja-JP, hi-IN and th-TH:

- `number_format`: the sample's digits are any `char::is_numeric`; the last separator before the
  final digits is the decimal separator, the others the group separator, and the group sizes are
  the digit runs from the right (`3;0`, `3;2;0`; no groups: no grouping).
- `first_day`: Java's numbering to chrono's `Weekday`.
- `Dates::format`: a Java date pattern formatter for the letters the skeletons produce: `y`
  (`yy` is two digits), `M`/`L` (number, two digits, abbreviated, wide; format and standalone
  names), `d`, `E`/`c` (abbreviated, wide), quoted text (`''` is an apostrophe), and any other
  letter (`G`, the era, in th-TH's Gregorian pattern) left out, with runs of spaces collapsed.
  Unlike `SimpleDateFormat`, it never throws: an odd pattern gives odd text, not a crash.
- The calendar's names: the standalone short weekday names without a period, cut to two
  characters, with the first letter in uppercase (`Пн`, `Lu`, as Windows' shortest names are);
  the standalone abbreviated month names without a period (`Jan`, `янв`).
- `country`: the region of a tag for the converters' default units (`DE` for `de-DE`,
  `IN` for `hi-IN-u-fw-mon`; an extension's two-letter keys don't count).

### Fallback

Every JNI call goes through `android::with_activity`, which logs the first failure and returns
`None`; `region::load` then uses `Formats::english()` (en-US) and logs it. The app never
panics on a missing class or method: a phone whose ICU lacks a name array, or a tag Java
doesn't know, gets en-US formats, as before this change.

### Settings

The Regional format card appears where `region::regions()` is non-empty; on Android the list is
the translations' regions, as in the web version (Android doesn't list its locales), with
"Use system setting" first. The example line under the card title shows the chosen format
(`1.234.567,89 · 15. Oktober 2026`). The choice is saved as `CalcCraftRegionalFormat`, like
everywhere else.

### Checking on a phone

Settings → Regional format: the list shows the regions by their own names and the card's
example changes with the choice. With the phone set to de-DE: `1.234.567,89`, `15. Oktober
2026`, `Donnerstag, 15. Oktober 2026`, the calendar starting on `Mo`, a date difference joined
with `; `. With ru-RU: `1 234 567,89`, `15 октября 2026 г.`, `четверг, 15 октября 2026 г.`,
the calendar's title `октябрь 2026 г.`, its header `Пн … Вс`. The device scenarios
(`tests/device/run.ps1`) already read numbers in the phone's format.

## Linux and macOS (proposal)

Both can be read the same way as Android: a pure module turning raw strings into `Formats`,
with the platform's calls kept apart.

- **macOS:** `CFLocaleCopyCurrent` and `CFLocaleGetValue` (`kCFLocaleDecimalSeparator`,
  `kCFLocaleGroupingSeparator`, `kCFLocaleCalendarIdentifier`), `CFCalendarGetFirstWeekday`,
  `CFDateFormatterCreateDateFormatFromTemplate` for the three skeletons (the same CLDR
  skeletons as Android), `CFDateFormatterCreate` for the names, and `CFLocaleCopyDisplayNameForPropertyValue` for the native names. The region list: `CFLocaleCopyAvailableLocaleIdentifiers`
  filtered like Windows' (regions with a drawable script and the Gregorian calendar), or the
  translations' regions as on Android. The `core-foundation` crate (MIT/Apache-2.0) wraps these.
  Grouping has no secondary size in CFLocale: format a sample number with `CFNumberFormatter`
  and read it off, as on Android.
- **Linux:** no system-wide ICU is guaranteed, so two routes. (1) `LC_NUMERIC`, `LC_TIME` and
  glibc's `nl_langinfo` (`RADIXCHAR`, `THOUSEP`, `_NL_TIME_FIRST_WEEKDAY`, `ABDAY_*`, `ABMON_*`,
  `D_FMT`): no CLDR date skeletons, so the picker date would come from `strftime`'s `%x`
  variants, which differ from Windows' long dates. (2) The `icu` crate (ICU4X, Unicode License
  v3, on `deny.toml`'s allow-list to confirm): `icu_decimal`, `icu_calendar`'s week data and
  `icu_datetime`'s skeleton-based formatting match Android and the browser exactly, and the
  region list would be the translations'. ICU4X adds about 2 MB of data to the binary, or less
  with a data provider limited to the translations' locales. Route 2 is the recommendation: the
  same CLDR data as the other platforms, and the pure assembly in `region/java.rs` could be
  shared (the pattern letters are CLDR's).

Either way `Formats::english()` stays the fallback and `regions()` decides whether the card
shows.
