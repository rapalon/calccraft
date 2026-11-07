//! Keyboard focus and screen reader support for the hand-painted controls.
//!
//! Every control goes through `control`, which makes it clickable, reachable with
//! `Tab`, `Shift+Tab` and the arrow keys (egui moves the focus between focusable
//! widgets: `Tab` in drawing order, the arrows to the nearest widget in that
//! direction), and describes it to screen readers through AccessKit. The control
//! with keyboard focus gets WinUI's focus visual. Space and Enter press the focused
//! control, except that Enter stays `=` on the calculator's keys (see
//! `CalculatorApp::handle_keyboard`).

use eframe::egui::{
    self, Context, Id, LayerId, Order, Rect, Response, Sense, Stroke, StrokeKind, Ui, WidgetInfo,
    WidgetType, accesskit, scroll_area::ScrollSource,
};

use super::palette::Palette;

/// Where the "covered by a popup" flag lives in egui's temporary data.
fn covered_id() -> Id {
    Id::new("access-covered")
}

/// Marks the controls drawn from now on as covered by a popup, or not. Covered controls
/// leave the `Tab` order, so the keyboard stays in the popup, like in WinUI's
/// light-dismiss popups and menus, and sense only hovers. The popup's backdrop takes the
/// pointer's clicks but not its drags, and egui takes a screen reader's (AccessKit's
/// `Click` action) for a real click. Covered scroll areas don't scroll either
/// (`scroll_source`).
pub(super) fn set_covered(ctx: &Context, covered: bool) {
    ctx.data_mut(|data| data.insert_temp(covered_id(), covered));
}

fn covered(ui: &Ui) -> bool {
    ui.ctx()
        .data(|data| data.get_temp(covered_id()))
        .unwrap_or(false)
}

/// Draws the controls that an open history or memory flyout lies over: in view, but covered
/// like the controls under a popup (out of the `Tab` order and not clickable, not even by a
/// screen reader), since Windows' flyout covers the whole window (`Placement="Full"`). The
/// flyout's own overlay takes the pointer's clicks.
pub(super) fn draw_covered(ui: &Ui, draw: impl FnOnce()) {
    let ctx = ui.ctx();
    let covered = covered(ui);
    set_covered(ctx, true);
    draw();
    set_covered(ctx, covered);
}

/// Registers a control drawn in `rect`: clickable and focusable when enabled, unless a
/// popup covers it, and described by `info`. Draws the focus visual when it has the
/// keyboard focus, with corners of `radius`.
pub(crate) fn control(
    ui: &Ui,
    p: &Palette,
    rect: Rect,
    id: Id,
    enabled: bool,
    radius: f32,
    info: impl Fn() -> WidgetInfo,
) -> Response {
    let sense = if enabled {
        Sense::CLICK
    } else {
        Sense::hover()
    };
    interact(ui, p, rect, id, sense, radius, info)
}

/// Like `control`, for a control that senses more than clicks (the graph and the sliders
/// are dragged): focusable unless it only senses hovers, as it does under a popup.
pub(super) fn interact(
    ui: &Ui,
    p: &Palette,
    rect: Rect,
    id: Id,
    sense: Sense,
    radius: f32,
    info: impl Fn() -> WidgetInfo,
) -> Response {
    let covered = covered(ui);
    let sense = if covered {
        Sense::hover()
    } else if sense == Sense::hover() {
        sense
    } else {
        sense | Sense::FOCUSABLE
    };
    // egui's arrow-key search remembers the focusable widgets of earlier frames for as
    // long as their ids are used, so covered controls take another id: otherwise the
    // arrows could leave a popup for a control under it.
    let id = if covered { id.with("covered") } else { id };
    let response = ui.interact(rect, id, sense);
    response.widget_info(info);
    // egui doesn't scroll to the focused widget itself: a list item reached with the
    // arrow keys has to come into view.
    if response.has_focus() && !ui.clip_rect().contains_rect(rect) {
        response.scroll_to_me(None);
    }
    focus_ring(ui, p, &response, radius);
    response
}

/// How a scroll area drawn now scrolls: with `source`, but not at all under a popup, as egui
/// scrolls an area under the pointer with the wheel whatever covers it in its layer.
pub(super) fn scroll_source(ui: &Ui, source: ScrollSource) -> ScrollSource {
    if covered(ui) {
        ScrollSource::NONE
    } else {
        source
    }
}

/// A button's description.
pub(crate) fn button(enabled: bool, name: &str) -> impl Fn() -> WidgetInfo + '_ {
    move || WidgetInfo::labeled(WidgetType::Button, enabled, name)
}

/// An item that is selected or not: a list or menu item, a tab, a radio button.
pub(super) fn item(typ: WidgetType, name: &str, selected: bool) -> impl Fn() -> WidgetInfo + '_ {
    move || WidgetInfo::selected(typ, true, selected, name)
}

/// A drop-down: its name and the value it shows.
pub(super) fn combo_box<'a>(name: &'a str, value: &'a str) -> impl Fn() -> WidgetInfo + 'a {
    move || {
        let mut info = WidgetInfo::labeled(WidgetType::ComboBox, true, name);
        info.current_text_value = Some(value.to_string());
        info
    }
}

/// Text that isn't a control, like a result, for screen readers. A `live` text is
/// read out when it changes, as Windows' `LiveSetting="Polite"` results are.
pub(super) fn text(ui: &Ui, rect: Rect, id: Id, name: &str, live: bool) {
    ui.interact(rect, id, Sense::hover());
    ui.ctx().accesskit_node_builder(id, |node| {
        // AccessKit names a label by its value.
        node.set_role(accesskit::Role::Label);
        node.set_value(name);
        if live {
            node.set_live(accesskit::Live::Polite);
        }
    });
}

/// WinUI's focus visual: a 2 px outer ring and a 1 px inner one, 3 px outside the control
/// (`FocusVisualMargin="-3"`). It's painted on a layer above everything, so the
/// neighbors drawn after the control don't cover it, and clipped like the control
/// (by a scrolling list, for example).
fn focus_ring(ui: &Ui, p: &Palette, response: &Response, radius: f32) {
    if !response.has_focus() {
        return;
    }
    let outer = response.rect.expand(3.0);
    let painter = ui
        .ctx()
        .layer_painter(LayerId::new(Order::Tooltip, Id::new("focus-visual")))
        .with_clip_rect(ui.clip_rect().expand(3.0));
    painter.rect_stroke(
        outer,
        radius + 3.0,
        Stroke::new(2.0, p.focus_outer),
        StrokeKind::Inside,
    );
    painter.rect_stroke(
        outer.shrink(2.0),
        radius + 1.0,
        Stroke::new(1.0, p.focus_inner),
        StrokeKind::Inside,
    );
}

/// The id of the widget with keyboard focus, if any.
pub(super) fn focused(ctx: &egui::Context) -> Option<Id> {
    ctx.memory(|memory| memory.focused())
}
