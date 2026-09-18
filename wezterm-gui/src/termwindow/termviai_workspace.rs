//! Stable session labels and reconnectable SSH workspace templates.
use super::termviai_ui::Page;
use super::{TermWindow, TermWindowNotif};
use crate::hosts::Host;
use crate::workspaces::{SavedLayout, SplitAxis, WorkspaceStore};
use anyhow::Context;
use config::keyassignment::SpawnTabDomain;
use config::{SshDomain, SshMultiplexing};
use mux::domain::{Domain, SplitSource};
use mux::localpane::LocalPane;
use mux::ssh::RemoteSshDomain;
use mux::tab::{PaneNode, SplitDirection, SplitRequest, SplitSize, Tab};
use mux::Mux;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use wezterm_term::TerminalSize;
use window::WindowOps;

#[derive(Default)]
pub(super) struct WorkspaceUi {
    pub data: WorkspaceStore,
    pub hosts: HashMap<usize, Host>,
    pub saved_ids: HashMap<usize, String>,
    pub renamed_tabs: HashSet<usize>,
    pub reconnecting: HashSet<usize>,
}

fn domain_name(host: &Host, ssh_keepalive_interval: u64) -> String {
    format!(
        "termviai:{}:{}:{}:{}:keepalive={}",
        host.username,
        host.endpoint(),
        host.identity_file,
        host.label,
        ssh_keepalive_interval
    )
}

fn ssh_domain(host: &Host, ssh_keepalive_interval: u64) -> SshDomain {
    let mut domain = SshDomain {
        name: domain_name(host, ssh_keepalive_interval),
        remote_address: host.endpoint(),
        username: (!host.username.is_empty()).then(|| host.username.clone()),
        multiplexing: SshMultiplexing::None,
        ..Default::default()
    };
    domain.ssh_option.insert(
        "serveraliveinterval".into(),
        ssh_keepalive_interval.to_string(),
    );
    if !host.identity_file.is_empty() {
        domain
            .ssh_option
            .insert("identityfile".into(), host.identity_file.clone());
    }
    domain
}

fn snapshot(
    node: PaneNode,
    hosts: &HashMap<usize, Host>,
    mux: &Mux,
) -> anyhow::Result<SavedLayout> {
    match node {
        PaneNode::Empty => anyhow::bail!("This workspace has no sessions."),
        PaneNode::Leaf(entry) => {
            let pane = mux
                .get_pane(entry.pane_id)
                .context("A session was closed.")?;
            let host = hosts
                .get(&pane.domain_id())
                .context("Only SSH hosts opened from TermViAI can be saved.")?;
            Ok(SavedLayout::Pane { host: host.clone() })
        }
        PaneNode::Split { left, right, node } => {
            let (first, second) = match node.direction {
                SplitDirection::Horizontal => (node.first.cols, node.second.cols),
                SplitDirection::Vertical => (node.first.rows, node.second.rows),
            };
            Ok(SavedLayout::Split {
                axis: if node.direction == SplitDirection::Horizontal {
                    SplitAxis::Horizontal
                } else {
                    SplitAxis::Vertical
                },
                first_percent: ((first * 100 + (first + second) / 2) / (first + second).max(1))
                    .clamp(1, 99) as u8,
                first: Box::new(snapshot(*left, hosts, mux)?),
                second: Box::new(snapshot(*right, hosts, mux)?),
            })
        }
    }
}

pub(super) fn default_tab_label(labels: &[String]) -> String {
    match labels {
        [] => "Workspace".into(),
        [label] => label.clone(),
        [first, second] => format!("{} + {}", first, second),
        [first, ..] => format!("{} + {} hosts", first, labels.len() - 1),
    }
}

fn tab_label(custom_title: Option<&str>, labels: &[String]) -> String {
    match custom_title.filter(|title| !title.is_empty()) {
        Some(title) => title.to_owned(),
        None => default_tab_label(labels),
    }
}

fn restored_name_is_custom(name: Option<&str>, layout: &SavedLayout) -> bool {
    let labels = layout
        .hosts()
        .into_iter()
        .map(|host| host.label.clone())
        .collect::<Vec<_>>();
    name.is_some_and(|name| !name.is_empty() && name != default_tab_label(&labels))
}

impl TermWindow {
    pub(super) fn termviai_reconnect_pane(&mut self, pane_id: usize) -> anyhow::Result<()> {
        let mux = Mux::get();
        let pane = match mux.get_pane(pane_id) {
            Some(pane) => pane,
            None => anyhow::bail!("This session was closed."),
        };
        let domain = match mux.get_domain(pane.domain_id()) {
            Some(domain) => domain,
            None => anyhow::bail!("The SSH connection profile is no longer available."),
        };
        anyhow::ensure!(
            pane.downcast_ref::<LocalPane>()
                .is_some_and(LocalPane::is_reconnectable_ssh),
            "This SSH session is still connected."
        );
        anyhow::ensure!(
            domain.downcast_ref::<RemoteSshDomain>().is_some(),
            "This pane is not a reconnectable SSH session."
        );
        let window = self.window.clone().context("The window closed.")?;
        anyhow::ensure!(
            self.termviai_ui.workspace.reconnecting.insert(pane_id),
            "This session is already reconnecting."
        );

        let dimensions = pane.get_dimensions();
        let size = TerminalSize {
            rows: dimensions.viewport_rows,
            cols: dimensions.cols,
            pixel_width: dimensions.pixel_width,
            pixel_height: dimensions.pixel_height,
            dpi: dimensions.dpi,
        };
        let label = self.termviai_pane_label(pane_id);
        promise::spawn::spawn(async move {
            let result = async {
                let local = pane
                    .downcast_ref::<LocalPane>()
                    .context("The SSH pane changed while reconnecting.")?;
                let remote = domain
                    .downcast_ref::<RemoteSshDomain>()
                    .context("The SSH domain changed while reconnecting.")?;
                remote.reconnect_pane(local, size).await
            }
            .await;

            window.notify(TermWindowNotif::Apply(Box::new(move |tw| {
                tw.termviai_ui.workspace.reconnecting.remove(&pane_id);
                match result {
                    Ok(()) => {
                        tw.termviai_ui.error.clear();
                        tw.termviai_broadcast.error.clear();
                        tw.termviai_ui.show_notice(format!("Reconnected {label}"));
                    }
                    Err(error) => {
                        tw.termviai_ui.error = format!("Reconnect {label}: {error:#}");
                    }
                }
                tw.termviai_sync_broadcast();
                tw.update_title();
                if let Some(window) = &tw.window {
                    window.invalidate();
                }
            })))
        })
        .detach();
        Ok(())
    }

    pub(super) fn termviai_pane_label(&self, pane_id: usize) -> String {
        Mux::get()
            .get_pane(pane_id)
            .and_then(|p| {
                self.termviai_ui
                    .workspace
                    .hosts
                    .get(&p.domain_id())
                    .map(|h| h.label.clone())
            })
            .filter(|label| !label.is_empty())
            .unwrap_or_else(|| "Terminal".into())
    }

    pub(super) fn termviai_tab_label(&self, tab: &Tab) -> String {
        let title = tab.get_title();
        // Generated mux titles may lag behind pane removal. Only a deliberate
        // rename is authoritative; otherwise describe the current live panes.
        tab_label(
            self.termviai_ui
                .workspace
                .renamed_tabs
                .contains(&tab.tab_id())
                .then_some(title.as_str()),
            &tab.iter_panes_ignoring_zoom()
                .iter()
                .map(|p| self.termviai_pane_label(p.pane.pane_id()))
                .collect::<Vec<_>>(),
        )
    }

    pub(super) fn termviai_workspace_changed(&mut self) {
        if let Some(tab) = Mux::get().get_active_tab_for_window(self.mux_window_id) {
            self.termviai_workspace_changed_for(tab.tab_id());
        }
    }

    pub(super) fn termviai_remember_window_groups(&mut self) {
        // Release the mux window guard before snapshotting individual pane trees.
        let tabs = Mux::get()
            .get_window(self.mux_window_id)
            .map(|window| {
                window
                    .iter_tabs()
                    .map(|tab| tab.tab_id())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for tab_id in tabs {
            self.termviai_remember_group(tab_id);
        }
    }

    fn termviai_workspace_changed_for(&mut self, tab_id: usize) {
        if let Some(tab) = Mux::get().get_tab(tab_id) {
            if !self.termviai_ui.workspace.renamed_tabs.contains(&tab_id) {
                let labels = tab
                    .iter_panes_ignoring_zoom()
                    .iter()
                    .map(|p| self.termviai_pane_label(p.pane.pane_id()))
                    .collect::<Vec<_>>();
                tab.set_title(&default_tab_label(&labels));
            }
            self.termviai_remember_group(tab_id);
        }
    }

    pub(super) fn termviai_remember_group(&mut self, tab_id: usize) {
        if let Some(tab) = Mux::get().get_tab(tab_id) {
            if tab.iter_panes_ignoring_zoom().len() < 2 {
                return;
            }
            if let Ok(layout) = snapshot(
                tab.codec_pane_tree(),
                &self.termviai_ui.workspace.hosts,
                &Mux::get(),
            ) {
                let name = self.termviai_tab_label(&tab);
                if let Err(error) = self
                    .termviai_ui
                    .workspace
                    .data
                    .record_group_history(&name, layout)
                {
                    self.termviai_ui.error = format!("Could not save workspace history: {error:#}");
                }
            }
        }
    }

    pub(super) fn termviai_save_workspace(&mut self, tab_id: usize) -> anyhow::Result<()> {
        let mux = Mux::get();
        let tab = mux.get_tab(tab_id).context("This tab was closed.")?;
        anyhow::ensure!(
            mux.get_window(self.mux_window_id)
                .map(|w| w.iter_tabs().any(|t| t.tab_id() == tab_id))
                .unwrap_or(false),
            "The tab moved to another window."
        );
        let name = self.termviai_tab_label(&tab);
        let layout = snapshot(tab.codec_pane_tree(), &self.termviai_ui.workspace.hosts, &mux)?;
        let previous = self.termviai_ui.workspace.saved_ids.get(&tab_id).cloned();
        let id = self
            .termviai_ui
            .workspace
            .data
            .save_group(previous.as_deref(), &name, layout)?;
        self.termviai_ui.workspace.saved_ids.insert(tab_id, id);
        self.termviai_ui
            .show_notice(format!("Saved \"{}\" to New Tab", name));
        Ok(())
    }

    pub(super) fn termviai_rename_tab(&mut self, tab_id: usize, name: &str) -> anyhow::Result<()> {
        let name = name.trim();
        anyhow::ensure!(
            !name.is_empty() && name.chars().count() <= 80 && !name.chars().any(char::is_control),
            "Enter a name with 1–80 characters."
        );
        let mux = Mux::get();
        let tab = mux.get_tab(tab_id).context("This tab was closed.")?;
        anyhow::ensure!(
            mux.get_window(self.mux_window_id)
                .map(|w| w.iter_tabs().any(|t| t.tab_id() == tab_id))
                .unwrap_or(false),
            "The tab moved to another window."
        );
        if let Some(id) = self.termviai_ui.workspace.saved_ids.get(&tab_id).cloned() {
            self.termviai_ui.workspace.data.rename_saved(&id, name)?;
        }
        tab.set_title(name);
        self.termviai_ui.workspace.renamed_tabs.insert(tab_id);
        self.termviai_remember_group(tab_id);
        self.update_title();
        Ok(())
    }

    pub(super) fn termviai_open_workspace(
        &mut self,
        layout: SavedLayout,
        name: Option<String>,
        saved_id: Option<String>,
        split: Option<(usize, usize, bool)>,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.termviai_ui.connecting,
            "An SSH connection is still opening."
        );
        let mux = Mux::get();
        let ssh_keepalive_interval = self.termviai_ssh_keepalive_interval();
        for host in layout.hosts() {
            let mut host = host.clone();
            host.validate()?;
            let domain_name = domain_name(&host, ssh_keepalive_interval);
            if mux.get_domain_by_name(&domain_name).is_none() {
                let dom = ssh_domain(&host, ssh_keepalive_interval);
                let domain: Arc<dyn Domain> =
                    Arc::new(mux::ssh::RemoteSshDomain::with_ssh_domain(&dom)?);
                mux.add_domain(&domain);
            }
            let domain = mux
                .get_domain_by_name(&domain_name)
                .context("SSH domain missing")?;
            self.termviai_ui
                .workspace
                .hosts
                .insert(domain.domain_id(), host);
        }
        if let Some((tab, pane, _)) = split {
            anyhow::ensure!(
                mux.resolve_pane_id(pane).map(|(_, w, t)| (w, t))
                    == Some((self.mux_window_id, tab)),
                "The target pane was closed or moved."
            );
        }
        let window = self.window.clone().context("The window closed.")?;
        let window_id = self.mux_window_id;
        let size = self.terminal_size;
        let config = Arc::new(config::TermConfig::with_config(self.config.clone()));
        let custom_name = restored_name_is_custom(name.as_deref(), &layout);
        self.termviai_ui.connecting = true;
        self.termviai_ui.page = Page::Terminal;
        self.termviai_ui.error.clear();
        promise::spawn::spawn(async move {
            let mut opened = Vec::new();
            let mut opened_tab = None;
            let result = async {
                let first = layout
                    .hosts()
                    .first()
                    .context("No hosts in workspace")?
                    .to_owned()
                    .clone();
                let domain = SpawnTabDomain::DomainName(domain_name(
                    &first,
                    ssh_keepalive_interval,
                ));
                let (tab_id, root_pane) = if let Some((tab_id, pane_id, vertical)) = split {
                    let (pane, _) = mux
                        .split_pane(
                            pane_id,
                            SplitRequest {
                                direction: if vertical {
                                    SplitDirection::Vertical
                                } else {
                                    SplitDirection::Horizontal
                                },
                                target_is_second: true,
                                top_level: false,
                                size: SplitSize::Percent(50),
                            },
                            SplitSource::Spawn {
                                command: None,
                                command_dir: None,
                            },
                            domain,
                        )
                        .await?;
                    (tab_id, pane)
                } else {
                    let (tab, pane, _) = mux
                        .spawn_tab_or_window(
                            Some(window_id),
                            domain,
                            None,
                            None,
                            size,
                            None,
                            mux.active_workspace(),
                            None,
                        )
                        .await?;
                    (tab.tab_id(), pane)
                };
                opened_tab = Some(tab_id);
                root_pane.set_config(config.clone());
                opened.push(first);
                if let Some(name) = &name {
                    if let Some(tab) = mux.get_tab(tab_id) {
                        tab.set_title(name);
                    }
                }
                let mut queue = VecDeque::from([(root_pane.pane_id(), layout)]);
                while let Some((pane_id, branch)) = queue.pop_front() {
                    if let SavedLayout::Split {
                        axis,
                        first_percent,
                        first,
                        second,
                    } = branch
                    {
                        anyhow::ensure!(
                            mux.resolve_pane_id(pane_id).map(|(_, w, t)| (w, t))
                                == Some((window_id, tab_id)),
                            "The restoring workspace was closed or moved."
                        );
                        let host = second
                            .hosts()
                            .first()
                            .context("Empty split")?
                            .to_owned()
                            .clone();
                        let (pane, _) = mux
                            .split_pane(
                                pane_id,
                                SplitRequest {
                                    direction: if axis == SplitAxis::Horizontal {
                                        SplitDirection::Horizontal
                                    } else {
                                        SplitDirection::Vertical
                                    },
                                    target_is_second: true,
                                    top_level: false,
                                    size: SplitSize::Percent(100 - first_percent),
                                },
                                SplitSource::Spawn {
                                    command: None,
                                    command_dir: None,
                                },
                                SpawnTabDomain::DomainName(domain_name(
                                    &host,
                                    ssh_keepalive_interval,
                                )),
                            )
                            .await?;
                        pane.set_config(config.clone());
                        opened.push(host);
                        queue.push_back((pane_id, *first));
                        queue.push_back((pane.pane_id(), *second));
                    }
                }
                Ok::<(), anyhow::Error>(())
            }
            .await;
            window.notify(TermWindowNotif::Apply(Box::new(move |tw| {
                tw.termviai_ui.connecting = false;
                for host in opened.iter().rev() {
                    if let Err(error) = tw.termviai_ui.workspace.data.record_recent(host) {
                        tw.termviai_ui.error =
                            format!("Recent connections could not be saved: {error:#}");
                    }
                }
                if let Some(tab_id) = opened_tab.filter(|id| {
                    Mux::get()
                        .get_window(window_id)
                        .map(|w| w.iter_tabs().any(|t| t.tab_id() == *id))
                        .unwrap_or(false)
                }) {
                    if custom_name {
                        tw.termviai_ui.workspace.renamed_tabs.insert(tab_id);
                    }
                    if result.is_ok() {
                        if let Some(id) = saved_id {
                            tw.termviai_ui.workspace.saved_ids.insert(tab_id, id);
                        }
                    }
                    tw.termviai_workspace_changed_for(tab_id);
                }
                if let Err(error) = result {
                    tw.termviai_ui.error = format!("SSH workspace: {error:#}");
                    if opened_tab.is_none() {
                        tw.termviai_ui.new_tab_open = true;
                        tw.termviai_ui.page = Page::NewTab;
                    }
                }
                tw.termviai_sync_broadcast();
                tw.update_title();
                if let Some(w) = &tw.window {
                    w.invalidate();
                }
            })));
        })
        .detach();
        Ok(())
    }
}

#[cfg(test)]
mod keepalive_tests {
    use super::*;

    #[test]
    fn termviai_ssh_domain_carries_keepalive_and_changes_cache_identity() {
        let host = Host {
            label: "server".into(),
            address: "example.org".into(),
            port: 22,
            username: "alice".into(),
            group: String::new(),
            identity_file: "key.pem".into(),
        };
        let enabled = ssh_domain(&host, 30);
        assert_eq!(enabled.ssh_option["serveraliveinterval"], "30");
        assert_eq!(enabled.ssh_option["identityfile"], "key.pem");
        let disabled = ssh_domain(&host, 0);
        assert_eq!(disabled.ssh_option["serveraliveinterval"], "0");
        assert_ne!(enabled.name, disabled.name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn labels_describe_hosts_without_shell_titles() {
        assert_eq!(default_tab_label(&["209".into()]), "209");
        assert_eq!(
            default_tab_label(&["209".into(), "212".into()]),
            "209 + 212"
        );
        assert_eq!(
            default_tab_label(&["209".into(), "212".into(), "217".into()]),
            "209 + 2 hosts"
        );
    }

    #[test]
    fn generated_tab_labels_follow_pane_removal_but_manual_names_remain() {
        assert_eq!(tab_label(None, &["209".into(), "212".into()]), "209 + 212");
        assert_eq!(tab_label(None, &["212".into()]), "212");
        assert_eq!(tab_label(Some("Production"), &["212".into()]), "Production");
        assert_eq!(tab_label(Some(""), &["212".into()]), "212");
    }

    #[test]
    fn restoring_generated_group_names_does_not_freeze_old_host_labels() {
        let layout = SavedLayout::Split {
            axis: SplitAxis::Horizontal,
            first_percent: 50,
            first: Box::new(SavedLayout::Pane {
                host: Host::quick_connect("209").unwrap(),
            }),
            second: Box::new(SavedLayout::Pane {
                host: Host::quick_connect("212").unwrap(),
            }),
        };
        assert!(!restored_name_is_custom(None, &layout));
        assert!(!restored_name_is_custom(Some("209 + 212"), &layout));
        assert!(restored_name_is_custom(Some("Production"), &layout));
    }
}
