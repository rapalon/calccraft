//! The interface's text in the user's language. The strings are the Windows Calculator's
//! own translations, imported into `locales/<language>.txt` by `tools/import-strings.ps1`,
//! and the code looks them up by their Windows resource keys (`tr("HistoryLabel.Text")`).
//! Like Windows, the language follows the Windows display language.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError, RwLock};

/// Every language the Windows Calculator is translated into, with its strings.
const LOCALES: &[(&str, &str)] = &[
    ("af-ZA", include_str!("../../locales/af-ZA.txt")),
    ("am-ET", include_str!("../../locales/am-ET.txt")),
    ("ar-SA", include_str!("../../locales/ar-SA.txt")),
    ("az-Latn-AZ", include_str!("../../locales/az-Latn-AZ.txt")),
    ("bg-BG", include_str!("../../locales/bg-BG.txt")),
    ("ca-ES", include_str!("../../locales/ca-ES.txt")),
    ("cs-CZ", include_str!("../../locales/cs-CZ.txt")),
    ("da-DK", include_str!("../../locales/da-DK.txt")),
    ("de-DE", include_str!("../../locales/de-DE.txt")),
    ("el-GR", include_str!("../../locales/el-GR.txt")),
    ("en-GB", include_str!("../../locales/en-GB.txt")),
    ("en-US", include_str!("../../locales/en-US.txt")),
    ("es-ES", include_str!("../../locales/es-ES.txt")),
    ("es-MX", include_str!("../../locales/es-MX.txt")),
    ("et-EE", include_str!("../../locales/et-EE.txt")),
    ("eu-ES", include_str!("../../locales/eu-ES.txt")),
    ("fa-IR", include_str!("../../locales/fa-IR.txt")),
    ("fi-FI", include_str!("../../locales/fi-FI.txt")),
    ("fil-PH", include_str!("../../locales/fil-PH.txt")),
    ("fr-CA", include_str!("../../locales/fr-CA.txt")),
    ("fr-FR", include_str!("../../locales/fr-FR.txt")),
    ("gl-ES", include_str!("../../locales/gl-ES.txt")),
    ("he-IL", include_str!("../../locales/he-IL.txt")),
    ("hi-IN", include_str!("../../locales/hi-IN.txt")),
    ("hr-HR", include_str!("../../locales/hr-HR.txt")),
    ("hu-HU", include_str!("../../locales/hu-HU.txt")),
    ("id-ID", include_str!("../../locales/id-ID.txt")),
    ("is-IS", include_str!("../../locales/is-IS.txt")),
    ("it-IT", include_str!("../../locales/it-IT.txt")),
    ("ja-JP", include_str!("../../locales/ja-JP.txt")),
    ("kk-KZ", include_str!("../../locales/kk-KZ.txt")),
    ("km-KH", include_str!("../../locales/km-KH.txt")),
    ("kn-IN", include_str!("../../locales/kn-IN.txt")),
    ("ko-KR", include_str!("../../locales/ko-KR.txt")),
    ("lo-LA", include_str!("../../locales/lo-LA.txt")),
    ("lt-LT", include_str!("../../locales/lt-LT.txt")),
    ("lv-LV", include_str!("../../locales/lv-LV.txt")),
    ("mk-MK", include_str!("../../locales/mk-MK.txt")),
    ("ml-IN", include_str!("../../locales/ml-IN.txt")),
    ("ms-MY", include_str!("../../locales/ms-MY.txt")),
    ("nb-NO", include_str!("../../locales/nb-NO.txt")),
    ("nl-NL", include_str!("../../locales/nl-NL.txt")),
    ("pl-PL", include_str!("../../locales/pl-PL.txt")),
    ("pt-BR", include_str!("../../locales/pt-BR.txt")),
    ("pt-PT", include_str!("../../locales/pt-PT.txt")),
    ("ro-RO", include_str!("../../locales/ro-RO.txt")),
    ("ru-RU", include_str!("../../locales/ru-RU.txt")),
    ("sk-SK", include_str!("../../locales/sk-SK.txt")),
    ("sl-SI", include_str!("../../locales/sl-SI.txt")),
    ("sq-AL", include_str!("../../locales/sq-AL.txt")),
    ("sr-Latn-RS", include_str!("../../locales/sr-Latn-RS.txt")),
    ("sv-SE", include_str!("../../locales/sv-SE.txt")),
    ("ta-IN", include_str!("../../locales/ta-IN.txt")),
    ("te-IN", include_str!("../../locales/te-IN.txt")),
    ("th-TH", include_str!("../../locales/th-TH.txt")),
    ("tr-TR", include_str!("../../locales/tr-TR.txt")),
    ("uk-UA", include_str!("../../locales/uk-UA.txt")),
    ("vi-VN", include_str!("../../locales/vi-VN.txt")),
    ("zh-CN", include_str!("../../locales/zh-CN.txt")),
    ("zh-TW", include_str!("../../locales/zh-TW.txt")),
];

/// Languages whose scripts egui can't draw: its text layout has neither bidirectional
/// text nor shaping. Arabic, Persian and Hebrew are written right to left (and the
/// Arabic script needs shaping too), and the Indic scripts, Khmer and Lao need shaping.
/// Windows users with one of these languages get the next language in their list, or
/// English.
const UNSUPPORTED: &[&str] = &[
    "ar-SA", "fa-IR", "he-IL", "hi-IN", "kn-IN", "km-KH", "lo-LA", "ml-IN", "ta-IN", "te-IN",
];

/// CalcCraft's own strings, which the Windows Calculator doesn't have (the Language and
/// Regional format settings), in `[language]` sections. Translated for CalcCraft.
const OWN: &str = include_str!("../../locales/calccraft.txt");

const ENGLISH: &str = "en-US";

/// The strings of one language, with English for anything missing.
struct Strings {
    language: &'static str,
    table: HashMap<&'static str, String>,
    english: HashMap<&'static str, String>,
}

/// The language in use. Every language loaded stays in `LOADED` for as long as the app
/// runs, since `tr` hands out `&'static str`; switching back doesn't load it again.
static CURRENT: RwLock<Option<&'static Strings>> = RwLock::new(None);
static LOADED: Mutex<Vec<&'static Strings>> = Mutex::new(Vec::new());

impl Strings {
    fn new(language: &'static str) -> Self {
        let mut table = parse(locale(language).unwrap_or_default());
        table.extend(parse(own(language)));
        let mut english = parse(locale(ENGLISH).unwrap_or_default());
        english.extend(parse(own(ENGLISH)));
        Self {
            language,
            table,
            english,
        }
    }

    fn get(&'static self, key: &str) -> &'static str {
        match self.table.get(key).or_else(|| self.english.get(key)) {
            Some(text) => text,
            None => {
                debug_assert!(false, "no string {key:?}");
                ""
            }
        }
    }
}

fn locale(language: &str) -> Option<&'static str> {
    LOCALES
        .iter()
        .find(|(tag, _)| *tag == language)
        .map(|(_, text)| *text)
}

/// The `[language]` section of CalcCraft's own strings (empty if there is none).
fn own(language: &str) -> &'static str {
    let header = format!("[{language}]");
    let Some(start) = OWN.lines().position(|line| line.trim() == header) else {
        return "";
    };
    // Byte offsets of the section: from the line after the header to the next header.
    let mut offset = 0;
    let mut begin = None;
    for (i, line) in OWN.split_inclusive('\n').enumerate() {
        if i == start + 1 {
            begin = Some(offset);
        }
        if i > start && line.starts_with('[') {
            return &OWN[begin.unwrap_or(offset)..offset];
        }
        offset += line.len();
    }
    begin.map_or("", |begin| &OWN[begin..])
}

/// Reads `key=value` lines; values write line breaks as `\n` and backslashes as `\\`.
fn parse(text: &'static str) -> HashMap<&'static str, String> {
    text.lines()
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| {
            let mut unescaped = String::with_capacity(value.len());
            let mut chars = value.chars();
            while let Some(c) = chars.next() {
                if c != '\\' {
                    unescaped.push(c);
                } else if let Some(next) = chars.next() {
                    unescaped.push(if next == 'n' { '\n' } else { next });
                }
            }
            (key, unescaped)
        })
        .collect()
}

/// Switches the interface to a language: `choice` is one of `languages()`, or `None` for the
/// first of the Windows display languages (`preferred_languages`) that has a translation
/// egui can draw, or English. Returns the language now in use.
pub(super) fn set_language(choice: Option<&str>) -> &'static str {
    let language = choice
        .and_then(match_language)
        .or_else(|| {
            preferred_languages()
                .iter()
                .find_map(|tag| match_language(tag))
        })
        .unwrap_or(ENGLISH);
    let strings = load(language);
    *CURRENT.write().unwrap_or_else(PoisonError::into_inner) = Some(strings);
    strings.language
}

/// The language in use.
pub(super) fn language() -> &'static str {
    strings().language
}

/// The languages to choose from: every translation egui can draw.
pub(super) fn languages() -> impl Iterator<Item = &'static str> {
    LOCALES
        .iter()
        .map(|(tag, _)| *tag)
        .filter(|tag| drawable(tag))
}

/// Whether egui can draw a language: not one of `UNSUPPORTED`, and in the web version and the
/// Android app not one that needs a Windows font from `script_font` either, since a browser
/// page can't read the system's fonts and Android has other ones.
fn drawable(tag: &str) -> bool {
    let windows_fonts = !cfg!(any(target_arch = "wasm32", target_os = "android"));
    !UNSUPPORTED.contains(&tag) && (windows_fonts || script_font(tag).is_none())
}

fn load(language: &'static str) -> &'static Strings {
    let mut loaded = LOADED.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(strings) = loaded.iter().find(|strings| strings.language == language) {
        return strings;
    }
    let strings: &'static Strings = Box::leak(Box::new(Strings::new(language)));
    loaded.push(strings);
    strings
}

/// The strings in use; English until `set_language` picks a language.
fn strings() -> &'static Strings {
    if let Some(strings) = *CURRENT.read().unwrap_or_else(PoisonError::into_inner) {
        return strings;
    }
    let english = load(ENGLISH);
    CURRENT
        .write()
        .unwrap_or_else(PoisonError::into_inner)
        .get_or_insert(english);
    english
}

/// The translation of `key`, from the Windows Calculator's resources.
pub(crate) fn tr(key: &str) -> &'static str {
    strings().get(key)
}

/// A translated format string with `%1`, `%2`, … replaced by `args`, the way the
/// Windows Calculator fills them in with `FormatMessage`.
pub(super) fn tr_format(key: &str, args: &[&str]) -> String {
    format_message(tr(key), args)
}

fn format_message(format: &str, args: &[&str]) -> String {
    let mut text = format.to_string();
    // From the highest number down, so `%1` doesn't match the start of `%10`.
    for (i, arg) in args.iter().enumerate().rev() {
        text = text.replace(&format!("%{}", i + 1), arg);
    }
    text
}

/// A unit's name in the drop-down lists (`key` is `Unit::key`).
pub(super) fn unit_name(key: &str) -> &'static str {
    tr(&format!("UnitName_{key}"))
}

/// A unit's short name in the "About equal to" results.
pub(super) fn unit_abbreviation(key: &str) -> &'static str {
    tr(&format!("UnitAbbreviation_{key}"))
}

/// The translation that best matches a language tag, like Windows' resource lookup:
/// the same tag, or the same language in another region, but not in another script
/// (Windows scores a script mismatch as no match, so `az-Cyrl-AZ` gets none). Chinese goes
/// by script, then by region, and Spanish, Portuguese, French and English pick the regional
/// variant that fits best.
fn match_language(tag: &str) -> Option<&'static str> {
    let tag = tag.to_ascii_lowercase();
    let mut parts = tag.split(['-', '_']);
    let language = parts.next()?;
    let rest: Vec<&str> = parts.collect();
    let has = |part: &str| rest.contains(&part);
    let script = script_subtag(&rest);
    let tags = || LOCALES.iter().map(|(tag, _)| *tag);
    let chosen = match language {
        // The script decides before the region: zh-Hans-HK is Simplified Chinese.
        "zh" => match script {
            Some("hans") => "zh-CN",
            Some("hant") => "zh-TW",
            Some(_) => return None,
            None if has("tw") || has("hk") || has("mo") => "zh-TW",
            None => "zh-CN",
        },
        "no" | "nn" => "nb-NO",
        "es" if rest.is_empty() || has("es") => "es-ES",
        "es" => "es-MX",
        "pt" if has("pt") => "pt-PT",
        "pt" => "pt-BR",
        "fr" if has("ca") => "fr-CA",
        "fr" => "fr-FR",
        "en" if rest.is_empty() || has("us") => ENGLISH,
        "en" => "en-GB",
        _ => tags()
            .find(|candidate| candidate.to_ascii_lowercase() == tag)
            .or_else(|| {
                tags().find(|candidate| {
                    let candidate = candidate.to_ascii_lowercase();
                    let subtags: Vec<&str> = candidate.split('-').collect();
                    let other = script_subtag(&subtags[1..]);
                    subtags[0] == language && other.zip(script).is_none_or(|(a, b)| a == b)
                })
            })?,
    };
    drawable(chosen).then_some(chosen)
}

/// The script subtag of a lowercase language tag (`hans` in `zh-hans-hk`), from its subtags
/// after the language: four letters, right after it.
fn script_subtag<'a>(subtags: &[&'a str]) -> Option<&'a str> {
    let first = subtags.first().copied()?;
    (first.len() == 4 && first.bytes().all(|b| b.is_ascii_alphabetic())).then_some(first)
}

/// The user's preferred display languages, most preferred first. The environment
/// variable `CALCCRAFT_LANGUAGE` (for example `de-DE`) overrides them; in the web version
/// they are the browser's languages, and the page address's `?lang=de-DE` overrides them;
/// in the Android app they are the phone's languages.
pub(super) fn preferred_languages() -> Vec<String> {
    if let Ok(language) = std::env::var("CALCCRAFT_LANGUAGE") {
        return vec![language];
    }
    #[cfg(target_arch = "wasm32")]
    return super::web::query("lang").map_or_else(super::web::languages, |language| vec![language]);
    #[cfg(target_os = "android")]
    return super::android::languages();
    #[cfg(windows)]
    {
        use windows_sys::Win32::Globalization::{GetUserPreferredUILanguages, MUI_LANGUAGE_NAME};
        let (mut count, mut length) = (0u32, 0u32);
        // SAFETY: the first call only asks for the buffer length; the second one writes at
        // most `length` UTF-16 units into a buffer of that length.
        let ok = unsafe {
            GetUserPreferredUILanguages(
                MUI_LANGUAGE_NAME,
                &mut count,
                std::ptr::null_mut(),
                &mut length,
            ) != 0
        };
        if ok {
            let mut buffer = vec![0u16; length as usize];
            // SAFETY: as above.
            let ok = unsafe {
                GetUserPreferredUILanguages(
                    MUI_LANGUAGE_NAME,
                    &mut count,
                    buffer.as_mut_ptr(),
                    &mut length,
                ) != 0
            };
            if ok {
                // Zero-terminated names, and an empty one at the end.
                return String::from_utf16_lossy(&buffer)
                    .split('\0')
                    .filter(|name| !name.is_empty())
                    .map(str::to_string)
                    .collect();
            }
        }
    }
    #[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
    Vec::new()
}

/// A Windows font file and its index in the font collection (the "UI" variants of the
/// CJK fonts).
pub(super) type ScriptFont = (&'static str, u32);

const JAPANESE: ScriptFont = ("YuGothM.ttc", 1);
const SIMPLIFIED_CHINESE: ScriptFont = ("msyh.ttc", 1);
const TRADITIONAL_CHINESE: ScriptFont = ("msjh.ttc", 1);
const KOREAN: ScriptFont = ("malgun.ttf", 0);
const THAI: ScriptFont = ("LeelawUI.ttf", 0);
const ETHIOPIC: ScriptFont = ("ebrima.ttf", 0);

/// The fonts for lists that show languages and regions in their own scripts. Yu Gothic has
/// the Chinese characters of their names too (`中文(台灣)`, `中文(香港特別行政區)`), so the
/// two Chinese fonts, some 40 MB, stay out.
pub(super) const LIST_FONTS: [ScriptFont; 4] = [JAPANESE, KOREAN, THAI, ETHIOPIC];

/// The Windows font with the script of a language or region (a tag like `ja-JP` or
/// `zh-Hant-HK`) where Segoe UI doesn't cover it.
pub(super) fn script_font(tag: &str) -> Option<ScriptFont> {
    let tag = tag.to_ascii_lowercase();
    let mut parts = tag.split(['-', '_']);
    let language = parts.next()?;
    let rest: Vec<&str> = parts.collect();
    // As in `match_language`, the script decides before the region: zh-Hans-HK is Simplified.
    let traditional = match script_subtag(&rest) {
        Some(script) => script == "hant",
        None => ["tw", "hk", "mo"].iter().any(|part| rest.contains(part)),
    };
    match language {
        "ja" => Some(JAPANESE),
        "zh" if traditional => Some(TRADITIONAL_CHINESE),
        "zh" => Some(SIMPLIFIED_CHINESE),
        "ko" => Some(KOREAN),
        "th" => Some(THAI),
        // Amharic, Tigrinya and the other languages written in Ge'ez.
        "am" | "ti" | "byn" | "gez" | "tig" | "wal" => Some(ETHIOPIC),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_language_has_every_string() {
        let english = parse(locale(ENGLISH).unwrap());
        assert!(english.len() > 400);
        for (language, text) in LOCALES {
            let table = parse(text);
            for key in english.keys() {
                assert!(table.contains_key(key), "{language} has no {key}");
            }
        }
    }

    #[test]
    fn every_bit_has_an_ordinal() {
        // The bit toggling keypad names bits 1 to 63 by their ordinals ("63rd bit") unless one
        // is the word's highest bit, so a DWORD, WORD or BYTE needs "63" too.
        for (language, text) in LOCALES {
            let table = parse(text);
            for bit in 1..=63 {
                let key = bit.to_string();
                assert!(table.contains_key(key.as_str()), "{language} has no {key}");
            }
        }
        assert_eq!(load(ENGLISH).get("63"), "63rd");
    }

    #[test]
    fn languages_match_like_windows() {
        assert_eq!(match_language("de-DE"), Some("de-DE"));
        assert_eq!(match_language("de-AT"), Some("de-DE"));
        assert_eq!(match_language("en-US"), Some("en-US"));
        assert_eq!(match_language("en-AU"), Some("en-GB"));
        assert_eq!(match_language("es-AR"), Some("es-MX"));
        assert_eq!(match_language("es-ES"), Some("es-ES"));
        assert_eq!(match_language("pt-PT"), Some("pt-PT"));
        assert_eq!(match_language("pt-AO"), Some("pt-BR"));
        assert_eq!(match_language("fr-CH"), Some("fr-FR"));
        assert_eq!(match_language("fr-CA"), Some("fr-CA"));
        assert_eq!(match_language("zh-Hans-CN"), Some("zh-CN"));
        assert_eq!(match_language("zh-Hant-HK"), Some("zh-TW"));
        assert_eq!(match_language("sr-Latn-RS"), Some("sr-Latn-RS"));
        assert_eq!(match_language("sr-Cyrl-RS"), None);
        assert_eq!(match_language("nb-NO"), Some("nb-NO"));
        assert_eq!(match_language("ja"), Some("ja-JP"));
        // Not drawable without bidirectional text and shaping.
        assert_eq!(match_language("ar-SA"), None);
        assert_eq!(match_language("he-IL"), None);
        assert_eq!(match_language("hi-IN"), None);
        // Not translated.
        assert_eq!(match_language("xx-YY"), None);
    }

    #[test]
    fn scripts_match_before_regions() {
        // Chinese (Simplified, Hong Kong SAR) and (Simplified, Macao SAR) in Windows.
        assert_eq!(match_language("zh-Hans-HK"), Some("zh-CN"));
        assert_eq!(match_language("zh-Hans-MO"), Some("zh-CN"));
        assert_eq!(match_language("zh-Hant"), Some("zh-TW"));
        assert_eq!(match_language("zh-Hant-SG"), Some("zh-TW"));
        // Without a script, the region tells it.
        assert_eq!(match_language("zh-HK"), Some("zh-TW"));
        assert_eq!(match_language("zh-SG"), Some("zh-CN"));
        // Windows doesn't fall back to a translation in another script.
        assert_eq!(match_language("az-Cyrl-AZ"), None);
        assert_eq!(match_language("sr-Cyrl"), None);
        assert_eq!(match_language("zh-Latn"), None);
        // The same script, or none, still matches.
        assert_eq!(match_language("az-Latn"), Some("az-Latn-AZ"));
        assert_eq!(match_language("az-AZ"), Some("az-Latn-AZ"));
        assert_eq!(match_language("sr"), Some("sr-Latn-RS"));
        assert_eq!(match_language("de-Latn-DE"), Some("de-DE"));
        // The Chinese fonts go by the script first too: a regional format can be zh-Hans-HK.
        assert_eq!(script_font("zh-Hans-HK"), Some(SIMPLIFIED_CHINESE));
        assert_eq!(script_font("zh-Hans-MO"), Some(SIMPLIFIED_CHINESE));
        assert_eq!(script_font("zh-Hant-SG"), Some(TRADITIONAL_CHINESE));
        assert_eq!(script_font("zh-HK"), Some(TRADITIONAL_CHINESE));
    }

    #[test]
    fn every_language_has_calccraft_strings() {
        let core_keys = [
            "CalcCraft.Language",
            "CalcCraft.LanguageDescription",
            "CalcCraft.RegionalFormat",
            "CalcCraft.Contribute",
        ];
        for language in languages() {
            let table = parse(own(language));
            for key in core_keys {
                assert!(table.contains_key(key), "{language} has no {key}");
            }
        }
        // The tables are looked up without the global language, which other tests use.
        assert_eq!(load("de-DE").get("CalcCraft.Language"), "Sprache");
        assert_eq!(load("de-DE").get("HistoryLabel.Text"), "Verlauf");
        assert_eq!(load("ja-JP").get("CalcCraft.RegionalFormat"), "地域設定");
        // Every translation has the place of its link.
        for language in languages() {
            let contribute = load(language).get("CalcCraft.Contribute");
            assert_eq!(contribute.matches("%1").count(), 1, "{language}");
        }
        // Languages without their own section fall back to English.
        assert_eq!(load("ar-SA").get("CalcCraft.Language"), "Language");
        // Support strings fall back to English for all languages without explicit translations.
        assert_eq!(
            load("de-DE").get("CalcCraft.Support.MenuButton"),
            "Please Support!"
        );
        assert_eq!(
            load("ja-JP").get("CalcCraft.Support.Title"),
            "Please support us!"
        );
        assert_eq!(load("fr-FR").get("CalcCraft.Support.Donate"), "Donate");
        assert_eq!(languages().count(), LOCALES.len() - UNSUPPORTED.len());
    }

    #[test]
    fn script_fonts() {
        assert_eq!(script_font("ja-JP"), Some(JAPANESE));
        assert_eq!(script_font("zh-CN"), Some(SIMPLIFIED_CHINESE));
        assert_eq!(script_font("zh-SG"), Some(SIMPLIFIED_CHINESE));
        assert_eq!(script_font("zh-Hant-HK"), Some(TRADITIONAL_CHINESE));
        assert_eq!(script_font("zh-TW"), Some(TRADITIONAL_CHINESE));
        assert_eq!(script_font("ko-KR"), Some(KOREAN));
        assert_eq!(script_font("ti-ET"), Some(ETHIOPIC));
        assert_eq!(script_font("de-DE"), None);
        assert_eq!(script_font("ru-RU"), None);
    }

    #[test]
    fn format_strings() {
        assert_eq!(format_message("Display is %1", &["5"]), "Display is 5");
        assert_eq!(
            format_message("%1 %2", &["Length", "Converter"]),
            "Length Converter"
        );
    }

    #[test]
    fn english_strings() {
        assert_eq!(tr("HistoryLabel.Text"), "History");
        assert_eq!(tr("negateButton.AutomationName"), "Positive negative");
        assert_eq!(tr("CEngine.99"), "Cannot divide by zero");
        assert_eq!(unit_name("Meter"), "Meters");
        assert_eq!(unit_abbreviation("Banana"), "bananas");
    }
}
