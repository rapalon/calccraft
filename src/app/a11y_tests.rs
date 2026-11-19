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

/// Marionet keeps a node's first 50 children (`MAX_CHILDREN` in its compactor) and 400 nodes.
const MAX_CHILDREN: usize = 50;
const MAX_NODES: usize = 400;

/// The tree of one more frame on `screen`, as an adapter sees it.
fn tree(ctx: &Context, app: &mut CalculatorApp, screen: &Screen) -> Tree {
    Tree::new(update(ctx, app, screen), true)
}

/// A node's name as Android shows it: its label (`content-desc`), else its value (`text`).
fn name(node: &Node) -> String {
    node.label()
        .filter(|label| !label.is_empty())
        .or_else(|| node.value())
        .unwrap_or_default()
}

/// The shown node named `name`, if any.
fn find<'a>(nodes: &[Node<'a>], name_of: &str) -> Option<Node<'a>> {
    nodes.iter().find(|node| name(node) == name_of).copied()
}

/// What Marionet and uiautomator need of every node an adapter shows on `page`: finite bounds
/// that aren't empty, a name, at most `MAX_CHILDREN` children, and at most `MAX_NODES` nodes.
fn check_nodes(page: &str, tree: &Tree) {
    let mut nodes = Vec::new();
    shown(&tree.state().root(), &mut nodes);
    // The fewest measured: 4, the Settings page of the 780 × 360 phone where the Regional
    // format card is left out (Linux, macOS; `show_region_card`; CI run 44462040358).
    assert!(nodes.len() >= 4, "{page}: only {} nodes", nodes.len());
    assert!(nodes.len() <= MAX_NODES, "{page}: {} nodes", nodes.len());
    let root = tree.state().root();
    for node in std::iter::once(&root).chain(&nodes) {
        let children = node.filtered_children(common_filter).count();
        assert!(
            children <= MAX_CHILDREN,
            "{page}: {:?} {:?} has {children} children",
            node.role(),
            name(node)
        );
    }
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
            name(node)
        );
        // egui's own scroll bars have no name; nothing taps them by one.
        let scroll_bar = node.role() == Role::ScrollBar;
        assert!(
            scroll_bar || !name(node).trim().is_empty(),
            "{page}: a {:?} node at {bounds:?} has no name",
            node.role()
        );
    }
}

/// A pointer tap at the center of the shown node named `name`.
fn tap(ctx: &Context, app: &mut CalculatorApp, screen: &Screen, name_of: &str) {
    let tree = tree(ctx, app, screen);
    let mut nodes = Vec::new();
    shown(&tree.state().root(), &mut nodes);
    let node = find(&nodes, name_of)
        .unwrap_or_else(|| panic!("{}: no node named {name_of:?}", screen.name));
    let b = node.bounding_box().unwrap();
    let at = pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32);
    tap_at(ctx, app, screen, at);
}

/// A pointer tap at `at`, then a frame to settle.
fn tap_at(ctx: &Context, app: &mut CalculatorApp, screen: &Screen, at: Pos2) {
    let press = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    frame_with(
        ctx,
        app,
        screen,
        vec![egui::Event::PointerMoved(at), press(true)],
    );
    frame_with(ctx, app, screen, vec![press(false)]);
    frame(ctx, app, screen, None);
}

/// A screen reader's or an accessibility service's click (AccessKit's `Click` action, which
/// Android's `ACTION_CLICK` becomes) on the shown node named `name`, then a frame to settle.
fn click(ctx: &Context, app: &mut CalculatorApp, screen: &Screen, name_of: &str) {
    let update = update(ctx, app, screen);
    let tree = Tree::new(update.clone(), true);
    let mut nodes = Vec::new();
    shown(&tree.state().root(), &mut nodes);
    assert!(
        find(&nodes, name_of).is_some(),
        "{}: no node named {name_of:?} in {:?}",
        screen.name,
        nodes.iter().map(name).collect::<Vec<_>>()
    );
    let (id, _) = update
        .nodes
        .iter()
        .find(|(_, node)| node.label().filter(|l| !l.is_empty()).or(node.value()) == Some(name_of))
        .unwrap();
    let request = accesskit::ActionRequest {
        action: accesskit::Action::Click,
        target_tree: accesskit::TreeId::ROOT,
        target_node: *id,
        data: None,
    };
    frame_with(
        ctx,
        app,
        screen,
        vec![egui::Event::AccessKitActionRequest(request)],
    );
    frame(ctx, app, screen, None);
}

/// Every mode on every Android screen size: every node an adapter shows has bounds and a
/// name, and no node has more children than Marionet keeps.
#[test]
fn every_mode_fits_an_accessibility_service() {
    for screen in &SCREENS {
        for mode in super::layout_tests::all_modes() {
            let ctx = Context::default();
            let mut app = settled(&ctx, mode, screen);
            let page = format!("{}: {mode:?}", screen.name);
            check_nodes(&page, &tree(&ctx, &mut app, screen));
        }
        for equations in [false, true] {
            let ctx = Context::default();
            let mut app = graphing(&ctx, screen, equations);
            let page = format!("{}: Graphing, equations {equations}", screen.name);
            check_nodes(&page, &tree(&ctx, &mut app, screen));
        }
        // The navigation menu, and the Settings page it opens.
        let ctx = Context::default();
        let mut app = settled(&ctx, Mode::Standard, screen);
        tap(&ctx, &mut app, screen, "Open Navigation");
        check_nodes(
            &format!("{}: navigation", screen.name),
            &tree(&ctx, &mut app, screen),
        );
        tap(&ctx, &mut app, screen, tr("SettingsHeader.Text"));
        check_nodes(
            &format!("{}: Settings", screen.name),
            &tree(&ctx, &mut app, screen),
        );
    }
}

/// Every popup menu, opened by a tap on its button: its items are a group named as the button,
/// all of them in the tree with their bounds, and the rest of the tree still fits.
#[test]
fn open_popup_menus_list_their_items() {
    let trig = tr("trigButton.AutomationName");
    let functions = tr("funcButton.AutomationName");
    let inequalities = tr("inequalityButton.AutomationName");
    let bitwise = tr("bitwiseButton.AutomationName");
    let bit_shift = tr("bitShiftButton.AutomationName");
    let menus = [
        (Mode::Scientific, trig, 8, tr("sinButton.AutomationName")),
        (
            Mode::Scientific,
            functions,
            6,
            tr("absButton.AutomationName"),
        ),
        (Mode::Graphing, trig, 8, tr("sinButton.AutomationName")),
        (Mode::Graphing, inequalities, 5, ""),
        (Mode::Graphing, functions, 3, tr("absButton.AutomationName")),
        (Mode::Programmer, bitwise, 6, ""),
        (Mode::Programmer, bit_shift, 4, ""),
    ];
    for screen in &SCREENS {
        for (mode, menu, items, item) in menus {
            let ctx = Context::default();
            let mut app = if mode == Mode::Graphing {
                graphing(&ctx, screen, true)
            } else {
                settled(&ctx, mode, screen)
            };
            tap(&ctx, &mut app, screen, menu);
            let page = format!("{}: {mode:?} {menu}", screen.name);
            let tree = tree(&ctx, &mut app, screen);
            check_nodes(&page, &tree);
            let mut nodes = Vec::new();
            shown(&tree.state().root(), &mut nodes);
            let group = nodes
                .iter()
                .find(|node| node.role() == Role::Group && name(node) == menu)
                .unwrap_or_else(|| panic!("{page}: no group"));
            let names: Vec<String> = group
                .filtered_children(common_filter)
                .map(|n| name(&n))
                .collect();
            assert_eq!(names.len(), items, "{page}: {names:?}");
            assert!(
                item.is_empty() || names.iter().any(|n| n == item),
                "{page}: {names:?}"
            );
        }
    }
}

/// The Programmer mode's bit toggling keypad: 64 bits in four groups of 16 under the keypad's
/// group, so that no node has more than 50 children.
#[test]
fn the_bit_toggling_keypad_is_grouped_by_rows() {
    for screen in &SCREENS {
        let ctx = Context::default();
        let mut app = settled(&ctx, Mode::Programmer, screen);
        tap(&ctx, &mut app, screen, tr("bitFlip.AutomationName"));
        let page = format!("{}: bit toggling keypad", screen.name);
        let tree = tree(&ctx, &mut app, screen);
        check_nodes(&page, &tree);
        let mut nodes = Vec::new();
        shown(&tree.state().root(), &mut nodes);
        let panel = nodes
            .iter()
            .find(|n| n.role() == Role::Group && name(n) == tr("bitFlip.AutomationName"))
            .unwrap_or_else(|| panic!("{page}: no group"));
        let rows: Vec<usize> = panel
            .filtered_children(common_filter)
            .map(|row| row.filtered_children(common_filter).count())
            .collect();
        assert_eq!(rows, [16; 4], "{page}");
    }
}

/// The expression line's numbers and operators are nodes of their own under the line's node,
/// in order, with bounds inside the line, and a click on one edits it as a tap does: 12 + 7 × 3
/// is 33, 7 → 9 makes it 39, and + → − then −15 (the device scenario `token-edit`).
#[test]
fn expression_parts_are_nodes_that_a_click_edits() {
    let screen = &SCREENS[2];
    let ctx = Context::default();
    let mut app = settled(&ctx, Mode::Scientific, screen);
    for key in [
        "One",
        "Two",
        "Plus",
        "Seven",
        "Multiply by",
        "Three",
        "Equals",
    ] {
        click(&ctx, &mut app, screen, key);
    }
    assert_eq!(app.scientific.calc.display(), "33");
    let parts = |app: &mut CalculatorApp| {
        let tree = tree(&ctx, app, screen);
        let mut nodes = Vec::new();
        shown(&tree.state().root(), &mut nodes);
        let line = nodes
            .iter()
            .find(|n| n.role() == Role::Label && name(n).starts_with("Expression is"))
            .expect("the expression line");
        let line_box = line.bounding_box().unwrap();
        line.filtered_children(common_filter)
            .map(|part| {
                let b = part.bounding_box().unwrap();
                assert!(b.width() > 0.0 && b.height() > 0.0, "{b:?}");
                assert!(
                    b.x0 >= line_box.x0 - 1.0 && b.x1 <= line_box.x1 + 1.0,
                    "{b:?} outside {line_box:?}"
                );
                name(&part)
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        parts(&mut app),
        [
            "Number 12",
            "Operator +",
            "Number 7",
            "Operator ×",
            "Number 3"
        ]
    );
    click(&ctx, &mut app, screen, "Number 7");
    assert!(app.scientific.calc.number_edit().is_some());
    for key in ["Backspace", "Nine", "Equals"] {
        click(&ctx, &mut app, screen, key);
    }
    assert_eq!(app.scientific.calc.display(), "39");
    click(&ctx, &mut app, screen, "Operator +");
    assert!(app.scientific.calc.selected_operator().is_some());
    click(&ctx, &mut app, screen, "Minus");
    click(&ctx, &mut app, screen, "Equals");
    assert_eq!(app.scientific.calc.display(), "-15");
}

/// Marionet's path to a part of the expression line: its node says `clickable="false"`, so it
/// taps the centre of the node's bounds (rounded to the phone's pixels) instead of clicking it.
/// A tap anywhere on the one-digit `Number 7` puts the caret after the 7, so ⌫ and 9 make it
/// 9 and the result 39 (on the Pixel the caret went before it: `97`). A longer number keeps the
/// nearest digit boundary. docs/research/editable-expression.md, "Where a tap puts the caret".
#[test]
fn a_tap_on_a_one_digit_number_puts_the_caret_after_it() {
    // The Pixel's 2.625 physical pixels a point: Marionet's integer pixels, back in points.
    let pixel = |v: f64| ((v * 2.625).floor() / 2.625) as f32;
    for screen in &SCREENS {
        for (where_, fraction) in [
            ("left", 0.1),
            ("centre", 0.5),
            ("pixel centre", -1.0),
            ("right", 0.9),
        ] {
            let ctx = Context::default();
            let mut app = settled(&ctx, Mode::Scientific, screen);
            for key in [
                "One",
                "Two",
                "Plus",
                "Seven",
                "Multiply by",
                "Three",
                "Equals",
            ] {
                click(&ctx, &mut app, screen, key);
            }
            let page = format!("{}: a tap at the {where_} of Number 7", screen.name);
            let node_box = |app: &mut CalculatorApp, name_of: &str| {
                let tree = tree(&ctx, app, screen);
                let mut nodes = Vec::new();
                shown(&tree.state().root(), &mut nodes);
                find(&nodes, name_of)
                    .unwrap_or_else(|| panic!("{page}: no {name_of:?}"))
                    .bounding_box()
                    .unwrap()
            };
            let b = node_box(&mut app, "Number 7");
            let y = ((b.y0 + b.y1) / 2.0) as f32;
            let x = if fraction < 0.0 {
                pixel((b.x0 + b.x1) / 2.0)
            } else {
                (b.x0 + (b.x1 - b.x0) * fraction) as f32
            };
            tap_at(&ctx, &mut app, screen, pos2(x, y));
            let edit = app.scientific.calc.number_edit().expect(&page);
            assert_eq!(edit.text, "7", "{page}");
            assert_eq!(edit.caret.index, 1, "{page}: the caret is before the 7");
            for key in ["Backspace", "Nine"] {
                click(&ctx, &mut app, screen, key);
            }
            assert!(
                names(&ctx, &mut app, screen).iter().any(|n| n
                    .starts_with("Expression is 12 + 9 × 3 =")
                    && n.ends_with("Editing 9")),
                "{page}: {:?}",
                names(&ctx, &mut app, screen)
            );
            click(&ctx, &mut app, screen, "Equals");
            assert_eq!(app.scientific.calc.display(), "39", "{page}");
        }
        // A longer number keeps the nearest boundary: its left edge is before the 1 of 12.
        let ctx = Context::default();
        let mut app = settled(&ctx, Mode::Scientific, screen);
        for key in ["One", "Two", "Plus", "Seven", "Equals"] {
            click(&ctx, &mut app, screen, key);
        }
        let tree = tree(&ctx, &mut app, screen);
        let mut nodes = Vec::new();
        shown(&tree.state().root(), &mut nodes);
        let b = find(&nodes, "Number 12").unwrap().bounding_box().unwrap();
        let at = pos2((b.x0 + 1.0) as f32, ((b.y0 + b.y1) / 2.0) as f32);
        tap_at(&ctx, &mut app, screen, at);
        let edit = app.scientific.calc.number_edit().unwrap();
        assert_eq!(
            (edit.text.as_str(), edit.caret.index),
            ("12", 0),
            "{}",
            screen.name
        );
        // A click on its node puts the caret at its end.
        click(&ctx, &mut app, screen, "Number 12");
        let edit = app.scientific.calc.number_edit().unwrap();
        assert_eq!(edit.caret.index, 2, "{}", screen.name);
    }
}

// ------------------------------------------------------------------ device scenarios

/// A JSON value, as far as the device scenarios (`tests/device/scenarios/*.json`) need one.
#[derive(Debug, Clone, PartialEq)]
enum Json {
    Null,
    Bool(bool),
    Number(f64),
    Text(String),
    List(Vec<Json>),
    Object(Vec<(String, Json)>),
}

/// Reads JSON: the scenarios' little of it, without a dependency for the tests.
struct JsonReader<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
}

impl JsonReader<'_> {
    fn parse(text: &str) -> Result<Json, String> {
        let mut reader = JsonReader {
            chars: text.chars().peekable(),
        };
        let value = reader.value()?;
        reader.space();
        match reader.chars.next() {
            None => Ok(value),
            Some(c) => Err(format!("unexpected {c:?} after the value")),
        }
    }

    fn space(&mut self) {
        while self.chars.next_if(|c| c.is_whitespace()).is_some() {}
    }

    fn expect(&mut self, wanted: char) -> Result<(), String> {
        self.space();
        match self.chars.next() {
            Some(c) if c == wanted => Ok(()),
            other => Err(format!("expected {wanted:?}, found {other:?}")),
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        self.space();
        match self.chars.peek().copied() {
            Some('{') => {
                self.chars.next();
                let mut fields = Vec::new();
                self.space();
                if self.chars.next_if_eq(&'}').is_some() {
                    return Ok(Json::Object(fields));
                }
                loop {
                    self.space();
                    let Json::Text(key) = self.value()? else {
                        return Err("an object key isn't a string".into());
                    };
                    self.expect(':')?;
                    fields.push((key, self.value()?));
                    self.space();
                    match self.chars.next() {
                        Some(',') => continue,
                        Some('}') => return Ok(Json::Object(fields)),
                        other => {
                            return Err(format!("expected , or }} in an object, found {other:?}"));
                        }
                    }
                }
            }
            Some('[') => {
                self.chars.next();
                let mut items = Vec::new();
                self.space();
                if self.chars.next_if_eq(&']').is_some() {
                    return Ok(Json::List(items));
                }
                loop {
                    items.push(self.value()?);
                    self.space();
                    match self.chars.next() {
                        Some(',') => continue,
                        Some(']') => return Ok(Json::List(items)),
                        other => return Err(format!("expected , or ] in a list, found {other:?}")),
                    }
                }
            }
            Some('"') => {
                self.chars.next();
                let mut text = String::new();
                loop {
                    match self.chars.next().ok_or("an unterminated string")? {
                        '"' => return Ok(Json::Text(text)),
                        '\\' => match self.chars.next().ok_or("an unterminated escape")? {
                            'n' => text.push('\n'),
                            't' => text.push('\t'),
                            'u' => {
                                let hex: String =
                                    (0..4).filter_map(|_| self.chars.next()).collect();
                                let code =
                                    u32::from_str_radix(&hex, 16).map_err(|e| e.to_string())?;
                                text.push(char::from_u32(code).ok_or("a surrogate escape")?);
                            }
                            c => text.push(c),
                        },
                        c => text.push(c),
                    }
                }
            }
            Some(c) if c == '-' || c.is_ascii_digit() => {
                let mut number = String::new();
                while let Some(c) = self
                    .chars
                    .next_if(|c| "+-.eE".contains(*c) || c.is_ascii_digit())
                {
                    number.push(c);
                }
                number
                    .parse()
                    .map(Json::Number)
                    .map_err(|e| format!("{number}: {e}"))
            }
            Some(_) => {
                let mut word = String::new();
                while let Some(c) = self.chars.next_if(|c| c.is_ascii_alphabetic()) {
                    word.push(c);
                }
                match word.as_str() {
                    "true" => Ok(Json::Bool(true)),
                    "false" => Ok(Json::Bool(false)),
                    "null" => Ok(Json::Null),
                    _ => Err(format!("unexpected {word:?}")),
                }
            }
            None => Err("the text ends before the value".into()),
        }
    }
}

impl Json {
    fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(fields) => fields.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    fn text(&self) -> Option<&str> {
        match self {
            Json::Text(text) => Some(text),
            _ => None,
        }
    }
}

/// A displayed number or expression without its digit groups and decimal separator, so that
/// the scenarios' en-US numbers match the phone's regional format (`1,000`, `1’000`, `6,25`):
/// what `tests/device/run.ps1` and the README's agent compare too.
fn loose(text: &str) -> String {
    text.chars()
        .filter(|c| !",.'’ \u{a0}\u{202f}".contains(*c))
        .collect()
}

/// The names of the shown nodes.
fn names(ctx: &Context, app: &mut CalculatorApp, screen: &Screen) -> Vec<String> {
    let tree = tree(ctx, app, screen);
    let mut nodes = Vec::new();
    shown(&tree.state().root(), &mut nodes);
    nodes.iter().map(name).collect()
}

/// The navigation list's swipe of an `open` step whose item isn't shown: a finger drawn up from
/// the center of the lowest item shown (named `… Calculator` or `… Converter`) to the center of
/// the highest, as the device runners do. Returns whether there was a list to swipe.
fn swipe_navigation(ctx: &Context, app: &mut CalculatorApp, screen: &Screen) -> bool {
    let tree = tree(ctx, app, screen);
    let mut nodes = Vec::new();
    shown(&tree.state().root(), &mut nodes);
    let centers: Vec<Pos2> = nodes
        .iter()
        .filter(|n| {
            let name = name(n);
            name.ends_with(" Calculator") || name.ends_with(" Converter")
        })
        .filter_map(|n| n.bounding_box())
        .map(|b| pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32))
        .collect();
    let (Some(from), Some(to)) = (
        centers.iter().copied().max_by(|a, b| a.y.total_cmp(&b.y)),
        centers.iter().copied().min_by(|a, b| a.y.total_cmp(&b.y)),
    ) else {
        return false;
    };
    // The phone gets a finger's drag; here the wheel scrolls the list as far, over the list
    // (egui's drag-to-scroll needs a pointer's timing that these frames don't have).
    let wheel = egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: vec2(0.0, to.y - from.y),
        phase: egui::TouchPhase::Move,
        modifiers: Default::default(),
    };
    frame_with(
        ctx,
        app,
        screen,
        vec![egui::Event::PointerMoved(from), wheel],
    );
    for _ in 0..3 {
        frame(ctx, app, screen, None);
    }
    true
}

/// Runs one scenario on `screen` as the device runners do: by the nodes' names, with pointer
/// taps at their centers. Returns the first failed step.
fn run_scenario(scenario: &Json, screen: &Screen) -> Result<(), String> {
    let steps = match scenario.get("steps") {
        Some(Json::List(steps)) => steps,
        _ => return Err("no steps".into()),
    };
    let ctx = Context::default();
    let mut app = settled(&ctx, Mode::Standard, screen);
    // The app's edition for `edition` steps: the free Play app, or one with every colour theme
    // unlocked (CalcCraft Pro, and the open-source default build).
    let edition = if crate::channel::CHANNEL == crate::channel::Channel::PlayFree {
        "free"
    } else {
        "pro"
    };
    let mut skipping = false;
    for (number, step) in steps.iter().enumerate() {
        let Json::Object(fields) = step else {
            return Err(format!("step {}: not an object", number + 1));
        };
        let [(op, arg)] = fields.as_slice() else {
            return Err(format!("step {}: not one key", number + 1));
        };
        // `edition`: the steps up to the next one run only in that edition ("any": in all).
        if op == "edition" {
            let wanted = arg.text().unwrap_or_default();
            skipping = wanted != "any" && wanted != edition;
            continue;
        }
        if skipping {
            continue;
        }
        let fail = |what: String| Err(format!("step {} ({op}): {what}", number + 1));
        let present = |ctx: &Context, app: &mut CalculatorApp, name: &str| {
            names(ctx, app, screen).iter().any(|n| n == name)
        };
        let tap_named = |ctx: &Context, app: &mut CalculatorApp, name: &str| {
            if present(ctx, app, name) {
                tap(ctx, app, screen, name);
                Ok(())
            } else {
                Err(format!("no node named {name:?}"))
            }
        };
        let text = arg.text().unwrap_or_default();
        let shown = names(&ctx, &mut app, screen);
        let result = match op.as_str() {
            // A fresh start (the device runners stop the app and start it again).
            "launch" => {
                app = settled(&ctx, Mode::Standard, screen);
                Ok(())
            }
            "open" => tap_named(&ctx, &mut app, "Open Navigation").and_then(|()| {
                for _ in 0..4 {
                    if present(&ctx, &mut app, text) || !swipe_navigation(&ctx, &mut app, screen) {
                        break;
                    }
                }
                tap_named(&ctx, &mut app, text)
            }),
            "tap" => tap_named(&ctx, &mut app, text),
            // Scrolls the page up until the node shows (Settings on a low screen): the phone
            // gets a finger drawn up the middle of the screen, here the wheel scrolls as far.
            "scroll_to" => {
                let center = screen.safe_rect().center();
                for _ in 0..6 {
                    if present(&ctx, &mut app, text) {
                        break;
                    }
                    let wheel = egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: vec2(0.0, -0.35 * screen.safe_rect().height()),
                        phase: egui::TouchPhase::Move,
                        modifiers: Default::default(),
                    };
                    frame_with(
                        &ctx,
                        &mut app,
                        screen,
                        vec![egui::Event::PointerMoved(center), wheel],
                    );
                    for _ in 0..3 {
                        frame(&ctx, &mut app, screen, None);
                    }
                }
                present(&ctx, &mut app, text)
                    .then_some(())
                    .ok_or_else(|| format!("no node named {text:?} after scrolling"))
            }
            // A checked radio button or switch (uiautomator's `checked="true"`).
            "expect_checked" => {
                let tree = tree(&ctx, &mut app, screen);
                let mut nodes = Vec::new();
                self::shown(&tree.state().root(), &mut nodes);
                match find(&nodes, text) {
                    Some(node) if node.toggled() == Some(accesskit::Toggled::True) => Ok(()),
                    Some(node) => Err(format!("{text:?} is {:?}, not checked", node.toggled())),
                    None => Err(format!("no node named {text:?}")),
                }
            }
            "taps" => match arg {
                Json::List(keys) => keys
                    .iter()
                    .try_for_each(|key| tap_named(&ctx, &mut app, key.text().unwrap_or_default())),
                _ => Err("not a list".into()),
            },
            "tap_if_present" => {
                if present(&ctx, &mut app, text) {
                    tap(&ctx, &mut app, screen, text);
                }
                Ok(())
            }
            "expect" => shown
                .iter()
                .any(|n| n == text)
                .then_some(())
                .ok_or_else(|| format!("no node named {text:?}")),
            "expect_absent" => (!shown.iter().any(|n| n == text))
                .then_some(())
                .ok_or_else(|| format!("a node named {text:?}")),
            "expect_prefix" => shown
                .iter()
                .any(|n| n.starts_with(text))
                .then_some(())
                .ok_or_else(|| format!("no node named {text:?}…")),
            "expect_contains" => shown
                .iter()
                .any(|n| n.contains(text))
                .then_some(())
                .ok_or_else(|| format!("no node named …{text:?}…")),
            "display" => {
                let wanted = loose(&format!("Display is {text}"));
                shown
                    .iter()
                    .any(|n| loose(n) == wanted)
                    .then_some(())
                    .ok_or_else(|| format!("no \"Display is {text}\""))
            }
            "expression" => {
                let wanted = loose(&format!("Expression is {text}"));
                shown
                    .iter()
                    .any(|n| loose(n).starts_with(&wanted))
                    .then_some(())
                    .ok_or_else(|| format!("no \"Expression is {text}\""))
            }
            _ => Err("an unknown step".into()),
        };
        if let Err(what) = result {
            let shown = names(&ctx, &mut app, screen);
            return fail(format!("{what}; the screen has {shown:?}"));
        }
    }
    Ok(())
}

/// The device scenarios (`tests/device/scenarios/*.json`) pass on the host, on every Android
/// screen size, through the same accessibility tree the phone gets: their names and expected
/// values are right before anyone takes them to a phone.
#[test]
fn device_scenarios_pass_on_the_host() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/device/scenarios");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|e| e == "json"))
        .collect();
    files.sort();
    assert!(files.len() >= 6, "{files:?}");
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        let scenario =
            JsonReader::parse(&text).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
        let stem = file.file_stem().unwrap().to_string_lossy().into_owned();
        assert_eq!(
            scenario.get("name").and_then(Json::text),
            Some(stem.as_str())
        );
        for screen in &SCREENS {
            if let Err(what) = run_scenario(&scenario, screen) {
                panic!("{stem} on {}: {what}", screen.name);
            }
        }
    }
}

#[test]
fn json_reader_reads_the_scenarios_json() {
    let value = JsonReader::parse(r#" { "a": [1, -2.5e1, "x\"×", true, null], "b": {} } "#);
    assert_eq!(
        value,
        Ok(Json::Object(vec![
            (
                "a".into(),
                Json::List(vec![
                    Json::Number(1.0),
                    Json::Number(-25.0),
                    Json::Text("x\"×".into()),
                    Json::Bool(true),
                    Json::Null
                ])
            ),
            ("b".into(), Json::Object(vec![])),
        ]))
    );
    assert!(JsonReader::parse("[1, 2").is_err());
    assert!(JsonReader::parse("{} x").is_err());
    assert_eq!(loose("Display is 1,000.5"), loose("Display is 1’000,5"));
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
