// Copyright 2026 Rapalon Labs. Licensed under the Apache License, Version 2.0 (found in the
// LICENSE-APACHE file), like the rest of this copy of accesskit_winit.

// CalcCraft's addition to the Android adapter (see android.rs's header). egui's root node, the
// window, has no bounds: only its scale transform. accesskit_android then gives the host view's
// node an empty rectangle ([0,0][0,0]), and an accessibility service that prunes empty nodes
// (Marionet does) drops the whole tree under it, every key and the display. The root gets the
// window's rectangle instead, which the host view (NativeActivity's content view) covers.
//
// Its own file so that the tests in CalcCraft's tests/vendor.rs can include it on every
// platform.

use accesskit::{Rect, TreeUpdate};

/// Gives the update's root node the window's rectangle (`width` × `height` physical pixels from
/// the host view's top left corner), in the root's own coordinates, if it has no bounds.
pub(crate) fn fill_root_bounds(update: &mut TreeUpdate, width: f64, height: f64) {
    let Some(root) = update.tree.as_ref().map(|tree| tree.root) else {
        return;
    };
    if width <= 0.0 || height <= 0.0 {
        return;
    }
    let window = Rect {
        x0: 0.0,
        y0: 0.0,
        x1: width,
        y1: height,
    };
    for (id, node) in &mut update.nodes {
        if *id == root && node.bounds().is_none() {
            // egui scales the root by its pixels per point: its children's bounds are in points.
            let bounds = node.transform().map_or(window, |transform| {
                transform.inverse().transform_rect_bbox(window)
            });
            node.set_bounds(bounds);
        }
    }
}
