use super::TermWindow;
use anyhow::Context;
use mux::localpane::LocalPane;
use mux::pane::Pane;
use mux::Mux;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use termviai_broadcast::{route_input, BroadcastState};
use window::WindowOps;

/// Only effective participants get the active indicator or checked tray state.
pub(super) fn pane_broadcasting(state: &BroadcastState<usize>, pane: usize) -> bool {
    state.enabled() && state.is_member(pane)
}

/// A group is active only when broadcast is running for every member.
/// Use the same predicate for the tab indicator and its select-all action.
pub(super) fn group_broadcasting(state: &BroadcastState<usize>) -> bool {
    state.enabled() && state.pane_count() > 0 && state.member_count() == state.pane_count()
}

/// The compact tray has no separate pause switch. Selecting a gray participant
/// starts that selection; stale targets retained by the core while paused must
/// not silently enable other terminals.
pub(super) fn toggle_participant(
    state: &mut BroadcastState<usize>,
    pane: usize,
) -> anyhow::Result<()> {
    state.targets_for(pane)?; // Validate before changing any existing selection.
    let selected = pane_broadcasting(state, pane);
    if !state.enabled() {
        state.select_none();
    }
    state.set_member(pane, !selected)?;
    state.set_enabled(state.member_count() > 0);
    Ok(())
}

fn pane_accepts_input(pane: &Arc<dyn Pane>) -> bool {
    !pane.is_dead()
        && !pane
            .downcast_ref::<LocalPane>()
            .is_some_and(LocalPane::is_reconnectable_ssh)
}

#[derive(Default)]
pub(super) struct TabBroadcasts {
    pub tabs: HashMap<usize, BroadcastState<usize>>,
    pub pane_errors: HashMap<usize, String>,
}
impl TabBroadcasts {
    /// The tab button acts on the entire group, including excluded or zoom-hidden
    /// panes. The tray offers direct member selection and All / None controls.
    pub(super) fn toggle_group(&mut self, tab: usize) {
        if let Some(state) = self.tabs.get_mut(&tab) {
            if group_broadcasting(state) {
                state.select_none();
            } else {
                state.select_all();
                state.set_enabled(true);
            }
        }
    }

    fn sync(&mut self, snapshots: impl IntoIterator<Item = (usize, Vec<usize>)>) {
        let mut live = HashSet::new();
        let mut live_panes = HashSet::new();
        for (tab, panes) in snapshots {
            live.insert(tab);
            live_panes.extend(panes.iter().copied());
            self.tabs.entry(tab).or_default().sync_panes(panes);
        }
        self.tabs.retain(|tab, _| live.contains(tab));
        self.pane_errors.retain(|pane, _| live_panes.contains(pane));
    }
}
impl TermWindow {
    pub(super) fn termviai_sync_broadcast(&mut self) {
        let mux = Mux::get();
        let snapshots = mux
            .get_window(self.mux_window_id)
            .map(|window| {
                window
                    .iter_tabs()
                    .map(|tab| {
                        (
                            tab.tab_id(),
                            tab.iter_panes_ignoring_zoom()
                                .iter()
                                .map(|p| p.pane.pane_id())
                                .collect(),
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        self.termviai_broadcast.sync(snapshots);
    }

    pub(super) fn termviai_input_targets(&mut self, pane: usize) -> Option<(usize, Vec<usize>)> {
        if !self.config.termviai_ui {
            return None;
        }
        self.termviai_sync_broadcast();
        let (_, window, tab) = Mux::get().resolve_pane_id(pane)?;
        if window != self.mux_window_id {
            return None;
        }
        Some((
            tab,
            self.termviai_broadcast
                .tabs
                .get(&tab)?
                .targets_for(pane)
                .ok()?,
        ))
    }

    /// Called only by user key, IME, paste, drop and SendKey/SendString paths.
    /// Overlays keep their own input; mouse/focus/protocol replies never call it.
    pub(super) fn termviai_send_input(
        &mut self,
        pane: &Arc<dyn Pane>,
        encode: impl FnOnce() -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        // A confirmation consumes user input, including drop and delayed paste.
        // Do not encode it into the source SSH session either.
        if self.termviai_confirmation_input_active() {
            return Ok(());
        }
        if !self.config.termviai_ui
            || self.termviai_library_active()
            || self.get_modal().is_some()
            || pane.downcast_ref::<LocalPane>().is_none()
        {
            return encode();
        }
        // A held SSH pane intentionally remains in the mux so its scrollback
        // survives. Consume input while it is disconnected; writing Return to
        // the ended transport must not remove the pane or duplicate the key on
        // a later reconnect.
        if !pane_accepts_input(pane) {
            return Ok(());
        }
        let mux = Mux::get();
        let source = pane.pane_id();
        let tab = match mux.resolve_pane_id(source) {
            Some((_, window, tab)) if window == self.mux_window_id => tab,
            _ => return encode(),
        };
        // An overlay may reuse the underlying pane id. Identity must match too.
        if !mux
            .get_pane(source)
            .map(|real| Arc::ptr_eq(&real, pane))
            .unwrap_or(false)
        {
            return encode();
        }
        self.termviai_sync_broadcast();
        let state = self
            .termviai_broadcast
            .tabs
            .get(&tab)
            .context("Tab was closed")?
            .clone();
        if state.targets_for(source)? == [source] {
            return encode();
        }
        let result = (|| -> anyhow::Result<()> {
            let bytes = mux::user_input::capture(source, encode)?;
            let report = route_input(
                &state,
                source,
                &bytes,
                |id| {
                    mux.get_pane(id)
                        .map(|p| pane_accepts_input(&p))
                        .unwrap_or(false)
                },
                |id, data| -> anyhow::Result<()> {
                    // Recheck ownership at dispatch; never send into another tab.
                    anyhow::ensure!(
                        mux.resolve_pane_id(id)
                            == Some((
                                mux.get_pane(id).context("Pane closed")?.domain_id(),
                                self.mux_window_id,
                                tab
                            )),
                        "Pane moved to another tab"
                    );
                    let target = mux.get_pane(id).context("Pane closed")?;
                    target.send_raw_input(data)
                },
            )?;
            if !bytes.is_empty() {
                for id in &report.delivered {
                    self.termviai_broadcast.pane_errors.remove(id);
                    // The key/paste callers scroll only the source. Successful
                    // broadcast recipients need the same live viewport, even
                    // when they were previously reading scrollback. Preserve
                    // copy/search overlays and the user's scroll-on-input setting.
                    if *id != source && self.pane_state(*id).overlay.is_none() {
                        if let Some(target) = mux.get_pane(*id) {
                            self.maybe_scroll_to_bottom_for_input(&target);
                        }
                    }
                }
                for id in &report.disconnected {
                    self.termviai_broadcast.pane_errors.insert(*id,
                        "Broadcast skipped: this session is offline. Reconnect to send new input. Skipped input will not be replayed.".into());
                }
                for (id, error) in &report.failures {
                    log::error!("Broadcast input failed for pane {id}: {error:#}");
                    self.termviai_broadcast.pane_errors.insert(
                        *id,
                        format!("Broadcast failed: {error:#}. Input was not replayed."),
                    );
                }
                if let Some(window) = &self.window {
                    window.invalidate();
                }
            }
            Ok(())
        })();
        if let Err(error) = &result {
            log::error!("Broadcast input failed for pane {source}: {error:#}");
            self.termviai_broadcast
                .pane_errors
                .insert(source, format!("{error:#}"));
            if let Some(window) = &self.window {
                window.invalidate();
            }
        }
        // A partial delivery is still a consumed input event. Returning an error
        // here would let raw-key fallback encode and send the same key again.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn direct_member_controls_follow_the_visible_state() {
        let mut state = BroadcastState::new([1, 2, 3]);
        assert!(toggle_participant(&mut state, 99).is_err());
        assert_eq!(state.member_count(), 3);
        assert!(!state.enabled());
        toggle_participant(&mut state, 1).unwrap();
        assert_eq!(state.targets_for(1).unwrap(), vec![1]);
        assert!(pane_broadcasting(&state, 1));
        assert!(!pane_broadcasting(&state, 2));
        toggle_participant(&mut state, 2).unwrap();
        assert_eq!(state.targets_for(1).unwrap(), vec![1, 2]);
        toggle_participant(&mut state, 1).unwrap();
        assert!(!pane_broadcasting(&state, 1));
        assert!(pane_broadcasting(&state, 2));
        toggle_participant(&mut state, 2).unwrap();
        assert!(!state.enabled());
        state.select_all();
        state.set_enabled(true);
        assert!(group_broadcasting(&state));
        state.set_enabled(false);
        toggle_participant(&mut state, 3).unwrap();
        assert_eq!(state.targets_for(3).unwrap(), vec![3]);
    }

    #[test]
    fn pane_errors_follow_identity_and_are_pruned_when_closed() {
        let mut all = TabBroadcasts::default();
        all.sync([(10, vec![1, 2]), (20, vec![3, 4])]);
        all.pane_errors.insert(2, "Offline".into());
        all.pane_errors.insert(4, "Write failed".into());
        all.sync([(10, vec![2, 1]), (20, vec![3, 4])]);
        assert_eq!(all.pane_errors.len(), 2);
        assert!(!all.pane_errors.contains_key(&1));
        // Moving a pane carries its error; closing it removes the stale alert.
        all.sync([(10, vec![1]), (20, vec![3, 2])]);
        assert_eq!(all.pane_errors.len(), 1);
        assert_eq!(all.pane_errors[&2], "Offline");
    }

    #[test]
    fn new_groups_and_paused_selections_do_not_show_active_broadcast() {
        let mut all = TabBroadcasts::default();
        all.sync([(10, vec![1, 2, 3])]);
        let state = all.tabs.get_mut(&10).unwrap();
        for id in [1, 2, 3] {
            assert!(state.is_member(id));
            assert!(!pane_broadcasting(state, id));
            assert_eq!(state.targets_for(id).unwrap(), vec![id]);
        }
        state.set_enabled(true);
        state.set_member(3, false).unwrap();
        assert!(pane_broadcasting(state, 1));
        assert!(pane_broadcasting(state, 2));
        assert!(!pane_broadcasting(state, 3));
        assert_eq!(state.targets_for(1).unwrap(), vec![1, 2]);
        assert_eq!(state.targets_for(3).unwrap(), vec![3]);
        state.set_enabled(false);
        assert!(state.is_member(1));
        for id in [1, 2, 3] {
            assert!(!pane_broadcasting(state, id));
            assert_eq!(state.targets_for(id).unwrap(), vec![id]);
        }
    }

    #[test]
    fn merging_into_a_disabled_tab_does_not_inherit_a_source_indicator() {
        let mut all = TabBroadcasts::default();
        all.sync([(10, vec![1]), (20, vec![2])]);
        all.toggle_group(20);
        assert!(pane_broadcasting(&all.tabs[&20], 2));
        // The moved pane is now in the destination's authoritative snapshot.
        all.sync([(10, vec![1, 2])]);
        assert!(!all.tabs.contains_key(&20));
        for id in [1, 2] {
            assert!(all.tabs[&10].is_member(id));
            assert!(!pane_broadcasting(&all.tabs[&10], id));
            assert_eq!(all.tabs[&10].targets_for(id).unwrap(), vec![id]);
        }
        all.toggle_group(10);
        for id in [1, 2] {
            assert!(pane_broadcasting(&all.tabs[&10], id));
            assert_eq!(all.tabs[&10].targets_for(id).unwrap(), vec![1, 2]);
        }
    }

    #[test]
    fn group_button_enables_every_pane_even_after_independent_selection_or_none() {
        let mut all = TabBroadcasts::default();
        all.sync([(10, vec![1, 2, 3])]);
        all.tabs.get_mut(&10).unwrap().set_member(3, false).unwrap();
        all.toggle_group(10);
        for source in [1, 2, 3] {
            assert_eq!(all.tabs[&10].targets_for(source).unwrap(), vec![1, 2, 3]);
        }
        all.tabs.get_mut(&10).unwrap().select_none();
        all.toggle_group(10);
        assert!(all.tabs[&10].enabled());
        assert_eq!(all.tabs[&10].targets_for(3).unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn tab_indicator_and_select_all_follow_partial_selection() {
        let mut all = TabBroadcasts::default();
        all.sync([(10, vec![1, 2, 3, 4]), (20, vec![5, 6])]);
        assert!(!group_broadcasting(&all.tabs[&10]));
        all.toggle_group(10);
        assert!(group_broadcasting(&all.tabs[&10]));
        all.tabs.get_mut(&10).unwrap().set_member(3, false).unwrap();
        assert!(!group_broadcasting(&all.tabs[&10]));
        assert!(pane_broadcasting(&all.tabs[&10], 1));
        assert_eq!(all.tabs[&10].targets_for(1).unwrap(), vec![1, 2, 4]);
        assert_eq!(all.tabs[&10].targets_for(3).unwrap(), vec![3]);
        // A gray partial selection must select all, not disable the others.
        all.toggle_group(10);
        assert!(group_broadcasting(&all.tabs[&10]));
        for pane in [1, 2, 3, 4] {
            assert_eq!(all.tabs[&10].targets_for(pane).unwrap(), vec![1, 2, 3, 4]);
        }
        assert!(!group_broadcasting(&all.tabs[&20]));
        // Completing the selection individually updates the indicator too.
        all.tabs.get_mut(&10).unwrap().set_member(3, false).unwrap();
        all.tabs.get_mut(&10).unwrap().set_member(3, true).unwrap();
        assert!(group_broadcasting(&all.tabs[&10]));
        all.tabs.get_mut(&10).unwrap().set_enabled(false);
        assert!(!group_broadcasting(&all.tabs[&10]));
        all.toggle_group(10);
        assert!(group_broadcasting(&all.tabs[&10]));
        all.toggle_group(10);
        assert!(!group_broadcasting(&all.tabs[&10]));
        assert_eq!(all.tabs[&10].member_count(), 0);
        assert!(!group_broadcasting(&BroadcastState::default()));
    }

    #[test]
    fn group_button_disables_all_members_without_touching_other_tabs() {
        let mut all = TabBroadcasts::default();
        all.sync([(10, vec![1, 2, 3]), (20, vec![4, 5])]);
        all.toggle_group(10);
        all.toggle_group(20);
        all.toggle_group(10);
        assert!(!all.tabs[&10].enabled());
        assert_eq!(all.tabs[&10].member_count(), 0);
        for source in [1, 2, 3] {
            assert_eq!(all.tabs[&10].targets_for(source).unwrap(), vec![source]);
        }
        assert_eq!(all.tabs[&20].targets_for(4).unwrap(), vec![4, 5]);
        // Re-enable from the authoritative layout after a close and a new pane.
        all.sync([(10, vec![1, 3, 6]), (20, vec![4, 5])]);
        all.toggle_group(10);
        assert_eq!(all.tabs[&10].targets_for(6).unwrap(), vec![1, 3, 6]);
        all.sync([(20, vec![4, 5])]);
        all.toggle_group(10);
        assert!(!all.tabs.contains_key(&10));
    }

    #[test]
    fn tabs_are_isolated_and_closed_panes_are_pruned() {
        let mut all = TabBroadcasts::default();
        all.sync([(10, vec![1, 2, 3, 4]), (20, vec![5, 6])]);
        let state = all.tabs.get_mut(&10).unwrap();
        state.set_member(4, false).unwrap();
        state.set_enabled(true);
        assert_eq!(state.targets_for(1).unwrap(), vec![1, 2, 3]);
        assert_eq!(state.targets_for(4).unwrap(), vec![4]);
        assert_eq!(all.tabs[&20].targets_for(5).unwrap(), vec![5]);
        all.sync([(10, vec![1, 3, 4, 7]), (20, vec![5, 6])]);
        assert_eq!(all.tabs[&10].targets_for(1).unwrap(), vec![1, 3, 7]);
        assert!(!all.tabs[&10].is_member(4));
        all.sync([(20, vec![5, 6])]);
        assert!(!all.tabs.contains_key(&10));
    }
}
