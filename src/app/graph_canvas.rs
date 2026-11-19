//! The graph of the Graphing mode (`GraphControl::Grapher` and its renderer): the grid,
//! the axes and their labels, the equations' lines and inequality regions, the traced
//! point and its value, and the buttons over the graph: tracing, share and graph options
//! at the top, zoom and the graph view at the bottom.

use eframe::egui::{
    self, Color32, ColorImage, CursorIcon, Event, EventFilter, FontFamily, FontId, Id, Key, Mesh,
    MouseWheelUnit, Painter, PointerButton, Pos2, Rect, Sense, Shape, Stroke, StrokeKind,
    TextFormat, Ui, ViewportCommand, WidgetInfo, WidgetType, pos2, text::LayoutJob, vec2,
};

use super::graphing_view::{Capture, Popup, Purpose, edit_id, paint_graph_icon};
use super::icons::{ICON_MINUS, ICON_PLUS, ICON_SETTINGS, ICON_SHARE, icon_label};
use super::palette::{GraphPalette, Palette};
use super::strings::{tr, tr_format};
use super::{CalculatorApp, RADIUS, access, numbers, share};
use crate::engine::Angle;
use crate::graphing::plot::{self, LineStyle, Plot, View};
use crate::graphing::{Env, Equation};

/// The tracing button's id; it gets the focus back when Esc stops tracing.
pub(super) const TRACE_ID: &str = "graph-trace";

/// The plots of a frame, and what they were made for: they're made again only when the
/// view, the size, the equations or the variables change.
pub(super) struct PlotCache {
    key: CacheKey,
    /// The index of each plotted row, and its plot.
    plots: Vec<(usize, Plot)>,
}

#[derive(PartialEq)]
struct CacheKey {
    view: View,
    size: (f32, f32),
    angle: Angle,
    rows: Vec<(u64, String, bool)>,
    values: Vec<(char, u64)>,
}

/// Mouse wheel movement in notches (`WHEEL_DELTA`), positive away from the user.
fn wheel_notches(events: &[Event]) -> Vec<f32> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::MouseWheel { unit, delta, .. } => Some(match unit {
                MouseWheelUnit::Line => delta.y,
                MouseWheelUnit::Point => delta.y / 50.0,
                MouseWheelUnit::Page => delta.y * 3.0,
            }),
            _ => None,
        })
        .collect()
}

/// A tick label: as many decimals as the grid step needs, Windows' minus sign and the
/// region's decimal separator.
fn tick_label(value: f64, step: f64) -> String {
    let decimals = (-step.log10().floor()).max(0.0) as usize;
    let text = format!("{:.*}", decimals, value);
    let text = if text
        .trim_start_matches('-')
        .chars()
        .all(|c| c == '0' || c == '.')
    {
        "0".to_string()
    } else {
        text
    };
    numbers().localize_plain(&text).replace('-', "−")
}

/// The traced point's value, as Windows writes it: x as rounded ("R"), y with 15
/// decimals ("N15").
pub(super) fn trace_text(x: f64, y: f64) -> String {
    // Windows' .NET Native formats like .NET before Core 3.0, which drops the sign of a
    // value that rounds to 0: −0 is "0" and −0.0000000000000004 is "0.000000000000000".
    let unsigned = |text: String| match text.strip_prefix('-') {
        Some(rest) if rest.chars().all(|c| c == '0' || c == '.') => rest.to_string(),
        _ => text,
    };
    let x = numbers().localize_plain(&unsigned(format!("{x}")));
    let y = numbers().localize(&unsigned(format!("{y:.15}")));
    format!("({x}, {y})")
}

impl CalculatorApp {
    /// Draws the graph in `rect`. Without `controls` (a copy of the graph) it draws only
    /// the graph itself.
    pub(super) fn draw_graph(&mut self, ui: &mut Ui, rect: Rect, controls: bool) {
        let p = self.palette;
        let ctx = ui.ctx().clone();
        let dark = ctx.theme() == egui::Theme::Dark;
        let gp: &GraphPalette = self.graphing.graph_palette(dark);
        let (w, h) = (rect.width().max(1.0), rect.height().max(1.0));
        self.graphing.graph_size = (w, h);
        let values = self.graphing.values();
        let env = Env {
            angle: self.graphing.angle,
            vars: &values,
        };

        // The automatic best fit, for new equations or a new size.
        if !self.graphing.manual && self.graphing.fitted != Some((w, h)) {
            let equations: Vec<(&Equation, Env)> = self
                .graphing
                .rows
                .iter()
                .filter_map(|r| r.graphable())
                .map(|e| (e, env))
                .collect();
            self.graphing.view = plot::best_fit(&equations, w, h);
            self.graphing.fitted = Some((w, h));
        }

        // The pointer: drag to pan, the wheel or a pinch to zoom, right-click for Copy.
        let id = Id::new("graph");
        let count = self
            .graphing
            .rows
            .iter()
            .filter(|r| r.graphable().is_some())
            .count();
        let view = self.graphing.view;
        let name = tr_format(
            "graphAutomationName",
            &[
                &g(view.x_min),
                &g(view.x_max),
                &g(view.y_min),
                &g(view.y_max),
                &count.to_string(),
            ],
        );
        let response = if controls {
            Some(access::interact(
                ui,
                p,
                rect,
                id,
                Sense::click_and_drag(),
                0.0,
                || WidgetInfo::labeled(WidgetType::Other, true, &name),
            ))
        } else {
            None
        };
        if let Some(response) = &response {
            // The name is Windows' (`graphAutomationName`), which counts the equations; the
            // value lists them, one a line, so that an accessibility service can read what is
            // graphed (Android shows the value as the node's text). The node is a canvas, not
            // egui's "unknown" role for `WidgetType::Other`. Under a popup the graph only
            // senses hovers, under another id (`access::interact`), and egui gives such a
            // node neither bounds nor a name: they're set here.
            let equations: Vec<&str> = self
                .graphing
                .rows
                .iter()
                .filter(|r| r.graphable().is_some())
                .map(|r| r.submitted.as_str())
                .collect();
            ctx.accesskit_node_builder(response.id, |node| {
                node.set_role(egui::accesskit::Role::Canvas);
                node.set_label(name.as_str());
                node.set_bounds(access::bounds(rect));
                node.set_value(equations.join("\n"));
            });
        }
        let mut pointer = None;
        if let Some(response) = &response {
            // A pinch on a touch screen that starts on the graph zooms around the point between
            // the fingers, and moving them pans, like the wheel and a drag. The first finger,
            // which egui also takes for the pointer, doesn't drag the graph meanwhile.
            let pinch = ctx
                .input(|i| i.multi_touch())
                .filter(|touch| response.sense.senses_drag() && rect.contains(touch.start_pos));
            if let Some(touch) = pinch {
                let d = touch.translation_delta;
                let at = touch.center_pos - rect.min;
                let mut view = self.graphing.view.dragged(d.x as f64, d.y as f64, w, h);
                if touch.zoom_delta > 0.0 {
                    let scale = 1.0 / touch.zoom_delta as f64;
                    view = view.zoomed(at.x as f64, at.y as f64, w, h, scale);
                }
                if view != self.graphing.view {
                    self.graphing.view = view;
                    self.graphing.manual = true;
                }
            } else if response.dragged_by(PointerButton::Primary) {
                let d = response.drag_delta();
                if d != egui::Vec2::ZERO {
                    self.graphing.view = self.graphing.view.dragged(d.x as f64, d.y as f64, w, h);
                    self.graphing.manual = true;
                }
            }
            if response.hovered() {
                let events = ctx.input(|i| i.events.clone());
                let at = ctx.pointer_hover_pos().unwrap_or(rect.center()) - rect.min;
                for notches in wheel_notches(&events) {
                    if notches == 0.0 {
                        continue;
                    }
                    // `scrollDamper`: 15 % per notch, in toward the pointer when turned away.
                    let mut scale = 1.0 + notches.abs() as f64 * 0.15;
                    if notches > 0.0 {
                        scale = 1.0 / scale;
                    }
                    self.graphing.view =
                        self.graphing
                            .view
                            .zoomed(at.x as f64, at.y as f64, w, h, scale);
                    self.graphing.manual = true;
                }
                for event in &events {
                    if let Event::Zoom(factor) = event
                        && *factor > 0.0
                    {
                        self.graphing.view = self.graphing.view.zoomed(
                            at.x as f64,
                            at.y as f64,
                            w,
                            h,
                            1.0 / *factor as f64,
                        );
                        self.graphing.manual = true;
                    }
                }
                if !response.dragged() && pinch.is_none() {
                    pointer = ctx.pointer_hover_pos().map(|p| p - rect.min);
                }
            }
            if response.clicked() {
                response.request_focus();
            }
            if response.secondary_clicked() {
                let at = ctx.pointer_interact_pos().unwrap_or(rect.center());
                self.graphing.popup = Some(Popup::GraphMenu { at });
            }
        }

        // Active tracing: the pointer, the arrow keys (5 px per tenth of a second, 1 px
        // with Shift) move the trace pointer (`HandleTracingMovementTick`).
        let tracing = controls && self.graphing.tracing;
        let mut trace_at = None;
        if tracing {
            let mut at = self
                .graphing
                .trace_at
                .unwrap_or(pos2(w / 2.0 + 40.0, h / 2.0 - 40.0));
            if let Some(pointer) = pointer {
                at = pointer.to_pos2();
                ctx.set_cursor_icon(CursorIcon::None);
            }
            // The arrow keys are the graph's only while it has the focus (`OnCoreKeyDown`)
            // or nothing has: in a text box or on a slider or a button they're its own.
            let graph_keys = ctx
                .memory(|m| m.focused())
                .is_none_or(|f| response.as_ref().is_some_and(|r| r.id == f));
            if graph_keys {
                if let Some(response) = &response
                    && response.has_focus()
                {
                    ctx.memory_mut(|m| {
                        m.set_focus_lock_filter(
                            id,
                            EventFilter {
                                horizontal_arrows: true,
                                vertical_arrows: true,
                                ..Default::default()
                            },
                        )
                    });
                }
                let now = ctx.input(|i| i.time);
                let (left, right, up, down, slow) = ctx.input(|i| {
                    (
                        i.key_down(Key::ArrowLeft),
                        i.key_down(Key::ArrowRight),
                        i.key_down(Key::ArrowUp),
                        i.key_down(Key::ArrowDown),
                        i.modifiers.shift,
                    )
                });
                if left || right || up || down {
                    if now - self.graphing.trace_moved >= 0.1 {
                        let step = if slow { 1.0 } else { 5.0 };
                        if left {
                            at.x = (at.x - step).max(0.0);
                        }
                        if right {
                            at.x = (at.x + step).min(w - step);
                        }
                        if up {
                            at.y = (at.y - step).max(0.0);
                        }
                        if down {
                            at.y = (at.y + step).min(h - step);
                        }
                        self.graphing.trace_moved = now;
                    }
                    ctx.request_repaint_after(std::time::Duration::from_millis(100));
                }
            }
            self.graphing.trace_at = Some(at);
            trace_at = Some(at);
        } else if controls {
            self.graphing.trace_at = None;
        }

        // The plots.
        let view = self.graphing.view;
        let key = CacheKey {
            view,
            size: (w, h),
            angle: self.graphing.angle,
            rows: self
                .graphing
                .rows
                .iter()
                .map(|r| (r.id, r.submitted.clone(), r.visible))
                .collect(),
            values: values.iter().map(|&(c, v)| (c, v.to_bits())).collect(),
        };
        if self.graphing.cache.as_ref().is_none_or(|c| c.key != key) {
            let plots = self
                .graphing
                .rows
                .iter()
                .enumerate()
                .filter_map(|(i, r)| r.graphable().map(|e| (i, plot::plot(e, &env, &view, w, h))))
                .collect();
            self.graphing.cache = Some(PlotCache { key, plots });
        }

        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, gp.background);
        self.paint_grid(&painter, rect, &view, gp);

        let focused = ctx.memory(|m| m.focused());
        let analyzed = self.graphing.analysis.as_ref().map(|a| a.row);
        let cache = self.graphing.cache.as_ref().map(|c| &c.plots);
        for (index, plot) in cache.into_iter().flatten() {
            let row = &self.graphing.rows[*index];
            let Some(equation) = row.graphable() else {
                continue;
            };
            let color = gp.equations[row.color];
            let selected = focused == Some(edit_id(row.id)) || analyzed == Some(row.id);
            let base = self.graphing.line_width;
            let width = if selected {
                base + if base <= 2.0 { 1.0 } else { 2.0 }
            } else {
                base
            };
            if !plot.dots.is_empty() {
                let mut mesh = Mesh::default();
                for d in &plot.dots {
                    let c = rect.min + vec2(d[0], d[1]);
                    mesh.add_colored_rect(Rect::from_center_size(c, vec2(2.4, 0.8)), color);
                    mesh.add_colored_rect(Rect::from_center_size(c, vec2(0.8, 2.4)), color);
                }
                painter.add(Shape::mesh(mesh));
            }
            let style = if equation.is_inequality() {
                if plot::is_dashed(equation.relation) {
                    LineStyle::Dash
                } else {
                    LineStyle::Solid
                }
            } else {
                row.style
            };
            for line in &plot.lines {
                let points: Vec<Pos2> = line.iter().map(|q| rect.min + vec2(q[0], q[1])).collect();
                paint_line(&painter, points, Stroke::new(width, color), style);
            }
        }

        if !controls {
            self.finish_capture(&ctx, rect);
            return;
        }

        // The traced point: the curve point nearest the pointer (or the trace pointer).
        let target = trace_at.or(pointer.map(|p| p.to_pos2()));
        let mut traced = None;
        if let (Some(target), Some(cache)) = (target, &self.graphing.cache) {
            // The row of each pair: `point.equation` counts the pairs, not the plots.
            let (rows, pairs): (Vec<usize>, Vec<(&Equation, &Plot)>) = cache
                .plots
                .iter()
                .filter_map(|(i, plot)| self.graphing.rows[*i].graphable().map(|e| (*i, (e, plot))))
                .unzip();
            if let Some(point) = plot::trace(&pairs, &env, &view, w, h, target.x, target.y) {
                let index = rows[point.equation];
                traced = Some((point, gp.equations[self.graphing.rows[index].color]));
            }
        }
        if let Some((point, color)) = traced {
            let at = rect.min + vec2(point.px, point.py);
            painter.circle_filled(at, self.graphing.line_width + 1.0, color);
            self.paint_trace_popup(ui, rect, at, &trace_text(point.x, point.y));
        }
        if let Some(at) = trace_at {
            paint_trace_pointer(&painter, rect.min + at.to_vec2());
        }

        self.draw_graph_buttons(ui, rect);
    }

    /// The grid: thin lines at every step, darker at the labeled ones, then the axes with
    /// their arrows, names and numbers.
    fn paint_grid(&self, painter: &Painter, rect: Rect, view: &View, gp: &GraphPalette) {
        let (w, h) = (rect.width(), rect.height());
        let ppp = painter.ctx().pixels_per_point();
        let hair = 1.0 / ppp;
        let gx = plot::grid_step(view.width(), w);
        let gy = plot::grid_step(view.height(), h);
        let px = |x: f64| rect.left() + ((x - view.x_min) / view.width()) as f32 * w;
        let py = |y: f64| rect.top() + ((view.y_max - y) / view.height()) as f32 * h;
        let is_major = |v: f64, step: f64| ((v / step) - (v / step).round()).abs() < 1e-6;
        for major in [false, true] {
            let color = if major { gp.grid } else { gp.minor_grid };
            let stroke = Stroke::new(hair, color);
            let (k0, k1) = (
                (view.x_min / gx.minor).ceil() as i64,
                (view.x_max / gx.minor).floor() as i64,
            );
            if k1 - k0 < 2000 {
                for k in k0..=k1 {
                    let x = k as f64 * gx.minor;
                    if is_major(x, gx.major) == major {
                        let at = painter.round_to_pixel_center(px(x));
                        painter.vline(at, rect.y_range(), stroke);
                    }
                }
            }
            let (k0, k1) = (
                (view.y_min / gy.minor).ceil() as i64,
                (view.y_max / gy.minor).floor() as i64,
            );
            if k1 - k0 < 2000 {
                for k in k0..=k1 {
                    let y = k as f64 * gy.minor;
                    if is_major(y, gy.major) == major {
                        let at = painter.round_to_pixel_center(py(y));
                        painter.hline(rect.x_range(), at, stroke);
                    }
                }
            }
        }

        // The axes, where they're in view, with arrows at their positive ends.
        let axis = Stroke::new(1.0, gp.axes);
        let ox = px(0.0);
        let oy = py(0.0);
        let x_axis = (rect.top()..=rect.bottom()).contains(&oy);
        let y_axis = (rect.left()..=rect.right()).contains(&ox);
        let italic = |s: &str, size: f32| {
            let mut job = LayoutJob::default();
            job.append(
                s,
                0.0,
                TextFormat {
                    font_id: FontId::new(size, FontFamily::Proportional),
                    color: gp.axes,
                    italics: true,
                    ..Default::default()
                },
            );
            job
        };
        if x_axis {
            let y = painter.round_to_pixel_center(oy);
            painter.hline(rect.x_range(), y, axis);
            let tip = pos2(rect.right() - 1.0, y);
            painter.line_segment([tip, tip + vec2(-5.0, -4.0)], axis);
            painter.line_segment([tip, tip + vec2(-5.0, 4.0)], axis);
            let x = painter.layout_job(italic("x", 12.0));
            painter.galley(pos2(rect.right() - 4.0 - x.size().x, y + 4.0), x, gp.axes);
        }
        if y_axis {
            let x = painter.round_to_pixel_center(ox);
            painter.vline(x, rect.y_range(), axis);
            let tip = pos2(x, rect.top() + 1.0);
            painter.line_segment([tip, tip + vec2(-4.0, 5.0)], axis);
            painter.line_segment([tip, tip + vec2(4.0, 5.0)], axis);
            let y = painter.layout_job(italic("y", 12.0));
            painter.galley(pos2(x - 8.0 - y.size().x, rect.top() + 1.0), y, gp.axes);
        }

        // The numbers at the labeled lines: under the x-axis and left of the y-axis, or
        // along the edge the axis is beyond.
        let label_y = if x_axis {
            oy + 3.0
        } else if oy < rect.top() {
            rect.top() + 3.0
        } else {
            rect.bottom() - 17.0
        };
        let (k0, k1) = (
            (view.x_min / gx.major).ceil() as i64,
            (view.x_max / gx.major).floor() as i64,
        );
        if k1 - k0 < 200 {
            for k in k0..=k1 {
                if k == 0 {
                    continue;
                }
                let x = px(k as f64 * gx.major);
                if x > rect.right() - 16.0 || x < rect.left() + 8.0 {
                    continue;
                }
                let label =
                    painter.layout_job(italic(&tick_label(k as f64 * gx.major, gx.major), 12.0));
                painter.galley(pos2(x - label.size().x / 2.0, label_y), label, gp.axes);
            }
        }
        let (k0, k1) = (
            (view.y_min / gy.major).ceil() as i64,
            (view.y_max / gy.major).floor() as i64,
        );
        if k1 - k0 < 200 {
            for k in k0..=k1 {
                if k == 0 {
                    continue;
                }
                let y = py(k as f64 * gy.major);
                if y < rect.top() + 16.0 || y > rect.bottom() - 8.0 {
                    continue;
                }
                let label =
                    painter.layout_job(italic(&tick_label(k as f64 * gy.major, gy.major), 12.0));
                let x = if y_axis {
                    ox - 4.0 - label.size().x
                } else if ox < rect.left() {
                    rect.left() + 3.0
                } else {
                    rect.right() - 3.0 - label.size().x
                };
                painter.galley(pos2(x, y - label.size().y / 2.0), label, gp.axes);
            }
        }
        if x_axis && y_axis {
            let zero = painter.layout_job(italic("0", 12.0));
            painter.galley(pos2(ox - 4.0 - zero.size().x, oy + 3.0), zero, gp.axes);
        }
    }

    /// The traced value in a tooltip, 15 px right of and 30 px above the point, or left
    /// of it near the right edge (`PositionGraphPopup`).
    fn paint_trace_popup(&self, ui: &Ui, rect: Rect, at: Pos2, text: &str) {
        let p = self.palette;
        let painter = ui.painter();
        let galley = painter.layout_no_wrap(text.to_string(), FontId::proportional(12.0), p.text);
        let size = galley.size() + vec2(16.0, 10.0);
        let x = if at.x + 15.0 + size.x >= rect.right() {
            at.x - 15.0 - size.x
        } else {
            at.x + 15.0
        };
        let y = if at.y - rect.top() >= 30.0 {
            at.y - 30.0
        } else {
            at.y
        };
        let popup = Rect::from_min_size(pos2(x, y), size);
        super::date_view::popup_frame(p, painter, popup, p.popup);
        painter.galley(popup.min + vec2(8.0, 5.0), galley, p.text);
        access::text(ui, popup, Id::new("graph-trace-value"), text, true);
    }

    /// The two button panels over the graph.
    fn draw_graph_buttons(&mut self, ui: &mut Ui, rect: Rect) {
        let p = self.palette;
        let dark = ui.ctx().theme() == egui::Theme::Dark;
        // `GraphControlCommandPanel`: 4 px corners in dark, 8 in light.
        let corner = if dark { 4.0 } else { 8.0 };
        let top = Rect::from_min_size(
            pos2(rect.right() - 8.0 - 108.0, rect.top() + 8.0),
            vec2(108.0, 40.0),
        );
        let painter = ui.painter().clone();
        painter.rect_filled(top, corner, p.graph_panel);
        painter.rect_stroke(
            top,
            corner,
            Stroke::new(1.0, p.graph_panel_border),
            StrokeKind::Inside,
        );
        let cell = |r: Rect, i: f32, vertical: bool| {
            if vertical {
                Rect::from_min_size(r.min + vec2(4.0, 4.0 + i * 36.0), vec2(32.0, 32.0))
            } else {
                Rect::from_min_size(r.min + vec2(4.0 + i * 36.0, 4.0), vec2(32.0, 32.0))
            }
        };

        // Tracing, a toggle.
        let on = self.graphing.tracing;
        let name = if on {
            tr("disableTracingButtonToolTip")
        } else {
            tr("enableTracingButtonToolTip")
        };
        let r = cell(top, 0.0, false);
        let info = access::item(WidgetType::Checkbox, name, on);
        let response =
            access::control(ui, p, r, Id::new(TRACE_ID), true, RADIUS, info).on_hover_text(name);
        let (fill, color) = panel_button_colors(p, &response, on);
        painter.rect_filled(r, RADIUS, fill);
        paint_trace_icon(&painter, r.center(), color);
        if response.clicked() {
            self.graphing.tracing = !on;
            if !on {
                ui.ctx().memory_mut(|m| m.request_focus(Id::new("graph")));
            }
        }

        // Share: the share sheet, with a picture of the graph.
        let r = cell(top, 1.0, false);
        let name = tr("shareButton.AutomationName");
        let response = access::control(
            ui,
            p,
            r,
            Id::new("graph-share"),
            true,
            RADIUS,
            access::button(true, name),
        )
        .on_hover_text(tr("shareButton.ToolTip"));
        let (fill, color) = panel_button_colors(p, &response, false);
        painter.rect_filled(r, RADIUS, fill);
        let icon = painter.layout_job(icon_label(ICON_SHARE, 16.0));
        painter.galley(r.center() - icon.size() / 2.0, icon, color);
        if response.clicked() {
            self.graphing.capture = Capture::Requested(Purpose::Share);
        }

        // Graph options.
        let r = cell(top, 2.0, false);
        let name = tr("graphSettingsButton.AutomationName");
        let response = access::control(
            ui,
            p,
            r,
            Id::new("graph-settings"),
            true,
            RADIUS,
            access::button(true, name),
        )
        .on_hover_text(tr("graphSettingsButton.ToolTip"));
        let open = matches!(self.graphing.popup, Some(Popup::Settings { .. }));
        let (fill, color) = panel_button_colors(p, &response, false);
        painter.rect_filled(r, RADIUS, if open { p.subtle_hover } else { fill });
        paint_graph_icon(&painter, r.center() - vec2(1.0, 1.0), 15.0, color);
        let gear = painter.layout_job(icon_label(ICON_SETTINGS, 9.0));
        painter.rect_filled(
            Rect::from_center_size(r.center() + vec2(5.0, 4.5), vec2(9.0, 9.0)),
            4.5,
            p.graph_panel,
        );
        painter.galley(r.center() + vec2(5.0, 4.5) - gear.size() / 2.0, gear, color);
        if response.clicked() {
            self.graphing.popup = if open {
                None
            } else {
                Some(Popup::Settings {
                    anchor: r,
                    widths_open: false,
                })
            };
            self.graphing.range_texts = Default::default();
        }

        // Zoom in, zoom out (held, they repeat after half a second every 40 ms), and the
        // graph view toggle.
        let bottom = Rect::from_min_size(
            pos2(rect.right() - 8.0 - 40.0, rect.bottom() - 8.0 - 108.0),
            vec2(40.0, 108.0),
        );
        painter.rect_filled(bottom, corner, p.graph_panel);
        painter.rect_stroke(
            bottom,
            corner,
            Stroke::new(1.0, p.graph_panel_border),
            StrokeKind::Inside,
        );
        let now = ui.ctx().input(|i| i.time);
        let size = (rect.width(), rect.height());
        for (i, zoom_in) in [(0.0, true), (1.0, false)] {
            let r = cell(bottom, i, true);
            let (glyph, key, tip) = if zoom_in {
                (
                    ICON_PLUS,
                    "zoomInButton.AutomationName",
                    "zoomInButton.ToolTip",
                )
            } else {
                (
                    ICON_MINUS,
                    "zoomOutButton.AutomationName",
                    "zoomOutButton.ToolTip",
                )
            };
            let id = Id::new("graph-zoom").with(zoom_in);
            let response =
                access::control(ui, p, r, id, true, RADIUS, access::button(true, tr(key)))
                    .on_hover_text(tr(tip));
            let (fill, color) = panel_button_colors(p, &response, false);
            painter.rect_filled(r, RADIUS, fill);
            let icon = painter.layout_job(icon_label(glyph, 14.0));
            painter.galley(r.center() - icon.size() / 2.0, icon, color);
            let down = response.is_pointer_button_down_on();
            if response.drag_started() || (down && self.graphing.zoom_held.is_none()) {
                self.zoom_graph(zoom_in, size);
                self.graphing.zoom_held = Some((zoom_in, now + 0.5));
            } else if down {
                if let Some((which, next)) = self.graphing.zoom_held
                    && which == zoom_in
                    && now >= next
                {
                    self.zoom_graph(zoom_in, size);
                    self.graphing.zoom_held = Some((zoom_in, now + 0.04));
                }
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(40));
            } else if self
                .graphing
                .zoom_held
                .is_some_and(|(which, _)| which == zoom_in)
            {
                self.graphing.zoom_held = None;
            } else if response.clicked() && self.graphing.zoom_held.is_none() {
                // A keyboard press (Space or Enter on the focused button).
                self.zoom_graph(zoom_in, size);
            }
        }
        let r = cell(bottom, 2.0, true);
        let manual = self.graphing.manual;
        let name = tr("graphViewButton.AutomationName");
        let info = access::item(WidgetType::Checkbox, name, manual);
        let response = access::control(ui, p, r, Id::new("graph-view"), true, RADIUS, info)
            .on_hover_text(tr("graphViewButton.ToolTip"));
        let (fill, color) = panel_button_colors(p, &response, false);
        painter.rect_filled(r, RADIUS, fill);
        paint_view_icon(&painter, r.center(), color, !manual);
        if response.clicked() {
            self.toggle_graph_view();
        }
    }

    /// Takes the screenshot of a graph drawn alone, then copies or shares it.
    fn finish_capture(&mut self, ctx: &egui::Context, rect: Rect) {
        match self.graphing.capture {
            Capture::Requested(purpose) => {
                ctx.send_viewport_cmd(ViewportCommand::Screenshot(Default::default()));
                let now = ctx.input(|i| i.time);
                self.graphing.capture = Capture::Waiting(purpose, rect, now);
                ctx.request_repaint();
            }
            Capture::Waiting(purpose, rect, since) => {
                let image = ctx.input(|i| {
                    i.events.iter().find_map(|e| match e {
                        Event::Screenshot { image, .. } => Some(image.clone()),
                        _ => None,
                    })
                });
                if let Some(image) = image {
                    let ppp = ctx.pixels_per_point();
                    let region = image.region(&rect, Some(ppp));
                    self.graphing.capture = Capture::None;
                    match purpose {
                        Purpose::Copy => ctx.copy_image(region),
                        Purpose::Share => self.share_graph(ctx, region),
                    }
                } else if ctx.input(|i| i.time) - since > 2.0 {
                    // No screenshot came: give up rather than hide the controls for good.
                    self.graphing.capture = Capture::None;
                }
                ctx.request_repaint();
            }
            Capture::None => {}
        }
    }

    /// Opens the share sheet with the picture of the graph, the equations in their colors
    /// and the variables (`OnDataRequested`), or the dialog that says it can't.
    fn share_graph(&mut self, ctx: &egui::Context, image: ColorImage) {
        let gp = self
            .graphing
            .graph_palette(ctx.theme() == egui::Theme::Dark);
        let equations: Vec<(String, Color32)> = self
            .graphing
            .rows
            .iter()
            .map(|r| (r.submitted.clone(), gp.equations[r.color]))
            .collect();
        let variables: Vec<(char, String)> = self
            .graphing
            .variables
            .iter()
            .map(|v| (v.name, numbers().localize_plain(&format!("{}", v.value))))
            .collect();
        let content = share::Content {
            title: "CalcCraft".to_string(),
            html: share::html(&equations, &variables),
            image,
        };
        if !share::show(self.title_bar.hwnd(), content) {
            self.graphing.popup = Some(Popup::ShareError);
            ctx.memory_mut(|m| m.request_focus(Id::new("share-error-ok")));
        }
    }
}

/// A view range end, for the graph's name: like `Double.ToString()`.
fn g(v: f64) -> String {
    numbers().localize_plain(&super::graphing_view::g6(v))
}

/// The fill and icon color of a button in the panels; `on` for the checked tracing
/// button (the accent).
fn panel_button_colors(p: &Palette, response: &egui::Response, on: bool) -> (Color32, Color32) {
    let pressed = response.is_pointer_button_down_on();
    let hovered = response.hovered();
    if on {
        let fill = if pressed {
            p.accent_pressed
        } else if hovered {
            p.accent_hover
        } else {
            p.accent
        };
        return (fill, p.text_on_accent);
    }
    let fill = if pressed {
        p.subtle_pressed
    } else if hovered {
        p.subtle_hover
    } else {
        Color32::TRANSPARENT
    };
    (fill, p.graph_panel_text)
}

/// A line of an equation in its style. Dashes and dots scale with the line, like
/// Direct2D's dash styles.
fn paint_line(painter: &Painter, points: Vec<Pos2>, stroke: Stroke, style: LineStyle) {
    if points.len() < 2 {
        return;
    }
    let w = stroke.width;
    match style {
        LineStyle::Solid => {
            painter.add(Shape::line(points, stroke));
        }
        LineStyle::Dash => {
            painter.extend(Shape::dashed_line(&points, stroke, 2.5 * w, 1.2 * w));
        }
        LineStyle::Dot => {
            painter.extend(Shape::dotted_line(&points, stroke.color, 2.2 * w, w / 2.0));
        }
    }
}

/// The trace pointer: Windows' arrow (`M0 0 l1371 1371 H538 l-538 538 Z` in 18 × 18 px),
/// white with a black outline and a soft shadow, its tip at `tip`.
fn paint_trace_pointer(painter: &Painter, tip: Pos2) {
    let s = 18.0 / 1909.0;
    let shape = |offset: egui::Vec2| {
        vec![
            tip + offset,
            tip + offset + vec2(1371.0 * s, 1371.0 * s),
            tip + offset + vec2(538.0 * s, 1371.0 * s),
            tip + offset + vec2(0.0, 1909.0 * s),
        ]
    };
    painter.add(Shape::convex_polygon(
        shape(vec2(2.0, 2.0)),
        Color32::from_black_alpha(60),
        Stroke::NONE,
    ));
    painter.add(Shape::closed_line(
        shape(vec2(0.0, 0.0)),
        Stroke::new(1.0, Color32::BLACK),
    ));
    let mut fill = shape(vec2(0.0, 0.0));
    // The arrow isn't convex at its notch: fill it as two triangles.
    let notch = fill.remove(2);
    painter.add(Shape::convex_polygon(
        vec![fill[0], fill[1], notch],
        Color32::WHITE,
        Stroke::NONE,
    ));
    painter.add(Shape::convex_polygon(
        vec![fill[0], notch, fill[2]],
        Color32::WHITE,
        Stroke::NONE,
    ));
    painter.add(Shape::closed_line(
        shape(vec2(0.0, 0.0)),
        Stroke::new(1.0, Color32::BLACK),
    ));
}

/// The tracing button's icon: an arrow pointer outline, like Windows'.
fn paint_trace_icon(painter: &Painter, center: Pos2, color: Color32) {
    let stroke = Stroke::new(1.2, color);
    let points = vec![
        center + vec2(-4.5, -6.5),
        center + vec2(6.0, 1.5),
        center + vec2(0.5, 1.5),
        center + vec2(-4.5, 7.0),
    ];
    painter.add(Shape::closed_line(points, stroke));
}

/// The graph view button's icon: four corners around a ring, with a dot in it while
/// the view fits itself (`E45E`, and `E45D` over it).
fn paint_view_icon(painter: &Painter, center: Pos2, color: Color32, automatic: bool) {
    let stroke = Stroke::new(1.2, color);
    let (r, arm) = (6.5, 3.0);
    for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        let corner = center + vec2(sx * r, sy * r);
        painter.line_segment([corner, corner - vec2(sx * arm, 0.0)], stroke);
        painter.line_segment([corner, corner - vec2(0.0, sy * arm)], stroke);
    }
    painter.circle_stroke(center, 3.0, stroke);
    if automatic {
        painter.circle_filled(center, 1.6, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_labels() {
        assert_eq!(tick_label(-20.0, 10.0), "−20");
        assert_eq!(tick_label(0.5, 0.5), "0.5");
        assert_eq!(tick_label(1.0, 0.5), "1.0");
        assert_eq!(tick_label(-0.0, 1.0), "0");
    }

    #[test]
    fn traced_values() {
        // Windows shows (3.82, 14.592400000000000).
        assert_eq!(trace_text(3.82, 3.82 * 3.82), "(3.82, 14.592400000000000)");
    }

    #[test]
    fn traced_zeros_have_no_sign() {
        // Just left of the origin, y = x is traced at x = −0 (−0.3 steps round to −0) with
        // y = −0: Windows shows (0, 0.000000000000000). A y that rounds to 0 loses its sign
        // too, while other negatives keep theirs.
        assert_eq!(trace_text(-0.0, -0.0), "(0, 0.000000000000000)");
        assert_eq!(trace_text(-0.0, -4e-16), "(0, 0.000000000000000)");
        assert_eq!(trace_text(-0.01, -0.01), "(-0.01, -0.010000000000000)");
    }

    #[test]
    fn reparsed_equations_are_plotted_again() {
        use crate::app::graphing_view::GraphingState;
        use crate::graphing::Syntax;
        let mut g = GraphingState::new();
        // A decimal comma is an error in en-US, a number in de-DE.
        g.rows[0].text = "y=1,5x".into();
        g.submit(0, Syntax::default());
        assert!(g.rows[0].graphable().is_none());
        g.cache = Some(PlotCache {
            key: CacheKey {
                view: View::DEFAULT,
                size: (100.0, 100.0),
                angle: Angle::Radians,
                rows: vec![(g.rows[0].id, g.rows[0].submitted.clone(), true)],
                values: Vec::new(),
            },
            plots: Vec::new(),
        });
        g.reparse(Syntax::for_decimal(','));
        assert!(g.rows[0].graphable().is_some());
        // Same text, so the cache's key would match: it has to go.
        assert!(g.cache.is_none());
    }

    #[test]
    fn wheel() {
        let events = vec![Event::MouseWheel {
            unit: MouseWheelUnit::Line,
            delta: vec2(0.0, 1.0),
            modifiers: Default::default(),
            phase: egui::TouchPhase::Move,
        }];
        assert_eq!(wheel_notches(&events), vec![1.0]);
    }
}
