//! Native view adapter around the *existing* TermViAI terminal and SSH engines.
//! UI threads never perform socket reads, SSH authentication or PTY writes.
use crate::{
    authentication::{self, Candidate, Control, Phase},
    hosts::{Authentication, Host, HostStore},
    terminal_notifications::{Filter as BroadcastFilter, Inbox},
    theme::Theme,
};
use anyhow::{Context, ensure};
use portable_pty::{Child, MasterPty, PtySize};
use std::{
    io::{Read, Write},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, SyncSender},
    },
    thread,
    time::{Duration, Instant},
};
use wezterm_term::{KeyCode, KeyModifiers, Terminal, TerminalConfiguration, TerminalSize};

#[derive(Debug)]
struct TermConfig {
    theme: Theme,
}
impl TerminalConfiguration for TermConfig {
    fn color_palette(&self) -> wezterm_term::color::ColorPalette {
        self.theme.terminal_palette()
    }
    fn scrollback_size(&self) -> usize {
        10000
    }
    fn reflow_cursor_line(&self) -> bool {
        false
    }
}
#[derive(Clone)]
pub enum Input {
    Key(KeyCode, KeyModifiers),
    KeyUp(KeyCode, KeyModifiers),
    Text(String),
    Paste(String),
}
enum Request {
    Input(Vec<u8>),
}
struct Writer(SyncSender<Request>);
impl Write for Writer {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.try_send(Request::Input(b.to_vec())).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::WouldBlock,
                "Terminal input queue is full or closed",
            )
        })?;
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
struct ChildCleanup(Box<dyn portable_pty::ChildKiller + Send + Sync>);
impl Drop for ChildCleanup {
    fn drop(&mut self) {
        let _ = self.0.kill();
    }
}
struct Alerts(Arc<AtomicU64>);
impl wezterm_term::AlertHandler for Alerts {
    fn alert(&mut self, alert: wezterm_term::Alert) {
        if matches!(alert, wezterm_term::Alert::Bell) {
            self.0.fetch_add(1, Ordering::Release);
        }
    }
}
pub struct State {
    pub term: Terminal,
    pub connected: bool,
    pub ended: bool,
    pub error: String,
    pub bytes: u64,
    pub notifications: Inbox,
    broadcast_filter: BroadcastFilter,
}
impl State {
    fn advance_output(&mut self, bytes: &[u8], now: Instant, remote: bool) -> bool {
        self.bytes += bytes.len() as u64;
        if !remote {
            self.term.advance_bytes(bytes);
            return !bytes.is_empty();
        }
        let output = self.broadcast_filter.feed(bytes, now);
        let changed = !output.terminal.is_empty() || !output.notices.is_empty();
        self.term.advance_bytes(&output.terminal);
        for notice in output.notices {
            self.notifications.push(notice);
        }
        changed
    }
    fn finish_output(&mut self) {
        self.term.advance_bytes(&self.broadcast_filter.finish());
    }
}
pub struct Session {
    pub id: usize,
    pub host: Option<Host>,
    pub state: Arc<Mutex<State>>,
    pub auth: Arc<Control>,
    pub revision: Arc<AtomicU64>,
    pub follow: AtomicU64,
    pub bells: Arc<AtomicU64>,
    pending_size: Arc<Mutex<Option<PtySize>>>,
    tx: SyncSender<Request>,
    cancel: Arc<AtomicBool>,
}
impl Session {
    pub fn new(
        id: usize,
        host: Option<Host>,
        store: &HostStore,
        theme: Theme,
        keepalive: u64,
    ) -> Arc<Self> {
        let plan = host
            .as_ref()
            .map(|h| authentication_plan(h, store, keepalive));
        Self::start(id, host, theme, plan)
    }
    fn start(
        id: usize,
        host: Option<Host>,
        theme: Theme,
        plan: Option<Vec<Candidate>>,
    ) -> Arc<Self> {
        let (tx, rx) = mpsc::sync_channel(64);
        let bells = Arc::new(AtomicU64::new(0));
        let pending_size = Arc::new(Mutex::new(None));
        let mut term = Terminal::new(
            TerminalSize::default(),
            Arc::new(TermConfig { theme }),
            "TermViAI",
            "GPUI-trial",
            Box::new(Writer(tx.clone())),
        );
        term.set_notification_handler(Box::new(Alerts(bells.clone())));
        term.set_input_capture_id(id);
        if host.is_none() {
            term.enable_conpty_quirks();
        }
        let s = Arc::new(Self {
            id,
            host: host.clone(),
            state: Arc::new(Mutex::new(State {
                term,
                connected: false,
                ended: false,
                error: String::new(),
                bytes: 0,
                notifications: Inbox::default(),
                broadcast_filter: BroadcastFilter::default(),
            })),
            auth: Control::new(),
            revision: Arc::new(AtomicU64::new(0)),
            follow: AtomicU64::new(0),
            tx,
            bells,
            pending_size: pending_size.clone(),
            cancel: Arc::new(AtomicBool::new(false)),
        });
        let weak = Arc::downgrade(&s);
        let control = s.auth.clone();
        let cancelled = s.cancel.clone();
        thread::spawn(move || {
            let result = (|| -> anyhow::Result<()> {
                let (pty, mut child): (Box<dyn MasterPty + Send>, Box<dyn Child + Send + Sync>) =
                    if let Some(plan) = plan {
                        let session = authentication::connect(&control, plan)?;
                        ensure!(!cancelled.load(Ordering::Acquire), "Connection closed");
                        let (pty, child) = smol::block_on(session.request_pty(
                            "xterm-256color",
                            PtySize::default(),
                            None,
                            None,
                        ))?;
                        (Box::new(pty), Box::new(child))
                    } else {
                        let pair = portable_pty::native_pty_system().openpty(PtySize::default())?;
                        let mut cmd = portable_pty::CommandBuilder::new(if cfg!(windows) {
                            "cmd.exe"
                        } else {
                            "/bin/sh"
                        });
                        if cfg!(windows) {
                            cmd.arg("/Q");
                        }
                        let child = pair.slave.spawn_command(cmd)?;
                        drop(pair.slave);
                        (pair.master, child)
                    };
                let _cleanup = ChildCleanup(child.clone_killer());
                let mut reader = pty.try_clone_reader()?;
                let mut writer = pty.take_writer()?;
                let size = if let Some(s) = weak.upgrade() {
                    let mut state = s.state.lock().unwrap();
                    state.connected = true;
                    state.term.get_size()
                } else {
                    return Ok(());
                };
                pty.resize(pty_size(size))?;
                control.update(Phase::Connected, String::new());
                let read_weak = weak.clone();
                let read_cancel = cancelled.clone();
                thread::spawn(move || {
                    let mut buf = [0; 16384];
                    loop {
                        match reader.read(&mut buf) {
                            Ok(0) => break,
                            Ok(n) => {
                                let Some(s) = read_weak.upgrade() else { break };
                                if read_cancel.load(Ordering::Acquire) {
                                    break;
                                }
                                let mut state = s.state.lock().unwrap();
                                state.advance_output(&buf[..n], Instant::now(), s.host.is_some());
                                s.revision.fetch_add(1, Ordering::Release);
                            }
                            Err(e) => {
                                if let Some(s) = read_weak.upgrade() {
                                    s.state.lock().unwrap().error = e.to_string();
                                }
                                break;
                            }
                        }
                    }
                    if let Some(s) = read_weak.upgrade() {
                        let mut st = s.state.lock().unwrap();
                        st.finish_output();
                        st.connected = false;
                        st.ended = true;
                        s.revision.fetch_add(1, Ordering::Release);
                    }
                });
                loop {
                    if cancelled.load(Ordering::Acquire) {
                        let _ = child.kill();
                        break;
                    }
                    match rx.recv_timeout(Duration::from_millis(30)) {
                        Ok(Request::Input(b)) => {
                            writer.write_all(&b)?;
                            writer.flush()?;
                        }
                        Err(mpsc::RecvTimeoutError::Disconnected) => {
                            let _ = child.kill();
                            break;
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                    }
                    if let Some(s) = weak.upgrade() {
                        if s.state.lock().unwrap().advance_output(
                            &[],
                            Instant::now(),
                            s.host.is_some(),
                        ) {
                            s.revision.fetch_add(1, Ordering::Release);
                        }
                    }
                    if let Some(size) = pending_size.lock().unwrap().take() {
                        pty.resize(size)?;
                    }
                    if child.try_wait()?.is_some() {
                        break;
                    }
                }
                drop(writer);
                drop(pty);
                Ok(())
            })();
            if let Some(s) = weak.upgrade() {
                let mut state = s.state.lock().unwrap();
                state.connected = false;
                state.ended = true;
                if let Err(e) = result {
                    state.error = format!("{e:#}");
                    control.update(Phase::Failed, state.error.clone());
                }
                s.revision.fetch_add(1, Ordering::Release);
            }
        });
        s
    }
    pub fn online(&self) -> bool {
        self.state.lock().unwrap().connected
    }
    pub fn encode(&self, input: Input) -> anyhow::Result<Vec<u8>> {
        let mut st = self.state.lock().unwrap();
        ensure!(st.connected, "This terminal is offline");
        wezterm_term::user_input::capture(self.id, || match input {
            Input::Key(k, m) => st.term.key_down(k, m),
            Input::KeyUp(k, m) => st.term.key_up(k, m),
            Input::Text(t) => {
                let mut chars = t.chars();
                match (chars.next(), chars.next()) {
                    (Some(c), None) => st.term.key_down(KeyCode::Char(c), KeyModifiers::NONE),
                    _ => st.term.send_raw_input(t.as_bytes()),
                }
            }
            Input::Paste(t) => st.term.send_paste(&t),
        })
    }
    pub fn send(&self, bytes: &[u8]) -> anyhow::Result<()> {
        if bytes.is_empty() {
            return Ok(());
        }
        ensure!(self.online(), "This terminal is offline");
        self.tx
            .try_send(Request::Input(bytes.to_vec()))
            .context("Terminal input queue full; input was not replayed")?;
        self.follow.fetch_add(1, Ordering::Release);
        Ok(())
    }
    pub fn resize(&self, rows: usize, cols: usize, width: usize, height: usize, dpi: u32) {
        let mut st = self.state.lock().unwrap();
        let size = TerminalSize {
            rows: rows.max(2),
            cols: cols.max(2),
            pixel_width: width,
            pixel_height: height,
            dpi,
        };
        if st.term.get_size() == size {
            return;
        }
        st.term.resize(size);
        if !st.ended {
            *self.pending_size.lock().unwrap() = Some(pty_size(size));
        }
        self.revision.fetch_add(1, Ordering::Release);
    }

    pub fn theme(&self, theme: Theme) {
        let mut st = self.state.lock().unwrap();
        st.term.set_config(Arc::new(TermConfig { theme }));
        st.term.implicit_palette_reset_if_same_as_configured();
        self.revision.fetch_add(1, Ordering::Release);
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
        self.auth.cancel();
    }
}
fn pty_size(s: TerminalSize) -> PtySize {
    PtySize {
        rows: s.rows.min(u16::MAX as usize) as u16,
        cols: s.cols.min(u16::MAX as usize) as u16,
        pixel_width: s.pixel_width.min(u16::MAX as usize) as u16,
        pixel_height: s.pixel_height.min(u16::MAX as usize) as u16,
    }
}

pub fn connection_config(host: &Host, keepalive: u64) -> anyhow::Result<wezterm_ssh::ConfigMap> {
    let mut cfg = wezterm_ssh::Config::new();
    cfg.add_default_config_files();
    let mut cfg = cfg.for_host(&host.address);
    cfg.insert("hostname".into(), host.address.clone());
    cfg.insert("port".into(), host.port.to_string());
    if !host.username.is_empty() {
        cfg.insert("user".into(), host.username.clone());
    }
    cfg.insert("wezterm_ssh_backend".into(), "libssh".into());
    cfg.insert("serveraliveinterval".into(), keepalive.to_string());
    cfg.insert("connecttimeout".into(), "15".into());
    cfg.insert("termviai_auth_attempt".into(), "yes".into());
    if let Some(mode) = host.authentication {
        cfg.insert(
            "termviai_auth_mode".into(),
            match mode {
                Authentication::Password => "password",
                Authentication::Key => "key",
                Authentication::Identity => "identity",
            }
            .into(),
        );
    }
    if !host.identity_file.is_empty() {
        cfg.insert(
            if host.authentication == Some(Authentication::Key) {
                "termviai_identity_file"
            } else {
                "identityfile"
            }
            .into(),
            host.identity_file.clone(),
        );
    }
    if !host.credential_ref().is_empty() {
        cfg.insert("termviai_credential".into(), host.credential_ref().into());
    }
    Ok(cfg)
}
pub fn candidate(label: String, host: anyhow::Result<Host>, keepalive: u64) -> Candidate {
    match host.and_then(|h| connection_config(&h, keepalive)) {
        Ok(config) => Candidate {
            label,
            config,
            error: None,
        },
        Err(e) => Candidate {
            label,
            config: Default::default(),
            error: Some(format!("{e:#}")),
        },
    }
}
pub fn authentication_plan(host: &Host, store: &HostStore, keepalive: u64) -> Vec<Candidate> {
    let attempts = host.terminal_authentication(store);
    if attempts.is_empty() && host.authentication.is_none() {
        return vec![candidate(
            "Default SSH authentication".into(),
            Ok(host.clone()),
            keepalive,
        )];
    }
    attempts
        .into_iter()
        .map(|(m, h)| candidate(format!("{m:?}"), h, keepalive))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn output_state() -> State {
        State {
            term: Terminal::new(
                TerminalSize::default(),
                Arc::new(TermConfig { theme: Theme::Ink }),
                "TermViAI",
                "test",
                Box::new(Vec::new()),
            ),
            connected: true,
            ended: false,
            error: String::new(),
            bytes: 0,
            notifications: Inbox::default(),
            broadcast_filter: BroadcastFilter::default(),
        }
    }
    #[test]
    fn remote_broadcast_does_not_change_prompt_or_cursor_and_local_is_untouched() {
        let frame = format!(
            "\r{}\r\n{:<79}\x07\x07\r\n{}\r\nhello\r\n{}\r\n",
            " ".repeat(79),
            "Broadcast message from root@host (pts/12) (Tue Nov 10 16:35:48 2026):",
            " ".repeat(79),
            " ".repeat(79)
        );
        let now = Instant::now();
        let mut state = output_state();
        state.advance_output(b"prompt> ", now, true);
        let cursor = state.term.cursor_pos();
        state.advance_output(frame.as_bytes(), now, true);
        state.advance_output(&[], now + Duration::from_millis(300), true);
        assert_eq!(state.notifications.items.len(), 1);
        assert_eq!(state.term.cursor_pos().x, cursor.x);
        assert_eq!(state.term.cursor_pos().y, cursor.y);
        assert_eq!(state.bytes, frame.len() as u64 + 8);
        state.advance_output(b"next", now + Duration::from_millis(300), true);
        assert_eq!(state.term.cursor_pos().x, 12);
        let mut local = output_state();
        local.advance_output(frame.as_bytes(), now, false);
        assert!(local.notifications.items.is_empty());
        assert!(local.term.cursor_pos().y > 0);
    }
    #[test]
    fn remote_output_eof_flushes_an_unfinished_candidate() {
        let now = Instant::now();
        let mut state = output_state();
        let text = format!("\r{}\r\nBroadcast message from root@host", " ".repeat(79));
        state.advance_output(text.as_bytes(), now, true);
        state.finish_output();
        assert!(state.notifications.items.is_empty());
        let sc = state.term.screen();
        let rows = sc.lines_in_phys_range(0..sc.scrollback_rows());
        assert!(
            rows.iter()
                .any(|line| line.as_str().contains("Broadcast message from"))
        );
    }
    #[test]
    fn production_core_encodes_tab_and_shift_tab_for_shells() {
        let mut t = Terminal::new(
            TerminalSize::default(),
            Arc::new(TermConfig { theme: Theme::Ink }),
            "TermViAI",
            "test",
            Box::new(Vec::new()),
        );
        t.set_input_capture_id(9);
        let tab =
            wezterm_term::user_input::capture(9, || t.key_down(KeyCode::Tab, KeyModifiers::NONE))
                .unwrap();
        let backtab =
            wezterm_term::user_input::capture(9, || t.key_down(KeyCode::Tab, KeyModifiers::SHIFT))
                .unwrap();
        assert_eq!(tab, b"\t");
        assert_eq!(backtab, b"\x1b[Z");
    }
    #[test]
    fn production_core_keeps_wide_cells_and_bracketed_paste_encoding() {
        let mut t = Terminal::new(
            TerminalSize::default(),
            Arc::new(TermConfig { theme: Theme::Ink }),
            "TermViAI",
            "test",
            Box::new(Vec::new()),
        );
        t.set_input_capture_id(7);
        t.advance_bytes("A中文B\x1b[?2004h".as_bytes());
        assert_eq!(t.cursor_pos().x, 6);
        let bytes = wezterm_term::user_input::capture(7, || t.send_paste("中文\x1b[201~")).unwrap();
        assert_eq!(bytes, "\x1b[200~中文\x1b[201~".as_bytes());
    }
    fn screen_text(s: &Session) -> String {
        let st = s.state.lock().unwrap();
        let sc = st.term.screen();
        sc.lines_in_phys_range(0..sc.scrollback_rows())
            .iter()
            .map(|l| l.as_str().to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }
    fn wait_for(s: &Session, mut ready: impl FnMut() -> bool) {
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        while !ready() {
            assert!(
                std::time::Instant::now() < deadline,
                "Session timeout: {}",
                s.state.lock().unwrap().error
            );
            thread::sleep(Duration::from_millis(20));
        }
    }
    #[test]
    fn local_pty_input_resize_broadcast_and_close() {
        let sessions: Vec<_> = (301..304)
            .map(|id| Session::new(id, None, &HostStore::default(), Theme::Ink, 30))
            .collect();
        for s in &sessions {
            wait_for(s, || s.online());
        }
        let mut broadcast = termviai_broadcast::BroadcastState::new(sessions.iter().map(|s| s.id));
        broadcast.set_enabled(true);
        let bytes = sessions[0]
            .encode(Input::Text("echo native-adapter-ok\r".into()))
            .unwrap();
        let report = termviai_broadcast::route_input(
            &broadcast,
            301,
            &bytes,
            |id| sessions.iter().find(|s| s.id == id).unwrap().online(),
            |id, bytes| sessions.iter().find(|s| s.id == id).unwrap().send(bytes),
        )
        .unwrap();
        assert_eq!(report.delivered.len(), 3);
        for s in &sessions {
            wait_for(s, || screen_text(s).contains("native-adapter-ok"));
            for cols in [100, 60, 120, 50, 80] {
                s.resize(24, cols, cols * 8, 480, 96);
            }
            assert_eq!(s.state.lock().unwrap().term.get_size().cols, 80);
            s.send(&[]).unwrap();
            assert_eq!(s.follow.load(Ordering::Acquire), 1);
            s.send(b"exit\r").unwrap();
            wait_for(s, || s.state.lock().unwrap().ended);
        }
    }
    #[test]
    #[ignore = "Requires the local synthetic SSH fixture and TERMVIAI_AUTH_FIXTURE"]
    fn ssh_pty_adapter_key_input_resize_and_close() {
        let dir = std::env::var("TERMVIAI_AUTH_FIXTURE").unwrap();
        let mut config = wezterm_ssh::Config::new().for_host("127.0.0.1");
        for (k, v) in [
            ("hostname", "127.0.0.1".to_string()),
            ("port", "22371".into()),
            ("user", "key-user".into()),
            ("wezterm_ssh_backend", "libssh".into()),
            ("termviai_auth_attempt", "yes".into()),
            ("termviai_auth_mode", "key".into()),
            ("termviai_identity_file", format!("{dir}/good_key")),
            ("userknownhostsfile", format!("{dir}/known_hosts")),
            ("connecttimeout", "5".into()),
        ] {
            config.insert(k.into(), v);
        }
        let s = Session::start(
            410,
            Some(Host::default()),
            Theme::Ink,
            Some(vec![Candidate {
                label: "fixture".into(),
                config,
                error: None,
            }]),
        );
        let mut revision = u64::MAX;
        wait_for(&s, || {
            let v = s.auth.view();
            if v.revision != revision {
                revision = v.revision;
                if matches!(v.phase, Phase::Verify) {
                    s.auth.send(authentication::Command::Trust(true)).unwrap();
                }
            }
            s.online()
        });
        wait_for(&s, || screen_text(&s).contains("fixture-ready"));
        s.resize(30, 90, 720, 600, 96);
        s.send(b"adapter-echo\r").unwrap();
        wait_for(&s, || screen_text(&s).contains("adapter-echo"));
        s.send(b"fixture-exit\r").unwrap();
        wait_for(&s, || s.state.lock().unwrap().ended);
    }
    #[test]
    fn stored_authentication_retains_priority() {
        let host = Host {
            address: "localhost".into(),
            port: 22,
            username: "u".into(),
            key_label: "missing".into(),
            identity_id: "missing".into(),
            credential_id: "ref".into(),
            ..Host::default()
        };
        let plan = authentication_plan(&host, &HostStore::default(), 30);
        assert_eq!(
            plan.iter().map(|p| p.label.as_str()).collect::<Vec<_>>(),
            vec!["Key", "Identity", "Password"]
        );
        assert!(plan[0].error.is_some());
        assert!(plan[1].error.is_some());
        assert_eq!(plan[2].config["termviai_credential"], "ref");
    }
}

/// Windows system bell is opt-in; it never runs on the SSH reader thread.
pub fn ring_bell() {
    #[cfg(windows)]
    {
        #[link(name = "user32")]
        unsafe extern "system" {
            fn MessageBeep(kind: u32) -> i32;
        }
        unsafe {
            MessageBeep(0);
        }
    }
}
