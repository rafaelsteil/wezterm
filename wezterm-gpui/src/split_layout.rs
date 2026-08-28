//! Binary pane tree for one GPUI tab.
//!
//! Layout is GPUI-side (not mux `Tab::split_and_insert`). Each leaf is an
//! independent `LocalDomain::spawn_pane`. Indices into `panes` stay stable
//! until a prune remaps them.

/// Left/right (`Horizontal`) or top/bottom (`Vertical`). Matches mux
/// `SplitDirection` and wezterm-gui `SplitHorizontal` / `SplitVertical`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitAxis {
    Horizontal,
    Vertical,
}

/// Palette `ActivatePaneDirection.*` (mux `PaneDirection` without Next/Prev).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneDir {
    Left,
    Right,
    Up,
    Down,
}

impl PaneDir {
    pub fn from_palette_suffix(s: &str) -> Option<Self> {
        match s {
            "Left" => Some(Self::Left),
            "Right" => Some(Self::Right),
            "Up" => Some(Self::Up),
            "Down" => Some(Self::Down),
            _ => None,
        }
    }

    /// Parent split axis mux walks for AdjustPaneSize / ActivatePaneDirection.
    pub fn split_axis(self) -> SplitAxis {
        match self {
            Self::Left | Self::Right => SplitAxis::Horizontal,
            Self::Up | Self::Down => SplitAxis::Vertical,
        }
    }

    /// Mux always mutates the first child: Left/Up shrinks it, Right/Down grows it.
    pub fn first_child_delta_sign(self) -> f32 {
        match self {
            Self::Left | Self::Up => -1.0,
            Self::Right | Self::Down => 1.0,
        }
    }
}

/// Unit-rect used only to pick a neighbor. Equal halves, not the live
/// gpui-component divider (AdjustPaneSize uses ResizableState).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LeafRect {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutNode {
    Leaf(usize),
    Split {
        axis: SplitAxis,
        id: u64,
        first: Box<LayoutNode>,
        second: Box<LayoutNode>,
    },
}

impl LayoutNode {
    pub fn leaf(index: usize) -> Self {
        Self::Leaf(index)
    }

    fn collect_leaves(&self, out: &mut Vec<usize>) {
        match self {
            Self::Leaf(i) => out.push(*i),
            Self::Split { first, second, .. } => {
                first.collect_leaves(out);
                second.collect_leaves(out);
            }
        }
    }

    pub fn collect_split_ids(&self, out: &mut Vec<u64>) {
        if let Self::Split {
            id,
            first,
            second,
            ..
        } = self
        {
            out.push(*id);
            first.collect_split_ids(out);
            second.collect_split_ids(out);
        }
    }

    fn contains_leaf(&self, leaf: usize) -> bool {
        match self {
            Self::Leaf(i) => *i == leaf,
            Self::Split { first, second, .. } => {
                first.contains_leaf(leaf) || second.contains_leaf(leaf)
            }
        }
    }

    /// Nearest ancestor split of `leaf` whose axis matches (mux walk-up).
    pub fn ancestor_split(&self, leaf: usize, axis: SplitAxis) -> Option<u64> {
        match self {
            Self::Leaf(_) => None,
            Self::Split {
                axis: a,
                id,
                first,
                second,
            } => {
                let in_first = first.contains_leaf(leaf);
                let in_second = second.contains_leaf(leaf);
                if !in_first && !in_second {
                    return None;
                }
                let deeper = if in_first {
                    first.ancestor_split(leaf, axis)
                } else {
                    second.ancestor_split(leaf, axis)
                };
                deeper.or_else(|| (*a == axis).then_some(*id))
            }
        }
    }

    fn collect_leaf_rects(&self, r: LeafRect, out: &mut Vec<(usize, LeafRect)>) {
        match self {
            Self::Leaf(i) => out.push((*i, r)),
            Self::Split {
                axis: SplitAxis::Horizontal,
                first,
                second,
                ..
            } => {
                let left_w = r.w / 2;
                first.collect_leaf_rects(LeafRect { w: left_w, ..r }, out);
                second.collect_leaf_rects(
                    LeafRect {
                        x: r.x + left_w,
                        w: r.w - left_w,
                        ..r
                    },
                    out,
                );
            }
            Self::Split {
                axis: SplitAxis::Vertical,
                first,
                second,
                ..
            } => {
                let top_h = r.h / 2;
                first.collect_leaf_rects(LeafRect { h: top_h, ..r }, out);
                second.collect_leaf_rects(
                    LeafRect {
                        y: r.y + top_h,
                        h: r.h - top_h,
                        ..r
                    },
                    out,
                );
            }
        }
    }

    /// Replace the leaf `target` with a split: old pane stays first (left/top),
    /// `new_leaf` is second (right/bottom), same as mux `target_is_second`.
    pub fn split_leaf(
        &mut self,
        target: usize,
        axis: SplitAxis,
        new_leaf: usize,
        split_id: u64,
    ) -> bool {
        match self {
            Self::Leaf(i) if *i == target => {
                *self = Self::Split {
                    axis,
                    id: split_id,
                    first: Box::new(Self::Leaf(target)),
                    second: Box::new(Self::Leaf(new_leaf)),
                };
                true
            }
            Self::Split { first, second, .. } => {
                first.split_leaf(target, axis, new_leaf, split_id)
                    || second.split_leaf(target, axis, new_leaf, split_id)
            }
            _ => false,
        }
    }

    /// Drop `target`. A split whose child disappeared collapses to the sibling.
    pub fn prune(self, target: usize) -> Option<Self> {
        match self {
            Self::Leaf(i) if i == target => None,
            Self::Leaf(i) => Some(Self::Leaf(i)),
            Self::Split {
                axis,
                id,
                first,
                second,
            } => match (first.prune(target), second.prune(target)) {
                (None, Some(s)) => Some(s),
                (Some(f), None) => Some(f),
                (Some(f), Some(s)) => Some(Self::Split {
                    axis,
                    id,
                    first: Box::new(f),
                    second: Box::new(s),
                }),
                (None, None) => None,
            },
        }
    }

    /// After removing `removed` from the pane vec, decrement later indices.
    pub fn remap_after_remove(&mut self, removed: usize) {
        match self {
            Self::Leaf(i) if *i > removed => *i -= 1,
            Self::Leaf(_) => {}
            Self::Split { first, second, .. } => {
                first.remap_after_remove(removed);
                second.remap_after_remove(removed);
            }
        }
    }
}

/// One tab's panes + which leaf has keys / copy / paste.
pub struct SplitLayout<T> {
    panes: Vec<T>,
    root: LayoutNode,
    active: usize,
    /// Palette TogglePaneZoomState: paint only the active leaf.
    zoomed: bool,
}

impl<T> SplitLayout<T> {
    pub fn leaf(pane: T) -> Self {
        Self {
            panes: vec![pane],
            root: LayoutNode::leaf(0),
            active: 0,
            zoomed: false,
        }
    }

    pub fn panes(&self) -> &[T] {
        &self.panes
    }

    pub fn pane_count(&self) -> usize {
        self.panes.len()
    }

    pub fn root(&self) -> &LayoutNode {
        &self.root
    }

    pub fn active_index(&self) -> usize {
        self.active.min(self.panes.len().saturating_sub(1))
    }

    pub fn active_pane(&self) -> Option<&T> {
        self.panes.get(self.active_index())
    }

    pub fn is_zoomed(&self) -> bool {
        self.zoomed
    }

    pub fn toggle_zoom(&mut self) {
        if self.panes.len() < 2 {
            self.zoomed = false;
            return;
        }
        self.zoomed = !self.zoomed;
    }

    pub fn unzoom(&mut self) {
        self.zoomed = false;
    }

    /// Neighbor in `dir` by equal-half geometry (mux largest shared edge).
    /// `None` if already at that edge.
    pub fn neighbor_in(&self, dir: PaneDir) -> Option<usize> {
        let mut rects = Vec::new();
        self.root.collect_leaf_rects(
            LeafRect {
                x: 0,
                y: 0,
                w: 1_048_576,
                h: 1_048_576,
            },
            &mut rects,
        );
        let active = self.active_index();
        let Some((_, ar)) = rects.iter().find(|(i, _)| *i == active) else {
            return None;
        };
        let ar = *ar;
        let mut best: Option<(u32, usize)> = None;
        for (i, r) in &rects {
            if *i == active {
                continue;
            }
            let adjacent = match dir {
                PaneDir::Right => r.x == ar.x + ar.w,
                PaneDir::Left => r.x + r.w == ar.x,
                PaneDir::Down => r.y == ar.y + ar.h,
                PaneDir::Up => r.y + r.h == ar.y,
            };
            if !adjacent {
                continue;
            }
            let overlap = match dir {
                PaneDir::Left | PaneDir::Right => overlap_len(ar.y, ar.h, r.y, r.h),
                PaneDir::Up | PaneDir::Down => overlap_len(ar.x, ar.w, r.x, r.w),
            };
            if overlap == 0 {
                continue;
            }
            match best {
                None => best = Some((overlap, *i)),
                Some((best_overlap, best_i)) => {
                    if overlap > best_overlap || (overlap == best_overlap && *i < best_i) {
                        best = Some((overlap, *i));
                    }
                }
            }
        }
        best.map(|(_, i)| i)
    }

    pub fn activate_direction(&mut self, dir: PaneDir) -> bool {
        let Some(i) = self.neighbor_in(dir) else {
            return false;
        };
        self.active = i;
        true
    }

    /// Split id of the nearest matching ancestor of the active leaf.
    pub fn ancestor_split(&self, axis: SplitAxis) -> Option<u64> {
        self.root.ancestor_split(self.active_index(), axis)
    }
}

fn overlap_len(a0: u32, a_len: u32, b0: u32, b_len: u32) -> u32 {
    let a1 = a0.saturating_add(a_len);
    let b1 = b0.saturating_add(b_len);
    a1.min(b1).saturating_sub(a0.max(b0))
}

impl<T: PartialEq> SplitLayout<T> {
    pub fn contains(&self, pane: &T) -> bool {
        self.panes.iter().any(|p| p == pane)
    }

    pub fn set_active_pane(&mut self, pane: &T) -> bool {
        if let Some(i) = self.panes.iter().position(|p| p == pane) {
            self.active = i;
            true
        } else {
            false
        }
    }

    /// Split the focused leaf. `new_pane` becomes active (right/bottom).
    pub fn split(&mut self, axis: SplitAxis, new_pane: T, split_id: u64) {
        self.zoomed = false;
        let at = self.active_index();
        let new_i = self.panes.len();
        let _ = self.root.split_leaf(at, axis, new_i, split_id);
        self.panes.push(new_pane);
        self.active = new_i;
    }

    /// Swap the active leaf with `other`. `keep_focus` follows the original pane.
    pub fn swap_active_with(&mut self, other: usize, keep_focus: bool) {
        let a = self.active_index();
        if other >= self.panes.len() || other == a {
            return;
        }
        self.panes.swap(a, other);
        if keep_focus {
            self.active = other;
        }
    }

    /// Remove `pane` and return it. Second value is true when the tab is empty.
    pub fn extract_pane(&mut self, pane: &T) -> Option<(T, bool)>
    where
        T: Clone + PartialEq,
    {
        if !self.contains(pane) {
            return None;
        }
        let taken = pane.clone();
        let empty = self.remove_pane(pane);
        Some((taken, empty))
    }

    /// Cycle pane identities around leaf slots. Tree shape stays.
    /// Clockwise: last preorder leaf moves to the first slot (mux rotate_clockwise).
    pub fn rotate(&mut self, clockwise: bool)
    where
        T: Clone + PartialEq,
    {
        let mut leaves = Vec::new();
        self.root.collect_leaves(&mut leaves);
        if leaves.len() < 2 {
            return;
        }
        let focused = self.panes.get(self.active_index()).cloned();
        if clockwise {
            for i in (1..leaves.len()).rev() {
                self.panes.swap(leaves[i], leaves[i - 1]);
            }
        } else {
            for i in 0..leaves.len() - 1 {
                self.panes.swap(leaves[i], leaves[i + 1]);
            }
        }
        if let Some(focused) = focused {
            let _ = self.set_active_pane(&focused);
        }
    }

    /// Remove `pane`. `true` if the tab is now empty.
    pub fn remove_pane(&mut self, pane: &T) -> bool {
        let Some(i) = self.panes.iter().position(|p| p == pane) else {
            return self.panes.is_empty();
        };
        match self.root.clone().prune(i) {
            None => {
                self.panes.clear();
                self.active = 0;
                self.zoomed = false;
                true
            }
            Some(new_root) => {
                self.root = new_root;
                self.panes.remove(i);
                self.root.remap_after_remove(i);
                if self.active == i {
                    self.active = self.active.min(self.panes.len().saturating_sub(1));
                } else if self.active > i {
                    self.active -= 1;
                }
                if self.panes.len() < 2 {
                    self.zoomed = false;
                }
                self.panes.is_empty()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_then_prune_second_collapses() {
        let mut layout = SplitLayout::leaf("a");
        layout.split(SplitAxis::Horizontal, "b", 1);
        assert_eq!(layout.pane_count(), 2);
        assert_eq!(layout.active_pane(), Some(&"b"));
        assert!(!layout.remove_pane(&"b"));
        assert_eq!(layout.panes(), &["a"]);
        assert_eq!(layout.root(), &LayoutNode::Leaf(0));
    }

    #[test]
    fn split_then_prune_first_keeps_second() {
        let mut layout = SplitLayout::leaf("a");
        layout.split(SplitAxis::Vertical, "b", 1);
        assert!(!layout.remove_pane(&"a"));
        assert_eq!(layout.panes(), &["b"]);
        assert_eq!(layout.active_pane(), Some(&"b"));
        assert_eq!(layout.root(), &LayoutNode::Leaf(0));
    }

    #[test]
    fn nested_split_prune_inner() {
        let mut layout = SplitLayout::leaf("a");
        layout.split(SplitAxis::Horizontal, "b", 1);
        layout.split(SplitAxis::Vertical, "c", 2);
        // a | (b / c), active = c
        assert_eq!(layout.pane_count(), 3);
        assert!(!layout.remove_pane(&"c"));
        assert_eq!(layout.panes(), &["a", "b"]);
        match layout.root() {
            LayoutNode::Split {
                axis: SplitAxis::Horizontal,
                first,
                second,
                ..
            } => {
                assert_eq!(**first, LayoutNode::Leaf(0));
                assert_eq!(**second, LayoutNode::Leaf(1));
            }
            other => panic!("expected H split, got {other:?}"),
        }
    }

    #[test]
    fn remove_last_leaf_empties() {
        let mut layout = SplitLayout::leaf("a");
        assert!(layout.remove_pane(&"a"));
        assert!(layout.panes().is_empty());
    }

    #[test]
    fn set_active_then_split_that_leaf() {
        let mut layout = SplitLayout::leaf("a");
        layout.split(SplitAxis::Horizontal, "b", 1);
        assert!(layout.set_active_pane(&"a"));
        layout.split(SplitAxis::Vertical, "c", 2);
        // (a / c) | b
        assert_eq!(layout.panes(), &["a", "b", "c"]);
        assert_eq!(layout.active_pane(), Some(&"c"));
        match layout.root() {
            LayoutNode::Split {
                axis: SplitAxis::Horizontal,
                first,
                second,
                ..
            } => {
                assert_eq!(**second, LayoutNode::Leaf(1));
                match first.as_ref() {
                    LayoutNode::Split {
                        axis: SplitAxis::Vertical,
                        first: top,
                        second: bot,
                        ..
                    } => {
                        assert_eq!(**top, LayoutNode::Leaf(0));
                        assert_eq!(**bot, LayoutNode::Leaf(2));
                    }
                    other => panic!("expected V split, got {other:?}"),
                }
            }
            other => panic!("expected H split, got {other:?}"),
        }
    }

    #[test]
    fn activate_direction_horizontal() {
        let mut layout = SplitLayout::leaf("a");
        layout.split(SplitAxis::Horizontal, "b", 1);
        assert_eq!(layout.active_pane(), Some(&"b"));
        assert!(layout.activate_direction(PaneDir::Left));
        assert_eq!(layout.active_pane(), Some(&"a"));
        assert!(!layout.activate_direction(PaneDir::Left));
        assert!(layout.activate_direction(PaneDir::Right));
        assert_eq!(layout.active_pane(), Some(&"b"));
        assert!(!layout.activate_direction(PaneDir::Up));
        assert!(!layout.activate_direction(PaneDir::Down));
    }

    #[test]
    fn activate_direction_nested_picks_overlapping_neighbor() {
        let mut layout = SplitLayout::leaf("a");
        layout.split(SplitAxis::Horizontal, "b", 1);
        layout.split(SplitAxis::Vertical, "c", 2);
        // a | (b / c), active = c
        assert!(layout.activate_direction(PaneDir::Up));
        assert_eq!(layout.active_pane(), Some(&"b"));
        assert!(layout.activate_direction(PaneDir::Down));
        assert_eq!(layout.active_pane(), Some(&"c"));
        assert!(layout.activate_direction(PaneDir::Left));
        assert_eq!(layout.active_pane(), Some(&"a"));
        // a is full height; b and c share the edge equally → smaller index (b)
        assert!(layout.activate_direction(PaneDir::Right));
        assert_eq!(layout.active_pane(), Some(&"b"));
    }

    #[test]
    fn rotate_clockwise_moves_last_leaf_to_first() {
        let mut layout = SplitLayout::leaf("a");
        layout.split(SplitAxis::Horizontal, "b", 1);
        layout.split(SplitAxis::Vertical, "c", 2);
        // leaves preorder: a, b, c
        layout.rotate(true);
        assert_eq!(layout.panes(), &["c", "a", "b"]);
        assert_eq!(layout.active_pane(), Some(&"c"));
        layout.rotate(false);
        assert_eq!(layout.panes(), &["a", "b", "c"]);
        assert_eq!(layout.active_pane(), Some(&"c"));
    }

    #[test]
    fn ancestor_split_picks_nearest_matching_axis() {
        let mut layout = SplitLayout::leaf("a");
        layout.split(SplitAxis::Horizontal, "b", 1);
        layout.split(SplitAxis::Vertical, "c", 2);
        // a | (b / c), active = c
        assert_eq!(
            layout.ancestor_split(SplitAxis::Vertical),
            Some(2),
            "Up/Down hits the inner V split"
        );
        assert_eq!(
            layout.ancestor_split(SplitAxis::Horizontal),
            Some(1),
            "Left/Right walks up to the outer H split"
        );
        assert!(layout.set_active_pane(&"a"));
        assert_eq!(layout.ancestor_split(SplitAxis::Horizontal), Some(1));
        assert_eq!(
            layout.ancestor_split(SplitAxis::Vertical),
            None,
            "a has no V ancestor"
        );
    }

    #[test]
    fn zoom_requires_two_panes_and_clears_on_split() {
        let mut layout = SplitLayout::leaf("a");
        layout.toggle_zoom();
        assert!(!layout.is_zoomed());
        layout.split(SplitAxis::Horizontal, "b", 1);
        layout.toggle_zoom();
        assert!(layout.is_zoomed());
        layout.split(SplitAxis::Vertical, "c", 2);
        assert!(!layout.is_zoomed());
        layout.toggle_zoom();
        assert!(layout.is_zoomed());
        assert!(!layout.remove_pane(&"c"));
        assert!(layout.is_zoomed());
        assert!(!layout.remove_pane(&"b"));
        assert!(!layout.is_zoomed());
    }

    #[test]
    fn extract_pane_returns_taken_and_empty_flag() {
        let mut layout = SplitLayout::leaf("a");
        layout.split(SplitAxis::Horizontal, "b", 1);
        let (taken, empty) = layout.extract_pane(&"b").expect("b");
        assert_eq!(taken, "b");
        assert!(!empty);
        assert_eq!(layout.panes(), &["a"]);
        let (taken, empty) = layout.extract_pane(&"a").expect("a");
        assert_eq!(taken, "a");
        assert!(empty);
        assert!(layout.panes().is_empty());
    }

    #[test]
    fn swap_active_keep_focus_follows_original() {
        let mut layout = SplitLayout::leaf("a");
        layout.split(SplitAxis::Horizontal, "b", 1);
        assert_eq!(layout.active_pane(), Some(&"b"));
        layout.swap_active_with(0, true);
        assert_eq!(layout.active_pane(), Some(&"b"));
        assert_eq!(layout.panes(), &["b", "a"]);
        layout.swap_active_with(1, false);
        assert_eq!(layout.active_pane(), Some(&"a"));
        assert_eq!(layout.panes(), &["a", "b"]);
    }
}
