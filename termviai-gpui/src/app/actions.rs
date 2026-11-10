use super::*;
use anyhow::{Context as _, ensure};
impl AppView {
    pub(super) fn open_selected_hosts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.page != Page::Hosts
            || self.drawer.is_some()
            || self.menu.is_some()
            || self.confirm.is_some()
        {
            return;
        }
        let query = self.search.read(cx).value().to_lowercase();
        let visible: Vec<_> = self
            .store
            .hosts
            .iter()
            .filter(|h| {
                self.store.host_in_group(h, &self.group)
                    && format!(
                        "{} {} {} {}",
                        h.label,
                        h.address,
                        h.username,
                        self.store.group_path(&h.group_id)
                    )
                    .to_lowercase()
                    .contains(&query)
            })
            .cloned()
            .collect();
        let hosts = self.host_selection.take(&visible);
        for host in hosts {
            self.connect(Some(host), window, cx);
        }
        cx.notify();
    }

    pub(super) fn change_store(
        &mut self,
        change: impl FnOnce(&mut HostStore) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        ensure!(self.load_error.is_empty(), "{}", self.load_error);
        let mut store = HostStore::load()?;
        change(&mut store)?;
        store.save()?;
        self.host_selection.retain(&store.hosts);
        self.store = store;
        Ok(())
    }
    pub(super) fn spawn_pane(
        &mut self,
        host: Option<Host>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> usize {
        let id = self.id();
        let session = Session::new(
            id,
            host.clone(),
            &self.store,
            self.theme,
            self.settings.keepalive(),
        );
        let s = session.clone();
        let view = cx.new(|cx| {
            TerminalView::new(
                s,
                self.settings.font_size(),
                self.theme,
                self.ime_epoch.clone(),
                window,
                cx,
            )
        });
        cx.subscribe(&view, |s, _, e: &TerminalEvent, cx| match e {
            TerminalEvent::Input(id, input) => s.route_input(*id, input.clone(), cx),
            TerminalEvent::Focused(id) => {
                s.focused = Some(*id);
                cx.notify();
            }
        })
        .detach();
        view.focus_handle(cx).focus(window);
        self.panes.insert(
            id,
            Pane {
                host,
                session,
                view,
                form: None,
                pending: None,
                save_attempted: false,
                error: String::new(),
                recorded: false,
                auth_revision: 0,
                auth_scroll: ScrollHandle::new(),
            },
        );
        self.focused = Some(id);
        id
    }
    pub(super) fn connect(
        &mut self,
        host: Option<Host>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.page == Page::NewTab {
            self.new_tab_open = false;
        }
        let name = host
            .as_ref()
            .map(|h| h.label.clone())
            .unwrap_or("Local terminal".into());
        let pane = self.spawn_pane(host, window, cx);
        let id = self.id();
        self.tabs.push(Tab {
            id,
            name,
            custom_name: false,
            layout: Layout::Pane(pane),
            broadcast: BroadcastState::new([pane]),
            saved: None,
            tray: false,
            tray_motion: None,
            zoom: None,
        });
        self.page = Page::Terminal(id);
        self.menu = None;
        cx.notify();
    }
    pub(super) fn restore(
        &mut self,
        saved: SavedWorkspace,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.page == Page::NewTab {
            self.new_tab_open = false;
        }
        let layout = Layout::restore(&saved.layout, &mut |h| {
            self.spawn_pane(Some(h.clone()), window, cx)
        });
        let id = self.id();
        let broadcast = BroadcastState::new(layout.panes());
        self.tabs.push(Tab {
            id,
            custom_name: saved.name
                != tab_policy::default_name(
                    &layout
                        .visual_panes()
                        .iter()
                        .map(|id| self.pane_label(*id))
                        .collect::<Vec<_>>(),
                ),
            name: saved.name,
            layout,
            broadcast,
            saved: Some(saved.id),
            tray: false,
            tray_motion: None,
            zoom: None,
        });
        self.page = Page::Terminal(id);
        cx.notify();
    }
    pub(super) fn route_input(&mut self, id: usize, input: session::Input, cx: &mut Context<Self>) {
        let Some(pane) = self.panes.get(&id) else {
            return;
        };
        let bytes = match pane.session.encode(input) {
            Ok(v) => v,
            Err(e) => {
                self.panes.get_mut(&id).unwrap().error = e.to_string();
                cx.notify();
                return;
            }
        };
        // Legacy key-up encodings are empty. They must not reset scrollback or
        // clear a selection just because a copy shortcut was released.
        if bytes.is_empty() {
            return;
        }
        let Some(tab) = self.tabs.iter().find(|t| t.layout.panes().contains(&id)) else {
            return;
        };
        let report = termviai_broadcast::route_input(
            &tab.broadcast,
            id,
            &bytes,
            |p| self.panes.get(&p).is_some_and(|p| p.session.online()),
            |p, b| self.panes[&p].session.send(b),
        );
        if let Ok(report) = report {
            for p in report.delivered {
                self.panes.get_mut(&p).unwrap().error.clear();
            }
            for p in report.disconnected {
                self.panes.get_mut(&p).unwrap().error =
                    "Broadcast not sent: this terminal is offline".into();
            }
            for (p, e) in report.failures {
                self.panes.get_mut(&p).unwrap().error = e.to_string();
            }
        }
        cx.notify();
    }
    pub(super) fn tick(&mut self, cx: &mut Context<Self>) {
        let mut record = vec![];
        let mut saves = vec![];
        let mut changed = false;
        let mut ring = false;
        for (&id, pane) in &mut self.panes {
            ring |= pane
                .session
                .bells
                .swap(0, std::sync::atomic::Ordering::AcqRel)
                > 0;
            let view = pane.session.auth.view();
            if view.revision != pane.auth_revision {
                pane.auth_revision = view.revision;
                changed = true;
            }
            if view.authenticated && pane.pending.is_some() && !pane.save_attempted {
                saves.push(id);
            }
            if pane.session.online() && !pane.recorded {
                pane.recorded = true;
                if pane.host.is_some() {
                    record.push(id);
                }
                changed = true;
            }
        }
        if ring && self.settings.bell() {
            session::ring_bell();
        }
        for id in saves {
            if let Err(e) = self.save_auth(id) {
                if let Some(p) = self.panes.get_mut(&id) {
                    p.error = format!("Connected; authentication not saved: {e:#}");
                }
            }
            changed = true;
        }
        for id in record {
            let Some(h) = self.panes[&id].host.as_ref() else {
                continue;
            };
            if let Err(e) = self.workspaces.record_recent(h) {
                self.fail(e);
            }
        }
        if self.last_tick.elapsed() >= Duration::from_millis(500) {
            self.last_tick = Instant::now();
            let keepalive = self.settings.keepalive();
            self.monitor
                .tick(&self.store, |h| session::connection_config(h, keepalive));
            let mut manager = forwarding::manager().lock().unwrap();
            self.forwarding = manager.views();
            self.forwarding_error = manager.load_error.clone();
            changed = true;
        }
        if changed {
            cx.notify();
        }
    }
    fn save_auth(&mut self, id: usize) -> anyhow::Result<()> {
        let Some(pane) = self.panes.get_mut(&id) else {
            return Ok(());
        };
        if !pane.session.auth.view().authenticated {
            return Ok(());
        }
        pane.save_attempted = true;
        let Some(pending) = pane.pending.as_ref() else {
            return Ok(());
        };
        let mut store = HostStore::load()?;
        let index = if pending.saved {
            Some(
                store
                    .hosts
                    .iter()
                    .position(|h| h == &pending.original)
                    .context("Host was changed or deleted; it was not overwritten")?,
            )
        } else {
            None
        };
        let mut selected = pending.selected.clone();
        let mut created = None;
        if !pending.secret.is_empty() {
            let password = wezterm_ssh::credentials::load(&pending.secret)?
                .context("Temporary password expired")?;
            let reference = new_credential_id();
            wezterm_ssh::credentials::save(&reference, &password, true)?;
            selected.credential_id = reference.clone();
            selected.session_credential.clear();
            created = Some(reference);
        }
        let mut host = pending.original.clone();
        host.apply_authentication(&selected, pending.mode);
        if let Some(i) = index {
            store.hosts[i] = host.clone();
            if let Err(e) = store.save() {
                if let Some(id) = created {
                    let _ = wezterm_ssh::credentials::delete(&id);
                }
                return Err(e);
            }
        }
        pane.pending = None;
        pane.host = Some(host);
        pane.form = None;
        pane.error.clear();
        self.store = store;
        Ok(())
    }
    pub(super) fn reconnect(&mut self, tab_id: usize, w: &mut Window, cx: &mut Context<Self>) {
        let ids = self
            .tabs
            .iter()
            .find(|t| t.id == tab_id)
            .map(|t| t.layout.panes())
            .unwrap_or_default();
        for id in ids {
            if !self.panes[&id].session.online()
                && self.panes[&id].session.state.lock().unwrap().ended
            {
                let host = self.panes[&id].host.clone();
                let session = Session::new(
                    id,
                    host.clone(),
                    &self.store,
                    self.theme,
                    self.settings.keepalive(),
                );
                let s = session.clone();
                let view = cx.new(|cx| {
                    TerminalView::new(
                        s,
                        self.settings.font_size(),
                        self.theme,
                        self.ime_epoch.clone(),
                        w,
                        cx,
                    )
                });
                cx.subscribe(&view, |s, _, e: &TerminalEvent, cx| match e {
                    TerminalEvent::Input(id, input) => s.route_input(*id, input.clone(), cx),
                    TerminalEvent::Focused(id) => {
                        s.focused = Some(*id);
                        cx.notify();
                    }
                })
                .detach();
                self.panes.insert(
                    id,
                    Pane {
                        host,
                        session,
                        view,
                        form: None,
                        pending: None,
                        save_attempted: false,
                        error: String::new(),
                        recorded: false,
                        auth_revision: 0,
                        auth_scroll: ScrollHandle::new(),
                    },
                );
            }
        }
        self.menu = None;
        if let Some(id) = self.focused {
            if let Some(p) = self.panes.get(&id) {
                p.view.focus_handle(cx).focus(w);
            }
        }
        cx.notify();
    }
    pub(super) fn close_tab(&mut self, id: usize, cx: &mut Context<Self>) {
        if let Some(index) = self.tabs.iter().position(|t| t.id == id) {
            let tab = self.tabs.remove(index);
            for p in tab.layout.panes() {
                self.panes.remove(&p);
            }
            if self.page == Page::Terminal(id) {
                self.page = self
                    .tabs
                    .get(index.min(self.tabs.len().saturating_sub(1)))
                    .map(|t| Page::Terminal(t.id))
                    .unwrap_or(Page::Hosts);
            }
        }
        self.menu = None;
        self.confirm = None;
        cx.notify();
    }
    pub(super) fn close_pane(&mut self, id: usize, cx: &mut Context<Self>) {
        if let Some(index) = self
            .tabs
            .iter()
            .position(|t| t.layout.panes().contains(&id))
        {
            let tab = &mut self.tabs[index];
            if let Some(layout) = tab.layout.clone().remove(id) {
                tab.layout = layout;
                tab.broadcast.sync_panes(tab.layout.panes());
                tab.zoom = None;
                self.panes.remove(&id);
            } else {
                let id = tab.id;
                self.close_tab(id, cx);
            }
        }
        self.refresh_tab_names();
        self.confirm = None;
        cx.notify();
    }
    pub(super) fn save_workspace(&mut self, id: usize) -> anyhow::Result<()> {
        let tab = self
            .tabs
            .iter_mut()
            .find(|t| t.id == id)
            .context("Tab closed")?;
        let layout = tab
            .layout
            .save(&|p| self.panes.get(&p).and_then(|p| p.host.clone()))
            .context("Save a group of SSH terminals")?;
        tab.saved = Some(
            self.workspaces
                .save_group(tab.saved.as_deref(), &tab.name, layout)?,
        );
        Ok(())
    }
    pub(super) fn broadcast_all(&mut self, id: usize, cx: &mut Context<Self>) {
        if let Some(t) = self.tabs.iter_mut().find(|t| t.id == id) {
            termviai_gpui::broadcast::toggle_all(&mut t.broadcast);
        }
        cx.notify();
    }
    pub(super) fn member(&mut self, tab: usize, pane: usize, cx: &mut Context<Self>) {
        if let Some(t) = self.tabs.iter_mut().find(|t| t.id == tab) {
            termviai_gpui::broadcast::toggle_member(&mut t.broadcast, pane);
        }
        cx.notify();
    }
    fn pane_label(&self, id: usize) -> String {
        self.panes
            .get(&id)
            .and_then(|p| p.host.as_ref())
            .map(|h| h.label.clone())
            .unwrap_or_else(|| "Local terminal".into())
    }
    fn refresh_tab_names(&mut self) {
        let names = self
            .tabs
            .iter()
            .map(|t| {
                tab_policy::default_name(
                    &t.layout
                        .visual_panes()
                        .iter()
                        .map(|id| self.pane_label(*id))
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>();
        for (tab, name) in self.tabs.iter_mut().zip(names) {
            if !tab.custom_name {
                tab.name = name;
            }
        }
    }
    pub(super) fn dock_source(&self, drag: &Drag) -> Option<usize> {
        let tab = self.tabs.iter().find(|t| t.id == drag.tab)?;
        tab_policy::dock_source(&tab.layout, drag.pane).filter(|id| self.panes.contains_key(id))
    }
    pub(super) fn clear_drag(&mut self) {
        self.drag_source = None;
        self.drop_target = None;
        self.tab_drop_target = None;
        self.tab_rail.edge = 0;
        self.tab_rail.motion = None;
    }
    // The same destination rule drives the preview and the committed move.
    pub(super) fn tab_drop_gap(&self, drag: &Drag, target: usize, after: bool) -> Option<usize> {
        let from = self.tabs.iter().position(|t| t.id == drag.tab)?;
        let gap = if target == usize::MAX {
            self.tabs.len()
        } else {
            self.tabs.iter().position(|t| t.id == target)? + usize::from(after)
        };
        if drag.pane.is_some() {
            (self.tabs[from].layout.panes().len() > 1 && self.dock_source(drag).is_some())
                .then_some(gap)
        } else {
            tab_policy::reorder_destination(self.tabs.len(), from, gap).map(|_| gap)
        }
    }
    pub(super) fn drop_on_tab(&mut self, drag: &Drag, target: usize, cx: &mut Context<Self>) {
        if let Some((id, after)) = self.tab_drop_target.filter(|(id, _)| *id == target) {
            if let Some(gap) = self.tab_drop_gap(drag, id, after) {
                if drag.pane.is_some() {
                    self.detach_pane_at(drag, gap, cx);
                } else if let Some(from) = self.tabs.iter().position(|t| t.id == drag.tab) {
                    if let Some(to) = tab_policy::reorder_destination(self.tabs.len(), from, gap) {
                        let tab = self.tabs.remove(from);
                        self.tabs.insert(to, tab);
                    }
                }
            }
        }
        self.clear_drag();
        cx.notify();
    }
    pub(super) fn detach_pane(&mut self, drag: &Drag, cx: &mut Context<Self>) {
        self.detach_pane_at(drag, self.tabs.len(), cx);
        self.clear_drag();
        cx.notify();
    }
    fn detach_pane_at(&mut self, drag: &Drag, gap: usize, cx: &mut Context<Self>) {
        let Some(pane) = drag.pane else { return };
        let Some(i) = self.tabs.iter().position(|t| t.id == drag.tab) else {
            return;
        };
        if self.tabs[i].layout.panes().len() < 2 || self.dock_source(drag) != Some(pane) {
            return;
        }
        let Some(layout) = self.tabs[i].layout.clone().remove(pane) else {
            return;
        };
        self.tabs[i].layout = layout;
        let ids = self.tabs[i].layout.panes();
        self.tabs[i].broadcast.sync_panes(ids);
        self.tabs[i].zoom = None;
        let id = self.id();
        let name = self.panes[&pane]
            .host
            .as_ref()
            .map(|h| h.label.clone())
            .unwrap_or("Local terminal".into());
        self.tabs.insert(
            gap.min(self.tabs.len()),
            Tab {
                id,
                name,
                custom_name: false,
                layout: Layout::Pane(pane),
                broadcast: BroadcastState::new([pane]),
                saved: None,
                tray: false,
                tray_motion: None,
                zoom: None,
            },
        );
        self.page = Page::Terminal(id);
        self.focused = Some(pane);
        self.refresh_tab_names();
        self.clear_drag();
        cx.notify();
    }
    pub(super) fn dock(
        &mut self,
        drag: &Drag,
        target_tab: usize,
        target: usize,
        side: Side,
        cx: &mut Context<Self>,
    ) {
        let source = self.dock_source(drag);
        let Some(source) = source else { return };
        if source == target {
            return;
        }
        let Some(target_index) = self.tabs.iter().position(|t| t.id == target_tab) else {
            return;
        };
        if self.tabs[target_index].layout.dock(source, target, side) {
            if drag.tab != target_tab {
                if let Some(i) = self.tabs.iter().position(|t| t.id == drag.tab) {
                    if let Some(l) = self.tabs[i].layout.clone().remove(source) {
                        self.tabs[i].layout = l;
                        let ids = self.tabs[i].layout.panes();
                        self.tabs[i].broadcast.sync_panes(ids);
                    } else {
                        self.tabs.remove(i);
                    }
                }
            }
            for t in &mut self.tabs {
                t.broadcast.sync_panes(t.layout.panes());
                t.zoom = None;
            }
            self.page = Page::Terminal(target_tab);
            self.focused = Some(source);
        }
        self.refresh_tab_names();
        self.clear_drag();
        cx.notify();
    }
    pub(super) fn confirm_action(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Some(action) = self.confirm.take() else {
            return;
        };
        let result = (|| -> anyhow::Result<()> {
            match action {
                Confirm::Host(h) => self.change_store(|s| s.remove_host(&h))?,
                Confirm::Group(g) => self.change_store(|s| s.remove_group(&g))?,
                Confirm::Key(k) => {
                    self.change_store(|s| s.remove_key(&k))?;
                    self.drawer = None;
                }
                Confirm::Identity(i) => {
                    self.change_store(|s| {
                        let n = s
                            .identities
                            .iter()
                            .position(|x| x == &i)
                            .context("Identity changed")?;
                        s.identities.remove(n);
                        Ok(())
                    })?;
                    self.drawer = None;
                }
                Confirm::Tab(id) => self.close_tab(id, cx),
                Confirm::Pane(id) => self.close_pane(id, cx),
                Confirm::Quit => {
                    self.quit_allowed = true;
                    w.remove_window();
                }
                Confirm::Stop(id, processes) => {
                    self.monitor.stop(
                        id,
                        processes
                            .iter()
                            .map(|p| monitor::Target {
                                pid: p.pid,
                                token: p.token.clone(),
                            })
                            .collect(),
                    )?;
                }
                Confirm::RemoveMonitor(id) => {
                    anyhow::ensure!(
                        id < self.monitor.entries.len(),
                        "Monitor host no longer exists"
                    );
                    self.monitor.entries.remove(id);
                    self.monitor_detail = self.monitor_detail.and_then(|(i, n, d)| {
                        if i == id {
                            None
                        } else {
                            Some((if i > id { i - 1 } else { i }, n, d))
                        }
                    });
                    self.monitor_reason = None;
                    self.monitor.save()?;
                }
                Confirm::Forward(id) => {
                    forwarding::manager().lock().unwrap().delete(&id)?;
                    self.drawer = None;
                }
            }
            Ok(())
        })();
        self.notify_error(result, cx);
    }
}
