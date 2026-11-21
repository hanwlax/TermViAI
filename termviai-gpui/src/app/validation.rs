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
    use super::{AppView, FormKind};
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

    #[test]
    fn settings_scroll_range_tracks_visible_controls() {
        // Keep the native event loop and application data out of the test runner's process.
        const CHILD: &str = "TERMVIAI_SETTINGS_LAYOUT_TEST";
        if std::env::var_os(CHILD).is_none() {
            let dir = tempfile::tempdir().unwrap();
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "app::validation::tests::settings_scroll_range_tracks_visible_controls",
                    "--nocapture",
                ])
                .env(CHILD, "1")
                .env("TERMVIAI_DATA_DIR", dir.path())
                .status()
                .unwrap();
            assert!(status.success());
            return;
        }

        Application::new().with_assets(crate::Assets).run(|cx| {
            gpui_component::init(cx);
            for (width, height, overflows) in [
                (1280., 820., false),
                (1280., 600., true),
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
                let viewport = scroll.bounds().size;
                let range = scroll.max_offset().height;
                eprintln!(
                    "Settings {width}x{height}: viewport {viewport:?}, vertical range {range:?}"
                );
                assert!((height - f32::from(viewport.height) - 46.).abs() <= 2.);
                assert_eq!(range > px(0.), overflows, "{width}x{height}");
            }
            cx.quit();
        });
    }
}
