use super::*;

pub(super) struct TabGroup {
    pub name: String,
    pub(super) saved_id: Option<String>,
    pub(super) number: usize,
    last_page: Option<Page>,
    pub new_tab_open: bool,
}

impl TabGroup {
    fn new(number: usize) -> Self {
        Self {
            name: number.to_string(),
            saved_id: None,
            number,
            last_page: None,
            new_tab_open: false,
        }
    }
}

pub(super) struct TabRail {
    pub scroll: ScrollHandle,
    pub metrics: tab_strip::Metrics,
    pub motion: Option<(Instant, f32, f32)>,
    pub edge: i8,
    pub query: String,
    last_frame: Instant,
    last_tabs: Vec<usize>,
    last_active: Option<Page>,
    reveal_pending: bool,
}
impl Default for TabRail {
    fn default() -> Self {
        Self {
            scroll: ScrollHandle::new(),
            metrics: Default::default(),
            motion: None,
            edge: 0,
            query: String::new(),
            last_frame: Instant::now(),
            last_tabs: vec![],
            last_active: None,
            reveal_pending: true,
        }
    }
}
impl TabRail {
    pub fn offset(&self) -> f32 {
        -self.scroll.offset().x / px(1.)
    }
    fn set_offset(&self, x: f32) {
        self.scroll
            .set_offset(point(px(-self.metrics.clamp(x)), px(0.)));
    }
}

struct Entry {
    page: Page,
    name: String,
    detail: String,
    search: String,
    offline: bool,
    grouped: bool,
}
impl AppView {
    pub(super) fn group_tabs(&self) -> impl DoubleEndedIterator<Item = &Tab> {
        self.tabs
            .iter()
            .filter(|t| t.group == self.active_tab_group)
    }

    pub(super) fn new_tab_open(&self) -> bool {
        self.tab_groups
            .get(self.active_tab_group)
            .is_some_and(|group| group.new_tab_open)
    }

    pub(super) fn ensure_tab_group(&mut self) {
        if self.tab_groups.is_empty() {
            self.tab_groups.push(TabGroup::new(1));
        }
    }

    pub(super) fn set_new_tab_open(&mut self, open: bool) {
        if open {
            self.ensure_tab_group();
        }
        if let Some(group) = self.tab_groups.get_mut(self.active_tab_group) {
            group.new_tab_open = open;
        }
    }

    pub(super) fn group_page(&self, group: usize) -> Page {
        let Some(state) = self.tab_groups.get(group) else {
            return Page::EmptyTabGroup;
        };
        match state.last_page {
            Some(Page::Terminal(id))
                if self.tabs.iter().any(|t| t.id == id && t.group == group) =>
            {
                return Page::Terminal(id);
            }
            Some(Page::NewTab) if state.new_tab_open => return Page::NewTab,
            _ => {}
        }
        self.tabs
            .iter()
            .rev()
            .find(|t| t.group == group)
            .map(|t| Page::Terminal(t.id))
            .unwrap_or(if state.new_tab_open {
                Page::NewTab
            } else {
                Page::EmptyTabGroup
            })
    }

    fn remember_group_page(&mut self) {
        if matches!(self.page, Page::Terminal(_) | Page::NewTab)
            && let Some(group) = self.tab_groups.get_mut(self.active_tab_group)
        {
            group.last_page = Some(self.page);
        }
        if let Page::Terminal(id) = self.page
            && let Some(pane) = self.focused
            && let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id)
            && tab.layout.panes().contains(&pane)
        {
            tab.last_focused = Some(pane);
        }
    }

    fn create_tab_group(&mut self) -> usize {
        let number = (1..)
            .find(|number| {
                let name = number.to_string();
                !self
                    .tab_groups
                    .iter()
                    .any(|group| group.number == *number || group.name == name)
            })
            .expect("A tab group number is available");
        let group = self.tab_groups.len();
        self.tab_groups.push(TabGroup {
            new_tab_open: true,
            ..TabGroup::new(number)
        });
        group
    }

    // Consume the launching New Tab before showing the destination. Removing an
    // empty source can shift the destination index, so callers use active_tab_group.
    fn enter_launched_group(&mut self, target: usize) {
        self.remember_group_page();
        let source = self.active_tab_group;
        let from_new_tab = self.page == Page::NewTab;
        if from_new_tab {
            self.set_new_tab_open(false);
        }
        self.active_tab_group = target;
        self.clear_drag();
        if from_new_tab && source != target {
            self.remove_empty_tab_group(source);
        }
    }

    pub(super) fn add_tab_group(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let target = self.create_tab_group();
        self.enter_launched_group(target);
        self.show_page(Page::NewTab, w, cx);
    }

    pub(super) fn group_has_terminals(&self, index: usize) -> bool {
        self.tabs.iter().any(|tab| tab.group == index)
    }

    pub(super) fn save_tab_group(&mut self, index: usize) -> anyhow::Result<()> {
        use anyhow::Context as _;
        let group = self.tab_groups.get(index).context("Tab group closed")?;
        let mut tabs = Vec::new();
        let mut active_tab = None;
        let active = if index == self.active_tab_group
            && matches!(self.page, Page::Terminal(_) | Page::NewTab)
        {
            self.page
        } else {
            self.group_page(index)
        };
        for tab in self.tabs.iter().filter(|t| t.group == index) {
            let focused = if self.page == Page::Terminal(tab.id) {
                self.focused
            } else {
                tab.last_focused
            };
            let layout = tab
                .layout
                .save(&|id| {
                    self.panes.get(&id).map(|p| SavedGroupPane {
                        host: p.host.clone(),
                        focused: focused == Some(id),
                        broadcast_member: tab.broadcast.is_member(id),
                    })
                })
                .context("A terminal in this group has closed")?;
            if active == Page::Terminal(tab.id) {
                active_tab = Some(tabs.len());
            }
            tabs.push(SavedGroupTab {
                name: tab.name.clone(),
                custom_name: tab.custom_name,
                layout,
                workspace_id: tab.saved.clone(),
            });
        }
        let id = self.workspaces.save_tab_group(
            group.saved_id.as_deref(),
            &group.name,
            tabs,
            group.new_tab_open,
            active_tab,
        )?;
        self.tab_groups[index].saved_id = Some(id);
        Ok(())
    }

    pub(super) fn open_saved_group(
        &mut self,
        saved: &SavedTabGroup,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let existing = self
            .tab_groups
            .iter()
            .position(|g| g.saved_id.as_ref() == Some(&saved.id));
        if let Some(index) = existing.filter(|index| self.group_has_terminals(*index)) {
            self.enter_launched_group(index);
            self.show_page(self.group_page(self.active_tab_group), w, cx);
            return;
        }

        // Restore into the emptied live group, or reuse a launcher-only group.
        let target = existing.unwrap_or_else(|| {
            if self.page == Page::NewTab
                && self.new_tab_open()
                && !self.group_has_terminals(self.active_tab_group)
            {
                self.active_tab_group
            } else {
                self.create_tab_group()
            }
        });
        self.enter_launched_group(target);
        let index = self.active_tab_group;
        let mut name = saved.name.clone();
        let mut suffix = 2;
        while self
            .tab_groups
            .iter()
            .enumerate()
            .any(|(i, g)| i != index && g.name == name)
        {
            name = format!("{} ({suffix})", saved.name);
            suffix += 1;
        }
        let group = &mut self.tab_groups[index];
        group.name = name;
        group.saved_id = Some(saved.id.clone());
        group.new_tab_open = saved.new_tab_open || saved.tabs.is_empty();
        let mut page = Page::NewTab;
        for (i, tab) in saved.tabs.iter().enumerate() {
            let mut focused = None;
            let mut broadcast = BroadcastState::default();
            let mut members = Vec::new();
            let layout = Layout::restore(&tab.layout, &mut |pane| {
                let id = self.spawn_pane(pane.host.clone(), w, cx);
                if pane.focused {
                    focused = Some(id);
                }
                members.push((id, pane.broadcast_member));
                id
            });
            broadcast.sync_panes(layout.panes());
            for (id, member) in members {
                let _ = broadcast.set_member(id, member);
            }
            let id = self.id();
            if saved.active_tab == Some(i) || (i == 0 && !saved.new_tab_open) {
                page = Page::Terminal(id);
            }
            self.tabs.push(Tab {
                id,
                group: index,
                last_focused: focused,
                name: tab.name.clone(),
                custom_name: tab.custom_name,
                layout,
                broadcast,
                saved: tab.workspace_id.clone(),
                tray: false,
                tray_motion: None,
                zoom: None,
            });
        }
        self.focused = self
            .tabs
            .iter()
            .find(|t| page == Page::Terminal(t.id))
            .and_then(|t| t.last_focused);
        self.show_page(page, w, cx);
    }

    pub(super) fn switch_tab_group(
        &mut self,
        group: usize,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.remember_group_page();
        self.active_tab_group = group;
        self.clear_drag();
        self.show_page(self.group_page(group), w, cx);
    }

    pub(super) fn close_new_tab(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let group = self.active_tab_group;
        self.set_new_tab_open(false);
        if self.page == Page::NewTab {
            self.activate_tab(self.group_page(group), w, cx);
        }
        self.remove_empty_tab_group(group);
        self.focus_active_tab(w, cx);
        cx.notify();
    }

    pub(super) fn remove_empty_tab_group(&mut self, group: usize) {
        if self.tab_groups.get(group).is_some_and(|g| !g.new_tab_open)
            && !self.group_has_terminals(group)
        {
            self.remove_tab_group(group);
        }
    }

    pub(super) fn move_tab_group(
        &mut self,
        source_number: usize,
        gap: usize,
        cx: &mut Context<Self>,
    ) {
        let Some(source) = self
            .tab_groups
            .iter()
            .position(|g| g.number == source_number)
        else {
            return;
        };
        let Some(target) = tab_policy::reorder_destination(self.tab_groups.len(), source, gap)
        else {
            return;
        };
        let group = self.tab_groups.remove(source);
        self.tab_groups.insert(target, group);
        let remap = |index: &mut usize| {
            *index = tab_policy::index_after_move(*index, source, target);
        };
        for tab in &mut self.tabs {
            remap(&mut tab.group);
        }
        remap(&mut self.active_tab_group);
        if let Some(Form {
            kind: FormKind::RenameTabGroup(id),
            ..
        }) = &mut self.drawer
        {
            remap(id);
        }
        if let Some(Confirm::TabGroup(id)) = &mut self.confirm {
            remap(id);
        }
        self.menu = None;
        self.hovered_tab_group = None;
        cx.notify();
    }

    fn remove_tab_group(&mut self, group: usize) {
        self.hovered_tab_group = None;
        self.tab_groups.remove(group);
        for tab in &mut self.tabs {
            if tab.group > group {
                tab.group -= 1;
            }
        }
        if let Some(form) = &mut self.drawer
            && let FormKind::RenameTabGroup(id) = &mut form.kind
        {
            if *id == group {
                self.drawer = None;
            } else if *id > group {
                *id -= 1;
            }
        }
        self.menu = None;
        if let Some(Confirm::TabGroup(id)) = &mut self.confirm {
            if *id == group {
                self.confirm = None;
            } else if *id > group {
                *id -= 1;
            }
        }
        if self.active_tab_group > group {
            self.active_tab_group -= 1;
        } else if self.active_tab_group == group {
            self.active_tab_group = group.min(self.tab_groups.len().saturating_sub(1));
            self.tab_picker = false;
            if matches!(
                self.page,
                Page::Terminal(_) | Page::NewTab | Page::EmptyTabGroup
            ) {
                self.page = if self.tab_groups.is_empty() {
                    Page::Hosts
                } else {
                    self.group_page(self.active_tab_group)
                };
            }
        }
    }

    pub(super) fn close_tab_group(&mut self, group: usize, w: &mut Window, cx: &mut Context<Self>) {
        let active = self.active_tab_group == group;
        if active {
            self.clear_page_search(w, cx);
        }
        self.tabs.retain(|tab| {
            if tab.group != group {
                return true;
            }
            for pane in tab.layout.panes() {
                self.panes.remove(&pane);
            }
            false
        });
        self.remove_tab_group(group);
        self.clear_drag();
        if active {
            self.focus_active_tab(w, cx);
        }
        cx.notify();
    }

    pub(super) fn can_drop_on_group(&self, drag: &Drag, group: usize) -> bool {
        drag.pane.is_none()
            && self
                .tabs
                .iter()
                .any(|t| t.id == drag.tab && t.group != group)
    }

    pub(super) fn drop_on_group(
        &mut self,
        drag: &Drag,
        group: usize,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.can_drop_on_group(drag, group) {
            return;
        }
        let index = self.tabs.iter().position(|t| t.id == drag.tab).unwrap();
        let mut tab = self.tabs.remove(index);
        let source = tab.group;
        tab.group = group;
        self.tabs.push(tab);
        self.clear_drag();
        self.activate_tab(Page::Terminal(drag.tab), w, cx);
        self.remove_empty_tab_group(source);
    }

    fn tab_entries(&self) -> Vec<Entry> {
        let mut entries = self
            .group_tabs()
            .map(|t| {
                let members = t.layout.visual_panes();
                let hosts = members
                    .iter()
                    .filter_map(|id| self.panes.get(id))
                    .map(|p| {
                        p.host
                            .as_ref()
                            .map(|h| {
                                format!("{} · {}@{}:{}", h.label, h.username, h.address, h.port)
                            })
                            .unwrap_or_else(|| "Local terminal".into())
                    })
                    .collect::<Vec<_>>();
                let detail = if members.len() > 1 {
                    format!("{} terminals · {}", members.len(), hosts.join(" · "))
                } else {
                    hosts.join(" · ")
                };
                Entry {
                    page: Page::Terminal(t.id),
                    search: format!("{} {}", t.name, detail).to_lowercase(),
                    name: t.name.clone(),
                    detail,
                    offline: members
                        .iter()
                        .any(|id| self.panes.get(id).is_some_and(|p| !p.session.online())),
                    grouped: members.len() > 1,
                }
            })
            .collect::<Vec<_>>();
        if self.new_tab_open() {
            entries.push(Entry {
                page: Page::NewTab,
                name: "New Tab".into(),
                detail: "Choose a host".into(),
                search: "new tab choose a host".into(),
                offline: false,
                grouped: false,
            });
        }
        entries
    }
    fn filtered_tabs(&self, cx: &App) -> Vec<Entry> {
        let query = self.tab_search.read(cx).value().trim().to_lowercase();
        self.tab_entries()
            .into_iter()
            .filter(|t| t.search.contains(&query))
            .collect()
    }
    pub(super) fn focus_active_tab(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        match self.page {
            Page::Terminal(id) => {
                if let Some(t) = self.tabs.iter().find(|t| t.id == id) {
                    let pane = t
                        .zoom
                        .or(t.last_focused)
                        .filter(|p| t.layout.panes().contains(p))
                        .or_else(|| t.layout.visual_panes().first().copied());
                    if let Some(id) = pane {
                        self.focused = Some(id);
                        if let Some(p) = self.panes.get(&id) {
                            p.view.focus_handle(cx).focus(w);
                        }
                    }
                }
            }
            Page::NewTab => self.new_tab_search.focus_handle(cx).focus(w),
            _ => self.focus.focus(w),
        }
    }
    pub(super) fn activate_tab(&mut self, page: Page, w: &mut Window, cx: &mut Context<Self>) {
        self.remember_group_page();
        if let Page::Terminal(id) = page {
            if let Some(tab) = self.tabs.iter().find(|t| t.id == id) {
                self.active_tab_group = tab.group;
            }
        }
        self.show_page(page, w, cx);
    }

    fn show_page(&mut self, page: Page, w: &mut Window, cx: &mut Context<Self>) {
        if self.drawer.is_some() {
            self.close_form(w, cx);
        }
        self.clear_page_search(w, cx);
        self.page = page;
        if page == Page::KnownHosts {
            self.reload_known_hosts();
        }
        self.remember_group_page();
        self.tab_picker = false;
        self.menu = None;
        self.tab_rail.reveal_pending = true;
        self.focus_active_tab(w, cx);
        cx.notify();
    }
    pub(super) fn open_new_tab(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.set_new_tab_open(true);
        self.activate_tab(Page::NewTab, w, cx);
    }
    pub(super) fn handle_shortcut(
        &mut self,
        action: termviai_gpui::terminal_keys::Shortcut,
        held: bool,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        use termviai_gpui::terminal_keys::Shortcut::*;
        if self.tab_picker
            || self.drawer.is_some()
            || self.confirm.is_some()
            || self.menu.is_some()
            || self.drag_source.is_some()
            || (self.page == Page::Monitor
                && (self.monitor_reason.is_some()
                    || self.monitor_detail.is_some_and(|(_, n, _)| n.is_some())))
        {
            return false;
        }
        match action {
            Tab(index) => {
                if let Some(entry) = self.tab_entries().get(index)
                    && entry.page != self.page
                {
                    self.activate_tab(entry.page, w, cx);
                }
            }
            Group(index) => {
                if index < self.tab_groups.len() {
                    self.switch_tab_group(index, w, cx);
                }
            }
            CycleTab { reverse } => self.cycle_tab(reverse, w, cx),
            CycleGroup { reverse } => {
                if let Some(index) = tab_strip::next_index(
                    self.tab_groups.len(),
                    Some(self.active_tab_group),
                    reverse,
                ) {
                    self.switch_tab_group(index, w, cx);
                }
            }
            _ => {
                let Page::Terminal(id) = self.page else {
                    return false;
                };
                let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) else {
                    return false;
                };
                let Some(pane) = tab
                    .zoom
                    .or(self.focused)
                    .or(tab.last_focused)
                    .filter(|p| tab.layout.panes().contains(p))
                else {
                    return false;
                };
                match action {
                    FocusPane(side) => {
                        if let Some(next) = tab.layout.neighbor(pane, side) {
                            tab.last_focused = Some(next);
                            tab.zoom = tab.zoom.map(|_| next);
                            self.focus_active_tab(w, cx);
                            cx.notify();
                        }
                    }
                    ToggleMember if !held => self.member(id, pane, cx),
                    ToggleBroadcast if !held => self.broadcast_all(id, cx),
                    Copy | Paste => {
                        let Some(p) = self.panes.get(&pane) else {
                            return false;
                        };
                        if !p.view.focus_handle(cx).is_focused(w) {
                            return false;
                        }
                        if !held {
                            p.view.update(cx, |v, cx| match action {
                                Copy => v.copy(cx),
                                _ => v.paste(cx),
                            });
                        }
                    }
                    _ => {}
                }
            }
        }
        true
    }

    pub(super) fn cycle_tab(&mut self, reverse: bool, w: &mut Window, cx: &mut Context<Self>) {
        let entries = self.tab_entries();
        let active = entries.iter().position(|t| t.page == self.page);
        if let Some(i) = tab_strip::next_index(entries.len(), active, reverse) {
            self.activate_tab(entries[i].page, w, cx);
        }
    }
    pub(super) fn toggle_tab_picker(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.tab_picker = !self.tab_picker;
        self.menu = None;
        if self.tab_picker {
            self.tab_search.update(cx, |s, cx| s.set_value("", w, cx));
            self.tab_rail.query.clear();
            self.tab_picker_index = self
                .tab_entries()
                .iter()
                .position(|t| t.page == self.page)
                .unwrap_or(0);
            self.scrollers["tab-picker"].scroll_to_item(self.tab_picker_index);
            self.tab_search.focus_handle(cx).focus(w);
        } else {
            self.focus_active_tab(w, cx);
        }
        cx.notify();
    }
    pub(super) fn picker_key(&mut self, key: &str, w: &mut Window, cx: &mut Context<Self>) -> bool {
        if !self.tab_picker {
            return false;
        }
        let entries = self.filtered_tabs(cx);
        match key {
            "up" | "down" => {
                self.tab_picker_index = if key == "up" {
                    self.tab_picker_index.saturating_sub(1)
                } else {
                    (self.tab_picker_index + 1).min(entries.len().saturating_sub(1))
                };
                self.scrollers["tab-picker"].scroll_to_item(self.tab_picker_index);
            }
            "enter" => {
                if let Some(entry) = entries.get(self.tab_picker_index) {
                    self.activate_tab(entry.page, w, cx);
                }
            }
            "escape" => {
                self.tab_picker = false;
                self.focus_active_tab(w, cx);
            }
            _ => return false,
        }
        cx.notify();
        true
    }
    pub(super) fn scroll_tabs(&mut self, delta: f32, smooth: bool, cx: &mut Context<Self>) {
        let from = self.tab_rail.offset();
        let base = if smooth {
            self.tab_rail.motion.map(|(_, _, to)| to).unwrap_or(from)
        } else {
            from
        };
        let to = self.tab_rail.metrics.clamp(base + delta);
        self.tab_rail.motion = if smooth && (to - from).abs() > 0.5 {
            Some((Instant::now(), from, to))
        } else {
            None
        };
        if self.tab_rail.motion.is_none() {
            self.tab_rail.set_offset(to);
        }
        cx.notify();
    }
    pub(super) fn prepare_tabs(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.page == Page::NewTab {
            self.set_new_tab_open(true);
        }
        self.remember_group_page();
        let ids = self.group_tabs().map(|t| t.id).collect::<Vec<_>>();
        let count = ids.len() + usize::from(self.new_tab_open());
        // Metrics use a 152px brand slot as their baseline.
        let width = w.viewport_size().width / px(1.) - (self.brand_width() - 152.);
        let metrics = tab_strip::Metrics::new(width, count);
        if self.tab_rail.last_tabs != ids
            || self.tab_rail.last_active != Some(self.page)
            || self.tab_rail.metrics.viewport != metrics.viewport
            || self.tab_rail.metrics.content != metrics.content
        {
            self.tab_rail.reveal_pending = true;
        }
        self.tab_rail.last_active = Some(self.page);
        self.tab_rail.metrics = metrics;
        if !metrics.overflow && self.tab_picker {
            self.tab_picker = false;
            self.focus_active_tab(w, cx);
        }
        let now = Instant::now();
        let dt = now
            .duration_since(self.tab_rail.last_frame)
            .as_secs_f32()
            .min(0.05);
        self.tab_rail.last_frame = now;
        if let Some((start, from, to)) = self.tab_rail.motion {
            let p = (now.duration_since(start).as_secs_f32() / 0.16).min(1.);
            self.tab_rail
                .set_offset(from + (to - from) * (1. - (1. - p).powi(3)));
            if p >= 1. {
                self.tab_rail.motion = None;
            } else {
                w.request_animation_frame();
            }
        }
        self.tab_rail.set_offset(self.tab_rail.offset());
        if self.drag_source.is_none() && self.tab_rail.reveal_pending {
            let index = match self.page {
                Page::Terminal(id) => ids.iter().position(|t| *t == id),
                Page::NewTab => Some(ids.len()),
                _ => None,
            };
            if let Some(index) = index {
                let from = self.tab_rail.offset();
                let to = metrics.reveal(from, index);
                // Resize/reveal is immediate: the newly selected tab is never hidden during easing.
                self.tab_rail.motion = None;
                self.tab_rail.set_offset(to);
            }
            self.tab_rail.reveal_pending = false;
        }
        self.tab_rail.last_tabs = ids;
        if let Some(drag) = self.drag_source.clone() {
            if self.tab_rail.edge != 0 {
                self.tab_rail.motion = None;
                let from = self.tab_rail.offset();
                self.tab_rail
                    .set_offset(from + f32::from(self.tab_rail.edge) * 420. * dt);
                if (self.tab_rail.offset() - from).abs() > 0.01 {
                    self.track_tab_drag(&drag, w.mouse_position(), cx);
                    if self.tab_rail.edge != 0 {
                        w.request_animation_frame();
                    }
                }
            }
        }
    }
    pub(super) fn track_tab_drag(
        &mut self,
        drag: &Drag,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let bounds = self.tab_rail.scroll.bounds();
        if !bounds.contains(&position) {
            self.tab_rail.edge = 0;
            self.tab_join_target = None;
            if self.tab_drop_target.is_some_and(|(id, _)| id != usize::MAX) {
                self.tab_drop_target = None;
                cx.notify();
            }
            return;
        }
        let x = (position.x - bounds.left()) / px(1.);
        self.tab_rail.edge = self.tab_rail.metrics.edge(x);
        self.tab_join_target = None;
        let next = self
            .tab_rail
            .metrics
            .insertion(self.tab_rail.offset(), x, self.tab_rail.last_tabs.len())
            .and_then(|(index, after)| {
                let id = self.tab_rail.last_tabs[index];
                let center =
                    self.tab_rail
                        .metrics
                        .over_tab_center(self.tab_rail.offset(), x, index);
                if drag.tab == id && center {
                    return None;
                }
                if drag.tab != id && self.dock_source(drag).is_some() {
                    self.page = Page::Terminal(id);
                    if center {
                        self.tab_join_target = Some(id);
                        return None;
                    }
                }
                self.tab_drop_gap(drag, id, after).map(|_| (id, after))
            });
        self.tab_drop_target = next;
        self.drop_target = None;
        cx.notify();
    }
    pub(super) fn finish_tab_drop(&mut self, drag: &Drag, w: &mut Window, cx: &mut Context<Self>) {
        if let Some(id) = self.tab_join_target.take()
            && let Some(target) = self
                .tabs
                .iter()
                .find(|t| t.id == id)
                .and_then(|t| t.layout.visual_panes().last().copied())
        {
            self.dock(drag, id, target, Side::Right, cx);
        } else if let Some((id, _)) = self.tab_drop_target {
            self.drop_on_tab(drag, id, cx);
        } else {
            self.clear_drag();
            cx.notify();
        }
        self.focus_active_tab(w, cx);
    }
    pub(super) fn tab_picker_view(&mut self, w: &Window, cx: &mut Context<Self>) -> AnyElement {
        if !self.tab_picker {
            return div().into_any_element();
        }
        let entries = self.filtered_tabs(cx);
        self.tab_picker_index = self.tab_picker_index.min(entries.len().saturating_sub(1));
        let mut list = div()
            .id("all-tab-list")
            .flex()
            .flex_col()
            .gap(px(2.))
            .max_h(px(286.))
            .relative()
            .overflow_y_scroll()
            .track_scroll(&self.scrollers["tab-picker"]);
        if entries.is_empty() {
            list = list.child(
                div()
                    .py_6()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_2()
                    .child(
                        Icon::new(IconName::Search)
                            .size(px(20.))
                            .text_color(self.c(self.col().muted)),
                    )
                    .child("No matching tabs")
                    .child(
                        self.muted("Try a tab name or host address.")
                            .text_size(px(12.)),
                    ),
            );
        }
        for (i, entry) in entries.into_iter().enumerate() {
            let page = entry.page;
            let current = self.page == page;
            list = list.child(
                div()
                    .id(("all-tab-row", i))
                    .group("all-tab-row")
                    .flex()
                    .items_center()
                    .gap_1()
                    .p_1()
                    .rounded_md()
                    .border_1()
                    .border_color(transparent_black())
                    .cursor_pointer()
                    .when(i == self.tab_picker_index, |d| {
                        d.border_color(self.c(self.col().accent))
                    })
                    .hover(|d| d.bg(self.c(self.col().hover)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .gap_2()
                            .items_center()
                            .px_1()
                            .child(
                                Icon::new(if entry.grouped {
                                    IconName::LayoutDashboard
                                } else {
                                    IconName::SquareTerminal
                                })
                                .size(px(16.)),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .child(div().truncate().text_size(px(13.)).child(entry.name))
                                    .child(
                                        div()
                                            .truncate()
                                            .text_size(px(11.))
                                            .text_color(self.c(self.col().muted))
                                            .child(entry.detail),
                                    ),
                            )
                            .when(entry.offline, |d| {
                                d.child(
                                    div().size(px(5.)).rounded_full().bg(self.c(self.col().red)),
                                )
                            })
                            .when(current, |d| {
                                d.child(
                                    div()
                                        .flex_none()
                                        .flex()
                                        .items_center()
                                        .gap_1()
                                        .text_size(px(11.))
                                        .text_color(self.c(self.col().accent))
                                        .child(Icon::new(IconName::Check).size(px(13.)))
                                        .child("Current"),
                                )
                            }),
                    )
                    .child(
                        self.close(("all-tab-close", i))
                            .opacity(0.)
                            .group_hover("all-tab-row", |d| d.opacity(1.))
                            .on_click(cx.listener(move |s, _, w, cx| {
                                cx.stop_propagation();
                                if let Page::Terminal(id) = page {
                                    if s.settings.confirm_close() {
                                        s.tab_picker = false;
                                        s.confirm = Some(Confirm::Tab(id));
                                    } else {
                                        s.close_tab(id, w, cx);
                                    }
                                } else {
                                    s.close_new_tab(w, cx);
                                }
                                if !s.tab_picker {
                                    s.focus_active_tab(w, cx);
                                }
                                cx.notify();
                            })),
                    )
                    .on_click(cx.listener(move |s, _, w, cx| s.activate_tab(page, w, cx))),
            );
        }
        div()
            .id("all-tabs-panel")
            .occlude()
            .absolute()
            .top(px(48.))
            .right(px(160.))
            .w(px(340.))
            .max_h(w.viewport_size().height - px(64.))
            .p_3()
            .rounded_lg()
            .shadow_lg()
            .bg(self.c(self.col().card))
            .border_1()
            .border_color(self.c(self.col().line))
            .flex()
            .flex_col()
            .gap_2()
            .key_context(tab_strip::PICKER_CONTEXT)
            .on_mouse_down_out(cx.listener(|s, e: &MouseDownEvent, w, cx| {
                let right = w.viewport_size().width - px(160.);
                let button = Bounds::new(point(right - px(56.), px(10.)), size(px(56.), px(32.)));
                if !button.contains(&e.position) {
                    s.tab_picker = false;
                    s.focus_active_tab(w, cx);
                    cx.notify();
                }
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().font_weight(FontWeight::SEMIBOLD).child("Group tabs"))
                    .child(
                        self.muted(
                            (self.group_tabs().count() + usize::from(self.new_tab_open()))
                                .to_string(),
                        )
                        .flex_1(),
                    )
                    .child(
                        self.close("all-tabs-dismiss")
                            .on_click(cx.listener(|s, _, w, cx| {
                                s.tab_picker = false;
                                s.focus_active_tab(w, cx);
                                cx.notify();
                            })),
                    ),
            )
            .child(
                Input::new(&self.tab_search)
                    .prefix(Icon::new(IconName::Search).size(px(15.)))
                    .focus_bordered(false)
                    .border_color(self.c(self.col().line))
                    .bg(self.c(self.col().bg)),
            )
            // Append the scrollbar after rows: ScrollHandle indexes direct children.
            .child(list.vertical_scrollbar(&self.scrollers["tab-picker"]))
            .child(
                div()
                    .pt_2()
                    .border_t_1()
                    .border_color(self.c(self.col().line))
                    .flex()
                    .justify_between()
                    .text_size(px(11.))
                    .text_color(self.c(self.col().muted))
                    .child("↑ ↓ Navigate · Enter Switch")
                    .child("Esc Close"),
            )
            .into_any_element()
    }
}
