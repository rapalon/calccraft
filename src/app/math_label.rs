//! The keys' formula labels: `x²`, `xʸ`, `10ˣ`, `¹/x`, `²√x`, `ʸ√x`, `|x|`, `logᵧx` …
//!
//! Windows draws them from the Calculator's own icon font, which may only be used inside
//! Windows. One line of glyphs can't stand in for it: egui's `italics` only skews the upright
//! glyph (its top then leans into the next one), Unicode's superscripts sit low and touch the
//! base, and a `√` glyph has no bar over the radicand. So a [`Formula`] is a row of pieces laid
//! out by their measured ink: the variables are Noto Sans Math's real italic letters
//! (`𝑥`, U+1D465), an exponent or index is [`SCRIPT`] × the size and raised by what the base
//! measures, and the radical is a vector path whose bar spans the radicand.

use std::sync::Arc;

use eframe::egui::{
    Color32, FontFamily, FontId, Galley, Painter, Pos2, Rect, Shape, Stroke, pos2, text::LayoutJob,
    vec2,
};

/// Noto Sans Math, the font of the italic variables (registered by `setup_fonts`).
pub(crate) fn math() -> FontFamily {
    FontFamily::Name("math".into())
}

/// An exponent's or index's size, relative to the base.
const SCRIPT: f32 = 0.6;
/// The room between two pieces' ink, in ems (of the formula's size).
const GAP: f32 = 0.07;
/// Before an exponent.
const SUP_GAP: f32 = 0.08;
/// Between a bar of `|x|` and the x.
const BAR_GAP: f32 = 0.12;
/// Around the slash of `¹/x`, whose ink leans over its neighbors.
const SLASH_GAP: f32 = 0.03;

/// The mathematical italic letter for a Latin lowercase letter: `x` is `𝑥` (U+1D465).
/// Unicode has `ℎ` (U+210E) for h, outside the block.
pub(crate) fn italic_letter(c: char) -> char {
    match c {
        'h' => 'ℎ',
        'a'..='z' => char::from_u32(0x1D44E + (c as u32 - 'a' as u32)).unwrap_or(c),
        _ => c,
    }
}

/// A piece of text and its font.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Piece {
    text: String,
    family: FontFamily,
}

/// Upright text in the text font: digits, `log`, `e`, `!`, `|`.
pub(crate) fn up(text: &str) -> Piece {
    Piece {
        text: text.to_owned(),
        family: FontFamily::Proportional,
    }
}

/// A variable: a mathematical italic letter.
pub(crate) fn var(letter: char) -> Piece {
    Piece {
        text: italic_letter(letter).to_string(),
        family: math(),
    }
}

/// Text in Segoe UI Symbol (`⌊ ⌋ ⌈ ⌉`, which Segoe UI lacks).
pub(crate) fn symbol(text: &str) -> Piece {
    Piece {
        text: text.to_owned(),
        family: super::symbols(),
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Item {
    Base(Piece),
    /// An exponent, raised over what comes before it.
    Sup(Piece),
    /// A lowered index (`logᵧ`).
    Sub(Piece),
    Root {
        index: Option<Piece>,
        radicand: Piece,
    },
}

/// A formula label at a font size, built piece by piece: `Formula::new(20.0).base(var('x'))
/// .sup(up("2"))` is x².
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Formula {
    size: f32,
    items: Vec<Item>,
}

/// A text piece where the layout put it.
#[derive(Clone, Debug)]
pub(crate) struct Placed {
    pub galley: Arc<Galley>,
    /// The galley's top left corner.
    pub pos: Pos2,
    /// The glyphs' ink.
    pub ink: Rect,
}

/// A laid-out formula, with the base line at y = 0.
#[derive(Clone, Debug)]
pub(crate) struct Layout {
    /// The text pieces in the order of the formula (a root's index before its radicand).
    pub pieces: Vec<Placed>,
    /// The radical's path: the tick, the stroke down and up, the bar (last two points).
    pub radical: Option<Vec<Pos2>>,
    /// The radical's stroke width.
    pub stroke: f32,
    /// The ink, and vertically the text font's line, so labels center like plain text.
    pub bounds: Rect,
}

/// A piece measured with its base line at y = 0 and its start at x = 0.
struct Shaped {
    galley: Arc<Galley>,
    /// The galley's top relative to the base line.
    top: f32,
    ink: Rect,
}

fn shape(layout: &mut dyn FnMut(LayoutJob) -> Arc<Galley>, piece: &Piece, size: f32) -> Shaped {
    let job = LayoutJob::simple_singleline(
        piece.text.clone(),
        FontId::new(size, piece.family.clone()),
        Color32::PLACEHOLDER,
    );
    let galley = layout(job);
    let (baseline, ink) = match galley.rows.first() {
        Some(row) => {
            let baseline = row.pos.y + row.glyphs.first().map_or(row.size.y, |g| g.pos.y);
            let ink = row
                .glyphs
                .iter()
                .filter(|g| !g.uv_rect.is_nothing())
                .map(|g| {
                    Rect::from_min_size(
                        row.pos + g.pos.to_vec2() + g.uv_rect.offset,
                        g.uv_rect.size,
                    )
                })
                .fold(Rect::NOTHING, |a, b| a.union(b));
            (baseline, ink)
        }
        None => (galley.size().y, Rect::NOTHING),
    };
    let ink = if ink.is_positive() {
        ink
    } else {
        Rect::from_min_size(Pos2::ZERO, galley.size())
    };
    Shaped {
        top: -baseline,
        ink: ink.translate(vec2(0.0, -baseline)),
        galley,
    }
}

impl Formula {
    pub(crate) fn new(size: f32) -> Self {
        Self {
            size,
            items: Vec::new(),
        }
    }

    pub(crate) fn base(mut self, piece: Piece) -> Self {
        self.items.push(Item::Base(piece));
        self
    }

    pub(crate) fn sup(mut self, piece: Piece) -> Self {
        self.items.push(Item::Sup(piece));
        self
    }

    pub(crate) fn sub(mut self, piece: Piece) -> Self {
        self.items.push(Item::Sub(piece));
        self
    }

    pub(crate) fn root(mut self, index: Option<Piece>, radicand: Piece) -> Self {
        self.items.push(Item::Root { index, radicand });
        self
    }

    /// The room before `item` after `prev`, in ems.
    fn gap(prev: Option<&Item>, item: &Item) -> f32 {
        let text = |item: &Item| match item {
            Item::Base(p) | Item::Sup(p) | Item::Sub(p) => p.text.clone(),
            Item::Root { .. } => String::new(),
        };
        let Some(prev) = prev else { return 0.0 };
        let (a, b) = (text(prev), text(item));
        if a == "/" || b == "/" {
            SLASH_GAP
        } else if a == "|" || b == "|" {
            BAR_GAP
        } else {
            match (prev, item) {
                (_, Item::Sup(_)) => SUP_GAP,
                (_, Item::Sub(_)) => 0.03,
                (Item::Sup(_) | Item::Sub(_), _) => 0.06,
                _ => GAP,
            }
        }
    }

    /// Lays the formula out with `layout` (a painter's or the fonts' `layout_job`).
    pub(crate) fn layout(&self, layout: &mut dyn FnMut(LayoutJob) -> Arc<Galley>) -> Layout {
        let s = self.size;
        let small = s * SCRIPT;
        // The italic x's height, and the text font's line for the vertical bounds.
        let x_height = -shape(layout, &var('x'), s).ink.top();
        let line = shape(layout, &up("x"), s);
        let mut bounds = Rect::from_min_max(
            pos2(f32::INFINITY, line.top),
            pos2(f32::NEG_INFINITY, line.top + line.galley.size().y),
        );
        let mut pieces = Vec::new();
        let mut radical = None;
        let stroke = (s * 0.07).max(1.0);
        // The right end of the ink so far, and the top of the last base piece's ink.
        let mut right = 0.0;
        let mut base_top = -x_height;
        let place = |shaped: Shaped, left: f32, baseline: f32, pieces: &mut Vec<Placed>| {
            let offset = vec2(left - shaped.ink.left(), baseline);
            let ink = shaped.ink.translate(offset);
            pieces.push(Placed {
                pos: pos2(offset.x, baseline + shaped.top),
                galley: shaped.galley,
                ink,
            });
            ink
        };
        let mut prev = None;
        for item in &self.items {
            let left = right + Self::gap(prev, item) * s;
            match item {
                Item::Base(piece) => {
                    let ink = place(shape(layout, piece, s), left, 0.0, &mut pieces);
                    base_top = ink.top();
                    right = ink.right();
                }
                Item::Sup(piece) => {
                    let shaped = shape(layout, piece, small);
                    // Its bottom at about half the x-height, its middle no lower than the
                    // base's top: over a 10 it goes higher than over an x.
                    let body = -shaped.ink.top();
                    let raise = (x_height * 0.5).max(-base_top - body * 0.5);
                    right = place(shaped, left, -raise, &mut pieces).right();
                }
                Item::Sub(piece) => {
                    let shaped = shape(layout, piece, small);
                    right = place(shaped, left, 0.2 * s, &mut pieces).right();
                }
                Item::Root { index, radicand } => {
                    let rad = shape(layout, radicand, s);
                    let top = rad.ink.top() - 0.14 * s;
                    let bottom = 0.04 * s;
                    let mut x0 = left;
                    let index = index.as_ref().map(|piece| shape(layout, piece, small));
                    // The index sits over the tick, its right end above the stroke down.
                    let tick_y = bottom - 0.42 * (bottom - top);
                    let index_right = |x0: f32| x0 + 0.17 * s;
                    if let Some(index) = &index {
                        let index_left = index_right(x0) - index.ink.width();
                        if prev.is_some() && index_left < left {
                            x0 += left - index_left;
                        }
                    }
                    let a = pos2(x0, tick_y);
                    let b = pos2(x0 + 0.1 * s, tick_y - 0.05 * s);
                    let c = pos2(x0 + 0.24 * s, bottom);
                    let d = pos2(x0 + 0.42 * s, top);
                    if let Some(index) = index {
                        let width = index.ink.width();
                        let lift = b.y - 0.06 * s - index.ink.bottom();
                        place(index, index_right(x0) - width, lift, &mut pieces);
                    }
                    let ink = place(rad, d.x + 0.08 * s, 0.0, &mut pieces);
                    let e = pos2(ink.right() + 0.06 * s, top);
                    let path = vec![a, b, c, d, e];
                    let half = stroke / 2.0;
                    for point in &path {
                        bounds = bounds.union(Rect::from_center_size(*point, vec2(stroke, stroke)));
                    }
                    radical = Some(path);
                    base_top = ink.top();
                    right = e.x + half;
                }
            }
            prev = Some(item);
        }
        for piece in &pieces {
            bounds = bounds.union(piece.ink);
        }
        Layout {
            pieces,
            radical,
            stroke,
            bounds,
        }
    }

    /// Paints the formula centered on `center` in `color`.
    pub(crate) fn paint(&self, painter: &Painter, center: Pos2, color: Color32) {
        let layout = self.layout(&mut |job| painter.layout_job(job));
        // Whole pixels, so the radical and the glyphs (which egui snaps) stay together.
        let ppp = painter.ctx().pixels_per_point();
        let offset = ((center - layout.bounds.center()) * ppp).round() / ppp;
        for piece in layout.pieces {
            painter.galley(piece.pos + offset, piece.galley, color);
        }
        if let Some(path) = layout.radical {
            let points = path.into_iter().map(|p| p + offset).collect();
            painter.add(Shape::line(points, Stroke::new(layout.stroke, color)));
        }
    }
}

/// A key's label: text, or a formula.
#[derive(Clone, Debug)]
pub(crate) enum Label {
    Text(LayoutJob),
    Formula(Formula),
}

impl From<LayoutJob> for Label {
    fn from(job: LayoutJob) -> Self {
        Label::Text(job)
    }
}

impl From<Formula> for Label {
    fn from(formula: Formula) -> Self {
        Label::Formula(formula)
    }
}

impl Label {
    /// Paints the label centered on `center` in `color` (the key's text color).
    pub(crate) fn paint(self, painter: &Painter, center: Pos2, color: Color32) {
        match self {
            Label::Text(job) => {
                let galley = painter.layout_job(job);
                painter.galley(center - galley.size() / 2.0, galley, color);
            }
            Label::Formula(formula) => formula.paint(painter, center, color),
        }
    }
}

#[cfg(test)]
mod tests {
    use eframe::egui::{FontData, FontDefinitions, epaint::text::Fonts};

    use super::*;

    /// The fonts the labels use where Segoe UI is missing: Inter and Noto Sans Math.
    fn fonts() -> Fonts {
        let mut definitions = FontDefinitions::empty();
        let mut add = |name: &str, data: &'static [u8]| {
            definitions
                .font_data
                .insert(name.into(), Arc::new(FontData::from_static(data)));
        };
        add("regular", super::super::INTER);
        add("math", super::super::NOTO_SANS_MATH);
        let families = &mut definitions.families;
        families.insert(FontFamily::Proportional, vec!["regular".into()]);
        families.insert(math(), vec!["math".into(), "regular".into()]);
        families.insert(super::super::symbols(), vec!["regular".into()]);
        Fonts::new(Default::default(), definitions)
    }

    fn lay(formula: &Formula) -> Layout {
        let mut fonts = fonts();
        let mut view = fonts.with_pixels_per_point(1.0);
        formula.layout(&mut |job| view.layout_job(job))
    }

    const SIZE: f32 = 20.0;

    #[test]
    fn variables_are_math_italic_letters() {
        assert_eq!(italic_letter('x'), '\u{1D465}');
        assert_eq!(italic_letter('y'), '\u{1D466}');
        assert_eq!(italic_letter('n'), '\u{1D45B}');
        assert_eq!(italic_letter('h'), '\u{210E}');
        // Noto Sans Math has them: they don't look like a missing character's box.
        // (`Fonts::has_glyph` can't tell: Noto Sans Math has the replacement glyph too.)
        let ink = |c: char| {
            let piece = Piece {
                text: c.to_string(),
                family: math(),
            };
            lay(&Formula::new(SIZE).base(piece)).pieces[0].ink
        };
        let missing = ink('\u{E000}');
        for c in ['x', 'y', 'n'] {
            assert_ne!(ink(italic_letter(c)), missing, "{c}");
        }
    }

    #[test]
    fn exponents_are_raised_with_a_gap() {
        let x_height = -lay(&Formula::new(SIZE).base(var('x'))).pieces[0].ink.top();
        for (base, exponent) in [
            (var('x'), up("2")),
            (var('x'), up("3")),
            (var('x'), var('y')),
            (up("10"), var('x')),
            (up("2"), var('x')),
            (up("e"), var('x')),
        ] {
            let layout = lay(&Formula::new(SIZE).base(base.clone()).sup(exponent.clone()));
            let (b, e) = (layout.pieces[0].ink, layout.pieces[1].ink);
            let what = format!("{base:?}^{exponent:?}: base {b:?}, exponent {e:?}");
            // Its bottom (the base line of a digit, the descender of a y) above half the
            // x-height, its top clearly above the base's.
            assert!(e.bottom() < -x_height * 0.25, "{what}");
            assert!(e.top() < b.top() - 0.1 * SIZE, "{what}");
            assert!(e.left() > b.right() + 0.05 * SIZE, "{what}");
            assert!(!e.intersects(b), "{what}");
            assert!(layout.bounds.contains_rect(e) && layout.bounds.contains_rect(b));
        }
    }

    #[test]
    fn the_exponent_is_smaller() {
        let layout = lay(&Formula::new(SIZE).base(var('x')).sup(var('x')));
        let (b, e) = (layout.pieces[0].ink, layout.pieces[1].ink);
        assert!((e.height() / b.height() - SCRIPT).abs() < 0.1);
    }

    #[test]
    fn the_radical_spans_its_radicand() {
        for index in [Some(up("2")), Some(up("3")), Some(var('y')), None] {
            let layout = lay(&Formula::new(SIZE).root(index.clone(), var('x')));
            let path = layout.radical.as_ref().expect("a radical");
            let radicand = layout.pieces.last().unwrap().ink;
            let (start, end) = (path[path.len() - 2], path[path.len() - 1]);
            assert_eq!(start.y, end.y);
            assert!(end.x - start.x >= radicand.width());
            assert!(start.x < radicand.left() && end.x > radicand.right());
            // The bar is above the x, and the stroke down reaches the base line.
            assert!(start.y < radicand.top() - layout.stroke);
            assert!(path.iter().any(|p| p.y >= 0.0));
            if index.is_some() {
                let index = layout.pieces[0].ink;
                assert!(!index.intersects(radicand), "{index:?} {radicand:?}");
                // Over the tick, left of the bar.
                assert!(index.bottom() < path[1].y && index.right() < start.x);
            }
            for piece in &layout.pieces {
                assert!(layout.bounds.contains_rect(piece.ink));
            }
        }
    }

    #[test]
    fn the_radical_scales_with_the_size() {
        let small = lay(&Formula::new(16.0).root(Some(up("2")), var('x')));
        let large = lay(&Formula::new(32.0).root(Some(up("2")), var('x')));
        assert!((large.stroke / small.stroke - 2.0).abs() < 0.01);
    }

    #[test]
    fn abs_centers_the_x_between_its_bars() {
        let layout = lay(&Formula::new(SIZE)
            .base(up("|"))
            .base(var('x'))
            .base(up("|")));
        let [left, x, right] = [0, 1, 2].map(|i| layout.pieces[i].ink);
        let (before, after) = (x.left() - left.right(), right.left() - x.right());
        assert!((before - after).abs() < 0.5, "{before} vs {after}");
        assert!(before > 0.08 * SIZE);
    }

    #[test]
    fn the_reciprocal_and_the_log_base_dont_overlap() {
        for formula in [
            Formula::new(SIZE).sup(up("1")).base(up("/")).base(var('x')),
            Formula::new(SIZE)
                .base(up("log"))
                .sub(var('y'))
                .base(var('x')),
            Formula::new(SIZE).base(var('n')).base(up("!")),
            Formula::new(SIZE)
                .base(symbol("⌊"))
                .base(var('x'))
                .base(symbol("⌋")),
        ] {
            let layout = lay(&formula);
            for pair in layout.pieces.windows(2) {
                assert!(pair[0].ink.right() < pair[1].ink.left(), "{formula:?}");
            }
        }
    }
}
