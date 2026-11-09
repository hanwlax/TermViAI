//! Opt-in native UI inspection, only permitted in an explicitly marked isolated data directory.
use super::*;
impl AppView {
    pub fn validation_scene(&mut self, scene: &str, w: &mut Window, cx: &mut Context<Self>) {
        let dir = HostStore::path().parent().unwrap().to_path_buf();
        assert!(dir.join(".gpui-validation").is_file(), "Validation requires a marked isolated data directory");
        self.store = HostStore::default();
        self.store.groups.push(HostGroup { id:"development".into(), name:"Development".into(), parent_id:String::new() });
        self.store.groups.push(HostGroup { id:"compute".into(), name:"Compute".into(), parent_id:"development".into() });
        for (i, label) in ["Build server", "Compute 01", "Compute 02", "Staging"].iter().enumerate() {
            self.store.hosts.push(Host {label:label.to_string(), address:format!("192.0.2.{}", i+1), port:22, username:"developer".into(), group_id:if i==3 {String::new()} else {"compute".into()}, ..Host::default()});
        }
        self.store.keys.push(KeyFile {label:"Development key".into(),path:"C:\\Keys\\development_ed25519".into(),..KeyFile::default()});
        self.store.identities.push(Identity{id:"validation-no-secret".into(),label:"Developer".into(),username:"developer".into()});
        match scene {
            "chrome" => {
                self.connect(None,w,cx); self.tabs[0].name="209".into();
                self.connect(None,w,cx); self.tabs[1].name="212".into();
                let sessions:Vec<_>=self.panes.values().map(|p|p.session.clone()).collect();
                cx.spawn_in(w,async move |_,cx| {
                    for _ in 0..100 {
                        cx.background_executor().timer(Duration::from_millis(50)).await;
                        if sessions.iter().all(|s|s.online()){break;}
                    }
                    for s in &sessions { let _=s.send(b"exit\r"); }
                }).detach();
            }
            "hosts" => self.page=Page::Hosts,
            "group" => {self.page=Page::Hosts; self.group="development".into();}
            "host-edit" => self.open_form(FormKind::Host(Some(self.store.hosts[0].clone())), w,cx),
            "keychain" => self.page=Page::Keychain,
            "key-edit" => {self.page=Page::Keychain;self.open_form(FormKind::Key(Some(self.store.keys[0].clone())),w,cx);}
            "identity-edit" => {self.page=Page::Keychain;self.open_form(FormKind::Identity(Some(self.store.identities[0].clone())),w,cx);}
            "settings" => self.open_form(FormKind::Settings,w,cx),
            "monitor" => self.page=Page::Monitor,
            "monitor-add" => {self.page=Page::Monitor;self.open_form(FormKind::Monitor,w,cx);}
            "forwarding" => self.page=Page::Forwarding,
            "forward-add" => {self.page=Page::Forwarding;self.open_form(FormKind::Forward(None),w,cx);}
            "new-tab" => {self.page=Page::NewTab; for h in &self.store.hosts {self.workspaces.record_recent(h).unwrap();}
                let h=self.store.hosts[0].clone();self.workspaces.save_group(None,"Development",SavedLayout::Split{axis:SplitAxis::Horizontal,first_percent:50,first:Box::new(SavedLayout::Pane{host:h}),second:Box::new(SavedLayout::Pane{host:self.store.hosts[1].clone()})}).unwrap();}
            "terminal" | "broadcast" => {
                self.connect(None,w,cx);
                let tab=self.tabs[0].id;let first=self.tabs[0].layout.panes()[0];
                let second=self.spawn_pane(None,w,cx);let third=self.spawn_pane(None,w,cx);let fourth=self.spawn_pane(None,w,cx);
                self.tabs[0].layout.dock(second,first,Side::Right);
                self.tabs[0].layout.dock(third,first,Side::Bottom);
                self.tabs[0].layout.dock(fourth,second,Side::Bottom);
                let ids=self.tabs[0].layout.panes();self.tabs[0].broadcast.sync_panes(ids);
                self.tabs[0].name="Terminal workspace".into();
                self.broadcast_all(tab,cx);
                self.tabs[0].tray=scene=="broadcast";
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
