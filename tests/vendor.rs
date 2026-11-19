//! The copy of accesskit_winit in `vendor/` (android/README.md, "Accessibility") must be the
//! one Cargo builds. When eframe moves to a newer accesskit_winit, Cargo only warns that the
//! `[patch.crates-io]` entry is unused and builds the release instead, and the Android app
//! would silently lose its accessibility tree: the copy has to be re-applied to the new
//! release first. These tests make that a failure.

use accesskit::{Affine, Node, NodeId, Rect, Role, Tree, TreeId, TreeUpdate};

// The Android adapter's root bounds, which build only for Android in the crate itself.
#[path = "../vendor/accesskit_winit/src/platform_impl/root_bounds.rs"]
mod root_bounds;

const LOCK: &str = include_str!("../Cargo.lock");
const MANIFEST: &str = include_str!("../vendor/accesskit_winit/Cargo.toml");
const ANDROID: &str = include_str!("../vendor/accesskit_winit/src/platform_impl/android.rs");

/// The `[[package]]` entry of `name` in the lockfile `lock`. Lines are trimmed: a Windows
/// checkout may have turned the line ends into CRLF.
fn locked_package<'a>(lock: &'a str, name: &str) -> &'a str {
    let header = format!("name = \"{name}\"");
    lock.split("[[package]]")
        .find(|entry| {
            entry.lines().map(str::trim).find(|line| !line.is_empty()) == Some(header.as_str())
        })
        .unwrap_or_else(|| panic!("Cargo.lock has no package {name}"))
}

/// The value of the first `version = "…"` line in `text`.
fn version(text: &str) -> &str {
    text.lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix("version = \""))
        .and_then(|rest| rest.strip_suffix('"'))
        .expect("a version line")
}

#[test]
fn vendored_accesskit_winit_matches_the_lockfile() {
    let locked = locked_package(LOCK, "accesskit_winit");
    // A crate taken from `[patch]` has no `source`; one from crates.io has
    // `source = "registry+…"`, and then the copy in vendor/ isn't what is built.
    assert!(
        !locked.contains("source = "),
        "Cargo builds accesskit_winit from crates.io, not from vendor/accesskit_winit: \
         re-apply the copy to the version egui-winit asks for\n{locked}"
    );
    assert_eq!(version(locked), version(MANIFEST));
}

#[test]
fn lockfile_with_crlf_line_ends_is_read_too() {
    // CI's Windows checkout converts the line ends (core.autocrlf).
    let crlf = LOCK.replace('\n', "\r\n");
    let locked = locked_package(&crlf, "accesskit_winit");
    assert_eq!(version(locked), version(MANIFEST));
    // A crates.io entry, for the shape of a `source` line.
    assert!(locked_package(&crlf, "accesskit_android").contains("source = \"registry+"));
}

#[test]
fn vendored_android_adapter_names_its_upstream_version() {
    let upstream = format!("accesskit_winit {}", version(MANIFEST));
    assert!(
        ANDROID.contains(&upstream),
        "android.rs's header must name the release it was changed from ({upstream})"
    );
    // The two changes the header promises.
    assert!(ANDROID.contains("android/R$id"), "hooks the content view");
    assert!(
        ANDROID.contains("const FIRST_SDK: i32 = 26;"),
        "skips Android before 8.0"
    );
    assert!(
        ANDROID.contains("fill_root_bounds(&mut update"),
        "fills the root's bounds"
    );
    assert!(
        ANDROID.contains("the accessibility delegate goes on"),
        "logs the view the delegate goes on"
    );
}

/// An update like egui's: a root window scaled by `pixels_per_point`, with a key in points.
fn egui_update(pixels_per_point: f64) -> TreeUpdate {
    let mut root = Node::new(Role::Window);
    root.set_transform(Affine::scale(pixels_per_point));
    root.push_child(NodeId(2));
    let mut key = Node::new(Role::Button);
    key.set_bounds(Rect {
        x0: 10.0,
        y0: 20.0,
        x1: 50.0,
        y1: 60.0,
    });
    TreeUpdate {
        nodes: vec![(NodeId(1), root), (NodeId(2), key)],
        tree: Some(Tree::new(NodeId(1))),
        tree_id: TreeId::ROOT,
        focus: NodeId(1),
    }
}

fn bounds(update: &TreeUpdate, id: u64) -> Option<Rect> {
    let (_, node) = update.nodes.iter().find(|(node, _)| *node == NodeId(id))?;
    node.bounds()
}

#[test]
fn the_root_gets_the_window_in_points() {
    // A Pixel 7 Pro: 1440 × 3120 pixels at 3.5 pixels per point.
    let mut update = egui_update(3.5);
    root_bounds::fill_root_bounds(&mut update, 1440.0, 3120.0);
    let root = bounds(&update, 1).expect("the root has bounds");
    assert_eq!((root.x0, root.y0), (0.0, 0.0));
    assert!((root.x1 - 1440.0 / 3.5).abs() < 1e-9);
    assert!((root.y1 - 3120.0 / 3.5).abs() < 1e-9);
    // On the screen (through the root's transform) it's the whole window.
    let (_, node) = &update.nodes[0];
    let screen = node.transform().unwrap().transform_rect_bbox(root);
    assert!((screen.x1 - 1440.0).abs() < 1e-9 && (screen.y1 - 3120.0).abs() < 1e-9);
    // The other nodes keep theirs.
    assert_eq!(bounds(&update, 2).unwrap().x1, 50.0);
}

#[test]
fn root_bounds_given_are_kept() {
    let mut update = egui_update(2.0);
    let given = Rect {
        x0: 1.0,
        y0: 2.0,
        x1: 3.0,
        y1: 4.0,
    };
    update.nodes[0].1.set_bounds(given);
    root_bounds::fill_root_bounds(&mut update, 1000.0, 2000.0);
    assert_eq!(bounds(&update, 1), Some(given));
}

#[test]
fn no_tree_or_no_window_changes_nothing() {
    // An update without `tree` doesn't name the root.
    let mut update = egui_update(2.0);
    update.tree = None;
    root_bounds::fill_root_bounds(&mut update, 1000.0, 2000.0);
    assert_eq!(bounds(&update, 1), None);
    // A window that has no size yet.
    let mut update = egui_update(2.0);
    root_bounds::fill_root_bounds(&mut update, 0.0, 2000.0);
    assert_eq!(bounds(&update, 1), None);
}
