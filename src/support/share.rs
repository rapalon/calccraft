//! The share link and the channel that sends it (docs/support-prompt.md, section 12).

use eframe::egui;

use super::donate::{UTM_MAX, sanitize};

/// The one link people share, for every platform: the page picks the right download and can
/// change without an app release. It is CalcCraft's page on rapalon.com, where the donate link
/// (`DONATE_URL`, under `/donate`) lives too.
pub const SHARE_URL: &str = "https://rapalon.com/calccraft";

/// The coarse, non-identifying context of a share link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShareContext {
    /// `win`, `mac`, `linux`, `android`, `web` or `other`.
    pub platform: &'static str,
    pub version: &'static str,
}

/// `base` with `utm_source`, `utm_medium` and `utm_content`. Unlike the donate link it has no
/// cycle count or age bucket: whoever opens it is somebody else.
pub fn share_url(base: &str, context: &ShareContext) -> String {
    let content = sanitize(
        &format!(
            "{}_{}",
            sanitize(context.platform, 16),
            sanitize(context.version, 32)
        ),
        UTM_MAX,
    );
    let separator = if base.contains('?') { '&' } else { '?' };
    format!("{base}{separator}utm_source=calccraft&utm_medium=share&utm_content={content}")
}

/// The text that is shared: the message, a space, the link.
pub fn share_text(message: &str, url: &str) -> String {
    format!("{message} {url}")
}

/// What a share did, which decides what the user sees.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShareOutcome {
    /// The text is on the clipboard: "Link copied" shows.
    Copied,
    /// A system share sheet took over, which is its own feedback: no note.
    Sheet,
    /// Nothing was shared.
    Failed,
}

impl ShareOutcome {
    /// Whether "Link copied" should show.
    pub fn shows_note(self) -> bool {
        self == Self::Copied
    }
}

/// How a share is sent. Desktops copy the text to the clipboard; the browser opens its share
/// sheet where there is one; Android's system share sheet (`ACTION_SEND`) is a follow-up.
pub trait ShareChannel {
    /// Sends `text`.
    fn share(&self, ctx: &egui::Context, text: &str) -> ShareOutcome;
}

/// Copies the text to the clipboard.
pub struct ClipboardShare;

impl ShareChannel for ClipboardShare {
    fn share(&self, ctx: &egui::Context, text: &str) -> ShareOutcome {
        ctx.copy_text(text.to_owned());
        ShareOutcome::Copied
    }
}

/// The browser's share sheet (`navigator.share`) where it exists, else the clipboard.
#[cfg(target_arch = "wasm32")]
pub struct WebShare;

#[cfg(target_arch = "wasm32")]
impl ShareChannel for WebShare {
    fn share(&self, ctx: &egui::Context, text: &str) -> ShareOutcome {
        if crate::app::web::share_sheet(text) {
            ShareOutcome::Sheet
        } else {
            ClipboardShare.share(ctx, text)
        }
    }
}

/// Android's share sheet (`ACTION_SEND`). egui's clipboard does nothing on Android, so when
/// the sheet can't open the text goes to the system clipboard directly, and only a failure of
/// both is `Failed`.
#[cfg(target_os = "android")]
pub struct AndroidShare;

#[cfg(target_os = "android")]
impl ShareChannel for AndroidShare {
    fn share(&self, _ctx: &egui::Context, text: &str) -> ShareOutcome {
        if crate::app::android::share_sheet(text) {
            ShareOutcome::Sheet
        } else if crate::app::android::copy_text(text) {
            ShareOutcome::Copied
        } else {
            ShareOutcome::Failed
        }
    }
}

/// The channel of this platform.
pub fn platform_channel() -> &'static dyn ShareChannel {
    #[cfg(target_arch = "wasm32")]
    {
        &WebShare
    }
    #[cfg(target_os = "android")]
    {
        &AndroidShare
    }
    #[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
    {
        &ClipboardShare
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context() -> ShareContext {
        ShareContext {
            platform: "win",
            version: "1.3.2",
        }
    }

    #[test]
    fn url_has_only_coarse_utm_parameters() {
        assert_eq!(
            share_url(SHARE_URL, &context()),
            "https://rapalon.com/calccraft?utm_source=calccraft&utm_medium=share\
             &utm_content=win_1-3-2"
        );
        assert_eq!(
            share_url("https://x.example/p?a=1", &context()),
            "https://x.example/p?a=1&utm_source=calccraft&utm_medium=share&utm_content=win_1-3-2"
        );
    }

    #[test]
    fn values_are_sanitized() {
        let odd = ShareContext {
            platform: "w i/n",
            version: "2.0.0&x=1",
        };
        assert!(share_url(SHARE_URL, &odd).ends_with("&utm_content=win_2-0-0x1"));
    }

    #[test]
    fn only_a_copy_shows_the_note() {
        assert!(ShareOutcome::Copied.shows_note());
        assert!(!ShareOutcome::Sheet.shows_note());
        assert!(!ShareOutcome::Failed.shows_note());
    }

    #[test]
    fn text_is_message_then_link() {
        assert_eq!(share_text("Hi:", "https://a.b"), "Hi: https://a.b");
    }

    #[test]
    fn clipboard_channel_copies() {
        let ctx = egui::Context::default();
        assert_eq!(ClipboardShare.share(&ctx, "text"), ShareOutcome::Copied);
        let copied = ctx.output(|o| {
            o.commands
                .iter()
                .any(|c| matches!(c, egui::OutputCommand::CopyText(t) if t == "text"))
        });
        assert!(copied);
    }
}
