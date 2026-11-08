//! The support prompt's runtime state and drawing: the red dots, the menu button and the
//! popup. It feeds the policy's events and draws its decisions (docs/support-prompt.md).

use std::sync::Arc;

use eframe::Storage;
use eframe::egui::{
    self, Align, Align2, Color32, Context, CornerRadius, Event, FontId, Galley, Id, Key, Modifiers,
    Painter, Rect, Sense, Stroke, StrokeKind, Ui, UiBuilder, pos2, scroll_area::ScrollArea,
    text::LayoutJob, vec2,
};

use crate::app::access;
use crate::app::icons::{ICON_CLOSE, ICON_HEART, ICON_SHARE, icon_label};
use crate::app::palette::Palette;
use crate::app::strings::tr;
use crate::engine::Command;

use super::clock::Clock;
use super::donate::{DonateContext, DonateSource, DonationChannel, ExternalLink, platform};
use super::policy::{
    self, MenuEntry, MomentContext, SupportDecision, SupportEvent, SupportPolicyConfig, Timestamp,
};
use super::share::{
    SHARE_URL, ShareChannel, ShareContext, platform_channel, share_text, share_url,
};
use super::stats::SupportStats;

/// The red dot's colors (Windows' critical red in light, its brighter red in dark).
const DOT_LIGHT: Color32 = Color32::from_rgb(196, 43, 28);
const DOT_DARK: Color32 = Color32::from_rgb(232, 17, 35);
const DOT_RADIUS: f32 = 3.5;

/// The temp data key of the display's rectangle in this pass (`note_display`).
fn display_id() -> Id {
    Id::new("support-display")
}

/// Remembers where the mode's display (result and expression) is drawn in this pass, so the
/// popup goes under it.
pub fn note_display(ctx: &Context, rect: Rect) {
    let pass = ctx.cumulative_pass_nr();
    ctx.data_mut(|data| data.insert_temp(display_id(), (rect, pass)));
}

fn noted_display(ctx: &Context) -> Option<Rect> {
    let pass = ctx.cumulative_pass_nr();
    ctx.data(|data| data.get_temp::<(Rect, u64)>(display_id()))
        .filter(|(_, at)| *at == pass)
        .map(|(rect, _)| rect)
}

fn dark(ctx: &Context) -> bool {
    ctx.theme() == egui::Theme::Dark
}

/// An open popup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OpenPopup {
    /// Opened by itself, not from the menu.
    pub automatic: bool,
    pub opened_at_ms: u64,
    /// "I already supported" was pressed: the thank-you text shows since then.
    pub thank_you_since_ms: Option<u64>,
}

/// What the user did in the popup this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PopupAction {
    Close,
    Donate,
    AlreadySupported,
    Share,
}

/// The support prompt's state for one app session.
pub struct SupportState {
    pub(crate) stats: SupportStats,
    pub(crate) cfg: SupportPolicyConfig,
    pub(crate) clock: Clock,
    channel: &'static dyn DonationChannel,
    pub(crate) share_channel: &'static dyn ShareChannel,
    /// "Link copied" shows until this time (wall clock milliseconds).
    pub(crate) copied_until_ms: Option<u64>,
    pub(crate) popup: Option<OpenPopup>,
    /// When the last successful calculation finished, if there was no input since.
    pub(crate) calculation_at_ms: Option<u64>,
    pub(crate) popup_shown_this_session: bool,
    /// The decision of this frame.
    pub(crate) decision: SupportDecision,
    /// The menu is open: opening it with the dot showing saw the dot, which then stays in
    /// the open menu (on its button and the support button) until it closes.
    menu_open: bool,
    seen_in_menu: bool,
    /// `CALCCRAFT_SUPPORT_DEMO=menu`.
    demo_menu: bool,
}

impl SupportState {
    /// Restores the state and counts this launch.
    pub fn new(storage: Option<&dyn Storage>) -> Self {
        Self::with_clock(storage, Clock::from_env())
    }

    pub(crate) fn with_clock(storage: Option<&dyn Storage>, clock: Clock) -> Self {
        let cfg = if clock.is_fast() {
            SupportPolicyConfig::fast()
        } else {
            SupportPolicyConfig::default()
        };
        let stats = SupportStats::load(storage, clock.is_fast(), |secs| clock.day_of(secs));
        let mut state = Self {
            stats,
            cfg,
            clock,
            channel: &ExternalLink,
            share_channel: platform_channel(),
            copied_until_ms: None,
            popup: None,
            calculation_at_ms: None,
            popup_shown_this_session: false,
            decision: SupportDecision::Nothing,
            menu_open: false,
            seen_in_menu: false,
            demo_menu: false,
        };
        state.apply(SupportEvent::AppLaunched);
        // Developer aids for screenshots: the popup open at start (as if from the menu), and
        // in fast mode a stage to start in.
        #[cfg(not(target_arch = "wasm32"))]
        {
            if std::env::var("CALCCRAFT_SUPPORT_OPEN").is_ok_and(|v| v == "1" || v == "true") {
                state.open_on_demand();
            }
            if state.clock.is_fast()
                && let Ok(demo) = std::env::var("CALCCRAFT_SUPPORT_DEMO")
            {
                state.start_demo(&demo);
            }
        }
        state
    }

    /// Starts in a stage of the flow (`CALCCRAFT_SUPPORT_DEMO`, fast mode only): `dot`,
    /// `menu` (the menu open over the dot), `popup` (opened by itself), `popup-supported`
    /// (after a Donate click, with "I already supported"), `thanks`, `supporter` (the
    /// supporter state, the menu open on its thank-you line) or `supporter-popup` (the popup
    /// as the thank-you line opens it), or `share` (the popup with "Link copied" showing).
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub(crate) fn start_demo(&mut self, demo: &str) {
        let stats = &mut self.stats;
        stats.launches = stats.launches.max(self.cfg.min_launches);
        stats.active_days = stats.active_days.max(self.cfg.min_active_days);
        stats.next_cycle_day = None;
        stats.pause_until_day = None;
        stats.supporter_until_day = None;
        stats.dot_since_day = None;
        let supporter = matches!(demo, "supporter" | "supporter-popup");
        if supporter || matches!(demo, "popup-supported" | "thanks") {
            stats.donate_clicked_ever = true;
        }
        if supporter {
            self.popup = None;
            self.apply(SupportEvent::AlreadySupported);
        }
        if demo == "supporter-popup" {
            self.open_on_demand();
        }
        self.demo_menu = matches!(demo, "menu" | "supporter");
        if demo == "share" {
            // The popup with "Link copied" already showing, for screenshots.
            self.copied_until_ms = Some(self.now().millis + SHARE_DEMO_MS);
        }
        if matches!(demo, "popup" | "popup-supported" | "thanks" | "share") {
            let now = self.now().millis;
            self.popup = Some(OpenPopup {
                automatic: true,
                opened_at_ms: now,
                thank_you_since_ms: (demo == "thanks").then_some(now),
            });
            self.popup_shown_this_session = true;
        }
    }

    /// The demo starts with the navigation menu open (`CALCCRAFT_SUPPORT_DEMO=menu`).
    pub fn demo_opens_menu(&self) -> bool {
        self.demo_menu
    }

    pub fn save(&self, storage: &mut dyn Storage) {
        self.stats.save(storage);
    }

    fn now(&self) -> Timestamp {
        self.clock.now()
    }

    fn apply(&mut self, event: SupportEvent) {
        let now = self.now();
        self.stats = policy::on_event(&self.stats, &self.cfg, event, now);
    }

    /// Whether the popup is open.
    pub fn is_open(&self) -> bool {
        self.popup.is_some()
    }

    /// Whether the red dots show.
    pub fn dot_visible(&self) -> bool {
        self.popup.is_none() && (self.decision != SupportDecision::Nothing || self.seen_in_menu)
    }

    /// After the calculator executed `command`: a successful `=` is a calculation
    /// (docs/support-prompt.md, section 4) and starts the calm-moment wait.
    pub fn after_command(&mut self, command: Command, has_error: bool) {
        if command == Command::Equals && !has_error {
            self.apply(SupportEvent::CalculationDone);
            self.calculation_at_ms = Some(self.now().millis);
        }
    }

    /// At the start of every frame: watches the input, closes the popup on Esc, follows the
    /// menu, and opens the popup at a calm moment. `calculator_mode` is Standard, Scientific
    /// or Programmer on screen; `covered` means something is open over it (a menu, flyout or
    /// page); `menu_open` is the navigation menu.
    pub fn begin_frame(
        &mut self,
        ctx: &Context,
        calculator_mode: bool,
        covered: bool,
        menu_open: bool,
    ) {
        let now = self.now();
        if self
            .copied_until_ms
            .is_some_and(|until| now.millis >= until)
        {
            self.copied_until_ms = None;
        }

        // Any input cancels the calm moment of the last calculation; the calculation of this
        // frame comes after this and starts a new one.
        let input = ctx.input(|i| {
            i.events.iter().any(|event| {
                matches!(
                    event,
                    Event::Key { pressed: true, .. }
                        | Event::Text(_)
                        | Event::Paste(_)
                        | Event::PointerButton { pressed: true, .. }
                        | Event::Touch { .. }
                )
            })
        });
        if input {
            self.calculation_at_ms = None;
        }

        if let Some(popup) = self.popup {
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
                self.close_popup(now);
            } else if let Some(since) = popup.thank_you_since_ms {
                let left = (since + self.cfg.thank_you_ms).saturating_sub(now.millis);
                if left == 0 {
                    self.popup = None;
                } else {
                    ctx.request_repaint_after(std::time::Duration::from_millis(left));
                }
            }
        }

        self.decision = self.evaluate(ctx, now, calculator_mode && !covered);
        if self.decision != SupportDecision::Nothing && self.stats.dot_since_day.is_none() {
            self.apply(SupportEvent::DotShown);
        }

        if menu_open && !self.menu_open {
            self.seen_in_menu = self.dot_visible();
            self.apply(SupportEvent::MenuOpened);
        } else if !menu_open {
            self.seen_in_menu = false;
        }
        self.menu_open = menu_open;

        self.decision = self.evaluate(ctx, now, calculator_mode && !covered);
        if self.decision == SupportDecision::ShowPopup {
            self.popup = Some(OpenPopup {
                automatic: true,
                opened_at_ms: now.millis,
                thank_you_since_ms: None,
            });
            self.popup_shown_this_session = true;
            self.calculation_at_ms = None;
            self.apply(SupportEvent::PopupShown);
            self.decision = SupportDecision::Nothing;
        } else if let Some(at) = self.calculation_at_ms
            && self.decision == SupportDecision::ShowDot
        {
            // Wake up when the calm delay is over, without any input.
            let left = (at + self.cfg.calm_delay_ms).saturating_sub(now.millis);
            ctx.request_repaint_after(std::time::Duration::from_millis(left.max(1)));
        }
    }

    fn evaluate(&self, ctx: &Context, now: Timestamp, calculator_idle: bool) -> SupportDecision {
        // Visible and focused: egui reports a minimized window, a background app or a hidden
        // browser tab as unfocused or minimized.
        let app_focused = ctx.input(|i| i.focused && i.viewport().minimized != Some(true));
        let moment = MomentContext {
            calculation_at_ms: self.calculation_at_ms,
            app_focused,
            calculator_idle: calculator_idle && self.popup.is_none(),
            popup_shown_this_session: self.popup_shown_this_session,
        };
        policy::evaluate(&self.stats, &self.cfg, now, moment)
    }

    /// Opens the popup from the menu (no stacking: an open one stays as it is).
    pub(crate) fn open_on_demand(&mut self) {
        if self.popup.is_none() {
            self.popup = Some(OpenPopup {
                automatic: false,
                opened_at_ms: self.now().millis,
                thank_you_since_ms: None,
            });
            self.apply(SupportEvent::PopupOpenedOnDemand);
        }
    }

    fn close_popup(&mut self, now: Timestamp) {
        // Closing the thank-you text early is no reaction: "I already supported" was one.
        if let Some(popup) = self.popup.take()
            && popup.thank_you_since_ms.is_none()
        {
            self.apply(SupportEvent::PopupClosed {
                after_ms: now.millis.saturating_sub(popup.opened_at_ms),
                automatic: popup.automatic,
            });
        }
    }

    fn donate(&mut self, ctx: &Context) {
        let now = self.now();
        let source = match self.popup {
            Some(OpenPopup {
                automatic: true, ..
            }) => DonateSource::Popup,
            _ => DonateSource::Menu,
        };
        let context = DonateContext {
            platform: platform(),
            version: env!("CARGO_PKG_VERSION"),
            source,
            cycles: self.stats.cycles,
            days_since_install: self.stats.first_day.map(|first| now.day - first),
        };
        self.channel.donate(ctx, &context);
        self.apply(SupportEvent::DonateClicked);
        self.popup = None;
    }

    /// Shares CalcCraft: "<message> <link>" goes through the share channel, "Link copied" shows
    /// for a moment, and the policy hears of it. The popup stays open, so Donate is still there.
    pub(crate) fn share(&mut self, ctx: &Context) {
        let context = ShareContext {
            platform: platform(),
            version: env!("CARGO_PKG_VERSION"),
        };
        let url = share_url(SHARE_URL, &context);
        let text = share_text(tr("CalcCraft.Support.ShareMessage"), &url);
        if super::clock::fast_mode() {
            eprintln!("support: share text {text}");
        }
        if self.share_channel.share(ctx, &text).shows_note() {
            self.copied_until_ms = Some(self.now().millis + COPIED_MS);
            ctx.request_repaint_after(std::time::Duration::from_millis(COPIED_MS));
        }
        self.apply(SupportEvent::ShareClicked);
    }

    pub(super) fn handle(&mut self, ctx: &Context, action: PopupAction) {
        let now = self.now();
        match action {
            PopupAction::Share => self.share(ctx),
            PopupAction::Close => self.close_popup(now),
            PopupAction::Donate => self.donate(ctx),
            PopupAction::AlreadySupported => {
                self.apply(SupportEvent::AlreadySupported);
                if let Some(popup) = &mut self.popup {
                    popup.thank_you_since_ms = Some(now.millis);
                }
                ctx.request_repaint_after(std::time::Duration::from_millis(self.cfg.thank_you_ms));
            }
        }
    }

    /// The red dot on a menu button at `rect` (the hamburger), with its screen reader
    /// description.
    pub(crate) fn draw_menu_badge(&self, ui: &Ui, rect: Rect, id: Id) {
        if !self.dot_visible() {
            return;
        }
        paint_dot(ui, pos2(rect.right() - 11.0, rect.top() + 9.0));
        describe(ui, id);
    }

    /// The menu's support entry now.
    pub fn menu_entry(&self) -> MenuEntry {
        policy::menu_entry(&self.stats, &self.cfg, self.now())
    }

    /// The amber "Please Support!" button in the menu's header, or the supporter state's
    /// thank-you line. Returns true when it was pressed: the popup opens and the menu should
    /// close.
    pub(crate) fn draw_menu_button(&mut self, ui: &mut Ui, rect: Rect, p: &Palette) -> bool {
        let clicked = match self.menu_entry() {
            MenuEntry::SupportButton => self.draw_support_button(ui, rect, p),
            MenuEntry::SupporterThanks => {
                // The share glyph takes the line's right end; the menu stays open for it.
                let (line, share) = split_share(rect);
                if draw_share_glyph(ui, p, share) {
                    self.share(ui.ctx());
                }
                draw_thanks_line(ui, line, p)
            }
        };
        if clicked {
            self.open_on_demand();
        }
        clicked
    }

    fn draw_support_button(&self, ui: &mut Ui, rect: Rect, p: &Palette) -> bool {
        let dark = dark(ui.ctx());
        let label = tr("CalcCraft.Support.MenuButton");
        let id = Id::new("support-menu-button");
        let radius = 6.0;

        let response = access::control(ui, p, rect, id, true, radius, access::button(true, label));
        let hovered = response.hovered();
        let pressed = response.is_pointer_button_down_on();

        let (fill, border, text_color) = if dark {
            if pressed {
                (
                    Color32::from_rgb(58, 38, 6),
                    Color32::from_rgb(110, 75, 18),
                    Color32::from_rgb(255, 230, 165),
                )
            } else if hovered {
                (
                    Color32::from_rgb(98, 66, 14),
                    Color32::from_rgb(165, 115, 30),
                    Color32::from_rgb(255, 242, 195),
                )
            } else {
                (
                    Color32::from_rgb(74, 50, 10),
                    Color32::from_rgb(142, 98, 24),
                    Color32::from_rgb(255, 235, 176),
                )
            }
        } else if pressed {
            (
                Color32::from_rgb(246, 212, 114),
                Color32::from_rgb(212, 172, 75),
                Color32::from_rgb(70, 44, 0),
            )
        } else if hovered {
            (
                Color32::from_rgb(253, 226, 146),
                Color32::from_rgb(235, 198, 105),
                Color32::from_rgb(60, 38, 0),
            )
        } else {
            (
                Color32::from_rgb(254, 236, 178),
                Color32::from_rgb(226, 188, 92),
                Color32::from_rgb(76, 48, 0),
            )
        };

        let painter = ui.painter();
        let cr = CornerRadius::same(radius as u8);
        painter.rect_filled(rect, cr, fill);
        painter.rect_stroke(rect, cr, Stroke::new(1.0, border), StrokeKind::Inside);

        let font = FontId::new(13.0, crate::app::semibold());
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            label,
            font,
            text_color,
        );
        if self.dot_visible() {
            paint_dot(ui, pos2(rect.right() - 7.0, rect.top() + 7.0));
            describe(ui, id);
        }
        response.clicked()
    }

    /// The popup, if open, under the display noted in this pass (`note_display`) and inside
    /// `window`.
    pub(crate) fn draw_overlay(&mut self, ui: &mut Ui, window: Rect, p: &Palette) {
        let Some(popup) = self.popup else {
            return;
        };
        let display = noted_display(ui.ctx());
        let width = card_width(display.unwrap_or(window).width());
        let already = policy::show_already_supported_button(&self.stats);
        let content = Content::new(ui, popup, already, width);
        let card = card_rect(window, display, width, content.height());
        if let Some(action) = draw_card(ui, p, card, &content) {
            self.handle(ui.ctx(), action);
        }
    }

    /// "Link copied": a small note near the window's bottom that goes away by itself. It lives
    /// on its own top layer, so the menu (drawn after the popup, over the whole window) and
    /// every mode show it: before, the menu's share glyph copied the link under an invisible
    /// note. It takes no clicks and no focus, and screen readers hear it as a polite live
    /// region. Call it once per frame, after everything else is drawn. It has no fade, which
    /// also is what reduced motion asks for.
    pub(crate) fn draw_toast(&self, ctx: &Context, p: &Palette) {
        if self.copied_until_ms.is_none() {
            return;
        }
        let text = tr("CalcCraft.Support.LinkCopied");
        egui::Area::new(Id::new("support-link-copied-area"))
            .order(egui::Order::Tooltip)
            .interactable(false)
            .anchor(Align2::CENTER_BOTTOM, vec2(0.0, -NOTE_BOTTOM_GAP))
            .show(ctx, |ui| {
                let job = LayoutJob::simple(
                    text.to_owned(),
                    FontId::proportional(13.0),
                    p.text,
                    (ctx.content_rect().width() - 48.0).max(80.0),
                );
                let galley = ui.painter().layout_job(job);
                let (rect, _) =
                    ui.allocate_exact_size(galley.size() + vec2(28.0, 16.0), Sense::hover());
                let radius = CornerRadius::same(8);
                let painter = ui.painter();
                painter.rect_filled(rect, radius, p.flyout);
                painter.rect_stroke(
                    rect,
                    radius,
                    Stroke::new(1.0, p.flyout_border),
                    StrokeKind::Inside,
                );
                painter.galley(rect.center() - galley.size() / 2.0, galley, p.text);
                access::text(ui, rect, Id::new("support-link-copied"), text, true);
            });
    }
}

/// How long "Link copied" shows.
const COPIED_MS: u64 = 2000;
/// How long the `share` demo stage keeps it (long enough to take a screenshot).
const SHARE_DEMO_MS: u64 = 60_000;
/// The note's distance from the window's bottom edge.
const NOTE_BOTTOM_GAP: f32 = 24.0;
const SHARE_GLYPH_SIZE: f32 = 32.0;

/// Splits the supporter line's rect into the line and the share glyph's square at its right end.
fn split_share(rect: Rect) -> (Rect, Rect) {
    let side = SHARE_GLYPH_SIZE.min(rect.height()).min(rect.width());
    let glyph = Rect::from_min_size(
        pos2(rect.right() - side, rect.center().y - side / 2.0),
        vec2(side, side),
    );
    let line = Rect::from_min_max(rect.min, pos2(glyph.left() - 2.0, rect.bottom()));
    (line, glyph)
}

/// The quiet share glyph next to the supporter line. Returns true when it was pressed.
fn draw_share_glyph(ui: &Ui, p: &Palette, rect: Rect) -> bool {
    let label = tr("CalcCraft.Support.ShareTooltip");
    let response = access::control(
        ui,
        p,
        rect,
        Id::new("support-share-glyph"),
        true,
        6.0,
        access::button(true, label),
    );
    let dark = dark(ui.ctx());
    let overlay = match (
        response.is_pointer_button_down_on(),
        response.hovered(),
        dark,
    ) {
        (true, _, true) => Some(Color32::from_white_alpha(20)),
        (true, _, false) => Some(Color32::from_black_alpha(18)),
        (false, true, true) => Some(Color32::from_white_alpha(12)),
        (false, true, false) => Some(Color32::from_black_alpha(10)),
        _ => None,
    };
    let painter = ui.painter();
    if let Some(fill) = overlay {
        painter.rect_filled(rect, CornerRadius::same(6), fill);
    }
    let color = if response.hovered() {
        p.text
    } else {
        p.text_secondary
    };
    let glyph = painter.layout_job(icon_label(ICON_SHARE, 14.0));
    painter.galley(rect.center() - glyph.size() / 2.0, glyph, color);
    response.on_hover_text(label).clicked()
}

/// The thank-you line's green: Windows' success green (`SystemFillColorSuccess`), darker on the
/// light menu and lighter on the dark one, both above 4.5:1 against it.
const THANKS_LIGHT: Color32 = Color32::from_rgb(15, 123, 15);
const THANKS_DARK: Color32 = Color32::from_rgb(108, 203, 95);
/// Narrower than this, the line is only its heart, with the text as a tooltip.
const THANKS_MIN_TEXT_WIDTH: f32 = 90.0;
const THANKS_HEART_SIZE: f32 = 14.0;
const THANKS_PAD: f32 = 8.0;
const THANKS_GAP: f32 = 6.0;

/// The supporter state's quiet thank-you line in `rect` (the amber button's place): the wrapped
/// text and a heart after it in green, no fill or outline. Returns true when it was pressed.
fn draw_thanks_line(ui: &Ui, rect: Rect, p: &Palette) -> bool {
    let dark = dark(ui.ctx());
    let color = if dark { THANKS_DARK } else { THANKS_LIGHT };
    let label = tr("CalcCraft.Support.Thanks");
    let heart = ui
        .painter()
        .layout_job(icon_label(ICON_HEART, THANKS_HEART_SIZE));
    // The header around the button's rect has a few points more above and below for two lines.
    let area = rect.expand2(vec2(0.0, 6.0));
    let text_width = area.width() - 2.0 * THANKS_PAD - heart.size().x - THANKS_GAP;
    let text = (area.width() >= THANKS_MIN_TEXT_WIDTH)
        .then(|| thanks_galley(ui, label, color, text_width, area.height()));

    let control = match &text {
        Some(text) => {
            // A long translation grows past the header rather than clip.
            let height = area.height().max(text.size().y + 4.0);
            Rect::from_center_size(area.center(), vec2(area.width(), height))
        }
        None => Rect::from_center_size(area.center(), vec2(32.0, 32.0).min(area.size())),
    };
    let id = Id::new("support-thanks");
    let response = access::control(ui, p, control, id, true, 6.0, access::button(true, label));
    let overlay = match (
        response.is_pointer_button_down_on(),
        response.hovered(),
        dark,
    ) {
        (true, _, true) => Some(Color32::from_white_alpha(20)),
        (true, _, false) => Some(Color32::from_black_alpha(18)),
        (false, true, true) => Some(Color32::from_white_alpha(12)),
        (false, true, false) => Some(Color32::from_black_alpha(10)),
        _ => None,
    };
    let painter = ui.painter();
    if let Some(fill) = overlay {
        painter.rect_filled(control, CornerRadius::same(6), fill);
    }
    let heart_size = heart.size();
    match text {
        Some(text) => {
            let origin = pos2(
                control.left() + THANKS_PAD,
                control.center().y - text.size().y / 2.0,
            );
            let heart_at = heart_position(origin, &text, heart_size);
            painter.galley(origin, text, color);
            painter.galley(heart_at, heart, color);
            response.clicked()
        }
        None => {
            painter.galley(control.center() - heart_size / 2.0, heart, color);
            response.on_hover_text(label).clicked()
        }
    }
}

/// Where the heart goes for the thanks text drawn at `origin`: after its last row, centered on
/// it, as in the owner's "Supported by you 💚".
fn heart_position(origin: egui::Pos2, text: &Galley, heart_size: egui::Vec2) -> egui::Pos2 {
    let last = text
        .rows
        .last()
        .map_or(Rect::from_min_size(origin, egui::Vec2::ZERO), |row| {
            row.rect().translate(origin.to_vec2())
        });
    pos2(
        last.right() + THANKS_GAP,
        last.center().y - heart_size.y / 2.0,
    )
}

/// The thanks text wrapped at `width`, in the largest of 13, 12 and 11 points that fits in
/// `height` (else 11, taller).
fn thanks_galley(ui: &Ui, label: &str, color: Color32, width: f32, height: f32) -> Arc<Galley> {
    let layout = |size: f32| {
        let job = LayoutJob::simple(
            label.to_owned(),
            FontId::proportional(size),
            color,
            width.max(1.0),
        );
        ui.painter().layout_job(job)
    };
    for size in [13.0, 12.0] {
        let galley = layout(size);
        if galley.size().y + 4.0 <= height {
            return galley;
        }
    }
    layout(11.0)
}

/// Adds the support reminder to the screen reader description of control `id`.
fn describe(ui: &Ui, id: Id) {
    let text = tr("CalcCraft.Support.Reminder");
    ui.ctx()
        .accesskit_node_builder(id, |node| node.set_description(text));
}

fn paint_dot(ui: &Ui, center: egui::Pos2) {
    let dark = dark(ui.ctx());
    let color = if dark { DOT_DARK } else { DOT_LIGHT };
    // A ring of the background's contrast keeps it apart from what is under it.
    let ring = if dark {
        Color32::from_black_alpha(160)
    } else {
        Color32::WHITE
    };
    ui.painter()
        .circle(center, DOT_RADIUS, color, Stroke::new(1.0, ring));
}

// ------------------------------------------------------------------- popup

const H_PAD: f32 = 24.0;
const V_PAD: f32 = 28.0;
const GAP_TITLE_BODY: f32 = 14.0;
const GAP_BODY_BUTTON: f32 = 22.0;
const GAP_BUTTONS: f32 = 8.0;
const DONATE_HEIGHT: f32 = 36.0;
const SECONDARY_HEIGHT: f32 = 32.0;
const SHARE_HEIGHT: f32 = 28.0;
const MIN_CARD_HEIGHT: f32 = 200.0;
/// The space kept between the display and the card, and around the card.
const CARD_MARGIN: f32 = 12.0;

/// The card's text and buttons, laid out for its width.
struct Content {
    width: f32,
    title: Arc<Galley>,
    body: Option<Arc<Galley>>,
    /// Donate, and "I already supported" under it; none in the thank-you state.
    buttons: Option<bool>,
    title_color: Color32,
    body_color: Color32,
}

impl Content {
    fn new(ui: &Ui, popup: OpenPopup, already_supported: bool, width: f32) -> Self {
        let dark = dark(ui.ctx());
        let title_color = if dark {
            Color32::from_rgb(255, 238, 185)
        } else {
            Color32::from_rgb(55, 38, 5)
        };
        let body_color = if dark {
            Color32::from_rgb(225, 210, 180)
        } else {
            Color32::from_rgb(95, 75, 45)
        };
        let thank_you = popup.thank_you_since_ms.is_some();
        let title_text = if thank_you {
            tr("CalcCraft.Support.ThankYou")
        } else {
            tr("CalcCraft.Support.Title")
        };
        let layout = |text: String, font: FontId, color: Color32, max: f32| {
            let mut job = LayoutJob::simple(text, font, color, max);
            job.halign = Align::Center;
            ui.painter().layout_job(job)
        };
        let title = layout(
            title_text.to_owned(),
            FontId::new(19.0, crate::app::semibold()),
            title_color,
            (width - 80.0).max(160.0),
        );
        let body = (!thank_you).then(|| {
            let raw = tr("CalcCraft.Support.Body");
            #[cfg(not(target_arch = "wasm32"))]
            let long =
                std::env::var("CALCCRAFT_SUPPORT_LONG_TEXT").is_ok_and(|v| v == "1" || v == "true");
            #[cfg(target_arch = "wasm32")]
            let long = false;
            let text = if long {
                format!("{raw} {raw} {raw}")
            } else {
                raw.to_owned()
            };
            layout(
                text,
                FontId::proportional(13.5),
                body_color,
                (width - 2.0 * H_PAD).max(160.0),
            )
        });
        Self {
            width,
            title,
            body,
            buttons: (!thank_you).then_some(already_supported),
            title_color,
            body_color,
        }
    }

    /// The height of the text and buttons, without the padding.
    fn height(&self) -> f32 {
        let mut height = self.title.size().y;
        if let Some(body) = &self.body {
            height += GAP_TITLE_BODY + body.size().y;
        }
        if let Some(already) = self.buttons {
            height += GAP_BODY_BUTTON + DONATE_HEIGHT;
            if already {
                height += GAP_BUTTONS + SECONDARY_HEIGHT;
            }
            height += GAP_BUTTONS + SHARE_HEIGHT;
        }
        height
    }
}

fn card_width(area: f32) -> f32 {
    (area * 0.78)
        .clamp(280.0, 480.0)
        .min((area - 24.0).max(200.0))
}

/// The card: centered in the space between the display and the window's bottom (over the
/// memory row and the keys), never over the display, as wide as `width`. Without a display
/// (an on-demand popup in another mode) the space starts under the header.
fn card_rect(window: Rect, display: Option<Rect>, width: f32, content_height: f32) -> Rect {
    let (left, right, top) = match display {
        Some(display) => (display.left(), display.right(), display.bottom()),
        None => (window.left(), window.right(), window.top() + 48.0),
    };
    let area = Rect::from_min_max(
        pos2(left, (top + CARD_MARGIN).min(window.bottom())),
        pos2(right, (window.bottom() - CARD_MARGIN).max(top)),
    );
    let width = width.min(area.width()).max(0.0);
    let height = (content_height + 2.0 * V_PAD)
        .max(MIN_CARD_HEIGHT)
        .min(area.height());
    Rect::from_center_size(area.center(), vec2(width, height))
}

/// Draws the card at `card` and returns what was pressed.
fn draw_card(ui: &mut Ui, p: &Palette, card: Rect, content: &Content) -> Option<PopupAction> {
    let dark = dark(ui.ctx());
    let painter = ui.painter().with_clip_rect(card);
    let (panel_fill, panel_stroke) = if dark {
        (
            Color32::from_rgba_unmultiplied(46, 32, 10, 225),
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(150, 105, 25, 210)),
        )
    } else {
        (
            Color32::from_rgba_unmultiplied(255, 246, 226, 230),
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(225, 188, 105, 220)),
        )
    };
    let cr = CornerRadius::same(12);
    painter.rect_filled(card, cr, panel_fill);
    painter.rect_stroke(card, cr, panel_stroke, StrokeKind::Inside);
    // The card takes the clicks on it, so the keys under it aren't pressed.
    ui.interact(card, Id::new("support-card-bg"), Sense::CLICK);

    let mut action = None;
    if close_button(ui, p, &painter, card) {
        action = Some(PopupAction::Close);
    }

    let height = content.height();
    if height + 32.0 <= card.height() {
        let top = card.top() + (card.height() - height) / 2.0;
        action = action.or(draw_content(ui, p, &painter, card.center().x, top, content));
    } else {
        // Too little room under the display (a short window, a long translation): the
        // content scrolls inside the card instead of moving over the display.
        let scroll = Rect::from_min_max(
            pos2(card.left() + 8.0, card.top() + 42.0),
            pos2(card.right() - 8.0, card.bottom() - 12.0),
        );
        ui.scope_builder(UiBuilder::new().max_rect(scroll), |ui| {
            ScrollArea::vertical()
                .id_salt("support-card-scroll")
                .auto_shrink(false)
                .show(ui, |ui| {
                    let (rect, _) =
                        ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
                    let painter = ui.painter().clone();
                    let pressed =
                        draw_content(ui, p, &painter, rect.center().x, rect.top(), content);
                    action = action.or(pressed);
                });
        });
    }
    action
}

/// The title, body and buttons from `top`, centered on `center_x`.
fn draw_content(
    ui: &Ui,
    p: &Palette,
    painter: &Painter,
    center_x: f32,
    top: f32,
    content: &Content,
) -> Option<PopupAction> {
    let mut y = top;
    painter.galley(
        pos2(center_x, y),
        content.title.clone(),
        content.title_color,
    );
    y += content.title.size().y;
    if let Some(body) = &content.body {
        y += GAP_TITLE_BODY;
        painter.galley(pos2(center_x, y), body.clone(), content.body_color);
        y += body.size().y;
    }
    let already = content.buttons?;
    y += GAP_BODY_BUTTON;
    let width = (content.width * 0.52).clamp(140.0, 200.0);
    let donate = Rect::from_min_size(pos2(center_x - width / 2.0, y), vec2(width, DONATE_HEIGHT));
    let mut action = donate_button(ui, p, painter, donate).then_some(PopupAction::Donate);
    y += DONATE_HEIGHT + GAP_BUTTONS;
    if already {
        let rect = Rect::from_min_size(
            pos2(center_x - width / 2.0, y),
            vec2(width, SECONDARY_HEIGHT),
        );
        if secondary_button(ui, p, painter, rect) {
            action = Some(PopupAction::AlreadySupported);
        }
        y += SECONDARY_HEIGHT + GAP_BUTTONS;
    }
    let rect = Rect::from_min_size(pos2(center_x - width / 2.0, y), vec2(width, SHARE_HEIGHT));
    if share_link(ui, p, painter, rect) {
        action = Some(PopupAction::Share);
    }
    action
}

/// The Fluent close X in the card's top right corner.
fn close_button(ui: &Ui, p: &Palette, painter: &Painter, card: Rect) -> bool {
    let dark = dark(ui.ctx());
    let size = 32.0;
    let rect = Rect::from_min_size(
        pos2(card.right() - size - 8.0, card.top() + 8.0),
        vec2(size, size),
    );
    let name = tr("CalcCraft.Support.Close");
    let response = access::control(
        ui,
        p,
        rect,
        Id::new("support-close"),
        true,
        4.0,
        access::button(true, name),
    );
    let hovered = response.hovered();
    let pressed = response.is_pointer_button_down_on();
    let overlay = match (pressed, hovered, dark) {
        (true, _, true) => Some(Color32::from_white_alpha(45)),
        (true, _, false) => Some(Color32::from_black_alpha(35)),
        (false, true, true) => Some(Color32::from_white_alpha(25)),
        (false, true, false) => Some(Color32::from_black_alpha(20)),
        _ => None,
    };
    if let Some(fill) = overlay {
        painter.rect_filled(rect, CornerRadius::same(4), fill);
    }
    let color = if hovered || pressed {
        p.text
    } else {
        p.text_secondary
    };
    let glyph = painter.layout_job(icon_label(ICON_CLOSE, 12.0));
    painter.galley(rect.center() - glyph.size() / 2.0, glyph, color);
    response.clicked()
}

/// The green Donate button.
fn donate_button(ui: &Ui, p: &Palette, painter: &Painter, rect: Rect) -> bool {
    let label = tr("CalcCraft.Support.Donate");
    let response = access::control(
        ui,
        p,
        rect,
        Id::new("support-donate"),
        true,
        6.0,
        access::button(true, label),
    );
    let fill = if response.is_pointer_button_down_on() {
        Color32::from_rgb(14, 107, 56)
    } else if response.hovered() {
        Color32::from_rgb(19, 137, 74)
    } else {
        Color32::from_rgb(16, 124, 65)
    };
    let cr = CornerRadius::same(6);
    painter.rect_filled(rect, cr, fill);
    painter.rect_stroke(
        rect,
        cr,
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 40)),
        StrokeKind::Inside,
    );
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::new(14.0, crate::app::semibold()),
        Color32::WHITE,
    );
    response.clicked()
}

/// "I already supported": a quiet, outlined button under Donate.
fn secondary_button(ui: &Ui, p: &Palette, painter: &Painter, rect: Rect) -> bool {
    let dark = dark(ui.ctx());
    let label = tr("CalcCraft.Support.AlreadySupported");
    let response = access::control(
        ui,
        p,
        rect,
        Id::new("support-already"),
        true,
        6.0,
        access::button(true, label),
    );
    let fill = match (
        response.is_pointer_button_down_on(),
        response.hovered(),
        dark,
    ) {
        (true, _, true) => Color32::from_white_alpha(30),
        (true, _, false) => Color32::from_black_alpha(25),
        (false, true, true) => Color32::from_white_alpha(18),
        (false, true, false) => Color32::from_black_alpha(14),
        _ => Color32::TRANSPARENT,
    };
    let (border, text) = if dark {
        (
            Color32::from_rgba_unmultiplied(200, 160, 90, 150),
            Color32::from_rgb(235, 220, 190),
        )
    } else {
        (
            Color32::from_rgba_unmultiplied(150, 110, 40, 140),
            Color32::from_rgb(85, 62, 25),
        )
    };
    let cr = CornerRadius::same(6);
    painter.rect_filled(rect, cr, fill);
    painter.rect_stroke(rect, cr, Stroke::new(1.0, border), StrokeKind::Inside);
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(13.0),
        text,
    );
    response.clicked()
}

/// "Tell a friend": the quietest action of the card, text with the share glyph, no fill or
/// outline, under Donate.
fn share_link(ui: &Ui, p: &Palette, painter: &Painter, rect: Rect) -> bool {
    let dark = dark(ui.ctx());
    let label = tr("CalcCraft.Support.Share");
    let response = access::control(
        ui,
        p,
        rect,
        Id::new("support-share"),
        true,
        6.0,
        access::button(true, label),
    );
    let fill = match (
        response.is_pointer_button_down_on(),
        response.hovered(),
        dark,
    ) {
        (true, _, true) => Color32::from_white_alpha(25),
        (true, _, false) => Color32::from_black_alpha(20),
        (false, true, true) => Color32::from_white_alpha(14),
        (false, true, false) => Color32::from_black_alpha(10),
        _ => Color32::TRANSPARENT,
    };
    painter.rect_filled(rect, CornerRadius::same(6), fill);
    let color = if dark {
        Color32::from_rgb(225, 210, 180)
    } else {
        Color32::from_rgb(95, 75, 45)
    };
    let glyph = painter.layout_job(icon_label(ICON_SHARE, 13.0));
    let text = painter.layout_no_wrap(label.to_owned(), FontId::proportional(13.0), color);
    let gap = 6.0;
    let total = glyph.size().x + gap + text.size().x;
    let left = rect.center().x - total / 2.0;
    painter.galley(
        pos2(left, rect.center().y - glyph.size().y / 2.0),
        glyph.clone(),
        color,
    );
    painter.galley(
        pos2(
            left + glyph.size().x + gap,
            rect.center().y - text.size().y / 2.0,
        ),
        text,
        color,
    );
    response.clicked()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn card_stays_under_the_display_and_inside_the_window() {
        for (width, height) in [(320.0, 500.0), (480.0, 640.0), (1200.0, 900.0)] {
            let window = Rect::from_min_size(pos2(0.0, 0.0), vec2(width, height));
            let display = Rect::from_min_max(pos2(0.0, 48.0), pos2(width, 200.0));
            for content in [150.0, 190.0, 400.0] {
                let card = card_rect(window, Some(display), card_width(width), content);
                assert!(
                    card.top() >= display.bottom() + CARD_MARGIN - 0.01,
                    "{card:?}"
                );
                assert!(window.contains_rect(card), "{card:?} in {window:?}");
                assert!(card.width() >= 200.0 && card.width() <= 480.0);
            }
        }
        // A docked panel: the card stays over the calculator part.
        let window = Rect::from_min_size(pos2(0.0, 0.0), vec2(900.0, 700.0));
        let display = Rect::from_min_max(pos2(0.0, 48.0), pos2(600.0, 250.0));
        let card = card_rect(window, Some(display), card_width(600.0), 200.0);
        assert!(card.right() <= 600.0 && card.left() >= 0.0);
        assert!((card.center().x - 300.0).abs() < 0.01);
        // Even a window shorter than the card's minimum keeps it under the display.
        let window = Rect::from_min_size(pos2(0.0, 0.0), vec2(320.0, 300.0));
        let display = Rect::from_min_max(pos2(0.0, 48.0), pos2(320.0, 180.0));
        let card = card_rect(window, Some(display), card_width(320.0), 200.0);
        assert!(card.top() >= 192.0 - 0.01 && card.bottom() <= 300.0);
    }

    #[test]
    fn thanks_line_reads_supported_by_you_with_the_heart_after_it() {
        // The key stays; the English text is the owner's wording, the heart is drawn, not text.
        let english = include_str!("../../locales/calccraft.txt")
            .lines()
            .find_map(|line| line.trim_end().strip_prefix("CalcCraft.Support.Thanks="));
        assert_eq!(english, Some("Supported by you"));

        let ctx = Context::default();
        let mut output = ctx.run_ui(Default::default(), |ui| {
            let heart = vec2(THANKS_HEART_SIZE, THANKS_HEART_SIZE);
            let origin = pos2(10.0, 20.0);
            for width in [200.0, 60.0] {
                let text = thanks_galley(ui, "Supported by you", Color32::WHITE, width, 40.0);
                let at = heart_position(origin, &text, heart);
                let last = text.rows.last().unwrap().rect().translate(origin.to_vec2());
                assert!(at.x >= last.right() + THANKS_GAP - 0.01, "{at:?} {last:?}");
                assert!(at.x + heart.x <= origin.x + width + THANKS_GAP + heart.x + 0.01);
                assert!((at.y + heart.y / 2.0 - last.center().y).abs() < 0.01);
            }
        });
        output.textures_delta.clear();
    }

    #[test]
    fn galley_layout_wrapping() {
        let ctx = Context::default();
        let mut output = ctx.run_ui(Default::default(), |ui| {
            let mut job = LayoutJob::simple(
                "CalcCraft is free and ad-free. Your support helps keep development active."
                    .to_string(),
                FontId::proportional(13.5),
                Color32::WHITE,
                300.0,
            );
            job.halign = Align::Center;
            let galley = ui.painter().layout_job(job);
            assert!(galley.rows.len() >= 2);
            for row in &galley.rows {
                let r = row.rect();
                let center_x = (r.min.x + r.max.x) / 2.0;
                assert!(
                    center_x.abs() < 2.0,
                    "row center {center_x} should be near 0"
                );
            }
        });
        output.textures_delta.clear();
    }
}
