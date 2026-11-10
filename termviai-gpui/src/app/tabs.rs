use super::*;

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
    fn tab_entries(&self) -> Vec<Entry> {
        let mut entries = self
            .tabs
            .iter()
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
        if self.new_tab_open {
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
                    let pane = self
                        .focused
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
            Page::NewTab => self.search.focus_handle(cx).focus(w),
            _ => self.focus.focus(w),
        }
    }
    pub(super) fn activate_tab(&mut self, page: Page, w: &mut Window, cx: &mut Context<Self>) {
        self.host_selection.clear();
        self.page = page;
        self.tab_picker = false;
        self.menu = None;
        self.tab_rail.reveal_pending = true;
        self.focus_active_tab(w, cx);
        cx.notify();
    }
    pub(super) fn open_new_tab(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.new_tab_open = true;
        self.activate_tab(Page::NewTab, w, cx);
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
            self.new_tab_open = true;
        }
        let count = self.tabs.len() + usize::from(self.new_tab_open);
        let metrics = tab_strip::Metrics::new(w.viewport_size().width / px(1.), count);
        let ids = self.tabs.iter().map(|t| t.id).collect::<Vec<_>>();
        if self.tab_rail.last_tabs != ids
            || self.tab_rail.last_active != Some(self.page)
            || self.tab_rail.metrics.viewport != metrics.viewport
            || self.tab_rail.metrics.content != metrics.content
        {
            self.tab_rail.reveal_pending = true;
        }
        self.tab_rail.last_tabs = ids;
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
                Page::Terminal(id) => self.tabs.iter().position(|t| t.id == id),
                Page::NewTab => Some(self.tabs.len()),
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
            if self.tab_drop_target.is_some_and(|(id, _)| id != usize::MAX) {
                self.tab_drop_target = None;
                cx.notify();
            }
            return;
        }
        let x = (position.x - bounds.left()) / px(1.);
        self.tab_rail.edge = self.tab_rail.metrics.edge(x);
        let next = self
            .tab_rail
            .metrics
            .insertion(self.tab_rail.offset(), x, self.tabs.len())
            .and_then(|(index, after)| {
                let id = self.tabs[index].id;
                if drag.tab != id && self.dock_source(drag).is_some() {
                    self.page = Page::Terminal(id);
                }
                self.tab_drop_gap(drag, id, after).map(|_| (id, after))
            });
        self.tab_drop_target = next;
        self.drop_target = None;
        cx.notify();
    }
    pub(super) fn finish_tab_drop(&mut self, drag: &Drag, w: &mut Window, cx: &mut Context<Self>) {
        if let Some((id, _)) = self.tab_drop_target {
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
                    .cursor_pointer()
                    .when(current, |d| d.bg(self.c(self.col().soft)))
                    .when(!current && i == self.tab_picker_index, |d| {
                        d.bg(self.c(self.col().hover))
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
                                    Icon::new(IconName::Check)
                                        .size(px(15.))
                                        .text_color(self.c(self.col().accent)),
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
                                        s.close_tab(id, cx);
                                    }
                                } else {
                                    s.new_tab_open = false;
                                    if s.page == Page::NewTab {
                                        s.page = s
                                            .tabs
                                            .last()
                                            .map(|t| Page::Terminal(t.id))
                                            .unwrap_or(Page::Hosts);
                                    }
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
                    .child(div().font_weight(FontWeight::SEMIBOLD).child("All tabs"))
                    .child(
                        self.muted((self.tabs.len() + usize::from(self.new_tab_open)).to_string())
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
