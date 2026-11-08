//! The share link and the channel that sends it (docs/support-prompt.md, section 12).

use eframe::egui;

use super::donate::{UTM_MAX, sanitize};

/// The one link people share, for every platform: the page picks the right download and can
/// change without an app release.
// TODO: Owner will confirm the real page (the CalcCraft section of the Rapalon site).
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

/// How a share is sent. Today every platform copies the text to the clipboard; a system share
/// sheet (Android's `ACTION_SEND`) would replace it there.
pub trait ShareChannel {
    /// Sends `text`. Returns true when the text is on the clipboard, so "Link copied" shows.
    fn share(&self, ctx: &egui::Context, text: &str) -> bool;
}

/// Copies the text to the clipboard.
pub struct ClipboardShare;

impl ShareChannel for ClipboardShare {
    fn share(&self, ctx: &egui::Context, text: &str) -> bool {
        ctx.copy_text(text.to_owned());
        true
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
    fn text_is_message_then_link() {
        assert_eq!(share_text("Hi:", "https://a.b"), "Hi: https://a.b");
    }

    #[test]
    fn clipboard_channel_copies() {
        let ctx = egui::Context::default();
        assert!(ClipboardShare.share(&ctx, "text"));
        let copied = ctx.output(|o| {
            o.commands
                .iter()
                .any(|c| matches!(c, egui::OutputCommand::CopyText(t) if t == "text"))
        });
        assert!(copied);
    }
}
