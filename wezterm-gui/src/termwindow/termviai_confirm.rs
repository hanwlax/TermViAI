//! Native close confirmations. The target is captured when the card opens so
//! a later tab switch or pane move cannot change what the user is confirming.
use super::termviai_motion::Tween;
use super::TermWindow;
use crate::frontend::front_end;
use config::WindowCloseConfirmation;
use mux::Mux;
use std::time::{Duration, Instant};
use window::{Connection, ConnectionOps, WindowOps};

const ENTER: Duration = Duration::from_millis(160);
const EXIT: Duration = Duration::from_millis(140);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CloseTarget {
    Pane { tab_id: usize, pane_id: usize },
    Tab(usize),
    Window,
    Application,
}

#[derive(Clone, Debug)]
pub(super) struct ClosePrompt {
    pub(super) title: String,
    pub(super) detail: String,
    pub(super) summary: String,
    pub(super) confirm_label: String,
    pub(super) target: CloseTarget,
    window_id: usize,
}

pub(super) struct ConfirmState {
    pub(super) prompt: Option<ClosePrompt>,
    pub(super) motion: Tween,
    pub(super) closing: bool,
    pub(super) focused_confirm: bool,
    quitting: bool,
}

impl Default for ConfirmState {
    fn default() -> Self {
        Self {
            prompt: None,
            motion: Tween::new(0.),
            closing: false,
            focused_confirm: false,
            quitting: false,
        }
    }
}

impl ConfirmState {
    pub(super) fn is_active(&self) -> bool {
        // Retain the modal input shield through the exit animation.
        self.prompt.is_some()
    }

    fn open(&mut self, prompt: ClosePrompt, now: Instant) {
        if self.is_active() {
            return;
        }
        self.prompt = Some(prompt);
        self.closing = false;
        self.focused_confirm = false;
        self.quitting = false;
        self.motion.set_target(1., now, ENTER);
    }

    fn dismiss(&mut self, now: Instant) {
        if self.is_active() && !self.closing {
            self.closing = true;
            self.motion.set_target(0., now, EXIT);
        }
    }

    fn accept(&mut self, now: Instant) -> Option<(CloseTarget, usize)> {
        if self.closing {
            return None;
        }
        let prompt = self.prompt.as_ref()?;
        let target = (prompt.target, prompt.window_id);
        if target.0 == CloseTarget::Application {
            // Keep the card visible and input modal until the
            // message loop ends, instead of briefly reopening the terminal.
            self.quitting = true;
            self.closing = true;
            self.motion.set_target(1., now, ENTER);
            if let Some(prompt) = self.prompt.as_mut() {
                prompt.confirm_label = "Closing…".into();
                prompt.detail = "Closing all windows and connections…".into();
            }
        } else {
            self.dismiss(now);
        }
        Some(target)
    }

    pub(super) fn settle(&mut self, now: Instant) {
        if self.closing && !self.quitting && !self.motion.is_active(now) {
            self.prompt = None;
            self.closing = false;
            self.focused_confirm = false;
        }
    }
}

fn quit_needs_confirmation(never_prompt: bool, has_connections: bool) -> bool {
    !never_prompt && has_connections
}

fn count_label(count: usize, singular: &str) -> String {
    format!("{count} {singular}{}", if count == 1 { "" } else { "s" })
}

/// Checking the whole ownership chain also rejects a pane moved into another
/// tab in this window, or a tab moved to a different window while the card is up.
fn target_is_current(
    target: CloseTarget,
    expected_window: usize,
    current_window: usize,
    tabs: &[(usize, Vec<usize>)],
) -> bool {
    if target == CloseTarget::Application {
        return true;
    }
    if expected_window != current_window {
        return false;
    }
    match target {
        CloseTarget::Pane { tab_id, pane_id } => tabs
            .iter()
            .any(|(id, panes)| *id == tab_id && panes.contains(&pane_id)),
        CloseTarget::Tab(tab_id) => tabs.iter().any(|(id, _)| *id == tab_id),
        CloseTarget::Window => true,
        CloseTarget::Application => unreachable!(),
    }
}

fn window_targets(window_id: usize) -> Vec<(usize, Vec<usize>)> {
    Mux::get()
        .get_window(window_id)
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
                .collect()
        })
        .unwrap_or_default()
}

impl TermWindow {
    pub(super) fn termviai_request_quit(&mut self) -> anyhow::Result<()> {
        if self.termviai_confirm.is_active() {
            return Ok(());
        }
        let never_prompt = matches!(
            self.config.window_close_confirmation,
            WindowCloseConfirmation::NeverPrompt
        );
        let has_connections = front_end().gui_windows().iter().any(|gui| {
            window_targets(gui.mux_window_id)
                .iter()
                .any(|(_, panes)| !panes.is_empty())
        });
        if quit_needs_confirmation(never_prompt, has_connections) {
            self.termviai_request_close(CloseTarget::Application);
        } else {
            self.termviai_close_target(CloseTarget::Application, self.mux_window_id)?;
        }
        Ok(())
    }

    pub(super) fn termviai_request_close(&mut self, target: CloseTarget) {
        if self.termviai_confirm.is_active() {
            return;
        }
        let tabs = window_targets(self.mux_window_id);
        if !target_is_current(target, self.mux_window_id, self.mux_window_id, &tabs) {
            return;
        }
        let (title, detail, summary, confirm_label) = match target {
            CloseTarget::Pane { pane_id, .. } => (
                "Disconnect terminal?".to_string(),
                "This terminal connection will be closed.".to_string(),
                format!("1 connection · {}", self.termviai_pane_label(pane_id)),
                "Disconnect".to_string(),
            ),
            CloseTarget::Tab(tab_id) => {
                let Some(tab) = Mux::get().get_tab(tab_id) else {
                    return;
                };
                let count = tab.iter_panes_ignoring_zoom().len();
                (
                    if count > 1 {
                        "Close tab group?"
                    } else {
                        "Close tab?"
                    }
                    .to_string(),
                    if count > 1 {
                        "All connections in this group will be disconnected."
                    } else {
                        "The connection in this tab will be disconnected."
                    }
                    .to_string(),
                    format!(
                        "{} · {}",
                        count_label(count, "connection"),
                        self.termviai_tab_label(&tab)
                    ),
                    if count > 1 {
                        "Disconnect all"
                    } else {
                        "Disconnect"
                    }
                    .to_string(),
                )
            }
            CloseTarget::Window => (
                "Close window?".to_string(),
                "All connections in this window will be disconnected.".to_string(),
                format!(
                    "{} in {}",
                    count_label(
                        tabs.iter().map(|(_, panes)| panes.len()).sum(),
                        "connection"
                    ),
                    count_label(tabs.len(), "tab")
                ),
                "Close window".to_string(),
            ),
            CloseTarget::Application => {
                let windows = front_end().gui_windows();
                let connections = windows
                    .iter()
                    .flat_map(|w| window_targets(w.mux_window_id))
                    .map(|(_, panes)| panes.len())
                    .sum();
                (
                    "Quit TermViAI?".to_string(),
                    "All windows and connections will close.".to_string(),
                    format!(
                        "{} in {}",
                        count_label(connections, "connection"),
                        count_label(windows.len(), "window")
                    ),
                    "Quit TermViAI".to_string(),
                )
            }
        };
        self.cancel_modal();
        self.termviai_prepare_close_confirmation();
        self.termviai_confirm.open(
            ClosePrompt {
                title,
                detail,
                summary,
                confirm_label,
                target,
                window_id: self.mux_window_id,
            },
            Instant::now(),
        );
        if let Some(window) = &self.window {
            window.invalidate();
        }
    }

    pub(super) fn termviai_cancel_close(&mut self) {
        self.termviai_confirm.dismiss(Instant::now());
        if let Some(window) = &self.window {
            window.invalidate();
        }
    }

    pub(super) fn termviai_accept_close(&mut self) -> anyhow::Result<()> {
        let target = self.termviai_confirm.accept(Instant::now());
        if let Some(window) = &self.window {
            window.invalidate();
        }
        if let Some((target, window_id)) = target {
            self.termviai_close_target(target, window_id)?;
        }
        Ok(())
    }

    pub(super) fn termviai_close_target(
        &mut self,
        target: CloseTarget,
        expected_window: usize,
    ) -> anyhow::Result<()> {
        if !target_is_current(
            target,
            expected_window,
            self.mux_window_id,
            &window_targets(self.mux_window_id),
        ) {
            // The target was closed or moved while waiting. Do not substitute
            // the current tab/pane or disconnect it in its new window.
            return Ok(());
        }
        let mux = Mux::get();
        match target {
            CloseTarget::Pane { pane_id, .. } => {
                mux.remove_pane(pane_id);
            }
            CloseTarget::Tab(tab_id) => {
                mux.remove_tab(tab_id);
            }
            CloseTarget::Window => {
                mux.kill_window(expected_window);
                if let Some(window) = &self.window {
                    window.close();
                    front_end().forget_known_window(window);
                }
                return Ok(());
            }
            CloseTarget::Application => {
                Connection::get()
                    .expect("call on gui thread")
                    .terminate_message_loop();
                return Ok(());
            }
        }
        self.termviai_sync_broadcast();
        self.update_title();
        if let Some(window) = &self.window {
            window.invalidate();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prompt(target: CloseTarget) -> ClosePrompt {
        ClosePrompt {
            title: "Close tab?".into(),
            detail: "Disconnect this connection.".into(),
            summary: "host-label · 1 connection".into(),
            confirm_label: "Disconnect".into(),
            target,
            window_id: 5,
        }
    }

    #[test]
    fn empty_application_can_quit_but_connections_in_other_windows_require_confirmation() {
        assert!(!quit_needs_confirmation(false, false));
        assert!(quit_needs_confirmation(false, true));
        assert!(!quit_needs_confirmation(true, true));
        assert!(!quit_needs_confirmation(true, false));
    }

    #[test]
    fn cancel_keeps_input_modal_until_exit_finishes() {
        let now = Instant::now();
        let mut state = ConfirmState::default();
        state.open(prompt(CloseTarget::Tab(11)), now);
        assert!(state.is_active());
        assert!(!state.focused_confirm);
        assert_eq!(state.motion.value(now + ENTER), 1.);
        state.dismiss(now + ENTER);
        state.settle(now + ENTER + EXIT / 2);
        assert!(state.is_active());
        assert!(state.closing);
        assert!(state.accept(now + ENTER + EXIT / 2).is_none());
        state.settle(now + ENTER + EXIT);
        assert!(!state.is_active());
        assert!(!state.closing);
    }

    #[test]
    fn accepting_is_once_only_and_keeps_the_original_target() {
        let now = Instant::now();
        let mut state = ConfirmState::default();
        state.open(prompt(CloseTarget::Tab(11)), now);
        state.open(prompt(CloseTarget::Tab(22)), now + ENTER);
        assert_eq!(state.accept(now + ENTER), Some((CloseTarget::Tab(11), 5)));
        assert!(state.accept(now + ENTER).is_none());
        state.settle(now + ENTER + EXIT);
        state.open(prompt(CloseTarget::Tab(22)), now + ENTER + EXIT);
        assert_eq!(
            state.accept(now + ENTER + EXIT),
            Some((CloseTarget::Tab(22), 5))
        );
    }

    #[test]
    fn accepted_quit_stays_visible_and_modal_until_the_application_exits() {
        let now = Instant::now();
        let mut state = ConfirmState::default();
        state.open(prompt(CloseTarget::Application), now);
        assert_eq!(
            state.accept(now + ENTER),
            Some((CloseTarget::Application, 5))
        );
        let later = now + Duration::from_secs(3);
        state.settle(later);
        assert!(state.is_active());
        assert!(state.closing);
        assert_eq!(state.motion.value(later), 1.);
        assert_eq!(state.prompt.as_ref().unwrap().confirm_label, "Closing…");
        state.dismiss(later);
        assert!(state.accept(later).is_none());
        assert_eq!(state.motion.value(later + EXIT), 1.);
    }

    #[test]
    fn cancelled_quit_exits_normally_and_restores_input() {
        let now = Instant::now();
        let mut state = ConfirmState::default();
        state.open(prompt(CloseTarget::Application), now);
        state.dismiss(now + ENTER);
        state.settle(now + ENTER + EXIT);
        assert!(!state.is_active());
        assert!(!state.closing);
        assert_eq!(state.motion.value(now + ENTER + EXIT), 0.);
    }

    #[test]
    fn target_check_rejects_a_pane_moved_to_another_tab_or_window() {
        let target = CloseTarget::Pane {
            tab_id: 11,
            pane_id: 3,
        };
        let original = vec![(11, vec![3, 4]), (22, vec![6])];
        assert!(target_is_current(target, 5, 5, &original));
        assert!(!target_is_current(target, 5, 9, &original));
        let moved = vec![(11, vec![4]), (22, vec![3, 6])];
        assert!(!target_is_current(target, 5, 5, &moved));
        assert!(!target_is_current(target, 5, 5, &[]));
    }

    #[test]
    fn tab_target_does_not_depend_on_order_or_active_tab() {
        let tabs = vec![(22, vec![4]), (11, vec![3])];
        assert!(target_is_current(CloseTarget::Tab(11), 5, 5, &tabs));
        assert!(!target_is_current(CloseTarget::Tab(11), 5, 5, &tabs[..1]));
        assert!(!target_is_current(CloseTarget::Window, 5, 9, &tabs));
        assert!(target_is_current(CloseTarget::Window, 5, 5, &[]));
    }
}
