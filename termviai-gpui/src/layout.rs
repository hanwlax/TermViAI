use crate::workspaces::{SavedLayout, SplitAxis};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
    Top,
    Bottom,
}
#[derive(Clone, Debug, PartialEq)]
pub enum Layout {
    Pane(usize),
    Split {
        axis: SplitAxis,
        ratio: f32,
        first: Box<Self>,
        second: Box<Self>,
    },
}
/// Four rectangular panes around one continuous divider. The other two
/// dividers may meet it at different positions after a single-side resize.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QuadSplit {
    pub axis: SplitAxis,
    pub spine: f32,
    pub branches: [f32; 2],
    panes: [[usize; 2]; 2],
}

#[derive(Clone, Copy, Debug)]
pub enum QuadGrip {
    Segment(usize),
    Center,
    Junction(usize),
}

impl QuadSplit {
    pub fn aligned(self) -> bool {
        self.branches[0] == self.branches[1]
    }

    fn tree(self) -> Layout {
        let branch = |i: usize| Layout::Split {
            axis: perpendicular(self.axis),
            ratio: self.branches[i],
            first: Box::new(Layout::Pane(self.panes[i][0])),
            second: Box::new(Layout::Pane(self.panes[i][1])),
        };
        Layout::Split {
            axis: self.axis,
            ratio: self.spine,
            first: Box::new(branch(0)),
            second: Box::new(branch(1)),
        }
    }

    fn transposed(self) -> Self {
        Self {
            axis: perpendicular(self.axis),
            spine: self.branches[0],
            branches: [self.spine; 2],
            panes: [
                [self.panes[0][0], self.panes[1][0]],
                [self.panes[0][1], self.panes[1][1]],
            ],
        }
    }

    /// Derive every move from the layout at mouse-down, even after the
    /// segment becomes part of a differently oriented split tree.
    pub fn resize(self, node: &mut Layout, grip: QuadGrip, position: [f32; 2], snap: [f32; 2]) {
        let Some(current) = node.quad_split() else {
            return;
        };
        let panes = if self.axis == current.axis {
            self.panes
        } else {
            self.transposed().panes
        };
        if panes != current.panes {
            return;
        }
        let axis = usize::from(self.axis == SplitAxis::Vertical);
        let along = position[axis].clamp(0.1, 0.9);
        let across = position[1 - axis].clamp(0.1, 0.9);
        let mut next = self;
        match grip {
            QuadGrip::Segment(i) if self.aligned() => {
                next = self.transposed();
                next.branches[i] = snap_ratio(along, self.spine, snap[axis]);
            }
            QuadGrip::Center => {
                next.spine = along;
                next.branches = [across; 2];
            }
            QuadGrip::Junction(i) => {
                next.spine = along;
                next.branches[i] = snap_ratio(across, self.branches[1 - i], snap[1 - axis]);
            }
            _ => return,
        }
        if next != current {
            *node = next.tree();
        }
    }
}

fn perpendicular(axis: SplitAxis) -> SplitAxis {
    match axis {
        SplitAxis::Horizontal => SplitAxis::Vertical,
        SplitAxis::Vertical => SplitAxis::Horizontal,
    }
}

fn snap_ratio(value: f32, other: f32, tolerance: f32) -> f32 {
    if (value - other).abs() <= tolerance {
        other
    } else {
        value
    }
}

impl Layout {
    /// Build equal-area splits for a selection of hosts. Alternate axes at each level.
    pub fn balanced(panes: &[usize], axis: SplitAxis) -> Self {
        if let [id] = panes {
            return Self::Pane(*id);
        }
        assert!(!panes.is_empty());
        let mid = panes.len().div_ceil(2);
        let next = match axis {
            SplitAxis::Horizontal => SplitAxis::Vertical,
            SplitAxis::Vertical => SplitAxis::Horizontal,
        };
        Self::Split {
            axis,
            ratio: mid as f32 / panes.len() as f32,
            first: Box::new(Self::balanced(&panes[..mid], next)),
            second: Box::new(Self::balanced(&panes[mid..], next)),
        }
    }
    pub fn panes(&self) -> Vec<usize> {
        let mut v = vec![];
        self.collect(&mut v);
        v
    }
    /// Screen reading order, independent of the split-tree traversal order.
    pub fn visual_panes(&self) -> Vec<usize> {
        self.rectangles().into_iter().map(|rect| rect.0).collect()
    }

    /// Choose the adjacent pane sharing the longest edge, breaking ties in reading order.
    pub fn neighbor(&self, pane: usize, side: Side) -> Option<usize> {
        let rectangles = self.rectangles();
        let &(_, x, y, w, h) = rectangles.iter().find(|r| r.0 == pane)?;
        let mut best = None;
        let mut longest = 0.;
        for (id, nx, ny, nw, nh) in rectangles {
            let (gap, overlap) = match side {
                Side::Left | Side::Right => (
                    if side == Side::Left {
                        x - (nx + nw)
                    } else {
                        nx - (x + w)
                    },
                    (y + h).min(ny + nh) - y.max(ny),
                ),
                Side::Top | Side::Bottom => (
                    if side == Side::Top {
                        y - (ny + nh)
                    } else {
                        ny - (y + h)
                    },
                    (x + w).min(nx + nw) - x.max(nx),
                ),
            };
            if id != pane && gap.abs() < 1e-9 && overlap > longest + 1e-9 {
                best = Some(id);
                longest = overlap;
            }
        }
        best
    }

    fn rectangles(&self) -> Vec<(usize, f64, f64, f64, f64)> {
        fn visit(
            tree: &Layout,
            x: f64,
            y: f64,
            w: f64,
            h: f64,
            out: &mut Vec<(usize, f64, f64, f64, f64)>,
        ) {
            match tree {
                Layout::Pane(id) => out.push((*id, x, y, w, h)),
                Layout::Split {
                    axis,
                    ratio,
                    first,
                    second,
                } => {
                    let r = *ratio as f64;
                    match axis {
                        SplitAxis::Horizontal => {
                            visit(first, x, y, w * r, h, out);
                            visit(second, x + w * r, y, w * (1. - r), h, out);
                        }
                        SplitAxis::Vertical => {
                            visit(first, x, y, w, h * r, out);
                            visit(second, x, y + h * r, w, h * (1. - r), out);
                        }
                    }
                }
            }
        }
        let mut positions = Vec::new();
        visit(self, 0., 0., 1., 1., &mut positions);
        positions.sort_by(|a, b| a.2.total_cmp(&b.2).then(a.1.total_cmp(&b.1)));
        positions
    }
    fn collect(&self, v: &mut Vec<usize>) {
        match self {
            Self::Pane(id) => v.push(*id),
            Self::Split { first, second, .. } => {
                first.collect(v);
                second.collect(v)
            }
        }
    }
    pub fn remove(self, id: usize) -> Option<Self> {
        match self {
            Self::Pane(p) => (p != id).then_some(Self::Pane(p)),
            Self::Split {
                axis,
                ratio,
                first,
                second,
            } => match (first.remove(id), second.remove(id)) {
                (Some(a), Some(b)) => Some(Self::Split {
                    axis,
                    ratio,
                    first: Box::new(a),
                    second: Box::new(b),
                }),
                (a, b) => a.or(b),
            },
        }
    }
    pub fn dock(&mut self, source: usize, target: usize, side: Side) -> bool {
        if source == target || !self.panes().contains(&target) {
            return false;
        }
        let Some(mut tree) = self.clone().remove(source) else {
            return false;
        };
        tree.insert(source, target, side);
        *self = tree;
        true
    }
    fn insert(&mut self, source: usize, target: usize, side: Side) {
        match self {
            Self::Pane(id) if *id == target => {
                let (first, second) = if matches!(side, Side::Left | Side::Top) {
                    (source, target)
                } else {
                    (target, source)
                };
                *self = Self::Split {
                    axis: if matches!(side, Side::Left | Side::Right) {
                        SplitAxis::Horizontal
                    } else {
                        SplitAxis::Vertical
                    },
                    ratio: 0.5,
                    first: Box::new(Self::Pane(first)),
                    second: Box::new(Self::Pane(second)),
                };
            }
            Self::Split { first, second, .. } => {
                first.insert(source, target, side);
                second.insert(source, target, side);
            }
            _ => {}
        }
    }
    pub fn save<T>(&self, host: &impl Fn(usize) -> Option<T>) -> Option<SavedLayout<T>> {
        Some(match self {
            Self::Pane(id) => SavedLayout::Pane { host: host(*id)? },
            Self::Split {
                axis,
                ratio,
                first,
                second,
            } => SavedLayout::Split {
                axis: *axis,
                first_percent: (ratio * 100.).round().clamp(1., 99.) as u8,
                first: Box::new(first.save(host)?),
                second: Box::new(second.save(host)?),
            },
        })
    }
    pub fn restore<T>(layout: &SavedLayout<T>, spawn: &mut impl FnMut(&T) -> usize) -> Self {
        match layout {
            SavedLayout::Pane { host } => Self::Pane(spawn(host)),
            SavedLayout::Split {
                axis,
                first_percent,
                first,
                second,
            } => Self::Split {
                axis: *axis,
                ratio: *first_percent as f32 / 100.,
                first: Box::new(Self::restore(first, spawn)),
                second: Box::new(Self::restore(second, spawn)),
            },
        }
    }
    pub fn quad_split(&self) -> Option<QuadSplit> {
        let Self::Split {
            axis,
            ratio,
            first,
            second,
        } = self
        else {
            return None;
        };
        let pair = |branch: &Self| {
            let Self::Split {
                axis: inner,
                ratio,
                first,
                second,
            } = branch
            else {
                return None;
            };
            let (Self::Pane(a), Self::Pane(b)) = (&**first, &**second) else {
                return None;
            };
            (*inner != *axis).then_some((*ratio, [*a, *b]))
        };
        let (a, first_pair) = pair(first)?;
        let (b, second_pair) = pair(second)?;
        Some(QuadSplit {
            axis: *axis,
            spine: *ratio,
            branches: [a, b],
            panes: [first_pair, second_pair],
        })
    }

    pub fn at_mut(&mut self, path: &[bool]) -> Option<&mut Self> {
        let mut node = self;
        for &right in path {
            let Self::Split { first, second, .. } = node else {
                return None;
            };
            node = if right { second } else { first };
        }
        Some(node)
    }

    pub fn resize_divider(&mut self, path: &[bool], value: f32, snap: f32) {
        let mut value = value.clamp(0.1, 0.9);
        if let Some((&branch, parent)) = path.split_last()
            && let Some(quad) = self.at_mut(parent).and_then(|node| node.quad_split())
        {
            value = snap_ratio(value, quad.branches[usize::from(!branch)], snap);
        }
        self.set_ratio(path, value);
    }

    pub fn set_ratio(&mut self, path: &[bool], value: f32) {
        if let Some(Self::Split { ratio, .. }) = self.at_mut(path) {
            *ratio = value.clamp(0.1, 0.9);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quad_segment_drag_preserves_opposite_pair_and_saved_geometry() {
        for axis in [SplitAxis::Horizontal, SplitAxis::Vertical] {
            for i in 0..2 {
                let mut layout = Layout::balanced(&[11, 12, 13, 14], axis);
                layout.set_ratio(&[], 0.36);
                layout.set_ratio(&[false], 0.60);
                layout.set_ratio(&[true], 0.60);
                let start = layout.quad_split().unwrap();
                for value in [0.40, 0.74] {
                    start.resize(&mut layout, QuadGrip::Segment(i), [value; 2], [0.005; 2]);
                    let actual = layout.quad_split().unwrap();
                    assert_eq!(actual.axis, perpendicular(axis));
                    assert_eq!(actual.spine, 0.60);
                    assert_eq!(actual.branches[i], value);
                    assert_eq!(actual.branches[1 - i], 0.36);
                    assert_eq!(actual.panes, [[11, 13], [12, 14]]);
                }
                let saved = layout.save(&Some).unwrap();
                assert_eq!(Layout::restore(&saved, &mut |id| *id), layout);
                layout.resize_divider(&[i == 1], 0.363, 0.005);
                assert!(layout.quad_split().unwrap().aligned());
            }
        }
    }

    #[test]
    fn quad_center_moves_both_axes_and_staggered_junction_keeps_other_branch() {
        for axis in [SplitAxis::Horizontal, SplitAxis::Vertical] {
            let mut layout = Layout::balanced(&[1, 2, 3, 4], axis);
            let start = layout.quad_split().unwrap();
            start.resize(&mut layout, QuadGrip::Center, [0.8, 0.3], [0.005; 2]);
            let (spine, branch) = if axis == SplitAxis::Horizontal {
                (0.8, 0.3)
            } else {
                (0.3, 0.8)
            };
            let cross = layout.quad_split().unwrap();
            assert_eq!((cross.spine, cross.branches), (spine, [branch; 2]));
            layout.resize_divider(&[true], 0.6, 0.005);
            let staggered = layout.quad_split().unwrap();
            staggered.resize(&mut layout, QuadGrip::Junction(1), [-1., 2.], [0.005; 2]);
            let limited = layout.quad_split().unwrap();
            let (s, b) = if axis == SplitAxis::Horizontal {
                (0.1, 0.9)
            } else {
                (0.9, 0.1)
            };
            assert_eq!((limited.spine, limited.branches), (s, [branch, b]));
            assert_eq!(limited.panes, start.panes);
        }
    }

    #[test]
    fn stale_quad_drag_cannot_restore_a_closed_or_replaced_pane() {
        let original = Layout::balanced(&[1, 2, 3, 4], SplitAxis::Horizontal);
        let start = original.quad_split().unwrap();
        for mut layout in [
            original.remove(2).unwrap(),
            Layout::balanced(&[1, 2, 3, 5], SplitAxis::Horizontal),
        ] {
            let before = layout.clone();
            start.resize(&mut layout, QuadGrip::Center, [0.8; 2], [0.005; 2]);
            assert_eq!(layout, before);
        }
    }

    #[test]
    fn directional_focus_follows_shared_edges_after_resizing() {
        let mut layout = Layout::Pane(1);
        layout.dock(2, 1, Side::Right);
        layout.dock(3, 2, Side::Bottom);
        assert_eq!(layout.neighbor(1, Side::Right), Some(2));
        layout.set_ratio(&[true], 0.25);
        assert_eq!(layout.neighbor(1, Side::Right), Some(3));
        assert_eq!(layout.neighbor(3, Side::Top), Some(2));
        assert_eq!(layout.neighbor(2, Side::Bottom), Some(3));
        assert_eq!(layout.neighbor(2, Side::Left), Some(1));
        assert_eq!(layout.neighbor(2, Side::Right), None);
        assert_eq!(layout.neighbor(99, Side::Left), None);
    }

    #[test]
    fn batch_layout_keeps_all_sessions_and_four_hosts_in_reading_order() {
        for size in 1..=17 {
            let ids = (0..size).collect::<Vec<_>>();
            let layout = Layout::balanced(&ids, SplitAxis::Vertical);
            assert_eq!(layout.panes(), ids);
            if size == 4 {
                assert_eq!(layout.visual_panes(), ids);
            }
        }
    }
    #[test]
    fn visual_order_reads_rows_across_column_splits_and_after_docking() {
        let mut layout = Layout::Pane(209);
        assert!(layout.dock(212, 209, Side::Right));
        assert!(layout.dock(216, 209, Side::Bottom));
        assert!(layout.dock(217, 212, Side::Bottom));
        assert_eq!(layout.panes(), vec![209, 216, 212, 217]);
        assert_eq!(layout.visual_panes(), vec![209, 212, 216, 217]);
        assert!(layout.dock(217, 209, Side::Left));
        assert_eq!(layout.visual_panes(), vec![217, 209, 212, 216]);
    }
    #[test]
    fn visual_order_uses_split_positions_for_unequal_rows() {
        let mut layout = Layout::Pane(1);
        layout.dock(2, 1, Side::Right);
        layout.dock(3, 1, Side::Bottom);
        layout.dock(4, 2, Side::Bottom);
        layout.set_ratio(&[false], 0.7);
        layout.set_ratio(&[true], 0.3);
        assert_eq!(layout.visual_panes(), vec![1, 2, 4, 3]);
    }
    #[test]
    fn docking_removes_source_before_nested_split_and_preserves_sessions() {
        let mut x = Layout::Pane(1);
        assert!(x.dock(2, 1, Side::Right));
        assert!(x.dock(3, 1, Side::Bottom));
        assert!(x.dock(1, 2, Side::Top));
        assert_eq!(x.panes(), vec![3, 1, 2]);
        assert!(!x.dock(2, 2, Side::Left));
        assert_eq!(x.clone().remove(1).unwrap().panes(), vec![3, 2]);
    }
}
