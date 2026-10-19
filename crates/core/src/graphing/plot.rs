//! Plots: the view, the grid, the lines and regions of the equations, the view Windows
//! calls "automatic best fit", and the curve point the tracer snaps to.
//!
//! Pixel coordinates are relative to the graph's top-left corner, with y down.

use super::analysis;
use super::expr::Env;
use super::{Equation, Relation, Shape};

/// The ranges shown, in graph units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub x_min: f64,
    pub x_max: f64,
    pub y_min: f64,
    pub y_max: f64,
}

impl View {
    /// An empty graph shows −10 to 10 on both axes, stretched to the graph's size.
    pub const DEFAULT: View = View {
        x_min: -10.0,
        x_max: 10.0,
        y_min: -10.0,
        y_max: 10.0,
    };

    pub fn width(&self) -> f64 {
        self.x_max - self.x_min
    }

    pub fn height(&self) -> f64 {
        self.y_max - self.y_min
    }

    /// Whether the ranges can be drawn: finite, not too wide, wider than 1e-10 of their
    /// ends (narrower ones lose their precision in doubles) and than `MIN_SPAN`. Around
    /// the origin no relative limit stops zooming in, which used to go on into the
    /// subnormal doubles, where the grid's powers of ten underflow.
    pub fn is_valid(&self) -> bool {
        let ok = |min: f64, max: f64| {
            min.is_finite()
                && max.is_finite()
                && max > min
                && (max - min) > 1e-10 * min.abs().max(max.abs())
                && (max - min) > MIN_SPAN
                && (max - min) < 1e100
        };
        ok(self.x_min, self.x_max) && ok(self.y_min, self.y_max)
    }

    /// Graph point → pixel, for a graph of `w` × `h` pixels.
    pub fn px_of(&self, x: f64, y: f64, w: f32, h: f32) -> (f64, f64) {
        (
            (x - self.x_min) / self.width() * w as f64,
            (self.y_max - y) / self.height() * h as f64,
        )
    }

    /// Pixel → graph point.
    pub fn point_at(&self, px: f64, py: f64, w: f32, h: f32) -> (f64, f64) {
        (
            self.x_min + px / w as f64 * self.width(),
            self.y_max - py / h as f64 * self.height(),
        )
    }

    /// The view zoomed by `scale` (below 1 zooms in) around the pixel (`px`, `py`),
    /// which stays where it is (Windows' `ScaleRange`).
    pub fn zoomed(&self, px: f64, py: f64, w: f32, h: f32, scale: f64) -> View {
        let (cx, cy) = self.point_at(px, py, w, h);
        let zoomed = View {
            x_min: cx - (cx - self.x_min) * scale,
            x_max: cx + (self.x_max - cx) * scale,
            y_min: cy - (cy - self.y_min) * scale,
            y_max: cy + (self.y_max - cy) * scale,
        };
        if zoomed.is_valid() { zoomed } else { *self }
    }

    /// The view moved so that the graph follows a pointer that moved by (`dx`, `dy`)
    /// pixels.
    pub fn dragged(&self, dx: f64, dy: f64, w: f32, h: f32) -> View {
        let ux = dx / w as f64 * self.width();
        let uy = dy / h as f64 * self.height();
        View {
            x_min: self.x_min - ux,
            x_max: self.x_max - ux,
            y_min: self.y_min + uy,
            y_max: self.y_max + uy,
        }
    }

    /// The view made proportional (one unit as long on both axes) by widening the
    /// shorter range around its center.
    pub fn proportional(&self, w: f32, h: f32) -> View {
        let (w, h) = (w.max(1.0) as f64, h.max(1.0) as f64);
        let x_scale = self.width() / w;
        let y_scale = self.height() / h;
        let mut view = *self;
        if x_scale > y_scale {
            let center = (self.y_min + self.y_max) / 2.0;
            let half = x_scale * h / 2.0;
            view.y_min = center - half;
            view.y_max = center + half;
        } else {
            let center = (self.x_min + self.x_max) / 2.0;
            let half = y_scale * w / 2.0;
            view.x_min = center - half;
            view.x_max = center + half;
        }
        view
    }

    /// The step the tracer rounds x to: 10^(⌊log₁₀(x range)⌋ − 3) (`RenderMain::GetPrecision`).
    pub fn precision(&self) -> f64 {
        10f64.powf(self.width().log10().floor() - 3.0)
    }
}

/// The narrowest range a view can have: 10⁸ times the smallest normal double (about
/// 2.2e-300), so that a pixel's share of it, the grid's steps and the tracer's precision
/// all stay normal numbers too.
const MIN_SPAN: f64 = f64::MIN_POSITIVE * 1e8;

/// The distance between the labeled grid lines and between all grid lines.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridStep {
    pub major: f64,
    pub minor: f64,
}

/// The grid of one axis: at most 7 labeled intervals (fewer when they would be closer
/// than 48 px), at 1, 2 or 5 times a power of ten, with lines between them every fifth
/// (or fourth, for a 2) of that. Windows labels 10s and draws 2s for x from −27 to 27.
pub fn grid_step(span: f64, pixels: f32) -> GridStep {
    let most = (pixels as f64 / 48.0).clamp(2.0, 7.0);
    // Valid views (`View::is_valid`) are well inside these bounds. Outside them (0,
    // subnormal, infinite or NaN spans) the powers of ten would be 0 or infinite, and
    // the search below would never end.
    let span = if span > 1e-300 {
        span.min(1e300)
    } else {
        1e-300
    };
    let mut power = 10f64.powf((span / most).log10().floor() - 1.0);
    loop {
        for (m, parts) in [(1.0, 5.0), (2.0, 4.0), (5.0, 5.0)] {
            let major = m * power;
            if span / major <= most {
                return GridStep {
                    major,
                    minor: major / parts,
                };
            }
        }
        power *= 10.0;
    }
}

/// How an equation's line is drawn (Windows' `EquationLineStyle`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LineStyle {
    #[default]
    Solid,
    Dash,
    Dot,
}

/// What to draw for an equation.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plot {
    /// Polylines, in pixels.
    pub lines: Vec<Vec<[f32; 2]>>,
    /// The points of the inequality's dot pattern that lie in its region.
    pub dots: Vec<[f32; 2]>,
}

/// How far a plotted point may lie outside the graph before it is clamped: keeps f32
/// coordinates sane near asymptotes.
const FAR: f64 = 20.0;

/// Plots `equation` in `view` on a `w` × `h` pixel graph.
pub fn plot(equation: &Equation, env: &Env, view: &View, w: f32, h: f32) -> Plot {
    let mut out = Plot::default();
    if !view.is_valid() || w < 1.0 || h < 1.0 {
        return out;
    }
    match &equation.shape {
        Shape::Function(f) => {
            let curve = |x: f64| f.eval(x, 0.0, env);
            let map = |x: f64, y: f64| view.px_of(x, y, w, h);
            out.lines = sample(&curve, view.x_min, view.x_max, w, &map, w, h);
        }
        Shape::Inverse(g) => {
            let curve = |y: f64| g.eval(0.0, y, env);
            let map = |y: f64, x: f64| view.px_of(x, y, w, h);
            out.lines = sample(&curve, view.y_min, view.y_max, h, &map, w, h);
        }
        Shape::Implicit(f) => {
            let field = |x: f64, y: f64| f.eval(x, y, env);
            out.lines = contour(&field, view, w, h);
        }
    }
    if equation.is_inequality() {
        out.dots = region_dots(equation, env, view, w, h);
    }
    out
}

/// Samples a curve t ↦ (t, f(t)) from `t0` to `t1`, about one sample per pixel along
/// `along` pixels, more where it bends, and breaks it where f isn't defined or jumps
/// (at an asymptote of tan x or 1/x). `map` gives the pixel of (t, f(t)).
fn sample(
    f: &dyn Fn(f64) -> f64,
    t0: f64,
    t1: f64,
    along: f32,
    map: &dyn Fn(f64, f64) -> (f64, f64),
    w: f32,
    h: f32,
) -> Vec<Vec<[f32; 2]>> {
    let n = (along as usize).clamp(2, 4000);
    let mut lines = Vec::new();
    let mut line: Vec<[f32; 2]> = Vec::new();
    let mut sampler = Sampler {
        f,
        map,
        w: w as f64,
        h: h as f64,
    };
    let mut prev = (t0, f(t0));
    if prev.1.is_finite() {
        line.push(sampler.px(prev.0, prev.1));
    }
    for i in 1..=n {
        let t = t0 + (t1 - t0) * i as f64 / n as f64;
        let next = (t, f(t));
        sampler.segment(prev, next, 10, &mut line, &mut lines);
        prev = next;
    }
    if line.len() > 1 {
        lines.push(line);
    }
    lines
}

struct Sampler<'a> {
    f: &'a dyn Fn(f64) -> f64,
    map: &'a dyn Fn(f64, f64) -> (f64, f64),
    w: f64,
    h: f64,
}

impl Sampler<'_> {
    fn px(&self, t: f64, v: f64) -> [f32; 2] {
        let (x, y) = (self.map)(t, v);
        let clamp = |p: f64, size: f64| p.clamp(-FAR * size, (FAR + 1.0) * size) as f32;
        [clamp(x, self.w), clamp(y, self.h)]
    }

    fn outside(&self, p: [f32; 2]) -> bool {
        let (x, y) = (p[0] as f64, p[1] as f64);
        x < -self.w || x > 2.0 * self.w || y < -self.h || y > 2.0 * self.h
    }

    /// Adds the curve from `a` (already in `line` if defined) to `b`.
    fn segment(
        &mut self,
        a: (f64, f64),
        b: (f64, f64),
        depth: u32,
        line: &mut Vec<[f32; 2]>,
        lines: &mut Vec<Vec<[f32; 2]>>,
    ) {
        let (fa, fb) = (a.1.is_finite(), b.1.is_finite());
        if !fa && !fb {
            return;
        }
        if fa != fb {
            // The curve starts or ends between them: find where (√x at 0).
            let (mut inside, mut out) = if fa { (a, b) } else { (b, a) };
            for _ in 0..40 {
                let t = (inside.0 + out.0) / 2.0;
                let v = (self.f)(t);
                if v.is_finite() {
                    inside = (t, v);
                } else {
                    out = (t, v);
                }
            }
            if fa {
                line.push(self.px(inside.0, inside.1));
                flush(line, lines);
            } else {
                flush(line, lines);
                line.push(self.px(inside.0, inside.1));
                line.push(self.px(b.0, b.1));
            }
            return;
        }
        let (pa, pb) = (self.px(a.0, a.1), self.px(b.0, b.1));
        let gap = (pa[0] - pb[0]).abs() + (pa[1] - pb[1]).abs();
        if gap <= 2.0 || (self.outside(pa) && self.outside(pb) && same_side(pa, pb, self.w, self.h))
        {
            line.push(pb);
            return;
        }
        if depth == 0 {
            // Still a big step: a jump or an asymptote, unless it's merely steep.
            if gap > self.h as f32 / 4.0 {
                flush(line, lines);
            }
            line.push(pb);
            return;
        }
        let t = (a.0 + b.0) / 2.0;
        let mid = (t, (self.f)(t));
        self.segment(a, mid, depth - 1, line, lines);
        self.segment(mid, b, depth - 1, line, lines);
    }
}

/// Whether both points lie beyond the same edge, so the line between them can't cross
/// the graph.
fn same_side(a: [f32; 2], b: [f32; 2], w: f64, h: f64) -> bool {
    let (w, h) = (w as f32, h as f32);
    (a[0] < 0.0 && b[0] < 0.0)
        || (a[0] > w && b[0] > w)
        || (a[1] < 0.0 && b[1] < 0.0)
        || (a[1] > h && b[1] > h)
}

fn flush(line: &mut Vec<[f32; 2]>, lines: &mut Vec<Vec<[f32; 2]>>) {
    if line.len() > 1 {
        lines.push(std::mem::take(line));
    } else {
        line.clear();
    }
}

/// The size of the cells implicit curves are traced in, in pixels.
const CELL: f32 = 4.0;

/// The curve F(x, y) = 0 by marching squares, joined into polylines.
fn contour(f: &dyn Fn(f64, f64) -> f64, view: &View, w: f32, h: f32) -> Vec<Vec<[f32; 2]>> {
    let nx = ((w / CELL).ceil() as usize).max(1);
    let ny = ((h / CELL).ceil() as usize).max(1);
    let gx = |i: usize| view.x_min + view.width() * i as f64 / nx as f64;
    let gy = |j: usize| view.y_max - view.height() * j as f64 / ny as f64;
    let values: Vec<f64> = (0..=ny)
        .flat_map(|j| (0..=nx).map(move |i| (i, j)))
        .map(|(i, j)| f(gx(i), gy(j)))
        .collect();
    let v = |i: usize, j: usize| values[j * (nx + 1) + i];

    // A crossing on the edge from node (i, j) to the node right of it (horizontal) or
    // below it, or `None` if the sign change there is a pole, not a root.
    let crossing = |i: usize, j: usize, horizontal: bool| -> Option<[f32; 2]> {
        let (i2, j2) = if horizontal { (i + 1, j) } else { (i, j + 1) };
        let (a, b) = (v(i, j), v(i2, j2));
        let (xa, ya, xb, yb) = (gx(i), gy(j), gx(i2), gy(j2));
        let at = |s: f64| f(xa + (xb - xa) * s, ya + (yb - ya) * s);
        // The linear guess, then a few Illinois steps on the real function: a curve as
        // steep as tan x near its asymptotes needs them.
        let (mut lo, mut hi, mut flo, mut fhi) = (0.0, 1.0, a, b);
        let mut s = a / (a - b);
        let mut best = s;
        for step in 0..5 {
            let mid = at(s);
            // Near a root the values shrink toward it. A value beyond the corner of its
            // sign means a pole: the first guess is a pole's crossing, a later one has
            // wandered toward a pole on the same edge.
            let corner = if (mid > 0.0) == (a > 0.0) { a } else { b };
            if !mid.is_finite() || mid.abs() > corner.abs() {
                if step == 0 {
                    return None;
                }
                s = best;
                break;
            }
            best = s;
            if mid == 0.0 {
                break;
            }
            if (mid > 0.0) == (flo > 0.0) {
                lo = s;
                flo = mid;
                fhi /= 2.0;
            } else {
                hi = s;
                fhi = mid;
                flo /= 2.0;
            }
            s = lo + (hi - lo) * flo / (flo - fhi);
        }
        let (x, y) = (xa + (xb - xa) * s, ya + (yb - ya) * s);
        let (px, py) = view.px_of(x, y, w, h);
        Some([px as f32, py as f32])
    };

    // Each segment joins two edges, named by their node and direction.
    type Edge = (usize, usize, bool);
    let mut segments: Vec<(Edge, [f32; 2], Edge, [f32; 2])> = Vec::new();
    for j in 0..ny {
        for i in 0..nx {
            let corners = [v(i, j), v(i + 1, j), v(i + 1, j + 1), v(i, j + 1)];
            if corners.iter().any(|c| !c.is_finite()) {
                continue;
            }
            let positive = corners.map(|c| c > 0.0);
            // The cell's edges: top, right, bottom, left.
            let edges: [Edge; 4] = [
                (i, j, true),
                (i + 1, j, false),
                (i, j + 1, true),
                (i, j, false),
            ];
            let crosses: Vec<usize> = (0..4)
                .filter(|&k| positive[k] != positive[(k + 1) % 4])
                .collect();
            let pairs: Vec<(usize, usize)> = match crosses.len() {
                2 => vec![(crosses[0], crosses[1])],
                4 => {
                    // A saddle: the center decides which corners are joined.
                    let center = f((gx(i) + gx(i + 1)) / 2.0, (gy(j) + gy(j + 1)) / 2.0);
                    if (center > 0.0) == positive[0] {
                        vec![(0, 1), (2, 3)]
                    } else {
                        vec![(3, 0), (1, 2)]
                    }
                }
                _ => vec![],
            };
            for (k1, k2) in pairs {
                let (e1, e2) = (edges[k1], edges[k2]);
                if let (Some(p1), Some(p2)) =
                    (crossing(e1.0, e1.1, e1.2), crossing(e2.0, e2.1, e2.2))
                {
                    segments.push((e1, p1, e2, p2));
                }
            }
        }
    }
    join(segments)
}

/// Joins segments that share an edge into polylines.
fn join<E: std::hash::Hash + Eq + Copy>(
    segments: Vec<(E, [f32; 2], E, [f32; 2])>,
) -> Vec<Vec<[f32; 2]>> {
    use std::collections::HashMap;
    let mut at: HashMap<E, Vec<usize>> = HashMap::new();
    for (k, (e1, _, e2, _)) in segments.iter().enumerate() {
        at.entry(*e1).or_default().push(k);
        at.entry(*e2).or_default().push(k);
    }
    let mut used = vec![false; segments.len()];
    let mut lines = Vec::new();
    for start in 0..segments.len() {
        if used[start] {
            continue;
        }
        used[start] = true;
        let (e1, p1, e2, p2) = segments[start];
        let mut line = std::collections::VecDeque::from([p1, p2]);
        // Extend forward from e2, then backward from e1.
        for (mut edge, forward) in [(e2, true), (e1, false)] {
            while let Some(&next) = at
                .get(&edge)
                .and_then(|list| list.iter().find(|&&k| !used[k]))
            {
                used[next] = true;
                let (a, pa, b, pb) = segments[next];
                let (point, other) = if a == edge { (pb, b) } else { (pa, a) };
                if forward {
                    line.push_back(point);
                } else {
                    line.push_front(point);
                }
                edge = other;
            }
        }
        lines.push(line.into_iter().collect());
    }
    lines
}

/// The dots that mark an inequality's region: Windows fills it with small crosses on a
/// 100 × 100 lattice over the graph.
fn region_dots(equation: &Equation, env: &Env, view: &View, w: f32, h: f32) -> Vec<[f32; 2]> {
    const N: usize = 100;
    let relation = equation.relation;
    let pos = |k: usize, size: f32| (k as f32 + 0.5) * size / N as f32;
    let mut dots = Vec::new();
    let mut add = |i: usize, j: usize| dots.push([pos(i, w), pos(j, h)]);
    let point = |i: usize, j: usize| view.point_at(pos(i, w) as f64, pos(j, h) as f64, w, h);
    match &equation.shape {
        Shape::Function(f) => {
            for i in 0..N {
                let fx = f.eval(point(i, 0).0, 0.0, env);
                if !fx.is_finite() {
                    continue;
                }
                for j in 0..N {
                    if relation.holds(point(i, j).1, fx) {
                        add(i, j);
                    }
                }
            }
        }
        Shape::Inverse(g) => {
            for j in 0..N {
                let gy = g.eval(0.0, point(0, j).1, env);
                if !gy.is_finite() {
                    continue;
                }
                for i in 0..N {
                    if relation.holds(point(i, j).0, gy) {
                        add(i, j);
                    }
                }
            }
        }
        Shape::Implicit(f) => {
            for j in 0..N {
                for i in 0..N {
                    let (x, y) = point(i, j);
                    if relation.holds(f.eval(x, y, env), 0.0) {
                        add(i, j);
                    }
                }
            }
        }
    }
    dots
}

/// Whether a strict inequality's line is drawn dashed (its points aren't in the region).
pub fn is_dashed(relation: Relation) -> bool {
    matches!(relation, Relation::Less | Relation::Greater)
}

/// A point the tracer snapped to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TracePoint {
    /// The index of the equation in the list passed to `trace`.
    pub equation: usize,
    /// x rounded to the view's precision, ready to be shown.
    pub x: f64,
    pub y: f64,
    /// Where it is, in pixels.
    pub px: f32,
    pub py: f32,
}

/// How close to a line the pointer has to be for the tracer to show its point.
pub const TRACE_RADIUS: f32 = 20.0;

/// The point of the plotted lines nearest to the pixel (`px`, `py`), within
/// `TRACE_RADIUS`. On a function, x is rounded to the view's precision and y is the
/// function's value there, which is what Windows shows.
pub fn trace(
    equations: &[(&Equation, &Plot)],
    env: &Env,
    view: &View,
    w: f32,
    h: f32,
    px: f32,
    py: f32,
) -> Option<TracePoint> {
    let mut best: Option<(f32, usize, [f32; 2])> = None;
    for (k, (_, plot)) in equations.iter().enumerate() {
        for line in &plot.lines {
            for pair in line.windows(2) {
                let p = nearest_on_segment(pair[0], pair[1], [px, py]);
                let d = ((p[0] - px).powi(2) + (p[1] - py).powi(2)).sqrt();
                if d <= TRACE_RADIUS && best.is_none_or(|(bd, _, _)| d < bd) {
                    best = Some((d, k, p));
                }
            }
        }
    }
    let (_, k, p) = best?;
    let (x, y) = view.point_at(p[0] as f64, p[1] as f64, w, h);
    let x = rounded(x, view.precision());
    let y = match &equations[k].0.shape {
        Shape::Function(f) => f.eval(x, 0.0, env),
        _ => y,
    };
    if !y.is_finite() {
        return None;
    }
    let (sx, sy) = view.px_of(x, y, w, h);
    Some(TracePoint {
        equation: k,
        x,
        y,
        px: sx as f32,
        py: sy as f32,
    })
}

/// x rounded to the view's precision so that it prints as short as it is: 382 / 100 is
/// the double nearest 3.82, 382 × 0.01 isn't. The tracer shows f at this x, so y has to
/// be computed from it and not from a neighbor that prints the same.
fn rounded(x: f64, precision: f64) -> f64 {
    let steps = (x / precision).round();
    if precision < 1.0 {
        steps / (1.0 / precision).round()
    } else {
        steps * precision
    }
}

fn nearest_on_segment(a: [f32; 2], b: [f32; 2], p: [f32; 2]) -> [f32; 2] {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len = dx * dx + dy * dy;
    if len == 0.0 {
        return a;
    }
    let t = (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len).clamp(0.0, 1.0);
    [a[0] + t * dx, a[1] + t * dy]
}

/// The view Windows calls "automatic best fit": the region around the curves' key
/// points (zeros, extrema, inflection points, the y-intercept; near the origin for
/// periodic curves), with the values the curves take there, a tenth more where a curve
/// turns, and proportional axes.
///
/// Windows' engine picks this view with its own rules, which aren't public; these give
/// the same view for x² (x from −27.2 to 27.2 and y from −3.025 to 30.25 in a 1.64:1
/// graph) and similar ones elsewhere.
pub fn best_fit(equations: &[(&Equation, Env)], w: f32, h: f32) -> View {
    // x = g(y) is y = g(x) on its side. Without functions y = f(x) to fit as well, it gets
    // that one's view on its side: x = y² the room y = x² has under its vertex.
    let has = |shape: fn(&Shape) -> bool| equations.iter().any(|(e, _)| shape(&e.shape));
    if has(|s| matches!(s, Shape::Inverse(_))) && !has(|s| matches!(s, Shape::Function(_))) {
        let mirrored: Vec<Equation> = equations.iter().map(|(e, _)| mirror(e)).collect();
        let pairs: Vec<(&Equation, Env)> = mirrored
            .iter()
            .zip(equations)
            .map(|(e, (_, env))| (e, *env))
            .collect();
        let view = fit(&pairs, h, w);
        return View {
            x_min: view.y_min,
            x_max: view.y_max,
            y_min: view.x_min,
            y_max: view.x_max,
        };
    }
    fit(equations, w, h)
}

/// `equation` with x and y swapped: x = g(y) becomes y = g(x).
fn mirror(equation: &Equation) -> Equation {
    let shape = match &equation.shape {
        Shape::Function(f) => Shape::Inverse(f.swap_xy()),
        Shape::Inverse(g) => Shape::Function(g.swap_xy()),
        Shape::Implicit(f) => Shape::Implicit(f.swap_xy()),
    };
    Equation {
        shape,
        relation: equation.relation,
        variables: equation.variables.clone(),
    }
}

/// The best fit of `best_fit`, with the values of x = g(y) taken as they are.
fn fit(equations: &[(&Equation, Env)], w: f32, h: f32) -> View {
    let mut x_range: Option<(f64, f64)> = None;
    let mut y_range: Option<Extent> = None;
    let widen = |range: &mut Option<(f64, f64)>, lo: f64, hi: f64| {
        *range = Some(match *range {
            None => (lo, hi),
            Some((a, b)) => (a.min(lo), b.max(hi)),
        });
    };
    for (equation, env) in equations {
        match &equation.shape {
            Shape::Function(f) | Shape::Inverse(f) => {
                let swap = matches!(equation.shape, Shape::Inverse(_));
                let curve: Box<dyn Fn(f64) -> f64> = if swap {
                    Box::new(move |t| f.eval(0.0, t, env))
                } else {
                    Box::new(move |t| f.eval(t, 0.0, env))
                };
                let derivative = if swap {
                    f.swap_xy().derivative(env.angle)
                } else {
                    f.derivative(env.angle)
                };
                let (lo, hi, focus) = interest(&*curve, derivative.as_ref(), env);
                let extent = Extent::of(&*curve, lo, hi, &focus);
                if swap {
                    // x = g(y): the interval is one of y, and the values are x.
                    widen(&mut x_range, extent.lo, extent.hi);
                    y_range = Some(Extent::union(y_range, Extent::plain(lo, hi)));
                } else {
                    widen(&mut x_range, lo, hi);
                    y_range = Some(Extent::union(y_range, extent));
                }
            }
            Shape::Implicit(f) => {
                if let Some((x0, x1, y0, y1)) = implicit_box(&|x, y| f.eval(x, y, env)) {
                    let pad = |a: f64, b: f64| {
                        let m = ((b - a) * 0.1).max(0.5);
                        (a - m, b + m)
                    };
                    let (x0, x1) = pad(x0, x1);
                    let (y0, y1) = pad(y0, y1);
                    widen(&mut x_range, x0, x1);
                    y_range = Some(Extent::union(y_range, Extent::plain(y0, y1)));
                }
            }
        }
    }
    let (Some((x0, x1)), Some(extent)) = (x_range, y_range) else {
        return View::DEFAULT;
    };
    let aspect = (w.max(1.0) / h.max(1.0)) as f64;
    let (y0, y1) = extent.fitted((x1 - x0) / aspect);
    let view = View {
        x_min: x0,
        x_max: x1,
        y_min: y0,
        y_max: y1,
    }
    .proportional(w, h);
    if view.is_valid() { view } else { View::DEFAULT }
}

/// The values a curve takes on an interval, and whether its lowest and highest values
/// are turning points (inside the interval) or where it leaves it.
#[derive(Clone, Copy, Debug)]
struct Extent {
    lo: f64,
    hi: f64,
    lo_turns: bool,
    hi_turns: bool,
    /// The values of interest (at the key points), which a cropped range keeps.
    focus: (f64, f64),
}

impl Extent {
    fn plain(lo: f64, hi: f64) -> Extent {
        Extent {
            lo,
            hi,
            lo_turns: false,
            hi_turns: false,
            focus: (lo, hi),
        }
    }

    /// The values of `f` from `a` to `b`; `focus` are its values at the key points.
    fn of(f: &dyn Fn(f64) -> f64, a: f64, b: f64, focus: &[f64]) -> Extent {
        const N: usize = 2000;
        let mut values: Vec<(f64, usize)> = (0..=N)
            .map(|i| (f(a + (b - a) * i as f64 / N as f64), i))
            .filter(|(v, _)| v.is_finite())
            .collect();
        if values.is_empty() {
            return Extent::plain(-1.0, 1.0);
        }
        values.sort_by(|p, q| p.0.total_cmp(&q.0));
        let quantile = |q: f64| values[((values.len() - 1) as f64 * q) as usize].0;
        let (mut lo, mut hi) = (values[0], values[values.len() - 1]);
        // Values near an asymptote would squash the rest: keep the middle 90 % then.
        let (q05, q95) = (quantile(0.05), quantile(0.95));
        if (hi.0 - lo.0) > 10.0 * (q95 - q05).max(1e-9) {
            let trimmed: Vec<_> = values
                .iter()
                .filter(|(v, _)| *v >= q05 - (q95 - q05) && *v <= q95 + (q95 - q05))
                .collect();
            lo = *trimmed[0];
            hi = *trimmed[trimmed.len() - 1];
        }
        let inside = |i: usize| i != 0 && i != N;
        let focus = focus
            .iter()
            .filter(|v| v.is_finite())
            .fold(None, |range: Option<(f64, f64)>, &v| {
                Some(range.map_or((v, v), |(a, b)| (a.min(v), b.max(v))))
            })
            .unwrap_or_else(|| {
                let mid = values[values.len() / 2].0;
                (mid, mid)
            });
        Extent {
            lo: lo.0,
            hi: hi.0,
            lo_turns: inside(lo.1),
            hi_turns: inside(hi.1),
            focus,
        }
    }

    fn union(a: Option<Extent>, b: Extent) -> Extent {
        let Some(a) = a else { return b };
        let lo = if a.lo <= b.lo {
            (a.lo, a.lo_turns)
        } else {
            (b.lo, b.lo_turns)
        };
        let hi = if a.hi >= b.hi {
            (a.hi, a.hi_turns)
        } else {
            (b.hi, b.hi_turns)
        };
        Extent {
            lo: lo.0,
            hi: hi.0,
            lo_turns: lo.1,
            hi_turns: hi.1,
            focus: (a.focus.0.min(b.focus.0), a.focus.1.max(b.focus.1)),
        }
    }

    /// The y range: the extent with a tenth more at a turning point, at most five
    /// times as high as the x range would be at equal scales (`unit_height`), keeping
    /// the values of interest and the x-axis in view when it has to be cropped.
    fn fitted(&self, unit_height: f64) -> (f64, f64) {
        let mut span = self.hi - self.lo;
        if span < 1e-9 * self.hi.abs().max(1.0) {
            // A constant: show it and the x-axis.
            let (lo, hi) = (self.lo.min(0.0), self.hi.max(0.0));
            let pad = ((hi - lo) * 0.1).max(1.0);
            return (lo - pad, hi + pad);
        }
        let margin = span * 0.1;
        let lo = self.lo - if self.lo_turns { margin } else { 0.0 };
        let hi = self.hi + if self.hi_turns { margin } else { 0.0 };
        span = hi - lo;
        let most = 5.0 * unit_height;
        if span <= most {
            return (lo, hi);
        }
        let focus = (self.focus.0 + self.focus.1) / 2.0;
        let mut start = (focus - most / 2.0).clamp(lo, hi - most);
        // The x-axis just above the bottom edge: let it show.
        if start > 0.0 && start < most {
            start = -0.1 * most;
        }
        (start, start + most)
    }
}

/// The x interval of interest of a function: its key points near the origin, at least
/// −5 to 5, widened by a twentieth on each side (−5.5 to 5.5); and the function's values
/// at those points (and at 0, the y-intercept).
fn interest(
    f: &dyn Fn(f64) -> f64,
    derivative: Option<&super::Expr>,
    env: &Env,
) -> (f64, f64, Vec<f64>) {
    let mut points = analysis::key_points(f, derivative, env, -100.0, 100.0);
    points.sort_by(|a, b| a.abs().total_cmp(&b.abs()));
    // A periodic curve has key points everywhere: the 16 nearest the origin will do.
    points.truncate(16);
    let lo = points.iter().copied().fold(-5.0f64, f64::min);
    let hi = points.iter().copied().fold(5.0f64, f64::max);
    let pad = (hi - lo) * 0.05;
    let mut focus: Vec<f64> = points.iter().map(|&x| f(x)).collect();
    focus.push(f(0.0));
    (lo - pad, hi + pad, focus)
}

/// The bounding box of the curve F(x, y) = 0, searched in ever larger squares.
fn implicit_box(f: &dyn Fn(f64, f64) -> f64) -> Option<(f64, f64, f64, f64)> {
    for size in [10.0, 100.0, 1000.0] {
        const N: usize = 200;
        let at = |k: usize| -size + 2.0 * size * k as f64 / N as f64;
        let mut found: Option<(f64, f64, f64, f64)> = None;
        let mut prev_row: Vec<f64> = (0..=N).map(|i| f(at(i), at(0))).collect();
        for j in 1..=N {
            let row: Vec<f64> = (0..=N).map(|i| f(at(i), at(j))).collect();
            for i in 1..=N {
                let corners = [prev_row[i - 1], prev_row[i], row[i - 1], row[i]];
                if corners.iter().all(|c| c.is_finite())
                    && corners.iter().any(|&c| c > 0.0) != corners.iter().all(|&c| c > 0.0)
                    || corners.contains(&0.0)
                {
                    let (x0, x1, y0, y1) = (at(i - 1), at(i), at(j - 1), at(j));
                    found = Some(match found {
                        None => (x0, x1, y0, y1),
                        Some((a, b, c, d)) => (a.min(x0), b.max(x1), c.min(y0), d.max(y1)),
                    });
                }
            }
            prev_row = row;
        }
        if found.is_some() {
            return found;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphing::Syntax;

    fn equation(text: &str) -> Equation {
        Equation::parse(text, Syntax::default()).unwrap()
    }

    #[test]
    fn grid_steps() {
        // Windows: x from −27.2 to 27.2 labels every 10 with lines every 2; y from −3 to
        // 30.25 labels every 5 with lines every 1; −10 to 10 labels every 5.
        assert_eq!(
            grid_step(54.44, 2140.0),
            GridStep {
                major: 10.0,
                minor: 2.0
            }
        );
        assert_eq!(
            grid_step(33.275, 1308.0),
            GridStep {
                major: 5.0,
                minor: 1.0
            }
        );
        assert_eq!(grid_step(20.0, 800.0).major, 5.0);
        // Narrow graphs get fewer labels.
        assert_eq!(grid_step(20.0, 150.0).major, 10.0);
        assert_eq!(grid_step(0.02, 800.0).major, 0.005);
    }

    #[test]
    fn grid_steps_of_degenerate_spans() {
        // Views don't get this narrow or wide, but these spans used to make the powers of
        // ten 0 or infinite, and the search never ended.
        for span in [5e-324, 1e-310, 0.0, -1.0, f64::INFINITY, f64::NAN] {
            let step = grid_step(span, 800.0);
            assert!(
                step.minor > 0.0 && step.major.is_finite(),
                "{span:e}: {step:?}"
            );
        }
    }

    #[test]
    fn view_math() {
        let view = View::DEFAULT;
        assert_eq!(view.px_of(0.0, 0.0, 200.0, 100.0), (100.0, 50.0));
        assert_eq!(view.point_at(200.0, 0.0, 200.0, 100.0), (10.0, 10.0));
        // Zooming keeps the point under the pointer.
        let zoomed = view.zoomed(150.0, 25.0, 200.0, 100.0, 0.5);
        assert_eq!(zoomed.point_at(150.0, 25.0, 200.0, 100.0), (5.0, 5.0));
        assert_eq!(zoomed.width(), 10.0);
        let moved = view.dragged(20.0, 10.0, 200.0, 100.0);
        assert_eq!((moved.x_min, moved.y_min), (-12.0, -8.0));
        let p = View::DEFAULT.proportional(200.0, 100.0);
        assert_eq!(
            (p.x_min, p.x_max, p.y_min, p.y_max),
            (-20.0, 20.0, -10.0, 10.0)
        );
        assert_eq!(
            View {
                x_min: -27.22,
                x_max: 27.22,
                y_min: 0.0,
                y_max: 1.0
            }
            .precision(),
            0.01
        );
    }

    #[test]
    fn zooming_in_stops_before_subnormal_views() {
        // The zoom buttons held down on an empty graph: no relative limit stops a view
        // around the origin, which used to shrink into the subnormal doubles.
        let (w, h) = (1340.0, 760.0);
        let mut view = View::DEFAULT;
        for _ in 0..20_000 {
            view = view.zoomed(w as f64 / 2.0, h as f64 / 2.0, w, h, 1.0 / 1.0625);
        }
        assert!(
            view.width() > MIN_SPAN && view.width() < 1.1 * MIN_SPAN,
            "{view:?}"
        );
        // Graph options' ranges: microscopic ones are drawn, subnormal ones refused.
        let x_range = |x_min, x_max| {
            View {
                x_min,
                x_max,
                ..View::DEFAULT
            }
            .is_valid()
        };
        assert!(x_range(0.0, 1e-12));
        assert!(x_range(-1e-200, 1e-200));
        assert!(!x_range(0.0, 1e-305));
        assert!(!x_range(0.0, 5e-324));
    }

    #[test]
    fn functions_break_at_asymptotes() {
        let plot = plot(
            &equation("y=1/x"),
            &Env::RADIANS,
            &View::DEFAULT,
            400.0,
            400.0,
        );
        assert_eq!(plot.lines.len(), 2);
        let plot = plot_of("tan(x)");
        // −10 to 10 has 6 asymptotes, so 7 branches.
        assert_eq!(plot.lines.len(), 7);
    }

    fn plot_of(text: &str) -> Plot {
        plot(&equation(text), &Env::RADIANS, &View::DEFAULT, 400.0, 400.0)
    }

    #[test]
    fn functions_end_where_they_are_defined() {
        let plot = plot_of("sqrt(x)");
        assert_eq!(plot.lines.len(), 1);
        // It starts at x = 0, the middle of the graph.
        let first = plot.lines[0][0];
        assert!((first[0] - 200.0).abs() < 0.01 && (first[1] - 200.0).abs() < 0.01);
        let plot = plot_of("sqrt(25-x^2)");
        let line = &plot.lines[0];
        assert!((line[0][0] - 100.0).abs() < 0.01);
        assert!((line[line.len() - 1][0] - 300.0).abs() < 0.01);
    }

    #[test]
    fn implicit_circle() {
        let plot = plot_of("x^2+y^2=25");
        assert_eq!(plot.lines.len(), 1);
        for p in &plot.lines[0] {
            let (x, y) = View::DEFAULT.point_at(p[0] as f64, p[1] as f64, 400.0, 400.0);
            assert!(((x * x + y * y).sqrt() - 5.0).abs() < 0.01, "({x}, {y})");
        }
        // tan x − y changes sign at the asymptotes too, but they aren't drawn.
        let plot = plot_of("tan(x)-y=0");
        for line in &plot.lines {
            for p in line {
                let (x, y) = View::DEFAULT.point_at(p[0] as f64, p[1] as f64, 400.0, 400.0);
                // Within a pixel of the curve, horizontally (it is steep there, and a
                // cell next to an asymptote holds a root and a pole).
                let pi = std::f64::consts::PI;
                let on_curve = y.atan() + ((x - y.atan()) / pi).round() * pi;
                assert!((x - on_curve).abs() < 0.05, "({x}, {y})");
            }
        }
    }

    #[test]
    fn inverse_functions() {
        let plot = plot_of("x=3");
        assert_eq!(plot.lines.len(), 1);
        assert!(plot.lines[0].iter().all(|p| (p[0] - 260.0).abs() < 0.01));
    }

    #[test]
    fn inequality_regions() {
        let plot = plot_of("y<0");
        // The lower half of the 100 × 100 lattice.
        assert_eq!(plot.dots.len(), 5000);
        assert!(plot.dots.iter().all(|d| d[1] > 200.0));
        let plot = plot_of("x^2+y^2<=25");
        assert!(plot.dots.len() > 1800 && plot.dots.len() < 2100);
        assert!(plot_of("y=x").dots.is_empty());
        assert!(is_dashed(Relation::Less));
        assert!(!is_dashed(Relation::LessEqual));
    }

    #[test]
    fn tracing_snaps_to_the_curve() {
        let eq = equation("x^2");
        let view = View {
            x_min: -27.2204,
            x_max: 27.2204,
            y_min: -3.025,
            y_max: 30.25,
        };
        let (w, h) = (2140.0, 1308.0);
        let p = plot(&eq, &Env::RADIANS, &view, w, h);
        // Just right of the curve at y = 14.6.
        let (px, py) = view.px_of(3.95, 14.6, w, h);
        let point = trace(
            &[(&eq, &p)],
            &Env::RADIANS,
            &view,
            w,
            h,
            px as f32,
            py as f32,
        )
        .unwrap();
        assert_eq!(point.equation, 0);
        assert!((point.x * 100.0 - (point.x * 100.0).round()).abs() < 1e-9);
        assert!((point.x - 3.82).abs() < 0.05, "{}", point.x);
        assert_eq!(point.y, point.x * point.x);
        // Far from the curve: nothing.
        let (px, py) = view.px_of(15.0, 5.0, w, h);
        assert!(
            trace(
                &[(&eq, &p)],
                &Env::RADIANS,
                &view,
                w,
                h,
                px as f32,
                py as f32
            )
            .is_none()
        );
    }

    #[test]
    fn traced_values_are_of_the_x_shown() {
        // Windows shows (3.82, 14.592400000000000) for x² near (3.95, 14.6): y is the
        // value at the double nearest 3.82, not at 382 × 0.01 = 3.8200000000000003.
        let eq = equation("x^2");
        let view = View {
            x_min: -27.2204,
            x_max: 27.2204,
            y_min: -3.025,
            y_max: 30.25,
        };
        let (w, h) = (2140.0, 1308.0);
        let p = plot(&eq, &Env::RADIANS, &view, w, h);
        let traced = |x: f64, y: f64| {
            let (px, py) = view.px_of(x, y, w, h);
            let at = (px as f32, py as f32);
            trace(&[(&eq, &p)], &Env::RADIANS, &view, w, h, at.0, at.1).unwrap()
        };
        let point = traced(3.95, 14.6);
        assert_eq!((point.x, point.y), (3.82, 3.82 * 3.82));
        let point = traced(-5.1, 26.01);
        assert_eq!((point.x, point.y), (-5.1, 5.1 * 5.1));
        assert_eq!(rounded(3.8213, 0.01).to_string(), "3.82");
        assert_eq!(rounded(1234.0, 10.0), 1230.0);
    }

    #[test]
    fn best_fit_views() {
        let (w, h) = (2140.0, 1308.0);
        let fit = |text: &str| best_fit(&[(&equation(text), Env::RADIANS)], w, h);
        // Windows: x from −27.2204 to 27.2204, y from −3.025 to 30.25.
        let view = fit("x^2");
        assert!(
            (view.y_min + 3.025).abs() < 0.01 && (view.y_max - 30.25).abs() < 0.01,
            "{view:?}"
        );
        assert!((view.x_max - 27.22).abs() < 0.05, "{view:?}");
        assert!((view.x_min + view.x_max).abs() < 1e-9);
        // The parabola's vertex and the sine's waves are in view.
        let view = fit("sin(x)");
        assert!(view.x_max > 6.3 && view.x_max < 30.0, "{view:?}");
        assert!(view.y_max > 1.0);
        let view = fit("x^3");
        assert!(view.x_max >= 5.0 && view.y_max >= 5.0, "{view:?}");
        let view = fit("x^2+y^2=25");
        assert!(view.y_max > 5.0 && view.y_max < 7.0, "{view:?}");
        let view = fit("1/x");
        assert!(view.y_max < 100.0, "{view:?}");
        let view = fit("e^x");
        assert!(view.y_min < 0.0 && view.y_max > 1.0, "{view:?}");
        assert_eq!(best_fit(&[], w, h), View::DEFAULT);
    }

    #[test]
    fn best_fit_of_inverse_functions() {
        let (w, h) = (2140.0, 1308.0);
        let fit = |text: &str| best_fit(&[(&equation(text), Env::RADIANS)], w, h);
        let proportional = |v: &View| (v.width() / v.height() - w as f64 / h as f64).abs() < 1e-9;
        // x = y² is y = x² on its side, with a tenth of room left of the vertex.
        let view = fit("x=y^2");
        assert!(
            (view.x_min + 3.025).abs() < 0.01 && (view.x_max - 30.25).abs() < 0.01,
            "{view:?}"
        );
        assert!((view.y_min + view.y_max).abs() < 1e-9 && proportional(&view));
        // x = 3, like y = 3, is shown with the axis next to it, at equal scales.
        let view = fit("x=3");
        assert!(view.x_min < 0.0 && view.x_max > 3.0, "{view:?}");
        assert!(proportional(&view), "{view:?}");
        let view = fit("y=3");
        assert!(view.y_min < 0.0 && view.y_max > 3.0 && proportional(&view));
    }
}
