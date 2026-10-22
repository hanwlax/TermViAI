//! Reorder whole tabs, or dock a single-session tab, without reconnecting.

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
    can_dock: bool,
    pane_reorder: bool,
    reorder: Option<(Option<usize>, f32)>,
}

pub(super) struct DragVisuals {
    pub preview: Option<Rect>,
    pub pointer: (f32, f32),
    pub title: String,
    pub insertion_x: Option<f32>,
    pub swapping: bool,
}

impl TabDrag {
    fn update_pointer(&mut self, x: f32, y: f32) {
        self.pointer = (x, y);
        self.moved |= drag_threshold_reached(self.start, self.pointer);
        // Leaving a valid target must immediately discard its previous preview.
        self.target = None;
        self.reorder = None;
    }
}

/// Match visible tab halves to stable insertion anchors in the full tab list.
/// Virtual New Tab and window controls are excluded by the strip snapshot.
fn strip_insertion(
    visible: &[(usize, Rect)],
    all: &[usize],
    x: f32,
    y: f32,
) -> Option<(Option<usize>, f32)> {
    let (_, first) = visible.first()?;
    let (_, last) = visible.last()?;
    if !x.is_finite()
        || !y.is_finite()
        || y < first.y
        || y >= first.y + first.h
        || x < first.x - 8.
        || x >= last.x + last.w + 8.
    {
        return None;
    }
    for (id, rect) in visible {
        if x < rect.x + rect.w / 2. {
            return Some((Some(*id), rect.x));
        }
    }
    let (last_id, rect) = visible.last()?;
    let index = all.iter().position(|id| id == last_id)?;
    Some((all.get(index + 1).copied(), rect.x + rect.w))
}

fn reorder_target(
    visible: &[(usize, Rect)],
    all: &[usize],
    source: usize,
    x: f32,
    y: f32,
) -> Option<(Option<usize>, f32)> {
    let (before, marker) = strip_insertion(visible, all, x, y)?;
    let from = all.iter().position(|id| *id == source)?;
    let boundary = match before {
        Some(id) => all.iter().position(|candidate| *candidate == id)?,
        None => all.len(),
    };
    // Both halves of the source tab resolve to its current position. Hide
    // those markers, and adjacent drop slots that would also leave it there.
    let to = boundary - usize::from(from < boundary);
    (from != to).then_some((before, marker))
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
    /// Any real tab can reorder. Only inactive, single-session tabs can dock.
    pub(super) fn begin_termviai_tab_drag(&mut self, tab_id: usize, x: f32, y: f32) -> bool {
        self.termviai_ui.tab_drag = None;
        if !self.config.termviai_ui || self.get_modal().is_some() {
            return false;
        }
        let mux = Mux::get();
        let Some(destination) = mux.get_active_tab_for_window(self.mux_window_id) else {
            return false;
        };
        let Some(source_tab) = mux.get_tab(tab_id) else {
            return false;
        };
        let Some(source) = source_tab.get_active_pane() else {
            return false;
        };
        if !mux
            .get_window(self.mux_window_id)
            .is_some_and(|w| w.get_tab_idx_for_id(tab_id).is_some())
        {
            return false;
        }
        // Drop each RefCell guard before inspecting another tab or pane.
        let source_tab_overlay = self.tab_state(tab_id).overlay.is_some();
        let destination_overlay = self.tab_state(destination.tab_id()).overlay.is_some();
        let source_pane_overlay = self.pane_state(source.pane_id()).overlay.is_some();
        let can_dock = !self.termviai_library_active()
            && destination.tab_id() != tab_id
            && destination.get_zoomed_pane().is_none()
            && valid_source(&mux, self.mux_window_id, tab_id, source.pane_id())
            && !source_tab_overlay
            && !destination_overlay
            && !source_pane_overlay;
        self.termviai_ui.tab_drag = Some(TabDrag {
            source_tab: tab_id,
            source_pane: source.pane_id(),
            destination_tab: destination.tab_id(),
            start: (x, y),
            pointer: (x, y),
            title: self.termviai_tab_label(&source_tab),
            moved: false,
            target: None,
            can_dock,
            pane_reorder: false,
            reorder: None,
        });
        true
    }

    pub(super) fn begin_termviai_pane_drag(&mut self, pane: usize, x: f32, y: f32) {
        let mux = Mux::get();
        let Some(tab) = mux.get_active_tab_for_window(self.mux_window_id) else {
            return;
        };
        if tab.get_zoomed_pane().is_some()
            || tab.iter_panes_ignoring_zoom().len() < 2
            || !tab.contains_pane(pane)
            || self.termviai_library_active()
            || self.get_modal().is_some()
            || self.tab_state(tab.tab_id()).overlay.is_some()
            || self.pane_state(pane).overlay.is_some()
        {
            return;
        }
        self.termviai_ui.tab_drag = Some(TabDrag {
            source_tab: tab.tab_id(),
            source_pane: pane,
            destination_tab: tab.tab_id(),
            start: (x, y),
            pointer: (x, y),
            title: self.termviai_pane_label(pane),
            moved: false,
            target: None,
            can_dock: false,
            pane_reorder: true,
            reorder: None,
        });
    }

    pub(super) fn update_termviai_tab_drag(&mut self, x: f32, y: f32) {
        let Some(mut drag) = self.termviai_ui.tab_drag.take() else {
            return;
        };
        drag.update_pointer(x, y);
        if drag.pane_reorder {
            let tab = Mux::get().get_active_tab_for_window(self.mux_window_id);
            if drag.moved
                && !self.termviai_library_active()
                && self.get_modal().is_none()
                && tab.as_ref().is_some_and(|t| {
                    t.tab_id() == drag.source_tab
                        && t.get_zoomed_pane().is_none()
                        && t.contains_pane(drag.source_pane)
                })
            {
                drag.target = self
                    .termviai_pane_headers()
                    .into_iter()
                    .find(|h| h.pane_id != drag.source_pane && h.frame.contains(x, y))
                    .map(|h| DropTarget {
                        tab: drag.source_tab,
                        pane: h.pane_id,
                        side: DockSide::Left,
                        rect: h.frame,
                    });
            }
            self.termviai_ui.tab_drag = Some(drag);
            if let Some(window) = &self.window {
                window.invalidate();
            }
            return;
        }
        if drag.moved && self.get_modal().is_none() {
            let mux = Mux::get();
            let all = mux
                .get_window(self.mux_window_id)
                .map(|w| w.iter_tabs().map(|tab| tab.tab_id()).collect::<Vec<_>>())
                .unwrap_or_default();
            drag.reorder = reorder_target(&self.termviai_tab_strip(), &all, drag.source_tab, x, y);
        }
        if drag.moved
            && drag.reorder.is_none()
            && drag.can_dock
            && !self.termviai_library_active()
            && self.get_modal().is_none()
        {
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
        if self.get_modal().is_some() {
            return Ok(true);
        }
        if drag.pane_reorder {
            if let Some(target) = drag.target {
                let mux = Mux::get();
                let tab = mux
                    .get_active_tab_for_window(self.mux_window_id)
                    .context("The group was closed")?;
                anyhow::ensure!(
                    tab.tab_id() == drag.source_tab && !self.termviai_library_active(),
                    "The active group changed"
                );
                anyhow::ensure!(
                    self.tab_state(tab.tab_id()).overlay.is_none()
                        && self.pane_state(drag.source_pane).overlay.is_none()
                        && self.pane_state(target.pane).overlay.is_none(),
                    "A session has an open overlay"
                );
                tab.swap_panes_by_id(drag.source_pane, target.pane)?;
                self.termviai_sync_terminal_layout();
                self.termviai_workspace_changed();
                self.update_title();
            }
            return Ok(true);
        }
        if let Some((before, _)) = drag.reorder {
            let mux = Mux::get();
            let mut window = mux
                .get_window_mut(self.mux_window_id)
                .context("The window closed.")?;
            window.reorder_tab_before(drag.source_tab, before)?;
            drop(window);
            self.update_title();
            if let Some(window) = &self.window {
                window.invalidate();
            }
            return Ok(true);
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
                    tw.termviai_ui
                        .terminal_errors
                        .insert(target.tab, format!("Could not place session: {error:#}"));
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
                window.set_cursor(Some(window::CursorIcon::Default));
                window.invalidate();
            }
        }
    }

    pub(super) fn termviai_tab_drag_visuals(&self) -> Option<DragVisuals> {
        let drag = self.termviai_ui.tab_drag.as_ref()?;
        drag.moved.then(|| DragVisuals {
            preview: drag.target.map(|target| {
                if drag.pane_reorder {
                    target.rect
                } else {
                    target.rect.preview(target.side)
                }
            }),
            swapping: drag.pane_reorder,
            pointer: drag.pointer,
            title: drag.title.clone(),
            insertion_x: drag.reorder.map(|(_, x)| x),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_drop_uses_midpoints_and_full_order_for_scrolled_tabs() {
        let visible = [
            (
                20,
                Rect {
                    x: 220.,
                    y: 8.,
                    w: 202.,
                    h: 36.,
                },
            ),
            (
                30,
                Rect {
                    x: 430.,
                    y: 8.,
                    w: 202.,
                    h: 36.,
                },
            ),
        ];
        let all = [10, 20, 30, 40];
        assert_eq!(
            strip_insertion(&visible, &all, 240., 20.),
            Some((Some(20), 220.))
        );
        assert_eq!(
            strip_insertion(&visible, &all, 380., 20.),
            Some((Some(30), 430.))
        );
        // Last visible tab isn't the last real tab: insert before hidden tab 40.
        assert_eq!(
            strip_insertion(&visible, &all, 620., 20.),
            Some((Some(40), 632.))
        );
        assert_eq!(
            strip_insertion(&visible, &[10, 20, 30], 620., 20.),
            Some((None, 632.))
        );
        assert!(strip_insertion(&visible, &all, 620., 100.).is_none());
        assert!(strip_insertion(&visible, &all, 700., 20.).is_none());
        assert!(strip_insertion(&visible, &all, f32::NAN, 20.).is_none());
        assert!(strip_insertion(&[], &all, 240., 20.).is_none());
        // No marker on either half of the dragged tab, including a last tab.
        for x in [240., 380.] {
            assert!(reorder_target(&visible, &all, 20, x, 20.).is_none());
        }
        for x in [450., 620.] {
            assert!(reorder_target(&visible, &all, 30, x, 20.).is_none());
            assert!(reorder_target(&visible, &[10, 20, 30], 30, x, 20.).is_none());
        }
        assert_eq!(
            reorder_target(&visible, &all, 30, 240., 20.),
            Some((Some(20), 220.))
        );
        assert_eq!(
            reorder_target(&visible, &all, 20, 620., 20.),
            Some((Some(40), 632.))
        );
    }

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
            can_dock: true,
            pane_reorder: false,
            reorder: None,
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
        drag.reorder = Some((Some(3), 240.));
        drag.update_pointer(240.0, 180.0);
        assert!(drag.reorder.is_none());
    }
}
