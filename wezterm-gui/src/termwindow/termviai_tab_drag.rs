//! Move an existing single-session tab into another tab without reconnecting.

use super::termviai_dock::{drag_threshold_reached, DockSide, Rect};
use super::{TermWindow, TermWindowNotif};
use anyhow::Context;
use config::keyassignment::SpawnTabDomain;
use mux::domain::{DomainState, SplitSource};
use mux::localpane::LocalPane;
use mux::tab::Tab;
use mux::Mux;
use window::WindowOps;

#[derive(Clone, Copy, Debug)]
struct DropTarget {
    tab: usize,
    pane: usize,
    side: DockSide,
    rect: Rect,
}

pub(super) struct TabDrag {
    source_tab: usize,
    source_pane: usize,
    destination_tab: usize,
    start: (f32, f32),
    pointer: (f32, f32),
    title: String,
    moved: bool,
    target: Option<DropTarget>,
}

pub(super) struct DragVisuals {
    pub preview: Option<Rect>,
    pub pointer: (f32, f32),
    pub title: String,
}

impl TabDrag {
    fn update_pointer(&mut self, x: f32, y: f32) {
        self.pointer = (x, y);
        self.moved |= drag_threshold_reached(self.start, self.pointer);
        // Leaving a valid target must immediately discard its previous preview.
        self.target = None;
    }
}

fn valid_source(mux: &Mux, window_id: usize, tab_id: usize, pane_id: usize) -> bool {
    let Some(tab) = mux.get_tab(tab_id) else {
        return false;
    };
    let panes = tab.iter_panes_ignoring_zoom();
    panes.len() == 1
        && panes[0].pane.pane_id() == pane_id
        && !panes[0].pane.is_dead()
        && panes[0].pane.downcast_ref::<LocalPane>().is_some()
        && mux.resolve_pane_id(pane_id).map(|(_, w, t)| (w, t)) == Some((window_id, tab_id))
        && mux
            .get_domain(panes[0].pane.domain_id())
            .map(|domain| domain.state() == DomainState::Attached)
            .unwrap_or(false)
}

/// Validate BEFORE MovePane removes the source from its original tab.
fn preflight_drop(
    mux: &Mux,
    window_id: usize,
    drag: &TabDrag,
    target: DropTarget,
) -> anyhow::Result<usize> {
    anyhow::ensure!(
        valid_source(mux, window_id, drag.source_tab, drag.source_pane),
        "The source session was closed, moved, or split. Drag a single-session tab."
    );
    anyhow::ensure!(
        target.tab != drag.source_tab && target.pane != drag.source_pane,
        "Choose another workspace to place this session."
    );
    anyhow::ensure!(
        target.tab == drag.destination_tab
            && mux
                .get_active_tab_for_window(window_id)
                .map(|tab| tab.tab_id())
                == Some(target.tab),
        "The destination workspace changed. Drag the session again."
    );
    let (domain, target_window, target_tab) = mux
        .resolve_pane_id(target.pane)
        .context("The destination session was closed.")?;
    anyhow::ensure!(
        (target_window, target_tab) == (window_id, target.tab),
        "The destination session moved to another workspace."
    );
    let tab = mux
        .get_tab(target.tab)
        .context("The destination tab closed.")?;
    anyhow::ensure!(
        tab.get_zoomed_pane().is_none(),
        "Restore the destination pane before adding another session."
    );
    let panes = tab.iter_panes_ignoring_zoom();
    let pane = panes
        .iter()
        .find(|pane| pane.pane.pane_id() == target.pane)
        .context("The destination pane closed.")?;
    anyhow::ensure!(
        !pane.pane.is_dead() && pane.pane.downcast_ref::<LocalPane>().is_some(),
        "This destination cannot accept a dragged session."
    );
    anyhow::ensure!(
        mux.get_domain(domain)
            .map(|domain| domain.state() == DomainState::Attached)
            .unwrap_or(false),
        "The destination connection is detached."
    );
    anyhow::ensure!(
        split_fits(&tab, pane.index, target.side),
        "This pane is too small to split. Enlarge the window and try again."
    );
    Ok(domain)
}

fn split_fits(tab: &Tab, pane_index: usize, side: DockSide) -> bool {
    // compute_split_size unzooms the tab, so check first to avoid changing it.
    if tab.get_zoomed_pane().is_some() {
        return false;
    }
    let Some(split) = tab.compute_split_size(pane_index, side.split_request()) else {
        return false;
    };
    let size = tab.get_size();
    split.first.rows > 0
        && split.first.cols > 0
        && split.second.rows > 0
        && split.second.cols > 0
        && split.width() <= size.cols
        && split.height() <= size.rows
}

impl TermWindow {
    /// A press on an inactive, single-session tab can become a docking drag.
    /// Return false for ordinary tabs that should retain their existing clicks.
    pub(super) fn begin_termviai_tab_drag(&mut self, tab_id: usize, x: f32, y: f32) -> bool {
        self.termviai_ui.tab_drag = None;
        if !self.config.termviai_ui || self.termviai_library_active() || self.get_modal().is_some() {
            return false;
        }
        let mux = Mux::get();
        let Some(destination) = mux.get_active_tab_for_window(self.mux_window_id) else {
            return false;
        };
        if destination.tab_id() == tab_id || destination.get_zoomed_pane().is_some() {
            return false;
        }
        let Some(source) = mux.get_tab(tab_id).and_then(|tab| tab.get_active_pane()) else {
            return false;
        };
        // Drop each RefCell guard before inspecting another tab or pane.
        let source_tab_overlay = self.tab_state(tab_id).overlay.is_some();
        let destination_overlay = self.tab_state(destination.tab_id()).overlay.is_some();
        let source_pane_overlay = self.pane_state(source.pane_id()).overlay.is_some();
        if !valid_source(&mux, self.mux_window_id, tab_id, source.pane_id())
            || source_tab_overlay
            || destination_overlay
            || source_pane_overlay
        {
            return false;
        }
        self.termviai_ui.tab_drag = Some(TabDrag {
            source_tab: tab_id,
            source_pane: source.pane_id(),
            destination_tab: destination.tab_id(),
            start: (x, y),
            pointer: (x, y),
            title: self.termviai_pane_label(source.pane_id()),
            moved: false,
            target: None,
        });
        true
    }

    pub(super) fn update_termviai_tab_drag(&mut self, x: f32, y: f32) {
        let Some(mut drag) = self.termviai_ui.tab_drag.take() else {
            return;
        };
        drag.update_pointer(x, y);
        if drag.moved && !self.termviai_library_active() && self.get_modal().is_none() {
            let mux = Mux::get();
            if let Some(tab) = mux.get_active_tab_for_window(self.mux_window_id) {
                if tab.tab_id() == drag.destination_tab
                    && tab.get_zoomed_pane().is_none()
                    && self.tab_state(tab.tab_id()).overlay.is_none()
                    && valid_source(&mux, self.mux_window_id, drag.source_tab, drag.source_pane)
                {
                    let scale = self.dimensions.dpi as f32 / 96.0;
                    let (padding_left, padding_top) = self.padding_left_top();
                    let border = self.get_os_border();
                    let tab_height = if self.show_tab_bar {
                        self.tab_bar_pixel_height().unwrap_or(0.0)
                    } else {
                        0.0
                    };
                    let cell_w = self.render_metrics.cell_size.width as f32;
                    let cell_h = self.render_metrics.cell_size.height as f32;
                    for pane in tab.iter_panes() {
                        if self.pane_state(pane.pane.pane_id()).overlay.is_some() {
                            continue;
                        }
                        let rect = Rect {
                            x: (padding_left
                                + border.left.get() as f32
                                + pane.left as f32 * cell_w)
                                / scale,
                            // termviai_mouse already subtracts the OS top border.
                            y: (tab_height + padding_top + pane.top as f32 * cell_h) / scale,
                            w: pane.width as f32 * cell_w / scale,
                            h: pane.height as f32 * cell_h / scale,
                        };
                        if let Some(side) = rect.dock_side(x, y) {
                            let target = DropTarget {
                                tab: tab.tab_id(),
                                pane: pane.pane.pane_id(),
                                side,
                                rect,
                            };
                            if preflight_drop(&mux, self.mux_window_id, &drag, target).is_ok() {
                                drag.target = Some(target);
                            }
                            break;
                        }
                    }
                }
            }
        }
        self.termviai_ui.tab_drag = Some(drag);
        if let Some(window) = &self.window {
            window.invalidate();
        }
    }

    /// True consumes the release even when an actual drag has no valid target.
    /// False lets the caller complete an ordinary tab click below the threshold.
    pub(super) fn finish_termviai_tab_drag(&mut self) -> anyhow::Result<bool> {
        let Some(drag) = self.termviai_ui.tab_drag.take() else {
            return Ok(false);
        };
        if !drag.moved {
            return Ok(false);
        }
        let Some(target) = drag.target else {
            return Ok(true);
        };
        let source_tab_overlay = self.tab_state(drag.source_tab).overlay.is_some();
        let target_tab_overlay = self.tab_state(target.tab).overlay.is_some();
        let source_pane_overlay = self.pane_state(drag.source_pane).overlay.is_some();
        let target_pane_overlay = self.pane_state(target.pane).overlay.is_some();
        if self.termviai_library_active()
            || self.get_modal().is_some()
            || source_tab_overlay
            || target_tab_overlay
            || source_pane_overlay
            || target_pane_overlay
        {
            return Ok(true);
        }
        let window = self.window.clone().context("The window closed.")?;
        let window_id = self.mux_window_id;
        promise::spawn::spawn(async move {
            let mux = Mux::get();
            let result = async {
                // Recheck inside the future, immediately before moving. Attached
                // LocalPane domains take the synchronous MovePane branch.
                let domain = preflight_drop(&mux, window_id, &drag, target)?;
                mux.split_pane(
                    target.pane,
                    target.side.split_request(),
                    SplitSource::MovePane(drag.source_pane),
                    SpawnTabDomain::DomainId(domain),
                )
                .await?;
                Ok::<(), anyhow::Error>(())
            }
            .await;
            window.notify(TermWindowNotif::Apply(Box::new(move |tw| {
                let workspace_changed = result.is_ok();
                if let Err(error) = result {
                    tw.termviai_ui.error = format!("Could not place session: {error:#}");
                }
                tw.termviai_sync_broadcast();
                if workspace_changed {
                    tw.termviai_workspace_changed();
                }
                tw.update_title();
                if let Some(window) = &tw.window {
                    window.invalidate();
                }
            })));
        })
        .detach();
        Ok(true)
    }

    pub(super) fn cancel_termviai_tab_drag(&mut self) {
        if self.termviai_ui.tab_drag.take().is_some() {
            if let Some(window) = &self.window {
                window.invalidate();
            }
        }
    }

    pub(super) fn termviai_tab_drag_visuals(&self) -> Option<DragVisuals> {
        let drag = self.termviai_ui.tab_drag.as_ref()?;
        drag.moved.then(|| DragVisuals {
            preview: drag.target.map(|target| target.rect.preview(target.side)),
            pointer: drag.pointer,
            title: drag.title.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returning_to_tab_after_drag_does_not_become_a_click() {
        let mut drag = TabDrag {
            source_tab: 1,
            source_pane: 2,
            destination_tab: 3,
            start: (240.0, 20.0),
            pointer: (240.0, 20.0),
            title: String::new(),
            moved: false,
            target: None,
        };
        drag.update_pointer(242.0, 23.0);
        assert!(!drag.moved);
        drag.update_pointer(240.0, 180.0);
        assert!(drag.moved);
        drag.target = Some(DropTarget {
            tab: 3,
            pane: 4,
            side: DockSide::Left,
            rect: Rect {
                x: 220.0,
                y: 52.0,
                w: 800.0,
                h: 500.0,
            },
        });
        drag.update_pointer(240.0, 20.0);
        assert!(drag.moved);
        assert!(drag.target.is_none());
    }
}
