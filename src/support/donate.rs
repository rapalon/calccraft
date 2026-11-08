//! The donate link with its Stripe tracking parameters, and the channel that opens it
//! (docs/support-prompt.md, sections 7 and 8).

use eframe::egui;

/// Donation URL for supporting the project.
// TODO: Owner will confirm the real Stripe page URL.
pub const DONATE_URL: &str = "https://rapalon.com/donate";

/// Stripe's limits: 150 characters for the UTM values, 200 for `client_reference_id`.
const UTM_MAX: usize = 150;
const REFERENCE_MAX: usize = 200;
/// `c10` stands for 10 cycles or more: the count stays coarse.
const MAX_CYCLES: u32 = 10;

/// Where Donate was clicked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DonateSource {
    /// The popup that opened by itself.
    Popup,
    /// The popup opened from the menu's support button.
    Menu,
}

impl DonateSource {
    fn code(self) -> &'static str {
        match self {
            DonateSource::Popup => "popup",
            DonateSource::Menu => "menu",
        }
    }
}

/// The coarse, non-identifying context of a Donate click.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DonateContext {
    /// `win`, `mac`, `linux`, `android`, `web` or `other`.
    pub platform: &'static str,
    pub version: &'static str,
    pub source: DonateSource,
    /// Complete reminder cycles so far.
    pub cycles: u32,
    /// Days since the first launch, if known.
    pub days_since_install: Option<i64>,
}

/// This build's platform code.
pub fn platform() -> &'static str {
    if cfg!(target_arch = "wasm32") {
        "web"
    } else if cfg!(target_os = "android") {
        "android"
    } else if cfg!(windows) {
        "win"
    } else if cfg!(target_os = "macos") {
        "mac"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "other"
    }
}

/// Keeps Stripe's allowed characters (ASCII letters, digits, `-`, `_`), turning dots into
/// dashes, and cuts the value to `max` characters.
pub fn sanitize(value: &str, max: usize) -> String {
    value
        .chars()
        .map(|c| if c == '.' { '-' } else { c })
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(max)
        .collect()
}

/// The days-since-install bucket.
fn age_bucket(days: Option<i64>) -> &'static str {
    match days.unwrap_or(0) {
        ..7 => "d0-7",
        7..30 => "d7-30",
        30..90 => "d30-90",
        90..365 => "d90-365",
        _ => "d365p",
    }
}

/// `base` with the tracking parameters.
pub fn donate_url(base: &str, context: &DonateContext) -> String {
    let platform = sanitize(context.platform, 16);
    let version = sanitize(context.version, 32);
    let source = context.source.code();
    let content = sanitize(&format!("{platform}_{version}"), UTM_MAX);
    let reference = sanitize(
        &format!(
            "cc_{platform}_{version}_{source}_c{}_{}",
            context.cycles.min(MAX_CYCLES),
            age_bucket(context.days_since_install),
        ),
        REFERENCE_MAX,
    );
    let separator = if base.contains('?') { '&' } else { '?' };
    format!(
        "{base}{separator}utm_source=calccraft&utm_medium={source}&utm_campaign=support\
         &utm_content={content}&client_reference_id={reference}"
    )
}

/// How a donation is made. Today it opens the donate page; a store build would take its
/// in-app purchase here instead (Google Play Billing, StoreKit), or offer the paid version.
pub trait DonationChannel {
    /// Starts a donation for a Donate click.
    fn donate(&self, ctx: &egui::Context, context: &DonateContext);
}

/// Opens [`DONATE_URL`] with its tracking parameters in the system browser.
pub struct ExternalLink;

impl DonationChannel for ExternalLink {
    fn donate(&self, ctx: &egui::Context, context: &DonateContext) {
        let url = donate_url(DONATE_URL, context);
        if super::clock::fast_mode() {
            eprintln!("support: donate URL {url}");
        }
        ctx.open_url(egui::OpenUrl::new_tab(url));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context(source: DonateSource, cycles: u32, days: Option<i64>) -> DonateContext {
        DonateContext {
            platform: "win",
            version: "1.4.0",
            source,
            cycles,
            days_since_install: days,
        }
    }

    #[test]
    fn url_with_tracking_parameters() {
        assert_eq!(
            donate_url(DONATE_URL, &context(DonateSource::Popup, 3, Some(45))),
            "https://rapalon.com/donate?utm_source=calccraft&utm_medium=popup\
             &utm_campaign=support&utm_content=win_1-4-0\
             &client_reference_id=cc_win_1-4-0_popup_c3_d30-90"
        );
        assert_eq!(
            donate_url(
                "https://buy.stripe.com/x?a=1",
                &context(DonateSource::Menu, 0, None)
            ),
            "https://buy.stripe.com/x?a=1&utm_source=calccraft&utm_medium=menu\
             &utm_campaign=support&utm_content=win_1-4-0\
             &client_reference_id=cc_win_1-4-0_menu_c0_d0-7"
        );
    }

    #[test]
    fn coarse_buckets() {
        let bucket = |days| age_bucket(Some(days));
        assert_eq!(age_bucket(None), "d0-7");
        assert_eq!(bucket(-3), "d0-7");
        assert_eq!(bucket(6), "d0-7");
        assert_eq!(bucket(7), "d7-30");
        assert_eq!(bucket(29), "d7-30");
        assert_eq!(bucket(30), "d30-90");
        assert_eq!(bucket(90), "d90-365");
        assert_eq!(bucket(364), "d90-365");
        assert_eq!(bucket(365), "d365p");
        assert_eq!(bucket(10_000), "d365p");
        let url = donate_url(DONATE_URL, &context(DonateSource::Popup, 57, Some(400)));
        assert!(url.ends_with("_popup_c10_d365p"), "{url}");
    }

    #[test]
    fn values_are_sanitized() {
        assert_eq!(sanitize("1.4.0-beta+build 7/ü", 150), "1-4-0-betabuild7");
        assert_eq!(sanitize("abcdef", 3), "abc");
        let odd = DonateContext {
            platform: "w i/n",
            version: "2.0.0&x=1",
            ..context(DonateSource::Popup, 1, Some(1))
        };
        let url = donate_url(DONATE_URL, &odd);
        assert!(url.contains("&utm_content=win_2-0-0x1&"), "{url}");
        let query = url.split_once('?').unwrap().1;
        for pair in query.split('&') {
            let (_, value) = pair.split_once('=').unwrap();
            assert!(value.len() <= REFERENCE_MAX);
            assert!(
                value
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
                "{value}"
            );
        }
    }

    #[test]
    fn long_values_are_cut_to_stripes_limits() {
        let version: &'static str = Box::leak("9".repeat(400).into_boxed_str());
        let url = donate_url(
            DONATE_URL,
            &DonateContext {
                version,
                ..context(DonateSource::Menu, 1, Some(1))
            },
        );
        let query = url.split_once('?').unwrap().1;
        for pair in query.split('&') {
            let (key, value) = pair.split_once('=').unwrap();
            let max = if key == "client_reference_id" {
                REFERENCE_MAX
            } else {
                UTM_MAX
            };
            assert!(value.len() <= max, "{key}");
        }
    }

    #[test]
    fn this_platform_has_a_code() {
        assert!(["win", "mac", "linux", "android", "web", "other"].contains(&platform()));
    }
}
