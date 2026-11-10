use super::*;

// Windows titlebar hit testing in GPUI uses the previous mouse hit-test state.
// Start a native system move from the actual press on these client-area elements
// instead. Do not put this handler on the whole bar: tabs and controls own input.
fn titlebar_drag_area(id: &'static str) -> Stateful<Div> {
    let area = div().id(id).h_full();
    #[cfg(windows)]
    return area.on_mouse_down(MouseButton::Left, |event, window, cx| {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        use windows::Win32::{
            Foundation::{HWND, LPARAM, WPARAM},
            UI::{
                Input::KeyboardAndMouse::ReleaseCapture,
                WindowsAndMessaging::{
                    HTCAPTION, PostMessageW, SC_MAXIMIZE, SC_MOVE, SC_RESTORE, WM_SYSCOMMAND,
                },
            },
        };
        let Ok(handle) = HasWindowHandle::window_handle(window) else {
            return;
        };
        let RawWindowHandle::Win32(handle) = handle.as_raw() else {
            return;
        };
        let command = if event.click_count == 2 {
            if window.is_maximized() {
                SC_RESTORE
            } else {
                SC_MAXIMIZE
            }
        } else {
            SC_MOVE | HTCAPTION
        };
        // Post, rather than send, to avoid reentering GPUI's input callback while
        // it holds the window state. Windows owns dragging, snapping and restore.
        unsafe {
            let _ = ReleaseCapture();
            if let Err(error) = PostMessageW(
                Some(HWND(handle.hwnd.get() as *mut std::ffi::c_void)),
                WM_SYSCOMMAND,
                WPARAM(command as usize),
                LPARAM(0),
            ) {
                log::error!("Could not start native titlebar action: {error}");
            }
        }
        window.prevent_default();
        cx.stop_propagation();
    });
    #[cfg(not(windows))]
    area.window_control_area(WindowControlArea::Drag)
}

#[cfg(windows)]
fn activate_window_control(area: WindowControlArea, window: &Window) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows::Win32::{
        Foundation::{HWND, LPARAM, WPARAM},
        UI::WindowsAndMessaging::{
            PostMessageW, SC_MAXIMIZE, SC_MINIMIZE, SC_RESTORE, WM_CLOSE, WM_SYSCOMMAND,
        },
    };
    let Ok(handle) = HasWindowHandle::window_handle(window) else {
        return;
    };
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return;
    };
    let (message, command) = match area {
        WindowControlArea::Min => (WM_SYSCOMMAND, SC_MINIMIZE),
        WindowControlArea::Max => (
            WM_SYSCOMMAND,
            if window.is_maximized() {
                SC_RESTORE
            } else {
                SC_MAXIMIZE
            },
        ),
        // WM_CLOSE preserves on_window_should_close and its confirmation flow.
        WindowControlArea::Close => (WM_CLOSE, 0),
        WindowControlArea::Drag => return,
    };
    unsafe {
        if let Err(error) = PostMessageW(
            Some(HWND(handle.hwnd.get() as *mut std::ffi::c_void)),
            message,
            WPARAM(command as usize),
            LPARAM(0),
        ) {
            log::error!("Could not perform native window control action: {error}");
        }
    }
}

#[derive(Clone)]
enum MenuAction {
    MonitorPause(usize),
    MonitorReason(usize),
    MonitorRemove(usize),
    MonitorInterval(u64),
    EditHost(Host),
    Duplicate(Host),
    DeleteHost(Host),
    NewHost(String),
    Existing(String),
    NewGroup(String),
    DeleteGroup(HostGroup),
    Rename(usize),
    Save(usize),
    Reconnect(usize),
    Close(usize),
}
impl AppView {
    fn top_bar(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let metrics = self.tab_rail.metrics;
        let offset = self.tab_rail.offset();
        let mut tabs = div()
            .id("tab-strip")
            .w(px(metrics.viewport))
            .flex_shrink_0()
            .min_w_0()
            .h_full()
            .flex()
            .items_center()
            .gap(px(tab_strip::GAP))
            // Keep programmatic scroll offsets, but let our wheel handler own easing.
            // Overflow::Scroll also installs GPUI's immediate wheel handler.
            .overflow_hidden()
            .track_scroll(&self.tab_rail.scroll)
            .on_scroll_wheel(cx.listener(|s, e: &ScrollWheelEvent, _, cx| {
                if !s.tab_rail.metrics.overflow || e.modifiers.control {
                    return;
                }
                let delta = e.delta.pixel_delta(px(28.));
                let x = if delta.x.abs() > delta.y.abs() {
                    delta.x
                } else {
                    delta.y
                };
                s.scroll_tabs(-x / px(1.), matches!(e.delta, ScrollDelta::Lines(_)), cx);
                cx.stop_propagation();
            }))
            .on_drag_move::<Drag>(cx.listener(|s, e: &DragMoveEvent<Drag>, _, cx| {
                let drag = e.drag(cx).clone();
                s.track_tab_drag(&drag, e.event.position, cx);
            }))
            .on_drop(cx.listener(|s, d: &Drag, w, cx| {
                cx.stop_propagation();
                s.finish_tab_drop(d, w, cx);
            }));
        for t in &self.tabs {
            let id = t.id;
            let active = self.page == Page::Terminal(id);
            let all =
                t.broadcast.enabled() && t.broadcast.member_count() == t.broadcast.pane_count();
            let offline = t
                .layout
                .panes()
                .iter()
                .any(|p| !self.panes[p].session.online());
            let drag = Drag {
                pane: None,
                tab: id,
                label: t.name.clone(),
            };
            let owner = cx.entity();
            tabs = tabs.child(
                div()
                    .id(("tab", id))
                    .group("tab")
                    .relative()
                    .h(px(36.))
                    .w(px(242.))
                    .px_2()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded_md()
                    .cursor_pointer()
                    .when(active, |d| d.bg(self.c(self.col().card)))
                    .hover(|d| d.bg(self.c(self.col().hover)))
                    .child(Icon::new(IconName::SquareTerminal).size(px(15.)))
                    .child(div().flex_1().min_w_0().truncate().child(t.name.clone()))
                    .when(t.layout.panes().len() > 1, |d| {
                        d.child(
                            div()
                                .id(("tab-bc", id))
                                .size(px(26.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_md()
                                .hover(|d| d.bg(self.c(self.col().icon_hover)))
                                .child(self.radio(all))
                                .on_click(cx.listener(move |s, _, _, cx| {
                                    cx.stop_propagation();
                                    s.broadcast_all(id, cx);
                                })),
                        )
                    })
                    .when(offline, |d| {
                        d.child(
                            div()
                                .id(("tab-reconnect", id))
                                .size(px(28.))
                                .flex_shrink_0()
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_md()
                                .cursor_pointer()
                                .text_color(self.c(self.col().blue))
                                .hover(|d| d.bg(self.c(self.col().icon_hover)))
                                .child(
                                    svg()
                                        .path("termviai/reconnect.svg")
                                        .size(px(16.))
                                        .text_color(self.c(self.col().blue)),
                                )
                                .on_click(cx.listener(move |s, _, w, cx| {
                                    cx.stop_propagation();
                                    s.reconnect(id, w, cx);
                                })),
                        )
                    })
                    .child(
                        self.icon(("tab-menu", id), IconName::Ellipsis)
                            .on_click(cx.listener(move |s, e: &ClickEvent, _, cx| {
                                cx.stop_propagation();
                                s.menu = Some((MenuKind::Tab(id), e.position()));
                                cx.notify();
                            })),
                    )
                    .child(self.close(("tab-close", id)).on_click(cx.listener(
                        move |s, _, _, cx| {
                            cx.stop_propagation();
                            if s.settings.confirm_close() {
                                s.confirm = Some(Confirm::Tab(id));
                                cx.notify();
                            } else {
                                s.close_tab(id, cx);
                            }
                        },
                    )))
                    .when(active, |d| {
                        d.child(
                            div()
                                .absolute()
                                .bottom_0()
                                .left(px(10.))
                                .right(px(10.))
                                .h(px(2.))
                                .bg(self.c(self.col().accent)),
                        )
                    })
                    .on_click(cx.listener(move |s, _, w, cx| {
                        s.activate_tab(Page::Terminal(id), w, cx);
                    }))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |s, e: &MouseDownEvent, _, cx| {
                            s.menu = Some((MenuKind::Tab(id), e.position));
                            cx.notify();
                        }),
                    )
                    .on_drag(drag, move |d: &Drag, _, _, cx| {
                        owner.update(cx, |s, cx| {
                            s.clear_drag();
                            s.drag_source = Some(d.clone());
                            cx.notify();
                        });
                        cx.new(|_| d.clone())
                    })
                    .on_drop(cx.listener(|s, d: &Drag, w, cx| {
                        cx.stop_propagation();
                        s.finish_tab_drop(d, w, cx);
                    }))
                    .when_some(
                        self.tab_drop_target.filter(|(t, _)| *t == id),
                        |d, (_, after)| {
                            d.child(
                                div()
                                    .absolute()
                                    .top(px(4.))
                                    .bottom(px(4.))
                                    .w(px(2.))
                                    .rounded_full()
                                    .bg(self.c(self.col().accent))
                                    .when(after, |d| d.right_0())
                                    .when(!after, |d| d.left_0()),
                            )
                        },
                    ),
            );
        }
        if self.new_tab_open {
            tabs = tabs.child(
                div()
                    .id("new-tab-title")
                    .w(px(242.))
                    .flex_shrink_0()
                    .h(px(36.))
                    .px_3()
                    .flex()
                    .gap_3()
                    .items_center()
                    .rounded_md()
                    .when(self.page == Page::NewTab, |d| d.bg(self.c(self.col().card)))
                    .hover(|d| d.bg(self.c(self.col().hover)))
                    .cursor_pointer()
                    .on_click(cx.listener(|s, _, w, cx| s.activate_tab(Page::NewTab, w, cx)))
                    .child(div().flex_1().min_w_0().truncate().child("New Tab"))
                    .child(
                        self.close("close-new-tab")
                            .on_click(cx.listener(|s, _, w, cx| {
                                cx.stop_propagation();
                                s.new_tab_open = false;
                                if s.page == Page::NewTab {
                                    s.page = s
                                        .tabs
                                        .last()
                                        .map(|t| Page::Terminal(t.id))
                                        .unwrap_or(Page::Hosts);
                                }
                                s.focus_active_tab(w, cx);
                                cx.notify();
                            })),
                    ),
            );
        }
        let rail = div()
            .h_full()
            .flex()
            .items_center()
            .gap_1()
            .flex_shrink_0()
            .when(metrics.overflow, |d| {
                d.child(
                    self.icon("tabs-prev", IconName::ChevronLeft)
                        .w(px(26.))
                        .h(px(30.))
                        .opacity(if offset <= 0.5 { 0.28 } else { 1. })
                        .on_click(cx.listener(|s, _, _, cx| {
                            s.scroll_tabs(-s.tab_rail.metrics.viewport * 0.8, true, cx)
                        })),
                )
            })
            .child(tabs)
            .when(metrics.overflow, |d| {
                d.child(
                    self.icon("tabs-next", IconName::ChevronRight)
                        .w(px(26.))
                        .h(px(30.))
                        .opacity(if offset >= metrics.max_offset() - 0.5 {
                            0.28
                        } else {
                            1.
                        })
                        .on_click(cx.listener(|s, _, _, cx| {
                            s.scroll_tabs(s.tab_rail.metrics.viewport * 0.8, true, cx)
                        })),
                )
            });
        div()
            .h(px(52.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_1()
            .pl_2()
            .bg(self.c(self.col().top))
            .border_b_1()
            .border_color(self.c(self.col().line))
            .child(
                div()
                    .w(px(152.))
                    .h_full()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        self.icon("sidebar-toggle", IconName::PanelLeft)
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.sidebar = !s.sidebar;
                                cx.notify();
                            })),
                    )
                    .child(
                        titlebar_drag_area("brand-drag")
                            .flex_1()
                            .flex()
                            .items_center()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("TermViAI"),
                    ),
            )
            .child(rail)
            .child(
                self.icon("new-tab", IconName::Plus)
                    .size(px(30.))
                    .on_drop(cx.listener(|s, d: &Drag, _, cx| {
                        s.detach_pane(d, cx);
                        cx.stop_propagation();
                    }))
                    .on_click(cx.listener(|s, _, w, cx| {
                        s.open_new_tab(w, cx);
                    })),
            )
            .when(metrics.overflow, |d| {
                d.child(
                    div()
                        .id("all-tabs-toggle")
                        .w(px(56.))
                        .h(px(30.))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .gap_1()
                        .rounded_md()
                        .cursor_pointer()
                        .text_size(px(11.))
                        .text_color(self.c(self.col().muted))
                        .when(self.tab_picker, |d| {
                            d.bg(self.c(self.col().soft))
                                .text_color(self.c(self.col().accent))
                        })
                        .hover(|d| d.bg(self.c(self.col().icon_hover)))
                        .child(
                            svg()
                                .path("termviai/list-filter.svg")
                                .size(px(16.))
                                .text_color(self.c(self.col().muted)),
                        )
                        .child((self.tabs.len() + usize::from(self.new_tab_open)).to_string())
                        .on_click(cx.listener(|s, _, w, cx| s.toggle_tab_picker(w, cx))),
                )
            })
            .child(
                titlebar_drag_area("titlebar-drag")
                    .flex_1()
                    .min_w(px(26.))
                    .relative()
                    .on_drag_move::<Drag>(cx.listener(|s, e: &DragMoveEvent<Drag>, _, cx| {
                        if !e.bounds.contains(&e.event.position) {
                            if s.tab_drop_target.is_some_and(|(t, _)| t == usize::MAX) {
                                s.tab_drop_target = None;
                                cx.notify();
                            }
                            return;
                        }
                        let next = s
                            .tab_drop_gap(e.drag(cx), usize::MAX, true)
                            .map(|_| (usize::MAX, true));
                        if s.tab_drop_target != next || s.drop_target.is_some() {
                            s.tab_drop_target = next;
                            s.drop_target = None;
                            cx.notify();
                        }
                    }))
                    .on_drop(cx.listener(|s, d: &Drag, _, cx| {
                        cx.stop_propagation();
                        s.drop_on_tab(d, usize::MAX, cx);
                    }))
                    .when(
                        self.tab_drop_target.is_some_and(|(t, _)| t == usize::MAX),
                        |d| {
                            d.child(
                                div()
                                    .absolute()
                                    .left_0()
                                    .top(px(12.))
                                    .bottom(px(12.))
                                    .w(px(2.))
                                    .rounded_full()
                                    .bg(self.c(self.col().accent)),
                            )
                        },
                    ),
            )
            .child(self.window_controls(window))
            .into_any_element()
    }
    fn window_controls(&self, window: &Window) -> impl IntoElement {
        div().flex().h_full().flex_shrink_0().children(
            [
                (
                    "window-minimize",
                    "icons/window-minimize.svg",
                    WindowControlArea::Min,
                ),
                (
                    "window-maximize",
                    if window.is_maximized() {
                        "icons/window-restore.svg"
                    } else {
                        "icons/window-maximize.svg"
                    },
                    WindowControlArea::Max,
                ),
                (
                    "window-close",
                    "icons/window-close.svg",
                    WindowControlArea::Close,
                ),
            ]
            .into_iter()
            .map(|(id, path, area)| {
                let close = id == "window-close";
                div()
                    .id(id)
                    .group("window-control")
                    .w(px(42.))
                    .h_full()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(self.c(self.col().muted))
                    .when(cfg!(not(windows)), |d| d.window_control_area(area))
                    .on_click(move |_, window, cx| {
                        cx.stop_propagation();
                        #[cfg(windows)]
                        activate_window_control(area, window);
                    })
                    .hover(|s| {
                        if close {
                            s.text_color(self.c(self.col().red))
                        } else {
                            s.bg(self.c(self.col().icon_hover))
                                .text_color(self.c(self.col().text))
                        }
                    })
                    .child(
                        svg()
                            .path(path)
                            .size(px(16.))
                            .text_color(self.c(self.col().muted))
                            .group_hover("window-control", |s| {
                                s.text_color(self.c(if close {
                                    self.col().red
                                } else {
                                    self.col().text
                                }))
                            }),
                    )
            }),
        )
    }
    fn sidebar_view(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .w(px(if self.sidebar { 176. } else { 48. }))
            .flex_shrink_0()
            .h_full()
            .px_2()
            .py_3()
            .flex()
            .flex_col()
            .gap_2()
            .bg(self.c(self.col().side))
            .border_r_1()
            .border_color(self.c(self.col().line))
            .children(
                [
                    (Page::Hosts, "Hosts", IconName::LayoutDashboard),
                    (Page::Keychain, "Keychain", IconName::Asterisk),
                    (Page::Monitor, "Monitor", IconName::ChartPie),
                    (Page::Forwarding, "Port Forwarding", IconName::Replace),
                ]
                .into_iter()
                .enumerate()
                .map(|(i, (page, label, icon))| {
                    div()
                        .id(("navigation", i))
                        .h(px(40.))
                        .flex()
                        .items_center()
                        .gap_3()
                        .px_2()
                        .rounded_md()
                        .cursor_pointer()
                        .when(self.page == page, |d| {
                            d.bg(self.c(self.col().soft))
                                .text_color(self.c(self.col().accent))
                        })
                        .hover(|d| d.bg(self.c(self.col().hover)))
                        .child(
                            Icon::new(icon)
                                .when(page == Page::Keychain, |i| i.path("termviai/key.svg"))
                                .size(px(18.)),
                        )
                        .when(self.sidebar, |d| d.child(label))
                        .on_click(cx.listener(move |s, _, _, cx| {
                            s.host_selection.clear();
                            s.page = page;
                            s.menu = None;
                            cx.notify();
                        }))
                }),
            )
            .child(div().flex_1())
            .child(
                div()
                    .id("settings-nav")
                    .h(px(40.))
                    .flex()
                    .items_center()
                    .gap_3()
                    .px_2()
                    .rounded_md()
                    .cursor_pointer()
                    .hover(|d| d.bg(self.c(self.col().hover)))
                    .child(Icon::new(IconName::Settings).size(px(18.)))
                    .when(self.sidebar, |d| d.child("Settings"))
                    .on_click(cx.listener(|s, _, w, cx| s.open_form(FormKind::Settings, w, cx))),
            )
            .into_any_element()
    }
    fn menu_view(&self, w: &Window, cx: &mut Context<Self>) -> AnyElement {
        let Some((kind, pos)) = &self.menu else {
            return div().into_any_element();
        };
        let items: Vec<(&str, IconName, MenuAction)> = match kind {
            MenuKind::Monitor(id) => {
                let Some(e) = self.monitor.entries.get(*id) else {
                    return div().into_any_element();
                };
                vec![
                    (
                        if !e.error.is_empty() {
                            "Retry monitoring"
                        } else if e.paused {
                            "Resume sampling"
                        } else {
                            "Pause sampling"
                        },
                        IconName::Redo,
                        MenuAction::MonitorPause(*id),
                    ),
                    (
                        "View connection",
                        IconName::Info,
                        MenuAction::MonitorReason(*id),
                    ),
                    (
                        "Remove monitor",
                        IconName::Delete,
                        MenuAction::MonitorRemove(*id),
                    ),
                ]
            }
            MenuKind::MonitorInterval => vec![
                (
                    "2 seconds",
                    IconName::Settings2,
                    MenuAction::MonitorInterval(2),
                ),
                (
                    "5 seconds",
                    IconName::Settings2,
                    MenuAction::MonitorInterval(5),
                ),
                (
                    "10 seconds",
                    IconName::Settings2,
                    MenuAction::MonitorInterval(10),
                ),
            ],
            MenuKind::Host(h) => vec![
                (
                    "Edit Host",
                    IconName::Settings2,
                    MenuAction::EditHost(h.clone()),
                ),
                (
                    "Duplicate Host",
                    IconName::Copy,
                    MenuAction::Duplicate(h.clone()),
                ),
                (
                    "Delete Host",
                    IconName::Delete,
                    MenuAction::DeleteHost(h.clone()),
                ),
            ],
            MenuKind::Group(g) => vec![
                (
                    "Add Host",
                    IconName::Plus,
                    MenuAction::NewHost(g.id.clone()),
                ),
                (
                    "Add Existing Hosts",
                    IconName::SquareTerminal,
                    MenuAction::Existing(g.id.clone()),
                ),
                (
                    "Add Group",
                    IconName::Folder,
                    MenuAction::NewGroup(g.id.clone()),
                ),
                (
                    "Delete Group",
                    IconName::Delete,
                    MenuAction::DeleteGroup(g.clone()),
                ),
            ],
            MenuKind::AddHost(g) => vec![
                ("New Host", IconName::Plus, MenuAction::NewHost(g.clone())),
                (
                    "Add Existing Hosts",
                    IconName::SquareTerminal,
                    MenuAction::Existing(g.clone()),
                ),
            ],
            MenuKind::Tab(id) => vec![
                ("Rename Tab", IconName::Settings2, MenuAction::Rename(*id)),
                (
                    "Save Workspace",
                    IconName::LayoutDashboard,
                    MenuAction::Save(*id),
                ),
                (
                    "Reconnect Offline",
                    IconName::Redo,
                    MenuAction::Reconnect(*id),
                ),
                ("Close Tab", IconName::Close, MenuAction::Close(*id)),
            ],
        };
        let interval_menu = matches!(kind, MenuKind::MonitorInterval);
        let height = items.len() as f32 * 34. + 12. + if interval_menu { 64. } else { 0. };
        let left = pos.x.min(w.viewport_size().width - px(225.)).max(px(0.));
        let top = pos.y.min(w.viewport_size().height - px(height)).max(px(0.));
        div()
            .absolute()
            .inset_0()
            .child(
                div()
                    .id("menu-dismiss")
                    .absolute()
                    .inset_0()
                    .on_click(cx.listener(|s, _, _, cx| {
                        s.menu = None;
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("action-menu-panel")
                    .occlude()
                    .absolute()
                    .left(left)
                    .top(top)
                    .w(px(220.))
                    .p_1()
                    .rounded_lg()
                    .bg(self.c(self.col().card))
                    .border_1()
                    .border_color(self.c(self.col().line))
                    .shadow_lg()
                    .flex()
                    .flex_col()
                    .when(interval_menu, |d| d.child(self.muted("Sampling interval").px_3().py_2()))
                    .children(
                        items
                            .into_iter()
                            .enumerate()
                            .map(|(i, (label, icon, action))| {
                                let selected = matches!(&action, MenuAction::MonitorInterval(v) if *v == self.monitor.interval);
                                div()
                                    .id(("menu-action", i))
                                    .h(px(34.))
                                    .px_3()
                                    .rounded_md()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .cursor_pointer()
                                    .hover(|d| d.bg(self.c(self.col().icon_hover)))
                                    .when(!interval_menu, |d| d.child(Icon::new(icon).size(px(15.))))
                                    .when(selected, |d| d.bg(self.c(self.col().soft)).text_color(self.c(self.col().accent)))
                                    .child(label)
                                    .when(interval_menu, |d| d.child(div().flex_1()))
                                    .when(selected, |d| d.child(Icon::new(IconName::Check).size(px(15.))))
                                    .on_click(cx.listener(move |s, _, w, cx| {
                                        cx.stop_propagation();
                                        s.menu = None;
                                        match &action {
                                            MenuAction::MonitorPause(id) => {
                                                if let Some(e) = s.monitor.entries.get_mut(*id) {
                                                    if e.error.is_empty() && !e.paused {
                                                        e.paused = true;
                                                    } else {
                                                        s.monitor.retry(*id);
                                                    }
                                                }
                                            }
                                            MenuAction::MonitorReason(id) => {
                                                s.monitor_reason = Some(*id);
                                            }
                                            MenuAction::MonitorRemove(id) => {
                                                s.confirm = Some(Confirm::RemoveMonitor(*id));
                                            }
                                            MenuAction::MonitorInterval(seconds) => {
                                                let previous = s.monitor.interval;
                                                s.monitor.interval = *seconds;
                                                let r = s.monitor.save();
                                                if r.is_err() { s.monitor.interval = previous; }
                                                s.notify_error(r, cx);
                                            }
                                            MenuAction::EditHost(h) => {
                                                s.open_form(FormKind::Host(Some(h.clone())), w, cx)
                                            }
                                            MenuAction::Duplicate(h) => {
                                                let r = s.change_store(|store| {
                                                    store.duplicate_host(h).map(|_| ())
                                                });
                                                s.notify_error(r, cx);
                                            }
                                            MenuAction::DeleteHost(h) => {
                                                s.confirm = Some(Confirm::Host(h.clone()))
                                            }
                                            MenuAction::NewHost(g) => {
                                                let mut f =
                                                    s.make_form(FormKind::Host(None), w, cx);
                                                f.group = g.clone();
                                                s.drawer = Some(f);
                                            }
                                            MenuAction::Existing(g) => {
                                                s.open_form(FormKind::Existing(g.clone()), w, cx)
                                            }
                                            MenuAction::NewGroup(g) => {
                                                s.open_form(FormKind::Group(g.clone()), w, cx)
                                            }
                                            MenuAction::DeleteGroup(g) => {
                                                s.confirm = Some(Confirm::Group(g.clone()))
                                            }
                                            MenuAction::Rename(id) => {
                                                s.open_form(FormKind::Rename(*id), w, cx)
                                            }
                                            MenuAction::Save(id) => {
                                                let r = s.save_workspace(*id);
                                                s.notify_error(r, cx);
                                            }
                                            MenuAction::Reconnect(id) => s.reconnect(*id, w, cx),
                                            MenuAction::Close(id) => {
                                                if s.settings.confirm_close() {
                                                    s.confirm = Some(Confirm::Tab(*id));
                                                } else {
                                                    s.close_tab(*id, cx);
                                                }
                                            }
                                        }
                                        cx.notify();
                                    }))
                            }),
                    )
                    .when(interval_menu, |d| d.child(self.muted("Applies to all monitored hosts").px_3().py_2().border_t_1().border_color(self.c(self.col().line)))),
            )
            .into_any_element()
    }
    fn confirm_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(action) = &self.confirm else {
            return div().into_any_element();
        };
        let (title, message, button) = match action {
            Confirm::RemoveMonitor(_) => (
                "Remove monitor",
                "Remove this host from Monitor? Its saved Host and terminal connections are kept."
                    .into(),
                "Remove",
            ),
            Confirm::Host(h) => (
                "Delete Host",
                format!(
                    "Remove {} from Hosts? Open terminals will remain connected.",
                    h.label
                ),
                "Delete",
            ),
            Confirm::Group(g) => (
                "Delete Group",
                format!(
                    "Delete {}? Its hosts and child groups will move to the parent group.",
                    g.name
                ),
                "Delete",
            ),
            Confirm::Key(k) => (
                "Delete Key",
                format!(
                    "Remove {} from Keychain? The key file will not be deleted.",
                    k.label
                ),
                "Delete",
            ),
            Confirm::Identity(i) => (
                "Delete Identity",
                format!("Remove {} from Keychain?", i.label),
                "Delete",
            ),
            Confirm::Stop(_, p) => (
                "End processes",
                format!(
                    "Send termination requests to {} processes? Review the working directories below.",
                    p.len()
                ),
                "End processes",
            ),
            Confirm::Forward(_) => (
                "Delete Service",
                "Stop the tunnel and remove this service?".into(),
                "Delete",
            ),
            Confirm::Quit => (
                "Close TermViAI",
                "Close all terminal sessions and exit?".into(),
                "Close",
            ),
            _ => (
                "Close terminal",
                "Close this terminal session? Running remote work may be interrupted.".into(),
                "Close",
            ),
        };
        div()
            .id("confirm-overlay")
            .occlude()
            .absolute()
            .inset_0()
            .bg(rgba(0x00000066))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .w(px(480.))
                    .max_h(relative(0.8))
                    .max_w(relative(0.9))
                    .p_5()
                    .rounded_lg()
                    .bg(self.c(self.col().card))
                    .border_1()
                    .border_color(self.c(self.col().line))
                    .shadow_lg()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div()
                            .text_size(px(20.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .child(message)
                    .when(matches!(action, Confirm::Stop(_, _)), |d| {
                        if let Confirm::Stop(_, processes) = action {
                            d.child(
                                div()
                                    .id("stop-process-list")
                                    .max_h(px(260.))
                                    .relative()
                                    .overflow_y_scroll()
                                    .track_scroll(&self.scrollers["stop-process-list"])
                                    .vertical_scrollbar(&self.scrollers["stop-process-list"])
                                    .flex()
                                    .flex_col()
                                    .gap_3()
                                    .children(processes.iter().map(|p| {
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child(format!("PID {} · {}", p.pid, p.name))
                                            .child(self.muted(format!(
                                                "Working directory: {}",
                                                if p.cwd.is_empty() {
                                                    "Unavailable"
                                                } else {
                                                    &p.cwd
                                                }
                                            )))
                                    })),
                            )
                        } else {
                            d
                        }
                    })
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap_2()
                            .pt_2()
                            .child(
                                self.button("cancel-confirm", "Cancel")
                                    .on_click(cx.listener(|s, _, _, cx| {
                                        s.confirm = None;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                self.button("accept-confirm", button)
                                    .hover(|d| {
                                        d.bg(self.c(self.col().red)).text_color(rgb(0xffffff))
                                    })
                                    .on_click(cx.listener(|s, _, w, cx| s.confirm_action(w, cx))),
                            ),
                    ),
            )
            .into_any_element()
    }
}
impl Render for AppView {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.drag_source.is_some() && !cx.has_active_drag() {
            self.clear_drag();
        }
        self.prepare_tabs(w, cx);
        self.prepare_auth(w, cx);
        let visible = if let Page::Terminal(tab) = self.page {
            self.tabs
                .iter()
                .find(|t| t.id == tab)
                .map(|t| {
                    t.zoom
                        .map(|id| vec![id])
                        .unwrap_or_else(|| t.layout.panes())
                })
                .unwrap_or_default()
        } else {
            vec![]
        };
        for (&id, p) in &self.panes {
            let error = if !p.error.is_empty() {
                p.error.clone()
            } else {
                p.session.state.lock().unwrap().error.clone()
            };
            p.notices.update(cx, |n, cx| {
                n.appearance(self.theme, error, visible.contains(&id), cx)
            });
        }
        let page = match self.page {
            Page::Hosts => self.hosts_page(cx),
            Page::Keychain => self.keychain_page(cx),
            Page::NewTab => self.new_tab_page(cx),
            Page::Terminal(id) => self.terminal_page(id, w, cx),
            Page::Monitor => self.monitor_page(w, cx),
            Page::Forwarding => self.forwarding_page(cx),
        };
        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(self.c(self.col().bg))
            .text_color(self.c(self.col().text))
            .font_family("Segoe UI")
            .text_size(px(14.))
            .track_focus(&self.focus)
            .key_context("TermViAI")
            .capture_key_down(cx.listener(|s, e: &KeyDownEvent, w, cx| {
                let k = &e.keystroke;
                if s.picker_key(&k.key, w, cx) {
                    cx.stop_propagation();
                    return;
                }
                if k.modifiers.control
                    && !k.modifiers.alt
                    && !k.modifiers.platform
                    && k.key == "tab"
                    && s.drawer.is_none()
                    && s.confirm.is_none()
                {
                    s.cycle_tab(k.modifiers.shift, w, cx);
                    cx.stop_propagation();
                    return;
                }
                if s.page == Page::Hosts
                    && !s.host_selection.is_empty()
                    && s.menu.is_none()
                    && s.confirm.is_none()
                    && s.drawer.is_none()
                    && !k.modifiers.alt
                    && !k.modifiers.platform
                    && (k.key == "enter" || k.key == "escape")
                {
                    cx.stop_propagation();
                    if k.key == "enter" {
                        s.open_selected_hosts(w, cx);
                    } else {
                        s.host_selection.clear();
                        cx.notify();
                    }
                } else if k.key == "escape"
                    && (s.menu.is_some()
                        || s.confirm.is_some()
                        || s.drawer.is_some()
                        || (s.page == Page::Monitor
                            && (s.monitor_reason.is_some()
                                || s.monitor_detail.is_some_and(|(_, n, _)| n.is_some())))
                        || s.drag_source.is_some())
                {
                    cx.stop_propagation();
                    if s.menu.is_none()
                        && s.confirm.is_none()
                        && s.drawer.is_none()
                        && s.page == Page::Monitor
                    {
                        s.close_monitor_overlay(cx);
                    }
                    s.menu = None;
                    s.confirm = None;
                    if s.drawer.is_some() {
                        s.close_form(w, cx);
                    }
                    s.clear_drag();
                    cx.notify();
                } else if k.modifiers.control && k.key == "t" {
                    s.open_new_tab(w, cx);
                    cx.stop_propagation();
                } else if k.modifiers.control && k.key == "s" && s.drawer.is_none() {
                    if let Page::Terminal(id) = s.page {
                        let r = s.save_workspace(id);
                        s.notify_error(r, cx);
                        cx.stop_propagation();
                    }
                }
            }))
            .child(self.top_bar(w, cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .flex()
                    .child(self.sidebar_view(cx))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .relative()
                            .flex()
                            .flex_col()
                            .child(page)
                            .child(self.drawer_view(cx)),
                    ),
            )
            .when(!self.load_error.is_empty(), |d| {
                d.child(self.notice("Saved data could not be loaded", self.load_error.clone()))
            })
            .when(!self.error.is_empty(), |d| {
                d.child(
                    self.notice("Action could not be completed", self.error.clone())
                        .child(
                            self.close("dismiss-error")
                                .on_click(cx.listener(|s, _, _, cx| {
                                    s.error.clear();
                                    cx.notify();
                                })),
                        ),
                )
            })
            .child(self.tab_picker_view(w, cx))
            .child(self.menu_view(w, cx))
            .child(self.confirm_view(cx))
    }
}
