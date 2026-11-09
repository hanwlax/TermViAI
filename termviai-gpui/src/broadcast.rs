//! Selection semantics shared by the tab, pane controls and compact member bar.
use termviai_broadcast::BroadcastState;

pub fn toggle_all(state: &mut BroadcastState<usize>) {
    let all = state.enabled() && state.member_count() == state.pane_count();
    if all {
        state.select_none();
    } else {
        state.select_all();
        state.set_enabled(true);
    }
}

pub fn toggle_member(state: &mut BroadcastState<usize>, pane: usize) {
    // Disabled broadcast can retain targets internally. The UI shows them as
    // unchecked, so selecting one must not enable the other retained targets.
    let mut next = state.clone();
    if !next.enabled() {
        next.select_none();
    }
    if next.toggle_member(pane).is_ok() {
        next.set_enabled(next.member_count() > 0);
        *state = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn disabled_retained_targets_do_not_become_implicitly_selected() {
        let mut state = BroadcastState::new([209, 212, 216, 217]);
        toggle_member(&mut state, 212);
        assert!(state.enabled());
        assert_eq!(state.member_count(), 1);
        assert!(state.is_member(212));
        toggle_member(&mut state, 212);
        assert!(!state.enabled());
    }
    #[test]
    fn partial_selection_becomes_all_then_none_and_unknown_panes_do_not_change_it() {
        let mut state = BroadcastState::new([209, 212, 216, 217]);
        toggle_all(&mut state);
        toggle_member(&mut state, 216);
        assert_eq!(state.member_count(), 3);
        toggle_member(&mut state, 999);
        assert_eq!(state.member_count(), 3);
        toggle_all(&mut state);
        assert!(state.enabled());
        assert_eq!(state.member_count(), 4);
        toggle_all(&mut state);
        assert!(!state.enabled());
        assert_eq!(state.member_count(), 0);
    }
}
