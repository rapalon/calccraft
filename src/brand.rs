//! The company behind CalcCraft, as the title, the About card and the support popup credit it.
//! These are names: they aren't translated.

/// The window's title and the app's full name. A launcher's short label stays "CalcCraft".
/// The browser takes it from `index.html` (checked by a test).
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub const TITLE: &str = "CalcCraft by Rapalon Labs";
/// The credit under the app's name on the About card.
pub const CREDIT: &str = "by Rapalon Labs";
/// The company's name: the part of a credit that links to `URL`.
pub const COMPANY: &str = "Rapalon Labs";
/// The company's site.
pub const URL: &str = "https://rapalon.com";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_credits_say_the_same() {
        assert_eq!(TITLE, format!("CalcCraft {CREDIT}"));
        assert!(CREDIT.ends_with(COMPANY));
    }

    #[test]
    fn every_platform_has_the_title() {
        // The files that can't use the constant: the web page, the installed web app and the
        // Windows executable's properties.
        let page = include_str!("../index.html");
        assert!(page.contains(&format!("<title>{TITLE}</title>")));
        let manifest = include_str!("../assets/web/manifest.webmanifest");
        assert!(manifest.contains(&format!("\"name\": \"{TITLE}\"")));
        let build = include_str!("../build.rs");
        assert!(build.contains(&format!(".set(\"FileDescription\", \"{TITLE}\")")));
    }

    #[test]
    fn launcher_labels_stay_short() {
        // A launcher cuts a long label off, so these keep the app's own name.
        let manifest = include_str!("../assets/web/manifest.webmanifest");
        assert!(manifest.contains("\"short_name\": \"CalcCraft\""));
        let android = include_str!("../android/AndroidManifest.xml");
        assert!(android.contains("android:label=\"CalcCraft\""));
    }
}
