use crate::hosts::Host;
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
impl Layout {
    pub fn panes(&self) -> Vec<usize> {
        let mut v = vec![];
        self.collect(&mut v);
        v
    }
    /// Screen reading order, independent of the split-tree traversal order.
    pub fn visual_panes(&self) -> Vec<usize> {
        fn visit(tree: &Layout, x: f64, y: f64, w: f64, h: f64, out: &mut Vec<(usize, f64, f64)>) {
            match tree {
                Layout::Pane(id) => out.push((*id, x, y)),
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
        positions.into_iter().map(|p| p.0).collect()
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
    pub fn save(&self, host: &impl Fn(usize) -> Option<Host>) -> Option<SavedLayout> {
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
    pub fn restore(layout: &SavedLayout, spawn: &mut impl FnMut(&Host) -> usize) -> Self {
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
    pub fn set_ratio(&mut self, path: &[bool], ratio: f32) {
        if let Self::Split {
            ratio: r,
            first,
            second,
            ..
        } = self
        {
            if let Some((branch, tail)) = path.split_first() {
                if *branch {
                    second.set_ratio(tail, ratio)
                } else {
                    first.set_ratio(tail, ratio)
                }
            } else {
                *r = ratio.clamp(0.1, 0.9)
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
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
