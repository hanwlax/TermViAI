//! Opt-in native UI inspection, only permitted in an explicitly marked isolated data directory.
use super::*;
impl AppView {
    pub fn validation_scene(&mut self, scene: &str, w: &mut Window, cx: &mut Context<Self>) {
        let dir = HostStore::path().parent().unwrap().to_path_buf();
        assert!(
            dir.join(".gpui-validation").is_file(),
            "Validation requires a marked isolated data directory"
        );
        self.store = HostStore::default();
        self.store.groups.push(HostGroup {
            id: "development".into(),
            name: "Development".into(),
            parent_id: String::new(),
        });
        self.store.groups.push(HostGroup {
            id: "compute".into(),
            name: "Compute".into(),
            parent_id: "development".into(),
        });
        for (i, label) in ["Build server", "Compute 01", "Compute 02", "Staging"]
            .iter()
            .enumerate()
        {
            self.store.hosts.push(Host {
                label: label.to_string(),
                address: format!("192.0.2.{}", i + 1),
                port: 22,
                username: "developer".into(),
                group_id: if i == 3 {
                    String::new()
                } else {
                    "compute".into()
                },
                ..Host::default()
            });
        }
        self.store.keys.push(KeyFile {
            label: "Development key".into(),
            path: "C:\\Keys\\development_ed25519".into(),
            ..KeyFile::default()
        });
        self.store.identities.push(Identity {
            id: "validation-no-secret".into(),
            label: "Developer".into(),
            username: "developer".into(),
        });
        match scene {
            "chrome" => {
                self.connect(None, w, cx);
                self.tabs[0].name = "209".into();
                self.connect(None, w, cx);
                self.tabs[1].name = "212".into();
                let sessions: Vec<_> = self.panes.values().map(|p| p.session.clone()).collect();
                cx.spawn_in(w, async move |_, cx| {
                    for _ in 0..100 {
                        cx.background_executor()
                            .timer(Duration::from_millis(50))
                            .await;
                        if sessions.iter().all(|s| s.online()) {
                            break;
                        }
                    }
                    for s in &sessions {
                        let _ = s.send(b"exit\r");
                    }
                })
                .detach();
            }
            "hosts" => self.page = Page::Hosts,
            "group" => {
                self.page = Page::Hosts;
                self.group = "development".into();
            }
            "host-edit" => self.open_form(FormKind::Host(Some(self.store.hosts[0].clone())), w, cx),
            "keychain" => self.page = Page::Keychain,
            "key-edit" => {
                self.page = Page::Keychain;
                self.open_form(FormKind::Key(Some(self.store.keys[0].clone())), w, cx);
            }
            "identity-edit" => {
                self.page = Page::Keychain;
                self.open_form(
                    FormKind::Identity(Some(self.store.identities[0].clone())),
                    w,
                    cx,
                );
            }
            "settings" => self.open_form(FormKind::Settings, w, cx),
            "monitor" | "monitor-add" if !self.settings.npu_monitor_enabled => {
                self.page = Page::Hosts
            }
            "monitor" => self.page = Page::Monitor,
            "monitor-add" => {
                self.page = Page::Monitor;
                self.open_form(FormKind::Monitor, w, cx);
            }
            "forwarding" => self.page = Page::Forwarding,
            "forward-add" => {
                self.page = Page::Forwarding;
                self.open_form(FormKind::Forward(None), w, cx);
            }
            "new-tab" => {
                self.page = Page::NewTab;
                for h in &self.store.hosts {
                    self.workspaces.record_recent(h).unwrap();
                }
                let h = self.store.hosts[0].clone();
                self.workspaces
                    .save_group(
                        None,
                        "Development",
                        SavedLayout::Split {
                            axis: SplitAxis::Horizontal,
                            first_percent: 50,
                            first: Box::new(SavedLayout::Pane { host: h }),
                            second: Box::new(SavedLayout::Pane {
                                host: self.store.hosts[1].clone(),
                            }),
                        },
                    )
                    .unwrap();
            }
            "terminal" | "broadcast" => {
                self.connect(None, w, cx);
                let tab = self.tabs[0].id;
                let first = self.tabs[0].layout.panes()[0];
                let second = self.spawn_pane(None, w, cx);
                let third = self.spawn_pane(None, w, cx);
                let fourth = self.spawn_pane(None, w, cx);
                self.tabs[0].layout.dock(second, first, Side::Right);
                self.tabs[0].layout.dock(third, first, Side::Bottom);
                self.tabs[0].layout.dock(fourth, second, Side::Bottom);
                let ids = self.tabs[0].layout.panes();
                self.tabs[0].broadcast.sync_panes(ids);
                self.tabs[0].name = "Terminal workspace".into();
                self.broadcast_all(tab, cx);
                self.tabs[0].tray = scene == "broadcast";
                cx.spawn(async move |this,cx| {
                    for _ in 0..100 {
                        cx.background_executor().timer(Duration::from_millis(100)).await;
                        if this.update(cx,|s,cx| {
                            if !s.panes.values().all(|p|p.session.online()){return false;}
                            s.route_input(first,session::Input::Text("echo Native GPUI - TermViAI terminal core\recho 中文终端 - A中文B\r".into()),cx);
                            true
                        }).unwrap_or(true){break;}
                    }
                }).detach();
            }
            _ => panic!("Unknown validation page"),
        }
        cx.notify();
    }
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::{AppView, FormKind, MenuKind, Page};
    use gpui::{
        AppContext, Application, Context, Entity, IntoElement, ParentElement, Pixels, Render,
        ScrollHandle, Size, Styled, Window, WindowOptions, div, px, size,
    };

    // A hidden Win32 window defers its placement until shown. Size the app
    // surface explicitly so the native layout pass uses the requested viewport.
    struct Surface(Size<Pixels>, Entity<AppView>);
    impl Render for Surface {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().w(self.0.width).h(self.0.height).child(self.1.clone())
        }
    }

    fn native_test(name: &str, run: impl FnOnce(&mut gpui::App) + 'static) {
        // Isolate the native event loop and application data from the test runner.
        const CHILD: &str = "TERMVIAI_LAYOUT_TEST";
        if std::env::var_os(CHILD).is_none() {
            let dir = tempfile::tempdir().unwrap();
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .arg("--exact")
                .arg(format!("app::validation::tests::{name}"))
                .arg("--nocapture")
                .env(CHILD, "1")
                .env("TERMVIAI_DATA_DIR", dir.path())
                .status()
                .unwrap();
            assert!(status.success());
            return;
        }
        Application::new()
            .with_assets(crate::Assets)
            .run(move |cx| {
                gpui_component::init(cx);
                run(cx);
            });
    }

    #[test]
    fn settings_scroll_range_tracks_visible_controls() {
        native_test("settings_scroll_range_tracks_visible_controls", |cx| {
            for (width, height, overflows) in [
                (1280., 820., false),
                (780., 520., true),
                (920., 960., false),
            ] {
                let mut scroll = ScrollHandle::new();
                cx.open_window(
                    WindowOptions {
                        show: false,
                        focus: false,
                        ..Default::default()
                    },
                    |w, cx| {
                        let view = cx.new(|cx| AppView::new(w, cx));
                        view.update(cx, |s, cx| {
                            s.initialize(false, w, cx);
                            s.open_form(FormKind::Settings, w, cx);
                            scroll = s.scrollers["drawer-scroll"].clone();
                        });
                        let surface = cx.new(|_| Surface(size(px(width), px(height)), view));
                        cx.new(|cx| gpui_component::Root::new(surface, w, cx))
                    },
                )
                .unwrap();
                let bounds = scroll.bounds();
                let viewport = bounds.size;
                let range = scroll.max_offset().height;
                eprintln!(
                    "Settings {width}x{height}: viewport {viewport:?}, vertical range {range:?}"
                );
                // Both the window chrome and the Settings toolbar sit above the scroller.
                assert!(bounds.origin.y >= px(46. + 38.));
                assert!((height - f32::from(bounds.bottom())).abs() <= 2.);
                assert_eq!(range > px(0.), overflows, "{width}x{height}");
            }
            cx.quit();
        });
    }

    #[test]
    fn tool_pages_keep_add_on_left_and_reflow_cards_on_resize() {
        use gpui::{Focusable, point};
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        use windows::Win32::{
            Foundation::{HWND, LPARAM, WPARAM},
            UI::WindowsAndMessaging::{
                SendMessageW, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_SIZE,
            },
        };
        native_test(
            "tool_pages_keep_add_on_left_and_reflow_cards_on_resize",
            |cx| {
                let mut app = None;
                let window = cx
                    .open_window(
                        WindowOptions {
                            show: false,
                            focus: false,
                            ..Default::default()
                        },
                        |w, cx| {
                            let view = cx.new(|cx| AppView::new(w, cx));
                            view.update(cx, |s, cx| s.initialize(false, w, cx));
                            app = Some(view.clone());
                            cx.new(|cx| gpui_component::Root::new(view, w, cx))
                        },
                    )
                    .unwrap();
                let app = app.unwrap();
                let (handle, scale) = cx
                    .update_window(window.into(), |_, w, cx| {
                        let view = app.read(cx);
                        assert!(view.page == Page::NewTab && view.new_tab_open());
                        assert_eq!(view.tab_groups.len(), 1);
                        assert!(view.tabs.is_empty() && view.panes.is_empty());
                        assert!(view.new_tab_search.focus_handle(cx).is_focused(w));
                        let RawWindowHandle::Win32(handle) =
                            HasWindowHandle::window_handle(w).unwrap().as_raw()
                        else {
                            unreachable!()
                        };
                        (handle.hwnd, w.scale_factor())
                    })
                    .unwrap();
                let coords = move |x: f32, y: f32| {
                    LPARAM((((y * scale).round() as isize) << 16) | (x * scale).round() as isize)
                };
                cx.spawn(async move |cx| {
                let hwnd = HWND(handle.get() as *mut _);
                // A hidden window needs a client viewport for GPUI's hit-test clip.
                // Send messages only to this HWND, outside GPUI's window update.
                // Use the startup page itself and hit the menu on its last first-row host.
                for (width, column) in [(780., 1), (1280., 3), (2076., 6)] {
                    unsafe { SendMessageW(hwnd, WM_SIZE, None, Some(coords(width, 820.))); }
                    let pos = cx.update_window(window.into(), |_, w, cx| {
                        app.update(cx, |s, cx| {
                            s.menu = None;
                            s.store.hosts = (1..=6).map(|i| super::Host {
                                label: i.to_string(), ..Default::default()
                            }).collect();
                            cx.notify();
                        });
                        w.refresh();
                        w.draw(cx).clear();
                        let pane = app.read(cx).scrollers["new-tab-page"].bounds();
                        let gutter = AppView::tool_gutter(w);
                        coords(f32::from(pane.right()) - gutter - 27.,
                               f32::from(pane.top()) + gutter + 72.)
                    }).unwrap();
                    unsafe {
                        SendMessageW(hwnd, WM_LBUTTONDOWN, Some(WPARAM(1)), Some(pos));
                        SendMessageW(hwnd, WM_LBUTTONUP, None, Some(pos));
                    }
                    let opened = cx.update(|cx| matches!(&app.read(cx).menu,
                        Some((MenuKind::Host(host), _)) if host.label == column.to_string())).unwrap();
                    if !opened {
                        eprintln!("New Tab {width} px: expected host menu {column} at the right edge");
                        std::process::exit(1);
                    }
                }
                unsafe { SendMessageW(hwnd, WM_SIZE, None, Some(coords(1280., 820.))); }
                let mut target = None;
                for (page, scroll_id) in [
                    (Page::Keychain, "keychain-page"), (Page::Hosts, "hosts-page"),
                    (Page::Forwarding, "forward-page"), (Page::Monitor, "monitor-page")
                ] {
                    let bounds = cx.update_window(window.into(), |_, w, cx| {
                        app.update(cx, |s, cx| {
                            s.menu = None;
                            s.drawer = None;
                            s.page = page;
                            cx.notify();
                        });
                        w.refresh();
                        w.draw(cx).clear();
                        app.read(cx).scrollers[scroll_id].bounds()
                    }).unwrap();
                    let position = *target.get_or_insert(point(bounds.left() + px(53.), bounds.top() - px(30.)));
                    let pos = coords(f32::from(position.x), f32::from(position.y));
                    unsafe {
                        SendMessageW(hwnd, WM_LBUTTONDOWN, Some(WPARAM(1)), Some(pos));
                        SendMessageW(hwnd, WM_LBUTTONUP, None, Some(pos));
                    }
                    let opened = cx
                        .update(|cx| {
                            let s = app.read(cx);
                            match page {
                                Page::Keychain => {
                                    matches!(s.menu, Some((MenuKind::AddCredential, _)))
                                }
                                Page::Hosts => matches!(s.menu, Some((MenuKind::AddHost(_), _))),
                                Page::Forwarding => matches!(
                                    s.drawer.as_ref().map(|f| &f.kind),
                                    Some(FormKind::Forward(None))
                                ),
                                Page::Monitor => {
                                    matches!(
                                        s.drawer.as_ref().map(|f| &f.kind),
                                        Some(FormKind::Monitor)
                                    )
                                }
                                _ => unreachable!(),
                            }
                        })
                        .unwrap();
                    if !opened {
                        eprintln!(
                            "{scroll_id}: click at {position:?} did not open the expected action (content {bounds:?})"
                        );
                        std::process::exit(1);
                    }
                }
                // Hit service menus in the responsive grid, including a filtered result set.
                for (width, active_only, expected_id) in [
                    (780., false, "1"), (1332., false, "3"),
                    (1332., true, "6"), (2076., false, "5"),
                ] {
                    unsafe { SendMessageW(hwnd, WM_SIZE, None, Some(coords(width, 820.))); }
                    let pos = cx.update_window(window.into(), |_, w, cx| {
                        app.update(cx, |s, cx| {
                            use super::forwarding::{Rule, State, View};
                            s.drawer = None;
                            s.menu = None;
                            s.page = Page::Forwarding;
                            s.forwarding_error.clear();
                            s.forward_filter = if active_only { super::ForwardFilter::Active } else { super::ForwardFilter::All };
                            s.forwarding = (1..=6).map(|i| View {
                                rule: Rule { id: i.to_string(), name: format!("Proxy {i}"), ..Rule::new(Default::default()) },
                                state: if i % 2 == 0 { State::Running } else { State::Stopped },
                                error: String::new(), last_failure: String::new(), attempt: 0,
                                last_attempt: None, retry_at: None, service: None, inspection: None, busy: false,
                            }).collect();
                            cx.notify();
                        });
                        w.refresh();
                        w.draw(cx).clear();
                        let pane = app.read(cx).scrollers["forward-page"].bounds();
                        let gutter = AppView::tool_gutter(w);
                        coords(f32::from(pane.right()) - gutter - 31., f32::from(pane.top()) + gutter + 87.)
                    }).unwrap();
                    unsafe {
                        SendMessageW(hwnd, WM_LBUTTONDOWN, Some(WPARAM(1)), Some(pos));
                        SendMessageW(hwnd, WM_LBUTTONUP, None, Some(pos));
                    }
                    let matched = cx.update(|cx| matches!(&app.read(cx).menu,
                        Some((MenuKind::Forward(id), _)) if id == expected_id)).unwrap();
                    if !matched {
                        eprintln!("Forwarding {width} px (active only: {active_only}): expected menu for service {expected_id}");
                        std::process::exit(1);
                    }
                }
                // Hit the far-right card on the first row after each native resize.
                // This catches both a capped container and an incorrect column count.
                for (width, expected_column, sidebar) in [
                    (2076., 6, true), (780., 1, true), (1280., 3, true),
                    (1877., 5, true), (1320., 4, false),
                ] {
                    unsafe { SendMessageW(hwnd, WM_SIZE, None, Some(coords(width, 820.))); }
                    let pos = cx.update_window(window.into(), |_, w, cx| {
                        app.update(cx, |s, cx| {
                            s.menu = None;
                            s.drawer = None;
                            s.page = Page::Keychain;
                            s.sidebar = sidebar;
                            s.store.keys = (1..=6).map(|i| super::KeyFile {
                                label: i.to_string(), ..Default::default()
                            }).collect();
                            cx.notify();
                        });
                        w.refresh();
                        w.draw(cx).clear();
                        let pane = app.read(cx).scrollers["keychain-page"].bounds();
                        coords(f32::from(pane.right()) - AppView::tool_gutter(w) - 8.,
                               f32::from(pane.top()) + AppView::tool_gutter(w) + 65.)
                    }).unwrap();
                    unsafe {
                        SendMessageW(hwnd, WM_LBUTTONDOWN, Some(WPARAM(1)), Some(pos));
                        SendMessageW(hwnd, WM_LBUTTONUP, None, Some(pos));
                    }
                    let selected = cx.update(|cx| {
                        match app.read(cx).drawer.as_ref().map(|f| &f.kind) {
                            Some(FormKind::Key(Some(key))) => Some(key.label.clone()),
                            _ => None,
                        }
                    }).unwrap();
                    eprintln!("Keychain {width} px, sidebar expanded {sidebar}: rightmost key {selected:?}");
                    if selected != Some(expected_column.to_string()) {
                        eprintln!("At {width} px (sidebar expanded: {sidebar}), expected key {expected_column} at the right edge, opened {selected:?}");
                        std::process::exit(1);
                    }
                }
                cx.update(|cx| app.update(cx, |s, cx| {
                    s.drawer = None;
                    s.page = Page::Monitor;
                    s.sidebar = true;
                    s.monitor.add(super::Host::default());
                    let entry = &mut s.monitor.entries[0];
                    entry.paused = true;
                    entry.snapshot = Some(super::monitor::Snapshot {
                        dies: (0..16).map(|id| super::monitor::Die {
                            npu: id / 2, die: id % 2,
                            used_mb: Some(2048.), total_mb: Some(4096.), core: Some(0.),
                        }).collect(),
                        processes: vec![], processes_available: true, stop_reason: String::new(),
                    });
                    cx.notify();
                })).unwrap();
                // The last NPU on row one changes with the width left by the inspector.
                for (width, inspecting, last_npu) in [
                    (2080., false, 3), (2080., true, 2), (2560., false, 4),
                    (1140., true, 0), (1280., false, 1), (780., false, 0),
                ] {
                    unsafe { SendMessageW(hwnd, WM_SIZE, None, Some(coords(width, 820.))); }
                    let pos = cx.update_window(window.into(), |_, w, cx| {
                        app.update(cx, |s, cx| {
                            s.monitor_detail = Some((0, inspecting.then_some(7), None));
                            cx.notify();
                        });
                        w.refresh();
                        w.draw(cx).clear();
                        if inspecting {
                            let edge = app.read(cx).scrollers["monitor-process"].bounds().right();
                            let gap = f32::from(w.viewport_size().width - edge);
                            if gap.abs() > 1. {
                                eprintln!("Monitor {width} px: docked process pane ends {gap} px from the window edge");
                                std::process::exit(1);
                            }
                        }
                        let pane = app.read(cx).scrollers["monitor-detail"].bounds();
                        let gutter = AppView::tool_gutter(w);
                        coords(f32::from(pane.right()) - gutter - 8.,
                               f32::from(pane.top()) + gutter + 65.)
                    }).unwrap();
                    unsafe {
                        SendMessageW(hwnd, WM_LBUTTONDOWN, Some(WPARAM(1)), Some(pos));
                        SendMessageW(hwnd, WM_LBUTTONUP, None, Some(pos));
                    }
                    let selected = cx.update(|cx| app.read(cx).monitor_detail).unwrap();
                    if selected != Some((0, Some(last_npu), None)) {
                        eprintln!("Monitor {width} px, inspector {inspecting}: expected NPU {last_npu} at the right edge, got {selected:?}");
                        std::process::exit(1);
                    }
                }
                let pin_path = super::HostStore::known_hosts_path();
                std::fs::create_dir_all(pin_path.parent().unwrap()).unwrap();
                std::fs::write(&pin_path, r#"{"example.test:22":"SHA256:test"}"#).unwrap();
                let pos = cx.update_window(window.into(), |_, w, cx| {
                    app.update(cx, |s, cx| s.activate_tab(Page::KnownHosts, w, cx));
                    w.refresh();
                    w.draw(cx).clear();
                    let bounds = app.read(cx).scrollers["known-hosts-page"].bounds();
                    coords(f32::from(bounds.right()) - AppView::tool_gutter(w) - 30.,
                           f32::from(bounds.top()) + AppView::tool_gutter(w) + 26.)
                }).unwrap();
                unsafe {
                    SendMessageW(hwnd, WM_MOUSEMOVE, None, Some(pos));
                    SendMessageW(hwnd, WM_LBUTTONDOWN, Some(WPARAM(1)), Some(pos));
                    SendMessageW(hwnd, WM_LBUTTONUP, None, Some(pos));
                }
                if !wezterm_ssh::known_hosts::load(&pin_path).unwrap().is_empty() {
                    eprintln!("Clicking the hovered known host did not remove its pin");
                    std::process::exit(1);
                }

                unsafe { SendMessageW(hwnd, WM_SIZE, None, Some(coords(1280., 820.))); }
                let (left, right, left_press, track, track_end, close) =
                    cx.update_window(window.into(), |_, w, cx| {
                        let (left, right) = app.update(cx, |s, cx| {
                            s.connect(None, w, cx);
                            let left = s.tabs[0].layout.panes()[0];
                            let right = s.spawn_pane(None, w, cx);
                            let tab = &mut s.tabs[0];
                            tab.layout.dock(right, left, super::Side::Right);
                            tab.broadcast.sync_panes([left, right].to_vec());
                            let tab_id = tab.id;
                            s.broadcast_all(tab_id, cx);
                            for id in [left, right] {
                                let mut state = s.panes[&id].session.state.lock().unwrap();
                                state.term.advance_bytes("scrollback\r\n".repeat(240).as_bytes());
                                state.bytes += 240;
                            }
                            cx.notify();
                            (left, right)
                        });
                        w.refresh();
                        w.draw(cx).clear();
                        let s = app.read(cx);
                        let a = s.panes[&left].view.read(cx).pointer_test_state().0;
                        let b = s.panes[&right].view.read(cx).pointer_test_state().0;
                        (left, right, coords(f32::from(a.left())+30., f32::from(a.top())+80.),
                         coords(f32::from(b.right())-3., f32::from(b.bottom())-8.),
                         coords(f32::from(b.right())-3., f32::from(b.top())+8.),
                         coords(f32::from(b.right())-15., f32::from(b.top())-22.))
                    }).unwrap();
                // Simulate a lost release on the left, then a fresh scrollbar gesture on the right.
                unsafe {
                    SendMessageW(hwnd, WM_LBUTTONDOWN, Some(WPARAM(1)), Some(left_press));
                    SendMessageW(hwnd, WM_LBUTTONDOWN, Some(WPARAM(1)), Some(track));
                    SendMessageW(hwnd, WM_MOUSEMOVE, Some(WPARAM(1)), Some(track_end));
                    SendMessageW(hwnd, WM_LBUTTONUP, None, Some(track_end));
                }
                let scrolled = cx.update(|cx| {
                    let s = app.read(cx);
                    let (_, left_offset, left_drag) = s.panes[&left].view.read(cx).pointer_test_state();
                    let (_, right_offset, right_drag) = s.panes[&right].view.read(cx).pointer_test_state();
                    left_offset == 0. && right_offset > 0. && !left_drag && !right_drag
                }).unwrap();
                unsafe {
                    SendMessageW(hwnd, WM_LBUTTONDOWN, Some(WPARAM(1)), Some(left_press));
                    SendMessageW(hwnd, WM_LBUTTONDOWN, Some(WPARAM(1)), Some(close));
                    SendMessageW(hwnd, WM_LBUTTONUP, None, Some(close));
                }
                let closed = cx.update(|cx| matches!(app.read(cx).confirm,
                    Some(super::Confirm::Pane(id)) if id == right)).unwrap();
                if !(scrolled && closed) {
                    eprintln!("Right broadcast pane: scrollbar drag={scrolled}, close={closed}");
                    std::process::exit(1);
                }
                cx.update(|cx| cx.quit()).unwrap();
            })
            .detach();
            },
        );
    }
}
