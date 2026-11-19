// Copyright 2026 Rapalon Labs. Licensed under the Apache License, Version 2.0 (found in the
// LICENSE-APACHE file), like the rest of this copy of accesskit_winit.

// CalcCraft's addition to the Android adapter (see android.rs's header). accesskit_consumer
// panics on a tree update whose focus isn't a node of the tree after the update ("Focused ID
// … is not in the node list", tree.rs's `validate_global`), and that panic ended the Android
// app: egui named a widget that was focused in the frame before it was drawn (CalcCraft's
// app::access::settle_focus has the details and the fix at the source). This guard checks
// every update before the consumer gets it, and makes such a focus the root's.
//
// Its own file so that the tests in CalcCraft's tests/vendor.rs can include it on every
// platform.

use accesskit::{NodeId, TreeId, TreeUpdate};
use std::collections::{HashMap, HashSet};

/// The tree's shape as the consumer will have it: the root and every node's children.
#[derive(Debug, Default)]
pub(crate) struct FocusGuard {
    root: Option<NodeId>,
    children: HashMap<NodeId, Vec<NodeId>>,
    /// Whether the warning has been logged (once a run).
    warned: bool,
}

impl FocusGuard {
    /// Makes `update`'s focus the root if it won't be a node of the tree once the update is
    /// applied, as accesskit_consumer requires; returns the focus it replaced. Logs that at
    /// Warn the first time.
    pub(crate) fn check(&mut self, update: &mut TreeUpdate) -> Option<NodeId> {
        if update.tree_id != TreeId::ROOT {
            // Subtrees (grafts): egui has none, and their focus is checked another way.
            return None;
        }
        if let Some(tree) = &update.tree {
            self.root = Some(tree.root);
        }
        for (id, node) in &update.nodes {
            self.children.insert(*id, node.children().to_vec());
        }
        let root = self.root?;
        // The consumer drops what the root no longer reaches.
        let mut reachable = HashSet::new();
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            // Only nodes the tree has (a child id without a node is invalid in another way).
            let Some(children) = self.children.get(&id) else {
                continue;
            };
            if reachable.insert(id) {
                stack.extend(children.iter().copied());
            }
        }
        self.children.retain(|id, _| reachable.contains(id));
        if !reachable.contains(&root) || reachable.contains(&update.focus) {
            // Without its root the update is invalid in other ways, which this guard leaves be.
            return None;
        }
        let missing = update.focus;
        update.focus = root;
        if !self.warned {
            self.warned = true;
            log::warn!(
                "AccessKit: the tree update's focus {missing:?} isn't in the tree; the root \
                 has it instead (logged once)"
            );
        }
        Some(missing)
    }
}
