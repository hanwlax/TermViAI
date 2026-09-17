use std::collections::HashSet;
use std::fmt;
use std::hash::Hash;

/// A pane is not present in this tab's latest authoritative Mux snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnknownPane<P>(pub P);

impl<P: fmt::Debug> fmt::Display for UnknownPane<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "pane {:?} is not in this tab", self.0)
    }
}

impl<P: fmt::Debug> std::error::Error for UnknownPane<P> {}

/// Own one instance per runtime TabId. P can be mux::pane::PaneId (usize).
///
/// `panes` is a membership snapshot supplied by Mux, not a lifecycle owner.
/// Disconnected panes must remain in the snapshot until actually closed.
/// Runtime IDs and this state must not be serialized directly for restore.
#[derive(Debug, Clone)]
pub struct BroadcastState<P> {
    enabled: bool,
    targets: HashSet<P>,
    panes: HashSet<P>,
}

impl<P: Copy + Eq + Hash + Ord> Default for BroadcastState<P> {
    fn default() -> Self {
        Self {
            enabled: false,
            targets: HashSet::new(),
            panes: HashSet::new(),
        }
    }
}

impl<P: Copy + Eq + Hash + Ord> BroadcastState<P> {
    pub fn new(panes: impl IntoIterator<Item = P>) -> Self {
        let mut state = Self::default();
        state.sync_panes(panes);
        state
    }

    /// Refresh using all tab panes, including panes hidden by zoom.
    /// New panes join by default; closed panes leave; existing exclusions stay.
    pub fn sync_panes(&mut self, panes: impl IntoIterator<Item = P>) {
        let current: HashSet<_> = panes.into_iter().collect();
        self.targets.retain(|pane| current.contains(pane));
        self.targets
            .extend(current.difference(&self.panes).copied());
        self.panes = current;
        if self.targets.is_empty() {
            self.enabled = false;
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Turning broadcast off preserves the member set.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled && !self.targets.is_empty();
    }

    pub fn toggle_enabled(&mut self) {
        self.set_enabled(!self.enabled);
    }

    pub fn is_member(&self, pane: P) -> bool {
        self.targets.contains(&pane)
    }

    pub fn member_count(&self) -> usize {
        self.targets.len()
    }

    pub fn pane_count(&self) -> usize {
        self.panes.len()
    }

    pub fn set_member(&mut self, pane: P, member: bool) -> Result<(), UnknownPane<P>> {
        self.require_pane(pane)?;
        if member {
            self.targets.insert(pane);
        } else {
            self.targets.remove(&pane);
            if self.targets.is_empty() {
                self.enabled = false;
            }
        }
        Ok(())
    }

    pub fn toggle_member(&mut self, pane: P) -> Result<(), UnknownPane<P>> {
        self.set_member(pane, !self.is_member(pane))
    }

    /// Select all without implicitly enabling broadcast.
    pub fn select_all(&mut self) {
        self.targets.clone_from(&self.panes);
    }

    pub fn select_none(&mut self) {
        self.targets.clear();
        self.enabled = false;
    }

    /// A non-member source always remains independent, even when enabled.
    pub fn targets_for(&self, source: P) -> Result<Vec<P>, UnknownPane<P>> {
        self.require_pane(source)?;
        if !self.enabled || !self.is_member(source) {
            return Ok(vec![source]);
        }
        let mut targets: Vec<_> = self.targets.iter().copied().collect();
        targets.sort_unstable();
        Ok(targets)
    }

    fn require_pane(&self, pane: P) -> Result<(), UnknownPane<P>> {
        if self.panes.contains(&pane) {
            Ok(())
        } else {
            Err(UnknownPane(pane))
        }
    }
}
