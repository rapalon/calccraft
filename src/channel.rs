//! The build's distribution channel (docs/support-prompt.md, section 8), chosen with Cargo
//! features: the default (GitHub, F-Droid, desktops, web) has the donate link, and the Google
//! Play builds have none, because Play's Payments policy forbids leading users to other
//! payment methods. `play-free` leads to the paid app's Play listing instead, and `play-paid`
//! shows no prompt at all. The channel also gates the colour themes (docs/color-themes.md),
//! so this module is in every build, with or without `support-prompt`.

// Without the prompt only the themes' part (the channel, the paid app) is used.
#![cfg_attr(not(feature = "support-prompt"), allow(dead_code))]

#[cfg(all(feature = "play-free", feature = "play-paid"))]
compile_error!("the features `play-free` and `play-paid` exclude each other");

/// Where this build is distributed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    /// GitHub releases, F-Droid, desktops and the web: Donate opens the donate page.
    Donate,
    /// The free Google Play app: the support action opens the paid app's listing.
    PlayFree,
    /// The paid Google Play app: no prompt, always "Supported by you".
    PlayPaid,
}

/// This build's channel.
pub const CHANNEL: Channel = if cfg!(feature = "play-free") {
    Channel::PlayFree
} else if cfg!(feature = "play-paid") {
    Channel::PlayPaid
} else {
    Channel::Donate
};

/// The paid app's application ID as given at build time (`CALCCRAFT_PAID_ID`), unchecked.
const PAID_ID_RAW: Option<&str> = option_env!("CALCCRAFT_PAID_ID");

/// How much of the support prompt a build shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptMode {
    /// The staged policy: dots, the popup and the menu's support button.
    Full,
    /// Nothing at all: no dot, no popup, no menu entry (a free Play build without a paid app).
    Hidden,
    /// No dot and no popup; the menu always shows "Supported by you" (the paid Play app).
    Supporter,
}

/// Whether `id` is a valid Android package name: at least two dot-separated parts of ASCII
/// letters, digits and `_`, each starting with a letter.
pub fn is_package_name(id: &str) -> bool {
    let mut parts = 0;
    for part in id.split('.') {
        parts += 1;
        let mut chars = part.chars();
        if !chars.next().is_some_and(|c| c.is_ascii_alphabetic())
            || !chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return false;
        }
    }
    parts >= 2
}

/// `raw` if it is a package name: anything else counts as unset.
fn checked_id(raw: Option<&'static str>) -> Option<&'static str> {
    raw.filter(|id| is_package_name(id))
}

/// The paid app's application ID, if this build was given a valid one.
pub fn paid_id() -> Option<&'static str> {
    checked_id(PAID_ID_RAW)
}

/// The prompt mode of `channel` with the paid app `paid_id`.
pub fn prompt_mode_of(channel: Channel, paid_id: Option<&str>) -> PromptMode {
    match channel {
        Channel::Donate => PromptMode::Full,
        Channel::PlayFree if paid_id.is_some() => PromptMode::Full,
        Channel::PlayFree => PromptMode::Hidden,
        Channel::PlayPaid => PromptMode::Supporter,
    }
}

/// This build's prompt mode.
pub fn prompt_mode() -> PromptMode {
    prompt_mode_of(CHANNEL, checked_id(PAID_ID_RAW))
}

/// Whether `channel` has the self-declared supporter state ("I already supported", then
/// "Supported by you" for 365 days). Only the default channel does: in the free Play app a user
/// either bought CalcCraft Pro or didn't, and the paid app is always a supporter by itself.
pub fn self_declared_support_of(channel: Channel) -> bool {
    channel == Channel::Donate
}

/// Whether this build has the self-declared supporter state.
pub fn self_declared_support() -> bool {
    self_declared_support_of(CHANNEL)
}

/// The paid app's listing: the Play Store app's `market://` address, and the web page for
/// where nothing takes it. No query parameters.
pub fn store_urls(paid_id: &str) -> [String; 2] {
    [
        format!("market://details?id={paid_id}"),
        format!("https://play.google.com/store/apps/details?id={paid_id}"),
    ]
}

/// Whether this build's links carry tracking parameters: only the default channel's do.
pub fn link_parameters() -> bool {
    CHANNEL == Channel::Donate
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_of_the_channels() {
        assert_eq!(prompt_mode_of(Channel::Donate, None), PromptMode::Full);
        let pro = Some("com.rapalon.calccraft.pro");
        assert_eq!(prompt_mode_of(Channel::PlayFree, pro), PromptMode::Full);
        assert_eq!(prompt_mode_of(Channel::PlayFree, None), PromptMode::Hidden);
        assert_eq!(
            prompt_mode_of(Channel::PlayPaid, pro),
            PromptMode::Supporter
        );
        assert_eq!(
            prompt_mode_of(Channel::PlayPaid, None),
            PromptMode::Supporter
        );
    }

    #[test]
    fn this_build() {
        let expected = match CHANNEL {
            Channel::Donate => PromptMode::Full,
            Channel::PlayFree if paid_id().is_some() => PromptMode::Full,
            Channel::PlayFree => PromptMode::Hidden,
            Channel::PlayPaid => PromptMode::Supporter,
        };
        assert_eq!(prompt_mode(), expected);
        assert_eq!(link_parameters(), CHANNEL == Channel::Donate);
        assert_eq!(self_declared_support(), CHANNEL == Channel::Donate);
        assert!(self_declared_support_of(Channel::Donate));
        assert!(!self_declared_support_of(Channel::PlayFree));
        assert!(!self_declared_support_of(Channel::PlayPaid));
        #[cfg(not(any(feature = "play-free", feature = "play-paid")))]
        assert_eq!(CHANNEL, Channel::Donate);
        #[cfg(feature = "play-free")]
        assert_eq!(CHANNEL, Channel::PlayFree);
        #[cfg(feature = "play-paid")]
        assert_eq!(CHANNEL, Channel::PlayPaid);
    }

    #[test]
    fn only_package_names_count() {
        assert_eq!(
            checked_id(Some("com.rapalon.calccraft.pro")),
            Some("com.rapalon.calccraft.pro")
        );
        assert_eq!(checked_id(Some("a.b_2")), Some("a.b_2"));
        for bad in [
            "",
            "calccraft",
            "com..pro",
            "com.rapalon.",
            ".com.rapalon",
            "com.1pro",
            "com._pro",
            "com.rapalon.pro&x=1",
            "com.rapalon pro",
            "com.rapalon.pro?id=1",
        ] {
            assert_eq!(checked_id(Some(bad)), None, "{bad:?}");
        }
        assert_eq!(checked_id(None), None);
    }

    #[test]
    fn listing_urls_have_only_the_id() {
        assert_eq!(
            store_urls("com.rapalon.calccraft.pro"),
            [
                "market://details?id=com.rapalon.calccraft.pro",
                "https://play.google.com/store/apps/details?id=com.rapalon.calccraft.pro",
            ]
        );
    }
}
