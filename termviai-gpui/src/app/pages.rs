use super::*;

const CARD_GAP: f32 = 12.;
const ASSET_CARD_MIN_WIDTH: f32 = 292.;
const SERVICE_CARD_MIN_WIDTH: f32 = 336.;

impl AppView {
    fn host_tiles(
        &self,
        query: &str,
        group: &str,
        columns: u16,
        cx: &mut Context<Self>,
    ) -> (Div, usize) {
        let hosts: Vec<_> = self
            .store
            .hosts
            .iter()
            .filter(|h| self.matches_host(h, query, group))
            .collect();
        let host_count = hosts.len();
        let grid =
            Self::card_grid(columns).children(hosts.into_iter().enumerate().map(|(i, h)| {
                let connect = h.clone();
                let menu = h.clone();
                let selected = self.host_selection.contains(h);
                self.asset_card(("host", i), IconName::SquareTerminal)
                    .group("host-card")
                    .when(selected, |d| {
                        d.bg(self.c(self.col().soft))
                            .border_color(self.c(self.col().accent))
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .truncate()
                                    .child(h.label.clone()),
                            )
                            .child(
                                self.muted(if h.username.is_empty() {
                                    h.address.clone()
                                } else {
                                    format!("{}@{}", h.username, h.address)
                                })
                                .truncate(),
                            ),
                    )
                    .when(selected, |d| {
                        d.child(
                            Icon::new(IconName::CircleCheck)
                                .size(px(16.))
                                .text_color(self.c(self.col().accent)),
                        )
                    })
                    .child(
                        self.icon(("host-menu", i), IconName::Ellipsis)
                            .on_click(cx.listener(move |s, e: &ClickEvent, _, cx| {
                                cx.stop_propagation();
                                s.menu = Some((MenuKind::Host(menu.clone()), e.position()));
                                cx.notify();
                            })),
                    )
                    .on_click(cx.listener(move |s, event: &ClickEvent, w, cx| {
                        if event.modifiers().control {
                            s.host_selection.toggle(connect.clone());
                            s.focus.focus(w);
                            cx.notify();
                        } else {
                            s.host_selection.clear();
                            s.connect(Some(connect.clone()), w, cx);
                        }
                    }))
            }));
        (grid, host_count)
    }

    pub(super) fn matches_host(&self, h: &Host, query: &str, group: &str) -> bool {
        self.store.host_in_group(h, group)
            && format!(
                "{} {} {} {}",
                h.label,
                h.address,
                h.username,
                self.store.group_path(&h.group_id)
            )
            .to_lowercase()
            .contains(query)
    }

    pub(super) fn tool_gutter(window: &Window) -> f32 {
        if f32::from(window.viewport_size().width) <= 1000. {
            16.
        } else {
            24.
        }
    }

    pub(super) fn tool_content_width(&self, window: &Window) -> f32 {
        (f32::from(window.viewport_size().width)
            - self.sidebar_width()
            - 2. * Self::tool_gutter(window))
        .max(1.)
    }

    pub(super) fn tool_content(&self, window: &Window) -> Div {
        let gutter = Self::tool_gutter(window);
        div()
            .w_full()
            .min_w_0()
            .px(px(gutter))
            .pt(px(gutter))
            .pb(px(32.))
            .flex()
            .flex_col()
            .gap_4()
    }

    pub(super) fn tool_header(&self, window: &Window) -> Div {
        self.tool_content(window).py_3().gap_2().items_start()
    }

    pub(super) fn tool_page(&self, id: &'static str, header: Div, body: Div) -> Div {
        div()
            .size_full()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .child(
                div()
                    .w_full()
                    .flex_shrink_0()
                    .bg(self.c(self.col().hover))
                    .border_b_1()
                    .border_color(self.c(self.col().line))
                    .child(header),
            )
            .child(div().flex_1().min_h_0().child(self.tool_scroll(id, body)))
    }

    pub(super) fn add_button(&self, id: &'static str, label: &'static str) -> Stateful<Div> {
        self.toolbar_button(id, label, IconName::Plus)
    }

    fn toolbar_button(
        &self,
        id: &'static str,
        label: &'static str,
        icon: IconName,
    ) -> Stateful<Div> {
        let colors = self.col();
        self.icon_label_button(id, label, icon)
            .min_w(px(96.))
            .h(px(38.))
            .bg(self.c(colors.add))
            .border_color(self.c(colors.add_border))
            .text_color(self.c(colors.text))
            .hover(|d| d.bg(self.c(colors.add_hover)))
            .active(|d| d.bg(self.c(colors.add_pressed)))
            .focusable()
            .tab_stop(true)
            .focus(|d| {
                d.shadow(vec![BoxShadow {
                    color: self.c(colors.accent),
                    offset: Point::default(),
                    blur_radius: px(0.),
                    spread_radius: px(2.),
                }])
            })
    }

    pub(super) fn tool_scroll(&self, id: &'static str, content: Div) -> Div {
        let scroll = &self.scrollers[id];
        div()
            .relative()
            .size_full()
            .min_w_0()
            .min_h_0()
            .child(
                div()
                    .id(id)
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(scroll)
                    .child(content),
            )
            .vertical_scrollbar(scroll)
    }

    pub(super) fn card_columns(available: f32, minimum: f32) -> u16 {
        (((available + CARD_GAP) / (minimum + CARD_GAP)).floor() as u16).max(1)
    }

    pub(super) fn card_grid(columns: u16) -> Div {
        div()
            .w_full()
            .grid()
            .grid_cols(columns)
            .gap(px(CARD_GAP))
            .items_start()
    }

    fn asset_icon(&self, icon: Icon) -> Div {
        div()
            .size(px(32.))
            .flex_shrink_0()
            .rounded_md()
            .bg(self.c(self.col().soft))
            .flex()
            .items_center()
            .justify_center()
            .child(icon.size(px(17.)).text_color(self.c(self.col().accent)))
    }

    fn credential_card(
        &self,
        id: impl Into<ElementId>,
        icon: Icon,
        name: &str,
        detail: &str,
    ) -> Stateful<Div> {
        div()
            .id(id)
            .w_full()
            .min_w_0()
            .min_h(px(72.))
            .p_3()
            .flex()
            .items_center()
            .gap_3()
            .bg(self.c(self.col().card))
            .border_1()
            .border_color(self.c(self.col().line))
            .rounded_lg()
            .cursor_pointer()
            .hover(|d| d.bg(self.c(self.col().hover)))
            .child(self.asset_icon(icon))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .truncate()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(name.to_owned()),
                    )
                    .child(self.muted(detail.to_owned()).truncate()),
            )
            .child(
                Icon::new(IconName::Settings2)
                    .size(px(16.))
                    .flex_shrink_0()
                    .text_color(self.c(self.col().muted)),
            )
    }

    fn search_bar(&self, input: &Entity<InputState>, cx: &mut Context<Self>) -> Div {
        let field = input.clone();
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(div().flex_1().min_w_0().child(Input::new(input)))
            .child(
                self.button("quick-connect", "Quick Connect")
                    .on_click(cx.listener(move |s, _, w, cx| {
                        let text = field.read(cx).value().to_string();
                        if text.trim().is_empty() {
                            field.focus_handle(cx).focus(w);
                        } else {
                            s.quick_connect(&text, w, cx);
                        }
                    })),
            )
    }

    fn search_toolbar(
        &self,
        w: &Window,
        action: Stateful<Div>,
        input: &Entity<InputState>,
        cx: &mut Context<Self>,
    ) -> Div {
        let compact = self.tool_content_width(w) < 640.;
        let search = self.search_bar(input, cx);
        div()
            .w_full()
            .flex()
            .gap_3()
            .when_else(
                compact,
                |d| d.flex_col().items_start(),
                |d| d.items_center(),
            )
            .child(action)
            .child(search.when_else(compact, |d| d.w_full(), |d| d.flex_1().min_w_0()))
    }

    fn selection_actions(&self, cx: &mut Context<Self>) -> Div {
        if self.host_selection.is_empty() {
            return self.muted("Ctrl + click to select hosts");
        }
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                self.action_button(
                    "open-selected-hosts",
                    format!("Open {} in a tab", self.host_selection.len()),
                    IconName::SquareTerminal,
                )
                .on_click(cx.listener(|s, _, w, cx| s.open_selected_hosts(w, cx))),
            )
            .child(
                self.button("clear-selected-hosts", "Clear")
                    .on_click(cx.listener(|s, _, _, cx| {
                        s.host_selection.clear();
                        cx.notify();
                    })),
            )
    }
    pub(super) fn hosts_page(&self, w: &Window, cx: &mut Context<Self>) -> AnyElement {
        let columns = Self::card_columns(self.tool_content_width(w), ASSET_CARD_MIN_WIDTH);
        let query = self.hosts_search.read(cx).value().trim().to_lowercase();
        let group = self.group.clone();
        let in_group = !group.is_empty();
        let mut crumb = div().flex().flex_wrap().items_center().gap_1().child(
            div()
                .id("all-hosts")
                .pr_2()
                .py_1()
                .rounded_md()
                .cursor_pointer()
                .text_color(self.c(self.col().blue))
                .hover(|d| d.bg(self.c(self.col().hover)))
                .child("All Hosts")
                .on_click(cx.listener(|s, _, w, cx| {
                    s.clear_page_search(w, cx);
                    s.group.clear();
                    cx.notify();
                })),
        );
        for g in self.store.group_trail(&group) {
            let id = g.id.clone();
            crumb = crumb
                .child(
                    Icon::new(IconName::ChevronRight)
                        .size(px(14.))
                        .text_color(self.c(self.col().muted)),
                )
                .child(
                    div()
                        .id(SharedString::from(format!("crumb-{id}")))
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .cursor_pointer()
                        .text_color(self.c(self.col().blue))
                        .hover(|d| d.bg(self.c(self.col().hover)))
                        .child(g.name.clone())
                        .on_click(cx.listener(move |s, _, w, cx| {
                            s.clear_page_search(w, cx);
                            s.group = id.clone();
                            cx.notify();
                        })),
                );
        }
        let title = div()
            .w_full()
            .flex_wrap()
            .flex()
            .justify_between()
            .items_center()
            .gap_4()
            .child(self.title("Hosts"))
            .child(self.selection_actions(cx));
        let add = self
            .add_button("hosts-add", "Add")
            .child(Icon::new(IconName::ChevronDown).size(px(13.)))
            .on_click(cx.listener(move |s, e: &ClickEvent, _, cx| {
                s.menu = Some((MenuKind::AddHost(group.clone()), e.position()));
                cx.notify();
            }));
        let groups = self.store.child_groups(&self.group);
        let (host_cards, host_count) = self.host_tiles(&query, &self.group, columns, cx);
        let actions = self.search_toolbar(w, add, &self.hosts_search, cx);
        let header = self.tool_header(w).child(title).child(actions);
        let body = self.tool_content(w).child(crumb)
            .when_some(self.drawer.as_ref().filter(|f|matches!(f.kind,FormKind::Group(_))),|d,f| d.child(self.name_editor(f,cx).max_w(px(320.))))
            .when(!groups.is_empty(), |d| {
                d.child(div().font_weight(FontWeight::SEMIBOLD).child(format!("Groups · {}", groups.len()))).child(Self::card_grid(columns).children(
                    groups.into_iter().enumerate().map(|(i, g)| {
                        let id = g.id.clone();
                        let menu = g.clone();
                        let count = self
                            .store
                            .hosts
                            .iter()
                            .filter(|h| self.store.host_in_group(h, &id))
                            .count();
                        div()
                            .id(("group", i))
                            .group("group-card")
                            .w_full().min_w_0().min_h(px(72.))
                            .flex()
                            .items_center()
                            .gap_3()
                            .p_3()
                            .rounded_lg()
                            .bg(self.c(self.col().card))
                            .border_1()
                            .border_color(self.c(self.col().line))
                            .cursor_pointer()
                            .hover(|d| d.bg(self.c(self.col().hover)))
                            .child(
                                self.asset_icon(Icon::new(IconName::Folder)),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(div().truncate().child(g.name.clone()))
                                    .child(self.muted(format!("{count} hosts"))),
                            )
                            .child(self.icon(("group-menu", i), IconName::Ellipsis).on_click(
                                cx.listener(move |s, e: &ClickEvent, _, cx| {
                                    cx.stop_propagation();
                                    s.menu = Some((MenuKind::Group(menu.clone()), e.position()));
                                    cx.notify();
                                }),
                            ))
                            .on_click(cx.listener(move |s, _, w, cx| {
                                s.clear_page_search(w,cx);
                            s.group = id.clone();
                                cx.notify();
                            }))
                    }),
                ))
            })
            .when(host_count > 0, |d| d.child(div().font_weight(FontWeight::SEMIBOLD).child(format!("Hosts · {host_count}"))).child(host_cards))
            .when(host_count == 0 && self.load_error.is_empty(), |d| {
                if !query.is_empty() {
                    d.child(self.empty_state(IconName::Search, format!("No hosts match “{query}”"), "Try a host name, address or group.")
                        .child(self.button("clear-host-search", "Clear search").on_click(cx.listener(|s, _, w, cx| {
                            s.clear_page_search(w,cx);
                        }))))
                } else {
                    d.child(self.empty_state(IconName::SquareTerminal,
                        if in_group { "No hosts in this group" } else { "Your first connection starts here" },
                        if in_group { "Add a host to this group to organize your connections." } else { "Save an SSH host once, then use it across terminals, Monitor and Port Forwarding." })
                        .child(self.action_button("empty-add-host", "Add host", IconName::Plus).on_click(cx.listener(|s, _, w, cx| s.open_form(FormKind::Host(None), w, cx)))))
                }
            })
            ;
        self.tool_page("hosts-page", header, body)
            .into_any_element()
    }
    pub(super) fn reload_known_hosts(&mut self) {
        self.known_hosts = wezterm_ssh::known_hosts::load(&HostStore::known_hosts_path())
            .map_err(|e| format!("{e:#}"));
    }

    pub(super) fn known_hosts_page(&self, w: &Window, cx: &mut Context<Self>) -> AnyElement {
        let header = self.tool_header(w).child(self.title("Known Hosts"));
        let body = self.tool_content(w);
        let body = match &self.known_hosts {
            Err(error) => {
                body.child(self.compact_state(IconName::File, "Could not read known hosts", error))
            }
            Ok(pins) if pins.is_empty() => body.child(self.empty_state(
                IconName::CircleCheck,
                "No trusted hosts yet",
                "Fingerprints appear here when you trust a host in a terminal.",
            )),
            Ok(pins) => {
                let columns = Self::card_columns(self.tool_content_width(w), 384.);
                body.child(
                    Self::card_grid(columns).children(pins.iter().enumerate().map(
                        |(i, (endpoint, pin))| {
                            let endpoint = endpoint.clone();
                            let pin = pin.clone();
                            div()
                                .id(("trusted-host", i))
                                .group("known-host-card")
                                .min_w_0()
                                .px_4()
                                .py_3()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .border_1()
                                .rounded_md()
                                .border_color(self.c(self.col().line))
                                .bg(self.c(self.col().card))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(Icon::new(IconName::CircleCheck).size(px(16.)))
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .truncate()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .child(endpoint.clone()),
                                        )
                                        .child(
                                            self.close(("forget-host", i))
                                                .opacity(0.)
                                                .group_hover("known-host-card", |d| d.opacity(1.))
                                                .focusable()
                                                .tab_stop(true)
                                                .focus(|d| d.opacity(1.))
                                                .tooltip(|_, cx| {
                                                    cx.new(|_| {
                                                        gpui_component::tooltip::Tooltip::new(
                                                            "Remove trusted host",
                                                        )
                                                    })
                                                    .into()
                                                })
                                                .on_click(cx.listener({
                                                    let pin = pin.clone();
                                                    move |s, _, _, cx| {
                                                        let result =
                                                            wezterm_ssh::known_hosts::remove(
                                                                &HostStore::known_hosts_path(),
                                                                &endpoint,
                                                                &pin,
                                                            );
                                                        s.reload_known_hosts();
                                                        s.notify_error(result, cx);
                                                    }
                                                })),
                                        ),
                                )
                                .child(self.muted(pin).text_size(px(12.)))
                        },
                    )),
                )
            }
        };
        self.tool_page("known-hosts-page", header, body)
            .into_any_element()
    }

    pub(super) fn keychain_page(&self, w: &Window, cx: &mut Context<Self>) -> AnyElement {
        let columns = Self::card_columns(self.tool_content_width(w), ASSET_CARD_MIN_WIDTH);
        let header = self.tool_header(w).child(self.title("Keychain")).child(
            self.add_button("add-credential", "Add")
                .child(Icon::new(IconName::ChevronDown).size(px(13.)))
                .on_click(cx.listener(|s, e: &ClickEvent, _, cx| {
                    s.menu = Some((MenuKind::AddCredential, e.position()));
                    cx.notify();
                })),
        );
        let body = self
            .tool_content(w)
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(format!("Keys · {}", self.store.keys.len())),
            )
            .child(
                Self::card_grid(columns)
                    .when(
                        self.drawer
                            .as_ref()
                            .is_some_and(|f| matches!(f.kind, FormKind::Key(None))),
                        |d| d.child(self.inline_credential(cx)),
                    )
                    .children(self.store.keys.iter().enumerate().map(|(i, k)| {
                        if self.drawer.as_ref().is_some_and(
                            |f| matches!(&f.kind, FormKind::Key(Some(item)) if item == k),
                        ) {
                            return self.inline_credential(cx);
                        }
                        let k = k.clone();
                        self.credential_card(
                            ("edit-key", i),
                            Icon::new(IconName::Settings2).path("termviai/key.svg"),
                            &k.label,
                            &k.path,
                        )
                        .on_click(cx.listener(move |s, _, w, cx| {
                            s.open_form(FormKind::Key(Some(k.clone())), w, cx)
                        }))
                        .into_any_element()
                    })),
            )
            .when(
                self.store.keys.is_empty()
                    && !self
                        .drawer
                        .as_ref()
                        .is_some_and(|f| matches!(f.kind, FormKind::Key(None))),
                |d| {
                    d.child(
                        self.compact_state(
                            IconName::File,
                            "No saved keys",
                            "Save a private key path for your hosts.",
                        )
                        .child(
                            self.action_button("empty-add-key", "Add key", IconName::Plus)
                                .on_click(cx.listener(|s, _, w, cx| {
                                    s.open_form(FormKind::Key(None), w, cx)
                                })),
                        ),
                    )
                },
            )
            .child(
                div()
                    .mt_3()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(format!("Identities · {}", self.store.identities.len())),
            )
            .child(
                Self::card_grid(columns)
                    .when(
                        self.drawer
                            .as_ref()
                            .is_some_and(|f| matches!(f.kind, FormKind::Identity(None))),
                        |d| d.child(self.inline_credential(cx)),
                    )
                    .children(self.store.identities.iter().enumerate().map(|(i, v)| {
                        if self.drawer.as_ref().is_some_and(
                            |f| matches!(&f.kind, FormKind::Identity(Some(item)) if item == v),
                        ) {
                            return self.inline_credential(cx);
                        }
                        let v = v.clone();
                        self.credential_card(
                            ("edit-identity", i),
                            Icon::new(IconName::CircleUser),
                            &v.label,
                            &format!("Username · {}", v.username),
                        )
                        .on_click(cx.listener(move |s, _, w, cx| {
                            s.open_form(FormKind::Identity(Some(v.clone())), w, cx)
                        }))
                        .into_any_element()
                    })),
            )
            .when(
                self.store.identities.is_empty()
                    && !self
                        .drawer
                        .as_ref()
                        .is_some_and(|f| matches!(f.kind, FormKind::Identity(None))),
                |d| {
                    d.child(
                        self.compact_state(
                            IconName::CircleUser,
                            "No saved identities",
                            "Reuse a username and password across hosts.",
                        )
                        .child(
                            self.action_button(
                                "empty-add-identity",
                                "Add identity",
                                IconName::Plus,
                            )
                            .on_click(cx.listener(|s, _, w, cx| {
                                s.open_form(FormKind::Identity(None), w, cx)
                            })),
                        ),
                    )
                },
            );
        self.tool_page("keychain-page", header, body)
            .into_any_element()
    }
    fn asset_card(&self, id: impl Into<ElementId>, icon: IconName) -> Stateful<Div> {
        div()
            .id(id)
            .w_full()
            .min_w_0()
            .min_h(px(72.))
            .flex()
            .items_center()
            .gap_3()
            .p_3()
            .rounded_lg()
            .bg(self.c(self.col().card))
            .border_1()
            .border_color(self.c(self.col().line))
            .cursor_pointer()
            .hover(|d| d.bg(self.c(self.col().hover)))
            .child(self.asset_icon(Icon::new(icon)))
    }

    pub(super) fn new_tab_page(&self, w: &Window, cx: &mut Context<Self>) -> AnyElement {
        let query = self.new_tab_search.read(cx).value().trim().to_lowercase();
        let columns = Self::card_columns(self.tool_content_width(w), ASSET_CARD_MIN_WIDTH);
        let groups: Vec<_> = self
            .workspaces
            .tab_groups
            .iter()
            .filter(|g| {
                g.name.to_lowercase().contains(&query)
                    || g.tabs.iter().any(|t| {
                        t.name.to_lowercase().contains(&query)
                            || t.layout.hosts().iter().any(|p| {
                                p.host
                                    .as_ref()
                                    .is_some_and(|h| self.matches_host(h, &query, ""))
                            })
                    })
            })
            .collect();
        let saved: Vec<_> = self
            .workspaces
            .saved
            .iter()
            .filter(|ws| ws.name.to_lowercase().contains(&query))
            .collect();
        let recent: Vec<_> = self
            .workspaces
            .recent
            .iter()
            .filter(|v| self.matches_host(&v.host, &query, ""))
            .collect();
        let (host_cards, host_count) = self.host_tiles(&query, "", columns, cx);
        let has_results =
            !groups.is_empty() || !saved.is_empty() || !recent.is_empty() || host_count > 0;
        let local = self
            .toolbar_button("local-terminal", "Local terminal", IconName::SquareTerminal)
            .on_click(cx.listener(|s, _, w, cx| s.connect(None, w, cx)));
        let header = self
            .tool_header(w)
            .child(self.title("New Tab"))
            .child(self.search_toolbar(w, local, &self.new_tab_search, cx))
            .child(self.muted("Enter user@hostname to connect, or reopen a saved workspace."));
        let body = self
            .tool_content(w)
            .when(!groups.is_empty(), |d| {
                d.child(div().font_weight(FontWeight::SEMIBOLD).child("Tab groups"))
                    .child(
                        Self::card_grid(columns).children(groups.into_iter().enumerate().map(
                            |(i, g)| {
                                let group = g.clone();
                                let menu = g.clone();
                                let count = g.tabs.len() + usize::from(g.new_tab_open);
                                self.asset_card(("saved-tab-group", i), IconName::Folder)
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child(div().truncate().child(g.name.clone()))
                                            .child(self.muted(format!(
                                                "{count} {}",
                                                if count == 1 { "tab" } else { "tabs" }
                                            ))),
                                    )
                                    .child(
                                        self.icon(("saved-group-menu", i), IconName::Ellipsis)
                                            .on_click(cx.listener(
                                                move |s, e: &ClickEvent, _, cx| {
                                                    cx.stop_propagation();
                                                    s.menu = Some((
                                                        MenuKind::SavedGroup(menu.clone()),
                                                        e.position(),
                                                    ));
                                                    cx.notify();
                                                },
                                            )),
                                    )
                                    .on_click(cx.listener(move |s, _, w, cx| {
                                        s.open_saved_group(&group, w, cx)
                                    }))
                            },
                        )),
                    )
            })
            .when(!saved.is_empty(), |d| {
                d.child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Saved workspaces"),
                )
                .child(Self::card_grid(columns).children(
                    saved.into_iter().enumerate().map(|(i, v)| {
                        if let Some(f) = self.drawer.as_ref().filter(
                            |f| matches!(&f.kind, FormKind::RenameWorkspace(ws) if ws.id == v.id),
                        ) {
                            return self
                                .name_editor(f, cx)
                                .w_full()
                                .min_w_0()
                                .into_any_element();
                        }
                        let open = v.clone();
                        let remove = v.clone();
                        self.asset_card(("workspace", i), IconName::LayoutDashboard)
                            .group("workspace")
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(div().truncate().child(v.name.clone()))
                                    .child(
                                        self.muted(format!("{} terminals", v.layout.pane_count())),
                                    ),
                            )
                            .child(self.icon(("saved-menu", i), IconName::Ellipsis).on_click(
                                cx.listener(move |s, e: &ClickEvent, _, cx| {
                                    cx.stop_propagation();
                                    s.menu = Some((MenuKind::Saved(remove.clone()), e.position()));
                                    cx.notify();
                                }),
                            ))
                            .on_click(
                                cx.listener(move |s, _, w, cx| s.restore(open.clone(), w, cx)),
                            )
                            .into_any_element()
                    }),
                ))
            })
            .when(!recent.is_empty(), |d| {
                d.child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Recent connections"),
                )
                .child(Self::card_grid(columns).children(
                    recent.into_iter().enumerate().map(|(i, v)| {
                        let host = v.host.clone();
                        let remove = v.clone();
                        self.asset_card(("recent", i), IconName::SquareTerminal)
                            .group("recent")
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(div().truncate().child(v.host.label.clone()))
                                    .child(self.muted(v.host.endpoint()).truncate()),
                            )
                            .child(
                                self.close(("remove-recent", i))
                                    .tooltip(|_, cx| {
                                        cx.new(|_| {
                                            gpui_component::tooltip::Tooltip::new(
                                                "Remove from recent connections",
                                            )
                                        })
                                        .into()
                                    })
                                    .on_click(cx.listener(move |s, _, w, cx| {
                                        cx.stop_propagation();
                                        s.clear_page_search(w, cx);
                                        let result = s.workspaces.remove_recent(&remove);
                                        s.notify_error(result, cx);
                                    })),
                            )
                            .on_click(
                                cx.listener(move |s, _, w, cx| {
                                    s.connect(Some(host.clone()), w, cx)
                                }),
                            )
                    }),
                ))
            })
            .when(host_count > 0, |d| {
                d.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .flex_wrap()
                        .gap_2()
                        .child(div().font_weight(FontWeight::SEMIBOLD).child("Saved hosts"))
                        .child(self.selection_actions(cx)),
                )
                .child(host_cards)
            })
            .when(!has_results, |d| {
                if query.is_empty() {
                    d.child(
                        self.compact_state(
                            IconName::SquareTerminal,
                            "No saved connections yet",
                            "Start a local terminal or enter user@hostname.",
                        )
                        .child(
                            self.button("empty-browse-hosts", "Browse hosts").on_click(
                                cx.listener(|s, _, w, cx| s.activate_tab(Page::Hosts, w, cx)),
                            ),
                        ),
                    )
                } else {
                    d.child(
                        self.compact_state(
                            IconName::Search,
                            &format!("No results for “{query}”"),
                            "Try another workspace, host name or address.",
                        )
                        .child(
                            self.button("clear-new-search", "Clear search")
                                .on_click(cx.listener(|s, _, w, cx| {
                                    s.clear_page_search(w, cx);
                                    s.new_tab_search.focus_handle(cx).focus(w);
                                })),
                        ),
                    )
                }
            });
        self.tool_page("new-tab-page", header, body)
            .into_any_element()
    }
    pub(super) fn forwarding_page(&self, w: &Window, cx: &mut Context<Self>) -> AnyElement {
        let filters = div().flex().flex_wrap().gap_2().children(
            [
                (ForwardFilter::All, "All services"),
                (ForwardFilter::Active, "Active"),
                (ForwardFilter::Attention, "Needs attention"),
                (ForwardFilter::Stopped, "Stopped"),
            ]
            .into_iter()
            .enumerate()
            .map(|(i, (filter, label))| {
                self.button(("service-filter", i), label)
                    .when(self.forward_filter == filter, |d| {
                        d.bg(self.c(self.col().soft))
                    })
                    .on_click(cx.listener(move |s, _, _, cx| {
                        s.forward_filter = filter;
                        cx.notify();
                    }))
            }),
        );
        let header = self
            .tool_header(w)
            .child(self.title("Port Forwarding"))
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_wrap()
                    .gap_3()
                    .when_else(
                        self.tool_content_width(w) < 640.,
                        |d| d.flex_col().items_start(),
                        |d| d.items_center().justify_between(),
                    )
                    .child(self.add_button("forward-add", "Add Service").on_click(
                        cx.listener(|s, _, w, cx| s.open_form(FormKind::Forward(None), w, cx)),
                    ))
                    .child(filters),
            );
        let columns = Self::card_columns(self.tool_content_width(w), SERVICE_CARD_MIN_WIDTH);
        let body = self
            .tool_content(w)
            .when(
                !self.forwarding.is_empty()
                    && !self
                        .forwarding
                        .iter()
                        .any(|v| self.forward_filter.matches(v.state)),
                |d| d.child(self.muted("No matching services")),
            )
            .when(!self.forwarding_error.is_empty(), |d| {
                d.child(self.notice("Forwarding needs attention", self.forwarding_error.clone()))
            })
            .when(
                self.forwarding.is_empty() && self.forwarding_error.is_empty(),
                |d| {
                    d.child(
                        self.empty_state(
                            IconName::Globe,
                            "No forwarding services yet",
                            "Connect a remote port to a local service using a saved SSH host.",
                        )
                        .child(
                            self.action_button("empty-add-service", "Add service", IconName::Plus)
                                .on_click(cx.listener(|s, _, w, cx| {
                                    s.open_form(FormKind::Forward(None), w, cx)
                                })),
                        ),
                    )
                },
            )
            .child(
                Self::card_grid(columns).children(
                    self.forwarding
                        .iter()
                        .enumerate()
                        .filter(|(_, v)| self.forward_filter.matches(v.state))
                        .map(|(i, v)| {
                            let id = v.rule.id.clone();
                            let menu = id.clone();
                            let active = v.state.active();
                            let failed = matches!(
                                v.state,
                                forwarding::State::Attention | forwarding::State::Failed
                            );
                            let color = if failed {
                                self.col().red
                            } else if v.state == forwarding::State::Running {
                                self.col().green
                            } else {
                                self.col().muted
                            };
                            let info = div()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(
                                    div()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(v.rule.name.clone()),
                                )
                                .child(self.muted(format!(
                                    "{}:{} → 127.0.0.1:{}",
                                    v.rule.host.label, v.rule.remote_port, v.rule.local_port
                                )));
                            let controls = div()
                                .flex()
                                .items_center()
                                .gap_3()
                                .w_full()
                                .child(
                                    div()
                                        .flex()
                                        .gap_2()
                                        .items_center()
                                        .flex_1()
                                        .min_w_0()
                                        .child(
                                            div()
                                                .size(px(7.))
                                                .flex_shrink_0()
                                                .rounded_full()
                                                .bg(self.c(color)),
                                        )
                                        .child(self.muted(v.status()).text_color(self.c(color))),
                                )
                                .child(
                                    self.button(
                                        ("service-toggle", i),
                                        if active {
                                            "Stop"
                                        } else if failed {
                                            "Retry"
                                        } else {
                                            "Start"
                                        },
                                    )
                                    .min_w(px(74.))
                                    .on_click(cx.listener(
                                        move |s, _, _, cx| {
                                            let r = if active {
                                                forwarding::manager().lock().unwrap().stop(&id)
                                            } else {
                                                forwarding::manager()
                                                    .lock()
                                                    .unwrap()
                                                    .start(&id, &s.store)
                                            };
                                            s.notify_error(r, cx);
                                        },
                                    )),
                                )
                                .child(
                                    self.icon(("edit-service", i), IconName::Ellipsis).on_click(
                                        cx.listener(move |s, e: &ClickEvent, _, cx| {
                                            s.menu = Some((
                                                MenuKind::Forward(menu.clone()),
                                                e.position(),
                                            ));
                                            cx.notify();
                                        }),
                                    ),
                                );
                            div()
                                .id(("service", i))
                                .w_full()
                                .min_w_0()
                                .p_4()
                                .rounded_lg()
                                .bg(self.c(self.col().card))
                                .border_1()
                                .border_color(self.c(self.col().line))
                                .flex()
                                .flex_col()
                                .gap_3()
                                .child(info)
                                .child(controls)
                                .when(self.forward_detail.as_ref() == Some(&v.rule.id), |d| {
                                    d.child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .items_start()
                                            .pt_3()
                                            .border_t_1()
                                            .border_color(self.c(self.col().line))
                                            .child(self.muted(v.rule.host.endpoint()))
                                            .when_some(v.last_attempt, |d, time| {
                                                let seconds =
                                                    time.elapsed().unwrap_or_default().as_secs();
                                                d.child(self.muted(format!(
                                                    "Last connection attempt: {seconds}s ago"
                                                )))
                                            })
                                            .when(
                                                !v.last_failure.is_empty()
                                                    && v.error != v.last_failure,
                                                |d| {
                                                    d.child(self.muted(format!(
                                                        "Last failure: {}",
                                                        v.last_failure
                                                    )))
                                                },
                                            )
                                            .when(!v.error.is_empty(), |d| d.child(v.error.clone()))
                                            .child(
                                                self.button(
                                                    ("close-service-detail", i),
                                                    "Close details",
                                                )
                                                .on_click(cx.listener(|s, _, _, cx| {
                                                    s.forward_detail = None;
                                                    cx.notify();
                                                })),
                                            ),
                                    )
                                })
                        }),
                ),
            );
        self.tool_page("forward-page", header, body)
            .into_any_element()
    }
}
