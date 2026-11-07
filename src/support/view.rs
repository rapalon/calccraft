//! UI rendering and interaction logic for the support prompt module.

use eframe::Storage;
use eframe::egui::{
    Align2, Color32, CornerRadius, FontId, Id, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2,
};

use crate::app::access;
use crate::app::icons::{ICON_REMOVE, icon_label};
use crate::app::palette::Palette;
use crate::app::strings::tr;

use super::policy;
use super::stats::SupportStats;

/// Donation URL for supporting the project.
// TODO: Owner will confirm the real Stripe page URL.
pub const DONATE_URL: &str = "https://rapalon.com/donate";

/// Outcome of user interaction with the support popup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportAction {
    None,
    Close,
    OpenDonateUrl,
}

/// Runtime state of the support prompt module.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SupportState {
    /// Whether the floating support card is currently visible.
    pub is_open: bool,
    /// Local calculation statistics.
    pub stats: SupportStats,
}

impl SupportState {
    /// Creates or restores support state from storage.
    pub fn new(storage: Option<&dyn Storage>) -> Self {
        Self {
            is_open: false,
            stats: SupportStats::load(storage),
        }
    }

    /// Persists stats to storage.
    pub fn save(&self, storage: &mut dyn Storage) {
        self.stats.save(storage);
    }

    /// Records that an evaluation completed and evaluates the trigger rule.
    ///
    /// If the policy returns true, opens the popup (ensuring it never stacks).
    /// Returns `true` if this calculation triggered the popup to open.
    pub fn on_calculation_completed(&mut self, now: Option<u64>) -> bool {
        self.stats.record_calculation(now);
        if policy::should_show(&self.stats, now) && !self.is_open {
            self.is_open = true;
            self.stats.record_popup_shown();
            return true;
        }
        false
    }

    /// Renders the amber "Please Support!" button at the top of the hamburger navigation pane.
    ///
    /// Returns `true` if activated by mouse or keyboard.
    pub(crate) fn draw_menu_button(
        &self,
        ui: &mut Ui,
        rect: Rect,
        dark: bool,
        p: &Palette,
    ) -> bool {
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

        response.clicked()
    }

    /// Renders the floating semi-transparent support card centered over `window_rect`.
    pub(crate) fn draw_popup(
        &mut self,
        ui: &mut Ui,
        window_rect: Rect,
        dark: bool,
        p: &Palette,
    ) -> SupportAction {
        if !self.is_open {
            return SupportAction::None;
        }

        let width = (window_rect.width() * 0.75).clamp(280.0, 520.0);
        let height = (window_rect.height() * 0.60).clamp(220.0, 360.0);
        let card_rect = Rect::from_center_size(window_rect.center(), vec2(width, height));

        let painter = ui.painter().clone();

        // 1. Semi-transparent amber card panel
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
        painter.rect_filled(card_rect, cr, panel_fill);
        painter.rect_stroke(card_rect, cr, panel_stroke, StrokeKind::Inside);

        // Consume pointer clicks within the card panel bounds so keys beneath aren't accidentally clicked
        ui.interact(card_rect, Id::new("support-card-bg"), Sense::CLICK);

        // 2. Red "X" close icon in top-right corner
        let close_rect = Rect::from_min_size(
            pos2(card_rect.right() - 36.0, card_rect.top() + 8.0),
            vec2(28.0, 28.0),
        );
        let close_name = tr("CalcCraft.Support.Close");
        let close_resp = access::control(
            ui,
            p,
            close_rect,
            Id::new("support-close"),
            true,
            4.0,
            access::button(true, close_name),
        );
        let close_hovered = close_resp.hovered();
        let close_pressed = close_resp.is_pointer_button_down_on();

        if close_pressed {
            painter.rect_filled(
                close_rect,
                CornerRadius::same(4),
                Color32::from_rgba_unmultiplied(200, 30, 30, 60),
            );
        } else if close_hovered {
            painter.rect_filled(
                close_rect,
                CornerRadius::same(4),
                Color32::from_rgba_unmultiplied(220, 50, 50, 35),
            );
        }

        let close_icon_color = if close_hovered || close_pressed {
            Color32::from_rgb(235, 45, 45)
        } else {
            Color32::from_rgb(210, 50, 50)
        };
        let close_glyph = painter.layout_job(icon_label(ICON_REMOVE, 14.0));
        let glyph_pos = pos2(
            close_rect.center().x - close_glyph.size().x / 2.0,
            close_rect.center().y - close_glyph.size().y / 2.0,
        );
        painter.galley(glyph_pos, close_glyph, close_icon_color);

        if close_resp.clicked() {
            self.is_open = false;
            return SupportAction::Close;
        }

        // 3. Title text ("Please support us!")
        let title_text = tr("CalcCraft.Support.Title");
        let title_color = if dark {
            Color32::from_rgb(255, 238, 185)
        } else {
            Color32::from_rgb(55, 38, 5)
        };
        let title_font = FontId::new(19.0, crate::app::semibold());
        painter.text(
            pos2(card_rect.center().x, card_rect.top() + 38.0),
            Align2::CENTER_CENTER,
            title_text,
            title_font,
            title_color,
        );

        // 4. Friendly body text
        let body_text = tr("CalcCraft.Support.Body");
        let body_color = if dark {
            Color32::from_rgb(225, 210, 180)
        } else {
            Color32::from_rgb(95, 75, 45)
        };
        let body_font = FontId::proportional(13.5);
        painter.text(
            pos2(card_rect.center().x, card_rect.top() + 72.0),
            Align2::CENTER_TOP,
            body_text,
            body_font,
            body_color,
        );

        // 5. Green "Donate" button in the lower part
        let btn_width = (card_rect.width() * 0.55).clamp(140.0, 220.0);
        let btn_height = 38.0;
        let donate_rect = Rect::from_center_size(
            pos2(card_rect.center().x, card_rect.bottom() - 42.0),
            vec2(btn_width, btn_height),
        );

        let donate_label = tr("CalcCraft.Support.Donate");
        let donate_resp = access::control(
            ui,
            p,
            donate_rect,
            Id::new("support-donate"),
            true,
            6.0,
            access::button(true, donate_label),
        );
        let donate_hovered = donate_resp.hovered();
        let donate_pressed = donate_resp.is_pointer_button_down_on();

        let donate_fill = if donate_pressed {
            Color32::from_rgb(14, 107, 56)
        } else if donate_hovered {
            Color32::from_rgb(19, 137, 74)
        } else {
            Color32::from_rgb(16, 124, 65)
        };

        let donate_cr = CornerRadius::same(6);
        painter.rect_filled(donate_rect, donate_cr, donate_fill);
        painter.rect_stroke(
            donate_rect,
            donate_cr,
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 40)),
            StrokeKind::Inside,
        );

        let donate_font = FontId::new(14.0, crate::app::semibold());
        painter.text(
            donate_rect.center(),
            Align2::CENTER_CENTER,
            donate_label,
            donate_font,
            Color32::WHITE,
        );

        if donate_resp.clicked() {
            ui.ctx()
                .open_url(eframe::egui::OpenUrl::new_tab(DONATE_URL));
            return SupportAction::OpenDonateUrl;
        }

        SupportAction::None
    }
}
