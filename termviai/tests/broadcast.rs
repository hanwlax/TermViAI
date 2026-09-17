use termviai_broadcast::{route_input, BroadcastState, UnknownPane};

fn four_panes() -> BroadcastState<usize> {
    let mut state = BroadcastState::new([1, 2, 3, 4]);
    state.set_member(4, false).unwrap();
    state
}

#[test]
fn off_sends_only_to_each_source_and_preserves_members() {
    let mut state = four_panes();
    state.set_enabled(true);
    state.set_enabled(false);
    for source in 1..=4 {
        assert_eq!(state.targets_for(source).unwrap(), [source]);
    }
    state.set_enabled(true);
    assert_eq!(state.targets_for(2).unwrap(), [1, 2, 3]);
}

#[test]
fn member_sources_broadcast_but_non_member_is_independent() {
    let mut state = four_panes();
    state.set_enabled(true);
    for source in 1..=3 {
        assert_eq!(state.targets_for(source).unwrap(), [1, 2, 3]);
    }
    assert_eq!(state.targets_for(4).unwrap(), [4]);
}

#[test]
fn repeated_mux_snapshots_preserve_exclusions_and_new_panes_join() {
    for enabled in [false, true] {
        let mut state = four_panes();
        state.set_enabled(enabled);
        state.sync_panes([1, 2, 3, 4, 5, 5]);
        assert!(!state.is_member(4));
        assert!(state.is_member(5));
        assert_eq!(state.member_count(), 4);
        assert_eq!(state.pane_count(), 5);
        assert_eq!(state.enabled(), enabled);
    }
}

#[test]
fn closing_panes_removes_stale_ids_and_disables_empty_group() {
    let mut state = four_panes();
    state.set_enabled(true);
    state.sync_panes([1, 3, 4]);
    assert_eq!(state.targets_for(1).unwrap(), [1, 3]);
    assert_eq!(state.targets_for(2), Err(UnknownPane(2)));
    state.sync_panes([4]);
    assert!(!state.enabled());
    assert_eq!(state.member_count(), 0);
}

#[test]
fn all_none_and_last_member_have_explicit_enable_semantics() {
    let mut state = four_panes();
    state.set_enabled(true);
    state.select_none();
    assert!(!state.enabled());
    state.set_enabled(true);
    assert!(!state.enabled());
    state.select_all();
    assert_eq!(state.member_count(), 4);
    assert!(!state.enabled());
    state.toggle_enabled();
    for id in 1..=4 {
        state.toggle_member(id).unwrap();
    }
    assert!(!state.enabled());
}

#[test]
fn tabs_cannot_inject_foreign_sources_or_members() {
    let mut a = four_panes();
    let b = BroadcastState::new([5, 6]);
    a.set_enabled(true);
    assert_eq!(a.set_member(5, true), Err(UnknownPane(5)));
    assert_eq!(a.targets_for(5), Err(UnknownPane(5)));
    assert_eq!(b.targets_for(5).unwrap(), [5]);
    assert!(!b.enabled());
    assert_eq!(a.targets_for(1).unwrap(), [1, 2, 3]);
}

#[test]
fn encoded_bytes_are_shared_unchanged_and_written_once_per_target() {
    let mut state = four_panes();
    state.set_enabled(true);
    // Encoded Ctrl+C/D/Z, Tab, arrows, F1, Alt, bracketed paste, UTF-8,
    // CR, DEL and NUL. This tests byte routing, not WezTerm's encoders.
    let input = "\u{3}\u{4}\u{1a}\t\u{1b}[A\u{1b}OP\u{1b}x\u{1b}[200~中文\r\n\u{1b}[201~\r\u{7f}\0"
        .as_bytes();
    let mut seen = Vec::new();
    let report = route_input(
        &state,
        2,
        input,
        |_| true,
        |pane, bytes| {
            assert!(std::ptr::eq(input, bytes));
            seen.push((pane, bytes.to_vec()));
            Ok::<_, ()>(())
        },
    )
    .unwrap();
    assert_eq!(report.delivered, [1, 2, 3]);
    assert_eq!(seen.len(), 3);
    assert!(seen.iter().all(|(_, bytes)| bytes == input));
}

#[test]
fn write_failure_is_isolated_and_not_retried() {
    let mut state = four_panes();
    state.set_enabled(true);
    let mut attempted = Vec::new();
    let report = route_input(
        &state,
        1,
        b"test",
        |_| true,
        |id, _| {
            attempted.push(id);
            if id == 2 {
                Err("broken pipe")
            } else {
                Ok(())
            }
        },
    )
    .unwrap();
    assert_eq!(attempted, [1, 2, 3]);
    assert_eq!(report.delivered, [1, 3]);
    assert_eq!(report.failures, [(2, "broken pipe")]);
}

#[test]
fn disconnection_preserves_membership_and_reconnect_resumes_delivery() {
    let mut state = four_panes();
    state.set_enabled(true);
    let report = route_input(&state, 1, b"x", |id| id != 3, |_, _| Ok::<_, ()>(())).unwrap();
    assert_eq!(report.delivered, [1, 2]);
    assert_eq!(report.disconnected, [3]);
    assert!(state.is_member(3));
    let report = route_input(&state, 1, b"y", |_| true, |_, _| Ok::<_, ()>(())).unwrap();
    assert_eq!(report.delivered, [1, 2, 3]);
}

#[test]
fn empty_input_and_unknown_source_never_write() {
    let state = four_panes();
    let report = route_input(
        &state,
        1,
        b"",
        |_| panic!("unexpected lookup"),
        |_, _| -> Result<(), ()> { panic!("unexpected write") },
    )
    .unwrap();
    assert!(report.delivered.is_empty());
    let result = route_input(
        &state,
        99,
        b"x",
        |_| panic!("unexpected lookup"),
        |_, _| -> Result<(), ()> { panic!("unexpected write") },
    );
    assert!(matches!(result, Err(UnknownPane(99))));
}

#[test]
fn empty_tab_stays_disabled() {
    let mut state = BroadcastState::<usize>::default();
    state.select_all();
    state.set_enabled(true);
    assert!(!state.enabled());
    assert_eq!(state.pane_count(), 0);
}
