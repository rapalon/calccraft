//! The web version's ties to the browser (see `docs/web-version-plan.md`): the page
//! address's parameters, the browser's languages and regional formats (`Intl`), the
//! clipboard, downloads, and the browser shortcuts that would take the calculator's keys.
//! What Windows' APIs give the desktop version, these give the browser one.

use std::sync::{Mutex, PoisonError};

use chrono::{Datelike, NaiveDate, Weekday};
use js_sys::{Array, Function, Object, Reflect};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};

use crate::locale::NumberFormat;

/// A parameter of the page's address (`?lang=de-DE`): in the browser they stand in for the
/// desktop's `CALCCRAFT_LANGUAGE` and `CALCCRAFT_REGION`.
pub(super) fn query(name: &str) -> Option<String> {
    let search = web_sys::window()?.location().search().ok()?;
    search.trim_start_matches('?').split('&').find_map(|pair| {
        let (key, value) = pair.split_once('=')?;
        (key == name && !value.is_empty()).then(|| value.to_string())
    })
}

/// The browser's languages, most preferred first (`navigator.languages`, like `de-DE`).
pub(super) fn languages() -> Vec<String> {
    let Some(navigator) = web_sys::window().map(|window| window.navigator()) else {
        return Vec::new();
    };
    let languages: Vec<String> = navigator
        .languages()
        .iter()
        .filter_map(|tag| tag.as_string())
        .collect();
    if languages.is_empty() {
        navigator.language().into_iter().collect()
    } else {
        languages
    }
}

/// The browser's regional format: browsers have none apart from the language, so it's the
/// first language, or the page address's `?region=de-DE`.
pub(super) fn region() -> String {
    query("region")
        .or_else(|| languages().into_iter().next())
        .unwrap_or_else(|| "en-US".into())
}

/// The country of the regional format (`DE` for `de-DE`), for the converters' default units.
pub(super) fn country() -> Option<String> {
    region()
        .split(['-', '_'])
        .skip(1)
        .find(|part| part.len() == 2 && part.chars().all(|c| c.is_ascii_alphabetic()))
        .map(|part| part.to_ascii_uppercase())
}

// ------------------------------------------------------------------ Intl

/// A new `Intl.<class>(tag, options)`, or `None` where the browser refuses the tag (a
/// `RangeError` for `?region=nonsense`) or doesn't have the class.
fn intl(class: &str, tag: &str, options: &Object) -> Option<JsValue> {
    let intl = Reflect::get(&js_sys::global(), &"Intl".into()).ok()?;
    let class: Function = Reflect::get(&intl, &class.into()).ok()?.dyn_into().ok()?;
    Reflect::construct(&class, &Array::of2(&tag.into(), options)).ok()
}

/// Calls `object.method(...args)`, catching what it throws.
fn call(object: &JsValue, method: &str, args: &Array) -> Option<JsValue> {
    let method: Function = Reflect::get(object, &method.into()).ok()?.dyn_into().ok()?;
    Reflect::apply(&method, object, args).ok()
}

fn options(pairs: &[(&str, &str)]) -> Object {
    let options = Object::new();
    for (key, value) in pairs {
        // Setting a property on a plain new object can't fail.
        let _ = Reflect::set(&options, &(*key).into(), &(*value).into());
    }
    options
}

/// The tag with the Gregorian calendar and Latin digits: the date mode counts Gregorian
/// years, as in the desktop version (where `th-TH` falls back to English for this reason),
/// and the engine writes Latin digits.
fn gregorian(tag: &str) -> String {
    if tag.contains("-u-") {
        tag.to_string()
    } else {
        format!("{tag}-u-ca-gregory-nu-latn")
    }
}

/// A date in a region's format (`Intl.DateTimeFormat` with these fields), at noon UTC so no
/// time zone moves it to another day.
pub(super) fn format_date(tag: &str, date: NaiveDate, fields: &[(&str, &str)]) -> Option<String> {
    let mut pairs = fields.to_vec();
    pairs.push(("timeZone", "UTC"));
    let format = intl("DateTimeFormat", &gregorian(tag), &options(&pairs))?;
    let millis = date.and_hms_opt(12, 0, 0)?.and_utc().timestamp_millis() as f64;
    let date = js_sys::Date::new(&millis.into());
    call(&format, "format", &Array::of1(&date))?.as_string()
}

/// The decimal and group separators and the digit grouping of a region, from how
/// `Intl.NumberFormat` writes numbers: `1,234,567,890,123` groups by 3 (`3;0`), India's
/// `12,34,56,78,90,123` by 3 and then 2 (`3;2;0`).
pub(super) fn number_format(tag: &str) -> Option<NumberFormat> {
    let format = intl("NumberFormat", &gregorian(tag), &Object::new())?;
    let parts: Array = call(&format, "formatToParts", &Array::of1(&1234567.5.into()))?
        .dyn_into()
        .ok()?;
    let (mut decimal, mut group) = (".".to_string(), String::new());
    for part in parts.iter() {
        let field = |name: &str| Reflect::get(&part, &name.into()).ok()?.as_string();
        match (field("type"), field("value")) {
            (Some(kind), Some(value)) if kind == "decimal" => decimal = value,
            (Some(kind), Some(value)) if kind == "group" => group = value,
            _ => {}
        }
    }
    let grouping = if group.is_empty() {
        String::new()
    } else {
        let long = call(&format, "format", &Array::of1(&1234567890123.0.into()))?.as_string()?;
        let mut sizes: Vec<usize> = long
            .split(decimal.as_str())
            .next()?
            .split(group.as_str())
            .map(|digits| digits.chars().count())
            .collect();
        // From the right, as Windows counts the groups.
        sizes.reverse();
        match sizes.as_slice() {
            [first, second, _, ..] if first != second => format!("{first};{second};0"),
            [first, ..] => format!("{first};0"),
            [] => "3;0".into(),
        }
    };
    Some(NumberFormat::new(&decimal, &group, &grouping))
}

/// The first day of the week (`Intl.Locale`'s week info, where the browser has it), else
/// Sunday in the regions that start the week on Sunday, and Monday elsewhere.
pub(super) fn first_day(tag: &str) -> Weekday {
    let info = intl("Locale", tag, &Object::new()).and_then(|locale| {
        call(&locale, "getWeekInfo", &Array::new())
            .or_else(|| Reflect::get(&locale, &"weekInfo".into()).ok())
    });
    let day = info
        .and_then(|info| Reflect::get(&info, &"firstDay".into()).ok())
        .and_then(|day| day.as_f64());
    match day {
        // ISO numbering: 1 is Monday, 7 is Sunday.
        Some(day) => match day as u32 {
            1 => Weekday::Mon,
            2 => Weekday::Tue,
            3 => Weekday::Wed,
            4 => Weekday::Thu,
            5 => Weekday::Fri,
            6 => Weekday::Sat,
            _ => Weekday::Sun,
        },
        None => {
            const SUNDAY_FIRST: [&str; 16] = [
                "US", "CA", "MX", "BR", "JP", "KR", "TW", "HK", "IL", "IN", "PH", "ZA", "SA", "CO",
                "PE", "VE",
            ];
            let country = tag
                .split(['-', '_'])
                .skip(1)
                .find(|part| part.len() == 2)
                .map(str::to_ascii_uppercase);
            if country.is_some_and(|country| SUNDAY_FIRST.contains(&country.as_str()))
                || tag.eq_ignore_ascii_case("en")
            {
                Weekday::Sun
            } else {
                Weekday::Mon
            }
        }
    }
}

/// The shortest weekday names, Sunday first, like Windows' `LOCALE_SSHORTESTDAYNAME`:
/// `Intl`'s short names (`Sun`, `So.`, `dim.`) without the period, cut to two letters.
pub(super) fn weekdays(tag: &str) -> Option<[String; 7]> {
    // January 1, 2023 was a Sunday.
    let names: Vec<String> = (1..=7)
        .map(|day| {
            let date = NaiveDate::from_ymd_opt(2023, 1, day)?;
            let name = format_date(tag, date, &[("weekday", "short")])?;
            let name = name.trim_end_matches('.');
            Some(name.chars().take(2).collect())
        })
        .collect::<Option<_>>()?;
    names.try_into().ok()
}

/// The abbreviated month names (`Jan`, `Feb`, …), without a closing period (`Jan.`).
pub(super) fn months(tag: &str) -> Option<[String; 12]> {
    let names: Vec<String> = (1..=12)
        .map(|month| {
            let date = NaiveDate::from_ymd_opt(2023, month, 1)?;
            debug_assert_eq!(date.month(), month);
            let name = format_date(tag, date, &[("month", "short")])?;
            Some(name.trim_end_matches('.').to_string())
        })
        .collect::<Option<_>>()?;
    names.try_into().ok()
}

/// A language's or region's name in its own language (`Deutsch (Deutschland)`), from
/// `Intl.DisplayNames`.
pub(super) fn display_name(tag: &str) -> Option<String> {
    let names = intl("DisplayNames", tag, &options(&[("type", "language")]))?;
    let name = call(&names, "of", &Array::of1(&tag.into()))?.as_string()?;
    // Some browsers write the language's name in lowercase (`español (España)`), as
    // Windows does too; only an unknown tag comes back as itself.
    (name != tag).then_some(name)
}

// ------------------------------------------------------------- clipboard

/// The clipboard's text, read when a menu with Paste opened.
static CLIPBOARD: Mutex<Option<String>> = Mutex::new(None);

/// Starts reading the clipboard's text. Browsers only read it asynchronously (and may ask
/// for permission first), so a menu with Paste starts the read when it opens, and the text
/// is there when Paste is picked.
pub(super) fn prefetch_clipboard() {
    let Some(window) = web_sys::window() else {
        return;
    };
    let promise = window.navigator().clipboard().read_text();
    wasm_bindgen_futures::spawn_local(async move {
        let text = wasm_bindgen_futures::JsFuture::from(promise)
            .await
            .ok()
            .and_then(|text| text.as_string());
        *CLIPBOARD.lock().unwrap_or_else(PoisonError::into_inner) = text;
    });
}

/// The text `prefetch_clipboard` read, if the browser allowed it.
pub(super) fn clipboard_text() -> Option<String> {
    CLIPBOARD
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}

// ------------------------------------------------------------- share

/// Opens the browser's share sheet (`navigator.share`) with `text`. Returns false where the
/// browser has none (most desktop browsers), so the caller copies the text instead. The sheet
/// is its own feedback; a cancelled sheet needs no note either.
pub(crate) fn share_sheet(text: &str) -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };
    let navigator = window.navigator();
    let Ok(share) = js_sys::Reflect::get(&navigator, &"share".into()) else {
        return false;
    };
    let Some(share) = share.dyn_ref::<js_sys::Function>() else {
        return false;
    };
    let data = js_sys::Object::new();
    if js_sys::Reflect::set(&data, &"text".into(), &text.into()).is_err() {
        return false;
    }
    match share.call1(&navigator, &data) {
        Ok(promise) => {
            // Ignore the outcome: a rejected promise is a cancelled sheet.
            if let Ok(promise) = promise.dyn_into::<js_sys::Promise>() {
                wasm_bindgen_futures::spawn_local(async move {
                    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
                });
            }
            true
        }
        Err(_) => false,
    }
}

// ------------------------------------------------------------- downloads

/// Has the browser download `bytes` as a file called `name`. Returns whether it started.
pub(super) fn download(name: &str, mime: &str, bytes: &[u8]) -> bool {
    let start = || -> Option<()> {
        let document = web_sys::window()?.document()?;
        let parts = Array::of1(&js_sys::Uint8Array::from(bytes));
        let properties = web_sys::BlobPropertyBag::new();
        properties.set_type(mime);
        let blob =
            web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &properties).ok()?;
        let url = web_sys::Url::create_object_url_with_blob(&blob).ok()?;
        let link: web_sys::HtmlAnchorElement =
            document.create_element("a").ok()?.dyn_into().ok()?;
        link.set_href(&url);
        link.set_download(name);
        link.click();
        // The object URL isn't revoked: a download may still be reading it, and a few
        // shared images don't matter for a page's lifetime.
        Some(())
    };
    start().is_some()
}

// ------------------------------------------------------------- shortcuts

/// Keeps the browser from acting on the calculator's shortcuts while the page has the
/// focus: Ctrl+R (MR) and F5 (GRAD) would reload the page, Ctrl+L (MC) and Ctrl+E (date
/// calculation) focus the address bar, Ctrl+H (history) opens the browser's history, F3
/// searches and F12 opens the developer tools. eframe itself never stops F5 or Ctrl+R.
/// Browsers don't let a page stop some shortcuts at all (Ctrl+W, Ctrl+T, Ctrl+N).
pub(super) fn guard_shortcuts() {
    let Some(window) = web_sys::window() else {
        return;
    };
    let listener =
        Closure::<dyn Fn(web_sys::KeyboardEvent)>::new(|event: web_sys::KeyboardEvent| {
            if takes_shortcut(&event) {
                event.prevent_default();
            }
        });
    let _ = window.add_event_listener_with_callback("keydown", listener.as_ref().unchecked_ref());
    // The listener stays for as long as the page.
    listener.forget();
}

/// Whether the calculator takes a key from the browser: Ctrl (Cmd) with a letter or with
/// the zoom keys (the graph zooms with them), and the function keys F1–F9 and F12. Copy,
/// cut and paste stay with the browser, which turns them into egui's clipboard events, and
/// so do the developer tools (Ctrl+Shift+I, J).
fn takes_shortcut(event: &web_sys::KeyboardEvent) -> bool {
    let key = event.key();
    if (event.ctrl_key() || event.meta_key()) && !event.alt_key() {
        let key = key.to_ascii_lowercase();
        return match key.as_str() {
            "c" | "v" | "x" => false,
            "i" | "j" if event.shift_key() => false,
            "+" | "-" | "=" | "0" => true,
            letter => letter.len() == 1 && letter.chars().all(|c| c.is_ascii_alphabetic()),
        };
    }
    matches!(
        key.as_str(),
        "F1" | "F2" | "F3" | "F4" | "F5" | "F6" | "F7" | "F8" | "F9" | "F12"
    )
}
