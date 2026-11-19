//! The layouts at the Android app's screen sizes: phones and tablets in both orientations, with
//! their system bars as egui's safe area. The tests check that every key stays at least 40 points
//! tall and inside the safe area; `write_layout_shots` (ignored) paints each mode into
//! `target/shots/` with a small software rasterizer of egui's meshes, to look at them.

use super::*;
use egui::epaint::{ClippedPrimitive, Primitive};
use egui::{ImageData, TextureId};
use std::collections::HashMap;

/// An Android screen: its size in points and the safe area's insets (left, top, right,
/// bottom), as `android::input_hook` hands them to egui.
#[derive(Clone, Copy, Debug)]
pub(super) struct Screen {
    pub name: &'static str,
    pub size: egui::Vec2,
    pub insets: [f32; 4],
}

impl Screen {
    pub fn safe_rect(&self) -> Rect {
        let [left, top, right, bottom] = self.insets;
        Rect::from_min_max(
            pos2(left, top),
            pos2(self.size.x - right, self.size.y - bottom),
        )
    }
}

/// The sizes the task names: a large phone (915 × 412) with three-button navigation on the
/// right, a small one (780 × 360) with a camera cutout on the left and gesture navigation, and
/// a tablet (1280 × 800) in both orientations with its taskbar-less status and gesture bars.
pub(super) const SCREENS: [Screen; 5] = [
    Screen {
        name: "phone-915x412",
        size: vec2(915.0, 412.0),
        insets: [0.0, 24.0, 48.0, 0.0],
    },
    Screen {
        name: "phone-780x360",
        size: vec2(780.0, 360.0),
        insets: [32.0, 24.0, 0.0, 16.0],
    },
    Screen {
        name: "phone-412x915",
        size: vec2(412.0, 915.0),
        insets: [0.0, 32.0, 0.0, 24.0],
    },
    Screen {
        name: "tablet-1280x800",
        size: vec2(1280.0, 800.0),
        insets: [0.0, 24.0, 0.0, 20.0],
    },
    Screen {
        name: "tablet-800x1280",
        size: vec2(800.0, 1280.0),
        insets: [0.0, 24.0, 0.0, 20.0],
    },
];

/// Every mode: the calculators, the date calculation and every converter.
fn all_modes() -> Vec<Mode> {
    Mode::CALCULATORS
        .into_iter()
        .chain(Category::ALL.map(Mode::Converter))
        .collect()
}

/// Runs one frame on `screen` and returns its output; its textures go to `canvas`, if any.
pub(super) fn frame(
    ctx: &Context,
    app: &mut CalculatorApp,
    screen: &Screen,
    canvas: Option<&mut Canvas>,
) -> egui::FullOutput {
    let [left, top, right, bottom] = screen.insets;
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, screen.size)),
        safe_area_insets: Some(egui::SafeAreaInsets(egui::epaint::MarginF32 {
            left,
            right,
            top,
            bottom,
        })),
        ..Default::default()
    };
    let mut frame = eframe::Frame::_new_kittest();
    let mut output = ctx.run_ui(input, |ui| eframe::App::ui(app, ui, &mut frame));
    if let Some(canvas) = canvas {
        canvas.update_textures(&output.textures_delta);
    }
    output.textures_delta.clear();
    output
}

/// The app in `mode` on `screen`, after a few frames (fonts and layout settle).
pub(super) fn settled(ctx: &Context, mode: Mode, screen: &Screen) -> CalculatorApp {
    let mut app = super::tests::start_in(ctx);
    app.set_mode(mode);
    for _ in 0..3 {
        frame(ctx, &mut app, screen, None);
    }
    app
}

/// The buttons of the accessibility tree with their bounds, in points.
fn buttons(ctx: &Context, app: &mut CalculatorApp, screen: &Screen) -> Vec<(String, Rect)> {
    ctx.enable_accesskit();
    let output = frame(ctx, app, screen, None);
    let nodes = output.platform_output.accesskit_update.unwrap().nodes;
    nodes
        .into_iter()
        .filter(|(_, node)| node.role() == egui::accesskit::Role::Button)
        .filter_map(|(_, node)| {
            let bounds = node.bounds()?;
            let rect = Rect::from_min_max(
                pos2(bounds.x0 as f32, bounds.y0 as f32),
                pos2(bounds.x1 as f32, bounds.y1 as f32),
            );
            Some((node.label().unwrap_or_default().to_string(), rect))
        })
        .collect()
}

/// The keypads stay usable on every screen: each calculator key (a button below the display,
/// in a keypad grid) is at least 40 points tall and inside the safe area.
#[test]
fn keys_fit_every_screen() {
    for screen in &SCREENS {
        // The Graphing mode twice: its graph and its equations with the keypad.
        let pages = all_modes()
            .into_iter()
            .map(|mode| (mode, false))
            .chain([(Mode::Graphing, true)]);
        for (mode, equations) in pages {
            let ctx = Context::default();
            let mut app = settled(&ctx, mode, screen);
            app.graphing.show_equations = equations;
            let safe = screen.safe_rect();
            let keys = buttons(&ctx, &mut app, screen);
            // The keypad's keys: the digit 7 and its neighbors, found by name.
            let seven = keys
                .iter()
                .find(|(name, _)| name == tr("num7Button.AutomationName"))
                .map(|(_, rect)| *rect);
            for (name, rect) in &keys {
                assert!(
                    safe.expand(0.5).contains_rect(*rect),
                    "{}: {mode:?}: {name} at {rect:?} is outside the safe area {safe:?}",
                    screen.name
                );
            }
            // Every calculator has its keypad on screen (the Graphing mode's with its equations).
            let keypad = match mode {
                Mode::Standard | Mode::Scientific | Mode::Programmer => true,
                Mode::Graphing => equations,
                _ => false,
            };
            assert!(
                seven.is_some() || !keypad,
                "{}: {mode:?}: no 7 key",
                screen.name
            );
            if let Some(seven) = seven {
                assert!(
                    seven.height() >= 40.0,
                    "{}: {mode:?}: the keys are {} points tall",
                    screen.name,
                    seven.height()
                );
            }
        }
    }
}

/// A software rasterizer of egui's output: enough to look at a layout without a GPU.
pub(super) struct Canvas {
    width: usize,
    height: usize,
    pixels: Vec<[f32; 4]>,
    textures: HashMap<TextureId, (usize, usize, Vec<Color32>)>,
}

impl Canvas {
    fn new(width: usize, height: usize, background: Color32) -> Self {
        let rgba = background.to_array().map(|c| c as f32 / 255.0);
        Self {
            width,
            height,
            pixels: vec![rgba; width * height],
            textures: HashMap::new(),
        }
    }

    fn update_textures(&mut self, delta: &egui::TexturesDelta) {
        let deltas = delta
            .set
            .iter()
            .flat_map(|(id, images)| images.iter().map(move |image| (id, image)));
        for (id, image) in deltas {
            let ImageData::Color(color) = &image.image;
            let [w, h] = color.size;
            match image.pos {
                None => {
                    self.textures.insert(*id, (w, h, color.pixels.clone()));
                }
                Some([x0, y0]) => {
                    let (tw, _, pixels) = self.textures.get_mut(id).expect("a texture");
                    for y in 0..h {
                        for x in 0..w {
                            pixels[(y0 + y) * *tw + x0 + x] = color.pixels[y * w + x];
                        }
                    }
                }
            }
        }
    }

    fn sample(&self, id: TextureId, u: f32, v: f32) -> [f32; 4] {
        let Some((w, h, pixels)) = self.textures.get(&id) else {
            return [1.0; 4];
        };
        let x = ((u * *w as f32) as usize).min(w - 1);
        let y = ((v * *h as f32) as usize).min(h - 1);
        pixels[y * w + x].to_array().map(|c| c as f32 / 255.0)
    }

    fn paint(&mut self, primitives: &[ClippedPrimitive], scale: f32) {
        for primitive in primitives {
            let Primitive::Mesh(mesh) = &primitive.primitive else {
                continue;
            };
            let clip = primitive.clip_rect;
            for triangle in mesh.indices.as_chunks::<3>().0 {
                let [a, b, c] = [0, 1, 2].map(|i| mesh.vertices[triangle[i] as usize]);
                let (pa, pb, pc) = (
                    a.pos.to_vec2() * scale,
                    b.pos.to_vec2() * scale,
                    c.pos.to_vec2() * scale,
                );
                let area = (pb - pa).x * (pc - pa).y - (pb - pa).y * (pc - pa).x;
                if area.abs() < 1e-6 {
                    continue;
                }
                let min_x = (pa
                    .x
                    .min(pb.x)
                    .min(pc.x)
                    .max(clip.left() * scale)
                    .floor()
                    .max(0.0)) as usize;
                let max_x = (pa.x.max(pb.x).max(pc.x).min(clip.right() * scale).ceil() as usize)
                    .min(self.width);
                let min_y = (pa
                    .y
                    .min(pb.y)
                    .min(pc.y)
                    .max(clip.top() * scale)
                    .floor()
                    .max(0.0)) as usize;
                let max_y = (pa.y.max(pb.y).max(pc.y).min(clip.bottom() * scale).ceil() as usize)
                    .min(self.height);
                for y in min_y..max_y {
                    for x in min_x..max_x {
                        let p = vec2(x as f32 + 0.5, y as f32 + 0.5);
                        let w0 = ((pb - p).x * (pc - p).y - (pb - p).y * (pc - p).x) / area;
                        let w1 = ((pc - p).x * (pa - p).y - (pc - p).y * (pa - p).x) / area;
                        let w2 = 1.0 - w0 - w1;
                        if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                            continue;
                        }
                        let u = w0 * a.uv.x + w1 * b.uv.x + w2 * c.uv.x;
                        let v = w0 * a.uv.y + w1 * b.uv.y + w2 * c.uv.y;
                        let texel = self.sample(mesh.texture_id, u, v);
                        let [ca, cb, cc] = [a.color, b.color, c.color]
                            .map(|c| c.to_array().map(|c| c as f32 / 255.0));
                        let mut src = [0.0; 4];
                        for i in 0..4 {
                            src[i] = (w0 * ca[i] + w1 * cb[i] + w2 * cc[i]) * texel[i];
                        }
                        // Premultiplied alpha, as egui's renderers blend.
                        let dst = &mut self.pixels[y * self.width + x];
                        for i in 0..4 {
                            dst[i] = src[i] + dst[i] * (1.0 - src[3]);
                        }
                    }
                }
            }
        }
    }

    fn save(&self, path: &std::path::Path) {
        let file = std::fs::File::create(path).unwrap();
        let mut encoder = png::Encoder::new(file, self.width as u32, self.height as u32);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        let data: Vec<u8> = self
            .pixels
            .iter()
            .flat_map(|p| [p[0], p[1], p[2]].map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8))
            .collect();
        writer.write_image_data(&data).unwrap();
    }
}

/// Paints `app` on `screen` into `target/shots/<name>.png`, the system bars in gray.
fn shoot(ctx: &Context, app: &mut CalculatorApp, screen: &Screen, name: &str, setup: &mut Canvas) {
    let output = frame(ctx, app, screen, Some(setup));
    let primitives = ctx.tessellate(output.shapes, output.pixels_per_point);
    let scale = 1.0;
    let mut canvas = Canvas::new(
        (screen.size.x * scale) as usize,
        (screen.size.y * scale) as usize,
        Color32::from_gray(60),
    );
    canvas.textures = std::mem::take(&mut setup.textures);
    canvas.paint(&primitives, scale);
    setup.textures = std::mem::take(&mut canvas.textures);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/shots");
    std::fs::create_dir_all(&dir).unwrap();
    canvas.save(&dir.join(format!("{name}.png")));
}

/// What puts the app in the state a picture shows.
type Setup = Box<dyn Fn(&mut CalculatorApp)>;

/// Writes a picture of every mode on every screen into `target/shots/`:
/// `cargo test write_layout_shots -- --ignored`. `CALCCRAFT_SHOTS=<filter>` keeps the names
/// that contain it.
#[test]
#[ignore]
fn write_layout_shots() {
    let filter = std::env::var("CALCCRAFT_SHOTS").unwrap_or_default();
    for screen in &SCREENS {
        let mut pages: Vec<(String, Setup)> = all_modes()
            .into_iter()
            .map(|mode| {
                let name = format!("{mode:?}").replace(['(', ')'], "-");
                let setup: Setup = Box::new(move |app: &mut CalculatorApp| app.set_mode(mode));
                (name, setup)
            })
            .collect();
        pages.push((
            "GraphingEquations".into(),
            Box::new(|app| {
                app.set_mode(Mode::Graphing);
                app.graphing.show_equations = true;
            }),
        ));
        pages.push(("Settings".into(), Box::new(|app| app.page = Page::Settings)));
        pages.push((
            "Navigation".into(),
            Box::new(|app| app.flyout = Flyout::Navigation),
        ));
        pages.push((
            "History".into(),
            Box::new(|app| {
                for key in [
                    Command::Digit(1),
                    Command::Operator(crate::engine::Op::Add),
                    Command::Digit(2),
                    Command::Equals,
                ] {
                    app.engine_mut().execute(key);
                }
                app.engine_mut().execute(Command::MemoryStore);
                if !app.docked {
                    app.flyout = Flyout::History;
                }
            }),
        ));
        pages.push((
            "Caret".into(),
            Box::new(|app| {
                app.set_mode(Mode::Scientific);
                for digit in [1, 2, 3, 4, 5, 6, 7] {
                    app.engine_mut().execute(Command::Digit(digit));
                }
                app.engine_mut().show_entry_caret(crate::engine::Caret {
                    exponent: false,
                    index: 2,
                });
            }),
        ));
        pages.push((
            "LineEdit".into(),
            Box::new(|app| {
                use crate::engine::Op;
                app.set_mode(Mode::Scientific);
                let number = |app: &mut CalculatorApp, n: &str| {
                    for c in n.chars() {
                        let digit = c.to_digit(10).unwrap() as u8;
                        app.engine_mut().execute(Command::Digit(digit));
                    }
                };
                let key = |app: &mut CalculatorApp, command| app.engine_mut().execute(command);
                number(app, "2");
                for (a, op, b) in [
                    ("5", Op::Multiply, "7"),
                    ("100", Op::Subtract, "92"),
                    ("6", Op::Multiply, "54"),
                ] {
                    key(app, Command::Operator(Op::Add));
                    key(app, Command::OpenParen);
                    number(app, a);
                    key(app, Command::Operator(op));
                    number(app, b);
                    key(app, Command::CloseParen);
                }
                let numbers: Vec<_> = app
                    .engine()
                    .expression_parts()
                    .1
                    .into_iter()
                    .filter(|part| part.kind == crate::engine::PartKind::Number)
                    .collect();
                let item = numbers[4].item;
                app.engine_mut().edit_operand(item, None);
                key(app, Command::Backspace);
                key(app, Command::Backspace);
                number(app, "85");
            }),
        ));
        pages.push((
            "OperatorEdit".into(),
            Box::new(|app| {
                use crate::engine::{Op, PartKind};
                // Konst's chain: 10 + 2 = 12 + 4 = 16 ÷ 4 =, then ÷ selected and made +.
                for command in [
                    Command::Digit(1),
                    Command::Digit(0),
                    Command::Operator(Op::Add),
                    Command::Digit(2),
                    Command::Equals,
                    Command::Operator(Op::Add),
                    Command::Digit(4),
                    Command::Equals,
                    Command::Operator(Op::Divide),
                    Command::Digit(4),
                    Command::Equals,
                ] {
                    app.engine_mut().execute(command);
                }
                let parts = app.engine().expression_parts().1;
                let operator = parts.iter().find(|part| part.kind == PartKind::Operator);
                app.engine_mut().edit_part(&operator.unwrap().clone(), None);
                app.engine_mut().execute(Command::Operator(Op::Add));
            }),
        ));
        for (name, setup) in pages {
            let name = format!("{}-{name}", screen.name);
            if !name.contains(&filter) {
                continue;
            }
            let ctx = Context::default();
            let mut app = super::tests::start_in(&ctx);
            let mut canvas = Canvas::new(1, 1, Color32::BLACK);
            for _ in 0..2 {
                frame(&ctx, &mut app, screen, Some(&mut canvas));
            }
            setup(&mut app);
            for _ in 0..3 {
                frame(&ctx, &mut app, screen, Some(&mut canvas));
            }
            shoot(&ctx, &mut app, screen, &name, &mut canvas);
        }
    }
}
