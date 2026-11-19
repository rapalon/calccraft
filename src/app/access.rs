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

/// Whether the controls drawn now are covered by a popup or a flyout (`set_covered`).
pub(super) fn is_covered(ui: &Ui) -> bool {
    covered(ui)
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
    clip_bounds(ui, &response);
    // egui doesn't scroll to the focused widget itself: a list item reached with the
    // arrow keys has to come into view.
    if response.has_focus() && !ui.clip_rect().contains_rect(rect) {
        response.scroll_to_me(None);
    }
    focus_ring(ui, p, &response, radius);
    response
}

/// A control that its scroll area shows only in part keeps only that part as its bounds, and
/// one it doesn't show at all leaves the tree (unless it has the focus). egui gives every
/// node its whole rectangle, so a list item scrolled out of view lay over the controls beside
/// the list: an accessibility service that taps a node's center (Marionet, a uiautomator
/// script) tapped the Settings button for a navigation item hidden under it.
fn clip_bounds(ui: &Ui, response: &Response) {
    let rect = response.rect;
    let visible = rect.intersect(ui.clip_rect());
    if visible == rect {
        return;
    }
    let has_focus = response.has_focus();
    ui.ctx().accesskit_node_builder(response.id, |node| {
        if visible.width() > 0.0 && visible.height() > 0.0 {
            node.set_bounds(bounds(visible));
        } else if !has_focus {
            node.set_hidden();
        }
    });
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
pub(crate) fn text(ui: &Ui, rect: Rect, id: Id, name: &str, live: bool) {
    ui.interact(rect, id, Sense::hover());
    ui.ctx().accesskit_node_builder(id, |node| {
        // AccessKit names a label by its value.
        node.set_role(accesskit::Role::Label);
        node.set_value(name);
        // egui gives bounds only to the nodes of interactive widgets. Without them Android
        // shows the text at [0,0][0,0], and services that skip empty nodes (Marionet) lose
        // the display and the expression line.
        node.set_bounds(bounds(rect));
        if live {
            node.set_live(accesskit::Live::Polite);
        }
    });
}

/// A group of controls for screen readers and accessibility services: a `Group` node named
/// `name` with the bounds of `rect`, and a child `Ui` (as large as `ui`, so the layout doesn't
/// change) whose controls become the group's children. egui's tree is otherwise flat, every
/// control a child of the window, and Marionet keeps only a node's first 50 children: the
/// keypads and the popup menus are groups so that none is cut (android/README.md).
pub(super) fn group(ui: &Ui, id: Id, rect: Rect, name: &str) -> Ui {
    ui.interact(rect, id, Sense::hover());
    ui.ctx().accesskit_node_builder(id, |node| {
        node.set_role(accesskit::Role::Group);
        node.set_label(name);
        node.set_bounds(bounds(rect));
    });
    children_of(ui, id)
}

/// A `Ui` like `ui` (as large, on the same layer, with the same clip and style) whose
/// controls become children of the node `id` in the accessibility tree, which must exist.
pub(super) fn children_of(ui: &Ui, id: Id) -> Ui {
    // The drawing code has the `Ui` only by reference, so this is a new one, not a `new_child`.
    let mut builder = egui::UiBuilder::new()
        .layer_id(ui.layer_id())
        .max_rect(ui.max_rect())
        .style(ui.style().clone())
        .accessibility_parent(id);
    if !ui.is_enabled() {
        builder = builder.disabled();
    }
    let mut children = Ui::new(ui.ctx().clone(), id.with("ui"), builder);
    children.set_clip_rect(ui.clip_rect());
    children
}

/// A rectangle in points as AccessKit's bounds (egui's root node scales points to pixels).
pub(super) fn bounds(rect: Rect) -> accesskit::Rect {
    accesskit::Rect {
        x0: rect.min.x.into(),
        y0: rect.min.y.into(),
        x1: rect.max.x.into(),
        y1: rect.max.y.into(),
    }
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

/// Where `settle_focus` keeps a focus request for the next frame.
fn deferred_focus_id() -> Id {
    Id::new("access-deferred-focus")
}

/// Where `begin_frame` notes the request it has just made again, so that it's made only once.
fn retried_focus_id() -> Id {
    Id::new("access-retried-focus")
}

/// At the start of a frame: gives the focus to the widget that `settle_focus` kept it for.
pub(super) fn begin_frame(ctx: &Context) {
    let deferred = ctx.data_mut(|data| data.remove_temp::<Option<Id>>(deferred_focus_id()));
    if let Some(id) = deferred.flatten() {
        ctx.memory_mut(|memory| memory.request_focus(id));
        ctx.data_mut(|data| data.insert_temp(retried_focus_id(), Some(id)));
    }
}

/// At the end of a frame, while a screen reader or another accessibility service reads the
/// tree: a focus request for a widget that isn't drawn in this frame waits for the next one.
///
/// egui keeps a focus requested in this frame even if the widget isn't drawn (it may appear
/// in the next one), and names it as the AccessKit tree's focus, but without its node. A
/// tree whose focus isn't in it is invalid, and AccessKit's consumer panics on it: that
/// ended the Android app when the Graphing keypad's ↵ (Submit) added a box and focused it,
/// as Windows does, in the frame before the new box is drawn (`submit_with_enter`). The
/// focus goes to the widget at the start of the next frame instead (`begin_frame`), once:
/// a widget that is still not drawn then loses it, as egui's own "dead man's switch" does a
/// frame later. Without AccessKit nothing changes.
pub(super) fn settle_focus(ctx: &Context) {
    let retried = ctx
        .data_mut(|data| data.remove_temp::<Option<Id>>(retried_focus_id()))
        .flatten();
    let Some(focused) = focused(ctx) else {
        return;
    };
    let has_node = ctx.viewport(|viewport| {
        viewport
            .this_pass
            .accesskit_state
            .as_ref()
            .map(|state| state.nodes.contains_key(&focused))
    });
    if has_node != Some(false) {
        return;
    }
    ctx.memory_mut(|memory| memory.surrender_focus(focused));
    if retried != Some(focused) {
        ctx.data_mut(|data| data.insert_temp(deferred_focus_id(), Some(focused)));
        ctx.request_repaint();
    }
}
