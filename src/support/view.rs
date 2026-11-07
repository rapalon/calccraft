//! UI rendering and interaction logic for the support prompt module.

use eframe::Storage;
use eframe::egui::{
    Align, Align2, Color32, CornerRadius, FontId, Id, Rect, Sense, Stroke, StrokeKind, Ui,
    UiBuilder, pos2, scroll_area::ScrollArea, text::LayoutJob, vec2,
};

use crate::app::access;
use crate::app::icons::{ICON_CLOSE, icon_label};
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

/// Helper function to render the green Donate button.
fn draw_donate_button(
    ui: &Ui,
    p: &Palette,
    painter: &eframe::egui::Painter,
    rect: Rect,
) -> (eframe::egui::Response, bool) {
    let donate_label = tr("CalcCraft.Support.Donate");
    let donate_resp = access::control(
        ui,
        p,
        rect,
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
    painter.rect_filled(rect, donate_cr, donate_fill);
    painter.rect_stroke(
        rect,
        donate_cr,
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 40)),
        StrokeKind::Inside,
    );

    let donate_font = FontId::new(14.0, crate::app::semibold());
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        donate_label,
        donate_font,
        Color32::WHITE,
    );

    let clicked = donate_resp.clicked();
    (donate_resp, clicked)
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
        #[cfg(not(target_arch = "wasm32"))]
        let is_open = std::env::var("CALCCRAFT_SUPPORT_OPEN")
            .map(|v| v == "1" || v == "true")
            .unwrap_or(false);
        #[cfg(target_arch = "wasm32")]
        let is_open = false;
        Self {
            is_open,
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

        // 1. Compute geometry with horizontal padding and text bounds
        let card_width = (window_rect.width() * 0.78)
            .clamp(280.0, 480.0)
            .min((window_rect.width() - 24.0).max(200.0));
        let h_pad = 24.0;
        let body_max_width = (card_width - 2.0 * h_pad).max(160.0);
        let title_max_width = (card_width - 80.0).max(160.0);

        // Text colors matching the warm amber theme
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

        // Layout text galleys with centered alignment and wrapping
        let title_text = tr("CalcCraft.Support.Title");
        let title_font = FontId::new(19.0, crate::app::semibold());
        let mut title_job = LayoutJob::simple(
            title_text.to_string(),
            title_font,
            title_color,
            title_max_width,
        );
        title_job.halign = Align::Center;
        let title_galley = ui.painter().layout_job(title_job);
        let title_height = title_galley.size().y;

        let body_text_raw = tr("CalcCraft.Support.Body");
        let body_text_long;
        #[cfg(not(target_arch = "wasm32"))]
        let long_env = std::env::var("CALCCRAFT_SUPPORT_LONG_TEXT")
            .map(|v| v == "1" || v == "true")
            .unwrap_or(false);
        #[cfg(target_arch = "wasm32")]
        let long_env = false;
        let body_text = if long_env {
            body_text_long = format!("{body_text_raw} {body_text_raw} {body_text_raw}");
            body_text_long.as_str()
        } else {
            body_text_raw
        };
        let body_font = FontId::proportional(13.5);
        let mut body_job =
            LayoutJob::simple(body_text.to_string(), body_font, body_color, body_max_width);
        body_job.halign = Align::Center;
        let body_galley = ui.painter().layout_job(body_job);
        let body_height = body_galley.size().y;

        // Content vertical block metrics (Fluent-like spacing: 14 px between title/body, 22 px before button)
        let spacing_title_body = 14.0;
        let spacing_body_button = 22.0;
        let btn_height = 36.0;
        let btn_width = (card_width * 0.52).clamp(140.0, 200.0);

        let content_height =
            title_height + spacing_title_body + body_height + spacing_body_button + btn_height;

        // Card height dynamically adapts to content within bounds
        let v_pad = 28.0;
        let ideal_card_height = content_height + 2.0 * v_pad;
        let min_card_height = 200.0;
        let max_card_height = (window_rect.height() - 32.0).clamp(min_card_height, 520.0);
        let card_height = ideal_card_height.clamp(min_card_height, max_card_height);
        let card_rect = Rect::from_center_size(window_rect.center(), vec2(card_width, card_height));

        // Clip all drawing to card_rect so nothing ever renders outside the card
        let painter = ui.painter().with_clip_rect(card_rect);

        // 2. Semi-transparent amber card panel
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

        // Consume clicks within the card panel bounds so keys beneath aren't clicked
        ui.interact(card_rect, Id::new("support-card-bg"), Sense::CLICK);

        // 3. Fluent Close "X" button in top-right corner (32x32 hit target, neutral color)
        let close_size = 32.0;
        let close_margin = 8.0;
        let close_rect = Rect::from_min_size(
            pos2(
                card_rect.right() - close_size - close_margin,
                card_rect.top() + close_margin,
            ),
            vec2(close_size, close_size),
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
            let fill = if dark {
                Color32::from_white_alpha(45)
            } else {
                Color32::from_black_alpha(35)
            };
            painter.rect_filled(close_rect, CornerRadius::same(4), fill);
        } else if close_hovered {
            let fill = if dark {
                Color32::from_white_alpha(25)
            } else {
                Color32::from_black_alpha(20)
            };
            painter.rect_filled(close_rect, CornerRadius::same(4), fill);
        }

        let close_icon_color = if close_hovered || close_pressed {
            p.text
        } else {
            p.text_secondary
        };
        let close_glyph = painter.layout_job(icon_label(ICON_CLOSE, 12.0));
        let glyph_pos = pos2(
            close_rect.center().x - close_glyph.size().x / 2.0,
            close_rect.center().y - close_glyph.size().y / 2.0,
        );
        painter.galley(glyph_pos, close_glyph, close_icon_color);

        if close_resp.clicked() {
            self.is_open = false;
            return SupportAction::Close;
        }

        // 4. Cohesive vertical block layout: Title, Body, Donate button
        let overflow = content_height + 32.0 > card_rect.height();
        if !overflow {
            let extra_v = card_rect.height() - content_height;
            let block_top = card_rect.top() + (extra_v / 2.0);

            // Title
            let title_top = block_top;
            painter.galley(
                pos2(card_rect.center().x, title_top),
                title_galley,
                title_color,
            );

            // Body
            let body_top = title_top + title_height + spacing_title_body;
            painter.galley(
                pos2(card_rect.center().x, body_top),
                body_galley,
                body_color,
            );

            // Donate button
            let donate_top = body_top + body_height + spacing_body_button;
            let donate_rect = Rect::from_min_size(
                pos2(card_rect.center().x - btn_width / 2.0, donate_top),
                vec2(btn_width, btn_height),
            );
            let (_resp, clicked) = draw_donate_button(ui, p, &painter, donate_rect);
            if clicked {
                ui.ctx()
                    .open_url(eframe::egui::OpenUrl::new_tab(DONATE_URL));
                return SupportAction::OpenDonateUrl;
            }
        } else {
            // Content exceeded card height (e.g. tiny window or extreme translation): scroll inside card
            let scroll_rect = Rect::from_min_max(
                pos2(card_rect.left() + 8.0, card_rect.top() + 42.0),
                pos2(card_rect.right() - 8.0, card_rect.bottom() - 12.0),
            );
            let mut donate_clicked = false;
            ui.scope_builder(UiBuilder::new().max_rect(scroll_rect), |ui| {
                ScrollArea::vertical()
                    .id_salt("support-card-scroll")
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        let (title_alloc, _) = ui.allocate_exact_size(
                            vec2(ui.available_width(), title_height),
                            Sense::hover(),
                        );
                        ui.painter().galley(
                            pos2(title_alloc.center().x, title_alloc.top()),
                            title_galley,
                            title_color,
                        );

                        ui.add_space(spacing_title_body);

                        let (body_alloc, _) = ui.allocate_exact_size(
                            vec2(ui.available_width(), body_height),
                            Sense::hover(),
                        );
                        ui.painter().galley(
                            pos2(body_alloc.center().x, body_alloc.top()),
                            body_galley,
                            body_color,
                        );

                        ui.add_space(spacing_body_button);

                        let (btn_alloc, _) = ui.allocate_exact_size(
                            vec2(ui.available_width(), btn_height),
                            Sense::hover(),
                        );
                        let donate_rect =
                            Rect::from_center_size(btn_alloc.center(), vec2(btn_width, btn_height));
                        let (_resp, clicked) =
                            draw_donate_button(ui, p, &ui.painter().clone(), donate_rect);
                        if clicked {
                            donate_clicked = true;
                        }
                    });
            });
            if donate_clicked {
                ui.ctx()
                    .open_url(eframe::egui::OpenUrl::new_tab(DONATE_URL));
                return SupportAction::OpenDonateUrl;
            }
        }

        SupportAction::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn galley_layout_wrapping() {
        let ctx = eframe::egui::Context::default();
        let mut output = ctx.run_ui(Default::default(), |ui| {
            let mut job = eframe::egui::text::LayoutJob::simple(
                "CalcCraft is free and ad-free. Your support helps keep development active."
                    .to_string(),
                FontId::proportional(13.5),
                Color32::WHITE,
                300.0,
            );
            job.halign = eframe::egui::Align::Center;
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

    #[test]
    fn support_popup_wrapping_and_bounds_test() {
        let ctx = eframe::egui::Context::default();
        crate::app::setup_fonts(&ctx, &[]);
        let mut state = SupportState {
            is_open: true,
            stats: SupportStats::new(),
        };

        // Window size ~480 x 600 px
        let window_size = vec2(480.0, 600.0);
        let window_rect = Rect::from_min_size(pos2(0.0, 0.0), window_size);

        let p = &crate::app::palette::LIGHT;

        let mut output = ctx.run_ui(
            eframe::egui::RawInput {
                screen_rect: Some(window_rect),
                ..Default::default()
            },
            |ui| {
                state.draw_popup(ui, window_rect, false, p);
            },
        );
        output.textures_delta.clear();

        // Check close button response
        let close_resp = ctx.read_response(Id::new("support-close")).unwrap();
        assert!(close_resp.rect.width() >= 32.0);
        assert!(close_resp.rect.height() >= 32.0);

        // Check donate button response
        let donate_resp = ctx.read_response(Id::new("support-donate")).unwrap();
        assert!(donate_resp.rect.width() >= 140.0);
        assert!(donate_resp.rect.height() >= 36.0);

        // Verify donate button is below close button and within window
        assert!(donate_resp.rect.top() > close_resp.rect.bottom());
        assert!(window_rect.contains_rect(close_resp.rect));
        assert!(window_rect.contains_rect(donate_resp.rect));
    }

    #[test]
    fn support_popup_long_text_wrapping_and_confinement() {
        let ctx = eframe::egui::Context::default();

        // Test artificially long body string (3x the English text)
        let long_body = "CalcCraft is free and ad-free. Your support helps keep development active. \
            CalcCraft is free and ad-free. Your support helps keep development active. \
            CalcCraft is free and ad-free. Your support helps keep development active.";

        let mut output = ctx.run_ui(Default::default(), |ui| {
            let card_width = 360.0;
            let h_pad = 24.0;
            let body_max_width = card_width - 2.0 * h_pad;

            let mut job = LayoutJob::simple(
                long_body.to_string(),
                FontId::proportional(13.5),
                Color32::WHITE,
                body_max_width,
            );
            job.halign = Align::Center;
            let galley = ui.painter().layout_job(job);

            // Should wrap into at least 5 lines
            assert!(
                galley.rows.len() >= 5,
                "long body text should wrap into multiple lines"
            );
            assert!(
                galley.size().x <= body_max_width + 1.0,
                "galley must not exceed text area width"
            );
        });
        output.textures_delta.clear();
    }
}
