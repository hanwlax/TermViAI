use super::*;

const TOOL_CONTENT_WIDTH: f32 = 1248.;
impl AppView {
    fn host_tiles(
        &self,
        query: &str,
        group: &str,
        width: f32,
        cx: &mut Context<Self>,
    ) -> (Div, usize) {
        let hosts: Vec<_> = self
            .store
            .hosts
            .iter()
            .filter(|h| self.matches_host(h, query, group))
            .collect();
        let host_count = hosts.len();
        let grid = div()
            .flex()
            .flex_wrap()
            .gap_3()
            .children(hosts.into_iter().enumerate().map(|(i, h)| {
                let connect = h.clone();
                let menu = h.clone();
                let selected = self.host_selection.contains(h);
                div()
                    .id(("host", i))
                    .group("host-card")
                    .w(px(width))
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
                    .when(selected, |d| {
                        d.bg(self.c(self.col().soft))
                            .border_color(self.c(self.col().accent))
                    })
                    .child(if self.page == Page::Hosts {
                        self.asset_icon(Icon::new(IconName::SquareTerminal))
                            .into_any_element()
                    } else {
                        Icon::new(IconName::SquareTerminal)
                            .size(px(19.))
                            .text_color(self.c(self.col().accent))
                            .into_any_element()
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
        .clamp(1., TOOL_CONTENT_WIDTH)
    }

    pub(super) fn tool_content(&self, window: &Window) -> Div {
        let gutter = Self::tool_gutter(window);
        div()
            .w_full()
            .max_w(px(TOOL_CONTENT_WIDTH + 2. * gutter))
            .px(px(gutter))
            .pt(px(gutter))
            .pb(px(32.))
            .flex()
            .flex_col()
            .gap_4()
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

    pub(super) fn grid_tile_width(available: f32, minimum: f32) -> f32 {
        let columns = ((available + 12.) / (minimum + 12.)).floor().max(1.);
        ((available + 12.) / columns - 12.).floor().max(1.)
    }

    fn tile_width(&self, window: &Window, minimum: f32) -> f32 {
        let available = if self.page == Page::NewTab {
            f32::from(window.viewport_size().width) - self.sidebar_width() - 50.
        } else {
            self.tool_content_width(window)
        };
        Self::grid_tile_width(available, minimum)
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
        width: f32,
    ) -> Stateful<Div> {
        div()
            .id(id)
            .w(px(width))
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
        let query = self.hosts_search.read(cx).value().trim().to_lowercase();
        let group = self.group.clone();
        let in_group = !group.is_empty();
        let mut crumb = div().flex().items_center().gap_1().child(
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
        let header = div()
            .flex()
            .justify_between()
            .items_center()
            .gap_4()
            .child(self.title("Hosts"))
            .child(self.selection_actions(cx));
        let add = self
            .action_button("hosts-add", "Add", IconName::Plus)
            .child(Icon::new(IconName::ChevronDown).size(px(13.)))
            .on_click(cx.listener(move |s, e: &ClickEvent, _, cx| {
                s.menu = Some((MenuKind::AddHost(group.clone()), e.position()));
                cx.notify();
            }));
        let groups = self.store.child_groups(&self.group);
        let (host_cards, host_count) =
            self.host_tiles(&query, &self.group, self.tile_width(w, 240.), cx);

        let body = self.tool_content(w)
            .child(header)
            .child(self.search_bar(&self.hosts_search,cx))
            .child(div().flex().flex_wrap().items_center().gap_3().min_h(px(32.))
                .child(crumb).child(div().flex_1())
                .child(add))
            .when_some(self.drawer.as_ref().filter(|f|matches!(f.kind,FormKind::Group(_))),|d,f| d.child(self.name_editor(f,cx).max_w(px(320.))))
            .when(!groups.is_empty(), |d| {
                d.child(div().font_weight(FontWeight::SEMIBOLD).child(format!("Groups · {}", groups.len()))).child(div().flex().flex_wrap().gap_3().children(
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
                            .w(px(self.tile_width(w, 240.))).min_h(px(72.))
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
        self.tool_scroll("hosts-page", body).into_any_element()
    }
    pub(super) fn keychain_page(&self, w: &Window, cx: &mut Context<Self>) -> AnyElement {
        let body = self
            .tool_content(w)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .justify_between()
                    .child(self.title("Keychain"))
                    .child(
                        self.action_button("add-credential", "Add", IconName::Plus)
                            .child(Icon::new(IconName::ChevronDown).size(px(13.)))
                            .on_click(cx.listener(|s, e: &ClickEvent, _, cx| {
                                s.menu = Some((MenuKind::AddCredential, e.position()));
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(format!("Keys · {}", self.store.keys.len())),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_start()
                    .gap_3()
                    .when(
                        self.drawer
                            .as_ref()
                            .is_some_and(|f| matches!(f.kind, FormKind::Key(None))),
                        |d| d.child(self.inline_credential(self.tile_width(w, 240.), cx)),
                    )
                    .children(self.store.keys.iter().enumerate().map(|(i, k)| {
                        if self.drawer.as_ref().is_some_and(
                            |f| matches!(&f.kind, FormKind::Key(Some(item)) if item == k),
                        ) {
                            return self.inline_credential(self.tile_width(w, 240.), cx);
                        }
                        let k = k.clone();
                        self.credential_card(
                            ("edit-key", i),
                            Icon::new(IconName::Settings2).path("termviai/key.svg"),
                            &k.label,
                            &k.path,
                            self.tile_width(w, 240.),
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
                div()
                    .flex()
                    .flex_wrap()
                    .items_start()
                    .gap_3()
                    .when(
                        self.drawer
                            .as_ref()
                            .is_some_and(|f| matches!(f.kind, FormKind::Identity(None))),
                        |d| d.child(self.inline_credential(self.tile_width(w, 240.), cx)),
                    )
                    .children(self.store.identities.iter().enumerate().map(|(i, v)| {
                        if self.drawer.as_ref().is_some_and(
                            |f| matches!(&f.kind, FormKind::Identity(Some(item)) if item == v),
                        ) {
                            return self.inline_credential(self.tile_width(w, 240.), cx);
                        }
                        let v = v.clone();
                        self.credential_card(
                            ("edit-identity", i),
                            Icon::new(IconName::CircleUser),
                            &v.label,
                            &format!("Username · {}", v.username),
                            self.tile_width(w, 240.),
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
        self.tool_scroll("keychain-page", body).into_any_element()
    }
    pub(super) fn new_tab_page(&self, w: &Window, cx: &mut Context<Self>) -> AnyElement {
        let query = self.new_tab_search.read(cx).value().trim().to_lowercase();
        let width = self.tile_width(w, 242.);
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
        let (host_cards, host_count) = self.host_tiles(&query, "", width, cx);
        let has_results =
            !groups.is_empty() || !saved.is_empty() || !recent.is_empty() || host_count > 0;
        div()
            .id("new-tab-page")
            .w_full()
            .flex_1()
            .min_h_0()
            .relative()
            .overflow_y_scroll()
            .track_scroll(&self.scrollers["new-tab-page"])
            .p_6()
            .flex()
            .flex_col()
            .gap_5()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(self.title("New Tab"))
                    .child(
                        self.button("local-terminal", "Local terminal")
                            .on_click(cx.listener(|s, _, w, cx| s.connect(None, w, cx))),
                    ),
            )
            .child(self.search_bar(&self.new_tab_search, cx))
            .child(self.muted("Enter user@hostname to connect, or reopen a saved workspace."))
            .when(!groups.is_empty(), |d| {
                d.child(div().font_weight(FontWeight::SEMIBOLD).child("Tab groups"))
                    .child(div().flex().flex_wrap().gap_3().children(
                        groups.into_iter().enumerate().map(|(i, g)| {
                            let group = g.clone();
                            let menu = g.clone();
                            let count = g.tabs.len() + usize::from(g.new_tab_open);
                            div()
                                .id(("saved-tab-group", i))
                                .w(px(width))
                                .flex()
                                .items_center()
                                .gap_3()
                                .p_3()
                                .rounded_md()
                                .bg(self.c(self.col().card))
                                .hover(|d| d.bg(self.c(self.col().hover)))
                                .cursor_pointer()
                                .child(Icon::new(IconName::Folder).size(px(18.)))
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
                                        .on_click(cx.listener(move |s, e: &ClickEvent, _, cx| {
                                            cx.stop_propagation();
                                            s.menu = Some((
                                                MenuKind::SavedGroup(menu.clone()),
                                                e.position(),
                                            ));
                                            cx.notify();
                                        })),
                                )
                                .on_click(
                                    cx.listener(move |s, _, w, cx| {
                                        s.open_saved_group(&group, w, cx)
                                    }),
                                )
                        }),
                    ))
            })
            .when(!saved.is_empty(), |d| {
                d.child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Saved workspaces"),
                )
                .child(div().flex().flex_wrap().gap_3().children(
                    saved.into_iter().enumerate().map(|(i, v)| {
                        if let Some(f) = self.drawer.as_ref().filter(
                            |f| matches!(&f.kind, FormKind::RenameWorkspace(ws) if ws.id == v.id),
                        ) {
                            return self.name_editor(f, cx).w(px(width)).into_any_element();
                        }
                        let open = v.clone();
                        let remove = v.clone();
                        div()
                            .id(("workspace", i))
                            .group("workspace")
                            .w(px(width))
                            .flex()
                            .items_center()
                            .gap_3()
                            .p_3()
                            .rounded_md()
                            .bg(self.c(self.col().card))
                            .hover(|d| d.bg(self.c(self.col().hover)))
                            .cursor_pointer()
                            .child(Icon::new(IconName::LayoutDashboard).size(px(18.)))
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
                        .mt_3()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Recent connections"),
                )
                .child(div().flex().flex_wrap().gap_3().children(
                    recent.into_iter().enumerate().map(|(i, v)| {
                        let host = v.host.clone();
                        let remove = v.clone();
                        div()
                            .id(("recent", i))
                            .group("recent")
                            .w(px(width))
                            .flex()
                            .items_center()
                            .gap_3()
                            .p_3()
                            .rounded_md()
                            .bg(self.c(self.col().card))
                            .hover(|d| d.bg(self.c(self.col().hover)))
                            .cursor_pointer()
                            .child(Icon::new(IconName::SquareTerminal).size(px(18.)))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(div().truncate().child(v.host.label.clone()))
                                    .child(self.muted(v.host.endpoint())),
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
                        .child("Saved hosts")
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
            })
            .into_any_element()
    }
    pub(super) fn forwarding_page(&self, w: &Window, cx: &mut Context<Self>) -> AnyElement {
        let body = self
            .tool_content(w)
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(self.title("Port Forwarding"))
                    .child(
                        self.action_button("forward-add", "Add Service", IconName::Plus)
                            .on_click(cx.listener(|s, _, w, cx| {
                                s.open_form(FormKind::Forward(None), w, cx)
                            })),
                    ),
            )
            .child(
                div().flex().flex_wrap().gap_2().children(
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
                ),
            )
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
                div().flex().flex_wrap().gap_3().children(
                    self.forwarding
                        .iter()
                        .enumerate()
                        .filter(|(_, v)| self.forward_filter.matches(v.state))
                        .map(|(i, v)| {
                            let id = v.rule.id.clone();
                            let menu = id.clone();
                            let active = v.state.active();
                            div()
                                .id(("service", i))
                                .w(px(self.tile_width(w, 240.)))
                                .p_4()
                                .rounded_lg()
                                .bg(self.c(self.col().card))
                                .border_1()
                                .border_color(self.c(self.col().line))
                                .flex()
                                .flex_col()
                                .gap_3()
                                .child(
                                    div()
                                        .flex()
                                        .justify_between()
                                        .items_center()
                                        .child(
                                            div()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .child(v.rule.name.clone()),
                                        )
                                        .child(
                                            self.icon(("edit-service", i), IconName::Ellipsis)
                                                .on_click(cx.listener(
                                                    move |s, e: &ClickEvent, _, cx| {
                                                        s.menu = Some((
                                                            MenuKind::Forward(menu.clone()),
                                                            e.position(),
                                                        ));
                                                        cx.notify();
                                                    },
                                                )),
                                        ),
                                )
                                .child(self.muted(format!(
                                    "{}:{} → 127.0.0.1:{}",
                                    v.rule.host.label, v.rule.remote_port, v.rule.local_port
                                )))
                                .child(
                                    div()
                                        .flex()
                                        .flex_wrap()
                                        .items_center()
                                        .justify_between()
                                        .gap_2()
                                        .pt_2()
                                        .border_t_1()
                                        .border_color(self.c(self.col().line))
                                        .child(self.muted(v.status()).text_color(self.c(
                                            if matches!(
                                                v.state,
                                                forwarding::State::Attention
                                                    | forwarding::State::Failed
                                            ) {
                                                self.col().red
                                            } else if v.state == forwarding::State::Running {
                                                self.col().green
                                            } else {
                                                self.col().muted
                                            },
                                        )))
                                        .child(
                                            self.button(
                                                ("service-toggle", i),
                                                if active {
                                                    "Stop"
                                                } else if matches!(
                                                    v.state,
                                                    forwarding::State::Attention
                                                        | forwarding::State::Failed
                                                ) {
                                                    "Retry"
                                                } else {
                                                    "Start"
                                                },
                                            )
                                            .on_click(
                                                cx.listener(move |s, _, _, cx| {
                                                    let r = if active {
                                                        forwarding::manager()
                                                            .lock()
                                                            .unwrap()
                                                            .stop(&id)
                                                    } else {
                                                        forwarding::manager()
                                                            .lock()
                                                            .unwrap()
                                                            .start(&id, &s.store)
                                                    };
                                                    s.notify_error(r, cx);
                                                }),
                                            ),
                                        ),
                                )
                                .when(self.forward_detail.as_ref() == Some(&v.rule.id), |d| {
                                    d.child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
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
        self.tool_scroll("forward-page", body).into_any_element()
    }
}
