//! Shared rules for tab previews and committed moves.
use crate::layout::Layout;

pub fn default_name(labels: &[String]) -> String {
    match labels {
        [] => "Workspace".into(),
        [only] => only.clone(),
        [first, second] => format!("{first} + {second}"),
        [first, ..] => format!("{first} + {} hosts", labels.len() - 1),
    }
}

/// A whole group can be sorted, but only individual terminals can be docked.
pub fn dock_source(layout: &Layout, pane: Option<usize>) -> Option<usize> {
    let members = layout.panes();
    match pane {
        Some(id) => members.contains(&id).then_some(id),
        None if members.len() == 1 => members.first().copied(),
        None => None,
    }
}

/// `gap` is in the original list (0..=len); the result is after removal.
/// No-op gaps deliberately return None so they never paint an insertion mark.
pub fn reorder_destination(len: usize, from: usize, gap: usize) -> Option<usize> {
    if from >= len || gap > len {
        return None;
    }
    let to = gap - usize::from(gap > from);
    (to != from).then_some(to)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::Side;

    #[test]
    fn names_follow_the_legacy_group_format() {
        for (labels, expected) in [
            (vec![], "Workspace"),
            (vec!["209"], "209"),
            (vec!["209", "212"], "209 + 212"),
            (vec!["209", "212", "216", "217"], "209 + 3 hosts"),
        ] {
            assert_eq!(
                default_name(&labels.into_iter().map(String::from).collect::<Vec<_>>()),
                expected
            );
        }
    }

    #[test]
    fn groups_are_sortable_but_not_dockable_as_a_whole() {
        let mut group = Layout::Pane(209);
        assert!(group.dock(212, 209, Side::Right));
        assert_eq!(dock_source(&group, None), None);
        assert_eq!(dock_source(&group, Some(212)), Some(212));
        assert_eq!(dock_source(&group, Some(999)), None);
        assert_eq!(dock_source(&Layout::Pane(209), None), Some(209));
        assert_eq!(reorder_destination(3, 0, 3), Some(2));
    }

    #[test]
    fn both_adjacent_gaps_and_own_halves_are_noops() {
        for from in 0..4 {
            assert_eq!(reorder_destination(4, from, from), None);
            assert_eq!(reorder_destination(4, from, from + 1), None);
        }
        assert_eq!(reorder_destination(4, 0, 2), Some(1));
        assert_eq!(reorder_destination(4, 3, 1), Some(1));
        assert_eq!(reorder_destination(4, 3, 0), Some(0));
        assert_eq!(reorder_destination(4, 0, 4), Some(3));
        assert_eq!(reorder_destination(0, 0, 0), None);
        assert_eq!(reorder_destination(4, 4, 0), None);
        assert_eq!(reorder_destination(4, 0, 5), None);
    }

    #[test]
    fn detaching_preserves_remaining_layout_and_pane_identity() {
        let mut group = Layout::Pane(209);
        group.dock(212, 209, Side::Right);
        group.dock(216, 209, Side::Bottom);
        let source = dock_source(&group, Some(209)).unwrap();
        let remaining = group.remove(source).unwrap();
        assert_eq!(remaining.visual_panes(), vec![216, 212]);
        assert_eq!(Layout::Pane(source).panes(), vec![209]);
        assert_eq!(dock_source(&remaining, Some(source)), None);
    }
}
