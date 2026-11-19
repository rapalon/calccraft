//! The accessibility tree as the platform adapters see it: egui's tree updates fed through
//! AccessKit's own consumer, with the filter that the Android, Windows and macOS adapters use.
//! An accessibility service reads only nodes with real bounds (Marionet drops empty ones),
//! and the consumer panics on an invalid update (a focus that isn't in the tree), which ended
//! the Android app once.

use super::layout_tests::{SCREENS, Screen, frame, settled};
use super::*;
use accesskit_consumer::{Node, Tree, TreeChangeHandler as ChangeHandler, common_filter};
use egui::accesskit::{self, Role};

/// The equations of the Graphing tests, as typed.
const EQUATIONS: [&str; 3] = ["x^2", "2x+1", "sin(x)"];

/// One frame with `events` (pointer taps) on `screen`.
fn frame_with(
    ctx: &Context,
    app: &mut CalculatorApp,
    screen: &Screen,
    events: Vec<egui::Event>,
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
        events,
        ..Default::default()
    };
    let mut frame = eframe::Frame::_new_kittest();
    let mut output = ctx.run_ui(input, |ui| eframe::App::ui(app, ui, &mut frame));
    output.textures_delta.clear();
    output
}

/// The tree update of one more frame, with AccessKit on.
fn update(ctx: &Context, app: &mut CalculatorApp, screen: &Screen) -> accesskit::TreeUpdate {
    ctx.enable_accesskit();
    frame(ctx, app, screen, None)
        .platform_output
        .accesskit_update
        .expect("AccessKit is on")
}

/// The nodes a platform adapter shows: what AccessKit's filter includes, under the root.
fn shown<'a>(node: &Node<'a>, out: &mut Vec<Node<'a>>) {
    for child in node.filtered_children(common_filter) {
        out.push(child);
        shown(&child, out);
    }
}

/// The Graphing mode with `EQUATIONS` graphed, its equations or its graph in view.
fn graphing(ctx: &Context, screen: &Screen, equations: bool) -> CalculatorApp {
    let mut app = settled(ctx, Mode::Graphing, screen);
    for (i, text) in EQUATIONS.iter().enumerate() {
        if i >= app.graphing.rows.len() {
            app.graphing.add_row();
        }
        app.graphing.rows[i].text = text.to_string();
        app.graphing.submit(i, graphing_view::region_syntax());
    }
    app.graphing.show_equations = equations;
    for _ in 0..3 {
        frame(ctx, &mut app, screen, None);
    }
    app
}

/// Every node of the Graphing mode that an adapter shows has finite bounds that aren't
/// empty, and the graph is a canvas named with its equations' count that lists them.
#[test]
fn graphing_nodes_have_bounds_and_the_graph_names_its_equations() {
    for screen in &SCREENS {
        for equations in [false, true] {
            let ctx = Context::default();
            let mut app = graphing(&ctx, screen, equations);
            let tree = Tree::new(update(&ctx, &mut app, screen), true);
            let root = tree.state().root();
            let mut nodes = Vec::new();
            shown(&root, &mut nodes);
            let page = format!("{}: equations {equations}", screen.name);
            assert!(nodes.len() > 5, "{page}: only {} nodes", nodes.len());
            for node in &nodes {
                let bounds = node.bounding_box();
                let fine = bounds.is_some_and(|b| {
                    [b.x0, b.y0, b.x1, b.y1].iter().all(|v| v.is_finite())
                        && b.width() > 0.0
                        && b.height() > 0.0
                });
                assert!(
                    fine,
                    "{page}: {:?} {:?} has the bounds {bounds:?}",
                    node.role(),
                    node.label()
                );
            }
            // The graph is on screen in both views but the narrow equations view.
            let graph = nodes.iter().find(|node| node.role() == Role::Canvas);
            if let Some(graph) = graph {
                let label = graph.label().unwrap_or_default();
                assert!(label.contains("displaying 3 equations"), "{page}: {label}");
                assert_eq!(
                    graph.value().as_deref(),
                    Some(EQUATIONS.join("\n").as_str())
                );
            } else {
                assert!(equations, "{page}: no graph node");
            }
        }
    }
}

/// The Standard mode's display and expression line have their rectangles, so that an
/// accessibility service can read the result from the tree.
#[test]
fn the_display_and_the_expression_line_have_bounds() {
    let screen = &SCREENS[2];
    let ctx = Context::default();
    let mut app = settled(&ctx, Mode::Standard, screen);
    for command in [
        Command::Digit(1),
        Command::Digit(2),
        Command::Operator(Op::Multiply),
        Command::Digit(3),
        Command::Equals,
    ] {
        app.calc.execute(command);
    }
    frame(&ctx, &mut app, screen, None);
    let update = update(&ctx, &mut app, screen);
    let tree = Tree::new(update, true);
    let mut nodes = Vec::new();
    shown(&tree.state().root(), &mut nodes);
    let text = |prefix: &str| {
        nodes
            .iter()
            .find(|n| n.role() == Role::Label && n.value().is_some_and(|v| v.starts_with(prefix)))
            .unwrap_or_else(|| panic!("no text {prefix}"))
            .bounding_box()
            .unwrap_or_else(|| panic!("{prefix} has no bounds"))
    };
    let display = text("Display is 36");
    let expression = text("Expression is 12");
    for rect in [display, expression] {
        assert!(rect.width() > 20.0 && rect.height() > 10.0, "{rect:?}");
    }
    // The expression line is at the top of the display.
    assert!(expression.y0 < display.y1, "{expression:?} {display:?}");
}

/// Keeps nothing of the consumer's changes.
struct NoChanges;

impl ChangeHandler for NoChanges {
    fn node_added(&mut self, _: &Node) {}
    fn node_updated(&mut self, _: &Node, _: &Node) {}
    fn focus_moved(&mut self, _: Option<&Node>, _: Option<&Node>) {}
    fn node_removed(&mut self, _: &Node) {}
}

/// The Graphing keypad's ↵ (Submit) adds a box and gives it the focus. With AccessKit on,
/// every frame's tree must name a focus that is one of its nodes: AccessKit's consumer
/// panicked on the frame of the tap ("Focused ID #812028608375119981 is not in the node
/// list": the new box's text edit, the id in the Pixel's log), which ended the Android app.
/// The new box still gets the focus, a frame later (`access::settle_focus`).
#[test]
fn submit_keeps_the_focus_in_the_tree() {
    for screen in &SCREENS {
        let ctx = Context::default();
        let mut app = settled(&ctx, Mode::Graphing, screen);
        app.graphing.show_equations = true;
        app.graphing.rows[0].text = "x^2".into();
        let mut tree = Tree::new(update(&ctx, &mut app, screen), true);
        let submit = {
            let mut nodes = Vec::new();
            shown(&tree.state().root(), &mut nodes);
            let node = nodes
                .iter()
                .find(|n| n.label().as_deref() == Some(tr("submitButton.AutomationName")))
                .unwrap_or_else(|| panic!("{}: no Submit key", screen.name));
            let b = node.bounding_box().unwrap();
            pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32)
        };
        let rows = app.graphing.rows.len();
        let press = |pressed| egui::Event::PointerButton {
            pos: submit,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        let taps = [
            vec![egui::Event::PointerMoved(submit), press(true)],
            vec![press(false)],
            vec![],
            vec![],
        ];
        for events in taps {
            let output = frame_with(&ctx, &mut app, screen, events);
            let update = output.platform_output.accesskit_update.unwrap();
            let focus = update.focus;
            assert!(
                update.nodes.iter().any(|(id, _)| *id == focus),
                "{}: the focus {focus:?} isn't in the tree",
                screen.name
            );
            tree.update_and_process_changes(update, &mut NoChanges);
        }
        assert_eq!(
            app.graphing.rows.len(),
            rows + 1,
            "{}: no new box",
            screen.name
        );
        assert!(
            app.graphing.rows[0].equation.is_some(),
            "{}: x^2 isn't graphed",
            screen.name
        );
        let new_box = graphing_view::edit_id(app.graphing.rows[rows].id);
        assert_eq!(access::focused(&ctx), Some(new_box), "{}", screen.name);
    }
}
