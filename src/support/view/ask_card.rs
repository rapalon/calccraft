//! The "share with three friends" card of the Android asks (docs/support-prompt.md, 13).
//! A child module of `view`, so it draws with the popup's colors and sizes.

use super::*;

/// What the user did on the card this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CardAction {
    Share,
    NotNow,
}

const BUTTON_HEIGHT: f32 = 36.0;
const BUTTON_GAP: f32 = 10.0;
const PAD: f32 = 20.0;

struct Layout {
    title: Arc<Galley>,
    body: Arc<Galley>,
    height: f32,
}

fn layout(ui: &Ui, p: &Palette, width: f32) -> Layout {
    let centered = |text: &str, font: FontId, color: Color32| {
        let mut job =
            LayoutJob::simple(text.to_owned(), font, color, (width - 2.0 * PAD).max(160.0));
        job.halign = Align::Center;
        ui.painter().layout_job(job)
    };
    let title = centered(
        tr("CalcCraft.Ask.ShareTitle"),
        FontId::new(17.0, crate::app::semibold()),
        p.text,
    );
    let body = centered(
        tr("CalcCraft.Ask.ShareBody"),
        FontId::proportional(13.5),
        p.text_secondary,
    );
    let height = title.size().y + 10.0 + body.size().y + 18.0 + BUTTON_HEIGHT;
    Layout {
        title,
        body,
        height,
    }
}

impl SupportState {
    /// The share card, if one is open, under the display like the popup. It takes Esc as
    /// "Not now" (`begin_frame`).
    pub(super) fn draw_share_card(&mut self, ui: &mut Ui, window: Rect, p: &Palette) {
        if self.share_card.is_none() {
            return;
        }
        let display = noted_display(ui.ctx());
        let width = card_width(display.unwrap_or(window).width());
        let content = layout(ui, p, width);
        let card = card_rect(window, display, width, content.height);
        let card = Rect::from_center_size(
            card.center(),
            vec2(
                card.width(),
                (content.height + 2.0 * PAD).min(card.height()),
            ),
        );
        let dark = dark(ui.ctx());
        let (fill, stroke) = if dark {
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
        let painter = ui.painter().with_clip_rect(card);
        let radius = CornerRadius::same(12);
        painter.rect_filled(card, radius, fill);
        painter.rect_stroke(card, radius, stroke, StrokeKind::Inside);
        // The card takes the clicks on it, so the keys under it aren't pressed.
        ui.interact(card, Id::new("support-ask-bg"), Sense::CLICK);

        let center = card.center().x;
        let mut y = card.top() + (card.height() - content.height) / 2.0;
        painter.galley(pos2(center, y), content.title.clone(), p.text);
        y += content.title.size().y + 10.0;
        painter.galley(pos2(center, y), content.body.clone(), p.text_secondary);
        y += content.body.size().y + 18.0;

        let half = ((card.width() - 2.0 * PAD - BUTTON_GAP) / 2.0).clamp(80.0, 190.0);
        let left = Rect::from_min_size(
            pos2(center - BUTTON_GAP / 2.0 - half, y),
            vec2(half, BUTTON_HEIGHT),
        );
        let right = Rect::from_min_size(
            pos2(center + BUTTON_GAP / 2.0, y),
            vec2(half, BUTTON_HEIGHT),
        );
        let mut action = None;
        if ask_button(
            ui,
            p,
            &painter,
            left,
            "support-ask-not-now",
            "CalcCraft.Ask.NotNow",
            false,
        ) {
            action = Some(CardAction::NotNow);
        }
        if ask_button(
            ui,
            p,
            &painter,
            right,
            "support-ask-share",
            "CalcCraft.Ask.ShareButton",
            true,
        ) {
            action = Some(CardAction::Share);
        }
        match action {
            Some(CardAction::Share) => self.share_from_card(ui.ctx()),
            Some(CardAction::NotNow) => self.share_card = None,
            None => {}
        }
    }
}

/// A button of the card: `primary` is the green one.
fn ask_button(
    ui: &Ui,
    p: &Palette,
    painter: &Painter,
    rect: Rect,
    id: &str,
    key: &str,
    primary: bool,
) -> bool {
    let label = tr(key);
    let response = access::control(
        ui,
        p,
        rect,
        Id::new(id),
        true,
        6.0,
        access::button(true, label),
    );
    let radius = CornerRadius::same(6);
    if primary {
        let fill = if response.is_pointer_button_down_on() {
            Color32::from_rgb(14, 107, 56)
        } else if response.hovered() {
            Color32::from_rgb(19, 137, 74)
        } else {
            Color32::from_rgb(16, 124, 65)
        };
        painter.rect_filled(rect, radius, fill);
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            label,
            FontId::new(14.0, crate::app::semibold()),
            Color32::WHITE,
        );
    } else {
        let dark = dark(ui.ctx());
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
        painter.rect_filled(rect, radius, fill);
        painter.rect_stroke(
            rect,
            radius,
            Stroke::new(1.0, p.text_secondary),
            StrokeKind::Inside,
        );
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(13.5),
            p.text,
        );
    }
    response.clicked()
}
