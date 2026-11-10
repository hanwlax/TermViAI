use super::*;
impl AppView {
    pub(super) fn hosts_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let query = self.search.read(cx).value().to_lowercase();
        let group = self.group.clone();
        let in_group = !group.is_empty();
        let mut crumb = div().flex().items_center().gap_1().child(
            div()
                .id("all-hosts")
                .px_2()
                .py_1()
                .rounded_md()
                .cursor_pointer()
                .text_color(self.c(self.col().blue))
                .hover(|d| d.bg(self.c(self.col().hover)))
                .child("All Hosts")
                .on_click(cx.listener(|s, _, _, cx| {
                    s.host_selection.clear();
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
                        .on_click(cx.listener(move |s, _, _, cx| {
                            s.host_selection.clear();
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
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(self.button("add-group", "+  Group").on_click(cx.listener(
                        |s, _, w, cx| s.open_form(FormKind::Group(s.group.clone()), w, cx),
                    )))
                    .child(self.button("add-host", "+  Host").on_click(cx.listener(
                        move |s, e: &ClickEvent, w, cx| {
                            if in_group {
                                s.menu = Some((MenuKind::AddHost(group.clone()), e.position()));
                                cx.notify();
                            } else {
                                s.open_form(FormKind::Host(None), w, cx);
                            }
                        },
                    ))),
            );
        let groups = self.store.child_groups(&self.group);
        let has_hosts = self.store.hosts.iter().any(|h| {
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
        });
        let host_cards =
            self.store
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
                .enumerate()
                .map(|(i, h)| {
                    let connect = h.clone();
                    let menu = h.clone();
                    let selected = self.host_selection.contains(h);
                    div()
                        .id(("host", i))
                        .group("host-card")
                        .w(px(286.))
                        .min_w(px(200.))
                        .flex_grow()
                        .max_w(px(380.))
                        .flex()
                        .items_center()
                        .gap_3()
                        .p_4()
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
                        .child(
                            Icon::new(IconName::SquareTerminal)
                                .size(px(19.))
                                .text_color(self.c(self.col().accent)),
                        )
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
                                    self.muted(format!("{}@{}", h.username, h.address))
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
                        .child(self.icon(("host-menu", i), IconName::Ellipsis).on_click(
                            cx.listener(move |s, e: &ClickEvent, _, cx| {
                                cx.stop_propagation();
                                s.menu = Some((MenuKind::Host(menu.clone()), e.position()));
                                cx.notify();
                            }),
                        ))
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
                });
        div()
            .id("hosts-page")
            .flex_1()
            .min_h_0()
            .relative()
            .overflow_y_scroll()
            .track_scroll(&self.scrollers["hosts-page"])
            .vertical_scrollbar(&self.scrollers["hosts-page"])
            .p_6()
            .flex()
            .flex_col()
            .gap_5()
            .child(header)
            .child(Input::new(&self.search))
            .child(div().flex().flex_wrap().items_center().gap_3().min_h(px(32.))
                .child(crumb).child(div().flex_1())
                .when(!self.host_selection.is_empty(), |d| d
                    .child(self.muted(format!("{} selected · Enter to open", self.host_selection.len())))
                    .child(self.action_button("open-selected-hosts", "Open selected", IconName::SquareTerminal)
                        .on_click(cx.listener(|s, _, w, cx| s.open_selected_hosts(w, cx))))
                    .child(self.button("clear-selected-hosts", "Clear · Esc").on_click(cx.listener(|s, _, _, cx| { s.host_selection.clear(); cx.notify(); }))))
            )
            .when(!groups.is_empty(), |d| {
                d.child(div().flex().flex_wrap().gap_3().children(
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
                            .w(px(210.))
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
                                Icon::new(IconName::Folder)
                                    .size(px(20.))
                                    .text_color(self.c(self.col().accent)),
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
                            .on_click(cx.listener(move |s, _, _, cx| {
                                s.host_selection.clear();
                            s.group = id.clone();
                                cx.notify();
                            }))
                    }),
                ))
            })
            .child(div().flex().flex_wrap().gap_3().children(host_cards))
            .when(!has_hosts && self.load_error.is_empty(), |d| {
                if !query.is_empty() {
                    d.child(self.empty_state(IconName::Search, format!("No hosts match “{query}”"), "Try a host name, address or group.")
                        .child(self.button("clear-host-search", "Clear search").on_click(cx.listener(|s, _, w, cx| {
                            s.search.update(cx, |input, cx| input.set_value("", w, cx));
                        }))))
                } else {
                    d.child(self.empty_state(IconName::SquareTerminal,
                        if in_group { "No hosts in this group" } else { "Your first connection starts here" },
                        if in_group { "Add a host to this group to organize your connections." } else { "Save an SSH host once, then use it across terminals, Monitor and Port Forwarding." })
                        .child(self.action_button("empty-add-host", "Add host", IconName::Plus).on_click(cx.listener(|s, _, w, cx| s.open_form(FormKind::Host(None), w, cx)))))
                }
            })
            .into_any_element()
    }
    pub(super) fn keychain_page(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("keychain-page")
            .flex_1()
            .min_h_0()
            .relative()
            .overflow_y_scroll()
            .track_scroll(&self.scrollers["keychain-page"])
            .vertical_scrollbar(&self.scrollers["keychain-page"])
            .p_6()
            .flex()
            .flex_col()
            .gap_5()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(self.title("Keychain"))
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(self.button("add-key", "+  Add Key").on_click(
                                cx.listener(|s, _, w, cx| s.open_form(FormKind::Key(None), w, cx)),
                            ))
                            .child(self.button("add-identity", "+  Add Identity").on_click(
                                cx.listener(|s, _, w, cx| {
                                    s.open_form(FormKind::Identity(None), w, cx)
                                }),
                            )),
                    ),
            )
            .child(div().font_weight(FontWeight::SEMIBOLD).child("Keys"))
            .child(div().flex().flex_wrap().gap_3().children(
                self.store.keys.iter().enumerate().map(|(i, k)| {
                    let k = k.clone();
                    self.choice(&format!("key-{i}"), &k.label, &k.path, false)
                        .w(px(300.))
                        .on_click(cx.listener(move |s, _, w, cx| {
                            s.open_form(FormKind::Key(Some(k.clone())), w, cx)
                        }))
                }),
            ))
            .when(self.store.keys.is_empty(), |d| {
                d.child(
                    self.compact_state(
                        IconName::File,
                        "No saved keys",
                        "Save a private key path for your hosts.",
                    )
                    .child(
                        self.action_button("empty-add-key", "Add key", IconName::Plus)
                            .on_click(
                                cx.listener(|s, _, w, cx| s.open_form(FormKind::Key(None), w, cx)),
                            ),
                    ),
                )
            })
            .child(
                div()
                    .mt_3()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Identities"),
            )
            .child(div().flex().flex_wrap().gap_3().children(
                self.store.identities.iter().enumerate().map(|(i, v)| {
                    let v = v.clone();
                    self.choice(&format!("identity-{i}"), &v.label, &v.username, false)
                        .w(px(300.))
                        .on_click(cx.listener(move |s, _, w, cx| {
                            s.open_form(FormKind::Identity(Some(v.clone())), w, cx)
                        }))
                }),
            ))
            .when(self.store.identities.is_empty(), |d| {
                d.child(
                    self.compact_state(
                        IconName::CircleUser,
                        "No saved identities",
                        "Reuse a username and password across hosts.",
                    )
                    .child(
                        self.action_button("empty-add-identity", "Add identity", IconName::Plus)
                            .on_click(cx.listener(|s, _, w, cx| {
                                s.open_form(FormKind::Identity(None), w, cx)
                            })),
                    ),
                )
            })
            .into_any_element()
    }
    pub(super) fn new_tab_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let query = self.search.read(cx).value().to_lowercase();
        div()
            .id("new-tab-page")
            .flex_1()
            .min_h_0()
            .relative()
            .overflow_y_scroll()
            .track_scroll(&self.scrollers["new-tab-page"])
            .vertical_scrollbar(&self.scrollers["new-tab-page"])
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
            .child(Input::new(&self.search))
            .child(self.muted("Enter user@hostname to connect, or reopen a saved workspace."))
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Saved workspaces"),
            )
            .child(
                div().flex().flex_wrap().gap_3().children(
                    self.workspaces
                        .saved
                        .iter()
                        .filter(|x| x.name.to_lowercase().contains(&query))
                        .enumerate()
                        .map(|(i, v)| {
                            let open = v.clone();
                            let remove = v.clone();
                            div()
                                .id(("workspace", i))
                                .group("workspace")
                                .w(px(300.))
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
                                            self.muted(format!(
                                                "{} terminals",
                                                v.layout.pane_count()
                                            )),
                                        ),
                                )
                                .child(
                                    self.close(("remove-saved", i))
                                        .invisible()
                                        .group_hover("workspace", |d| d.visible())
                                        .on_click(cx.listener(move |s, _, _, cx| {
                                            cx.stop_propagation();
                                            let r = s.workspaces.remove_saved(&remove);
                                            s.notify_error(r, cx);
                                        })),
                                )
                                .on_click(
                                    cx.listener(move |s, _, w, cx| s.restore(open.clone(), w, cx)),
                                )
                        }),
                ),
            )
            .when(self.workspaces.saved.is_empty(), |d| {
                d.child(self.compact_state(
                    IconName::LayoutDashboard,
                    "No saved workspaces",
                    "Save a terminal group from its Tab menu or with Ctrl+S.",
                ))
            })
            .child(
                div()
                    .mt_3()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Recent connections"),
            )
            .child(
                div().flex().flex_wrap().gap_3().children(
                    self.workspaces
                        .recent
                        .iter()
                        .filter(|x| {
                            format!("{} {}", x.host.label, x.host.address)
                                .to_lowercase()
                                .contains(&query)
                        })
                        .enumerate()
                        .map(|(i, v)| {
                            let host = v.host.clone();
                            let remove = v.clone();
                            div()
                                .id(("recent", i))
                                .group("recent")
                                .w(px(300.))
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
                                        .invisible()
                                        .group_hover("recent", |d| d.visible())
                                        .on_click(cx.listener(move |s, _, _, cx| {
                                            cx.stop_propagation();
                                            let r = s.workspaces.remove_recent(&remove);
                                            s.notify_error(r, cx);
                                        })),
                                )
                                .on_click(cx.listener(move |s, _, w, cx| {
                                    s.connect(Some(host.clone()), w, cx)
                                }))
                        }),
                ),
            )
            .when(self.workspaces.recent.is_empty(), |d| {
                d.child(
                    self.compact_state(
                        IconName::SquareTerminal,
                        "No recent connections",
                        "Hosts you connect to will appear here.",
                    )
                    .child(
                        self.button("empty-browse-hosts", "Browse hosts")
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.page = Page::Hosts;
                                cx.notify();
                            })),
                    ),
                )
            })
            .into_any_element()
    }
    pub(super) fn forwarding_page(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("forward-page")
            .flex_1()
            .min_h_0()
            .relative()
            .overflow_y_scroll()
            .track_scroll(&self.scrollers["forward-page"])
            .vertical_scrollbar(&self.scrollers["forward-page"])
            .p_6()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(self.title("Port Forwarding"))
                    .child(self.button("forward-add", "+  Add Service").on_click(
                        cx.listener(|s, _, w, cx| s.open_form(FormKind::Forward(None), w, cx)),
                    )),
            )
            .child(self.muted("Publish a local service through one of your SSH hosts."))
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
            .child(div().flex().flex_wrap().gap_3().children(
                self.forwarding.iter().enumerate().map(|(i, v)| {
                    let id = v.rule.id.clone();
                    let edit = v.rule.clone();
                    let active = v.state.active();
                    div()
                        .id(("service", i))
                        .w(px(285.))
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
                                    self.icon(("edit-service", i), IconName::Ellipsis).on_click(
                                        cx.listener(move |s, _, w, cx| {
                                            s.open_form(
                                                FormKind::Forward(Some(edit.clone())),
                                                w,
                                                cx,
                                            )
                                        }),
                                    ),
                                ),
                        )
                        .child(self.muted(v.rule.host.label.clone()))
                        .child(format!(
                            "{} → 127.0.0.1:{}",
                            v.rule.remote_port, v.rule.local_port
                        ))
                        .child(self.muted(v.state.label()))
                        .when(!v.error.is_empty(), |d| {
                            d.child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(self.c(self.col().red))
                                    .child(v.error.clone()),
                            )
                        })
                        .child(
                            self.button(
                                ("service-toggle", i),
                                if active { "Stop" } else { "Start" },
                            )
                            .on_click(cx.listener(
                                move |s, _, _, cx| {
                                    let r = if active {
                                        forwarding::manager().lock().unwrap().stop(&id)
                                    } else {
                                        forwarding::manager().lock().unwrap().start(&id, &s.store)
                                    };
                                    s.notify_error(r, cx);
                                },
                            )),
                        )
                }),
            ))
            .into_any_element()
    }
}
