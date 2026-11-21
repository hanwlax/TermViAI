//! Application-owned reverse tunnels. Only opaque Host credential references are persisted.
#[path = "forward_paths.rs"]
mod forward_paths;
use crate::hosts::{new_credential_id, Host, HostStore};
use anyhow::{ensure, Context};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::net::{SocketAddr, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use wezterm_ssh::forward_network::Path as NetworkPath;
use wezterm_ssh::{ConfigMap, Session, SessionEvent};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Rule {
    pub id: String,
    pub name: String,
    pub host: Host,
    pub remote_port: u16,
    pub local_port: u16,
    pub network: bool,
    pub retry: bool,
    #[serde(default)]
    pub desired: bool,
}
impl Rule {
    pub fn new(host: Host) -> Self {
        Self {
            id: new_credential_id(),
            name: String::new(),
            host,
            remote_port: 18080,
            local_port: 8787,
            network: false,
            retry: true,
            desired: false,
        }
    }
    pub fn bind(&self) -> &str {
        if self.network {
            "0.0.0.0"
        } else {
            "127.0.0.1"
        }
    }
    pub fn access_address(&self) -> String {
        if !self.network {
            return format!("127.0.0.1:{}", self.remote_port);
        }
        let host = self.host.address.trim_matches(['[', ']']);
        if host.contains(':') {
            format!("[{host}]:{}", self.remote_port)
        } else {
            format!("{host}:{}", self.remote_port)
        }
    }
    pub fn validate(&mut self, others: &[Rule]) -> anyhow::Result<()> {
        ensure!(
            !self.id.is_empty()
                && self.id.len() <= 128
                && self
                    .id
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_'),
            "Invalid forwarding ID"
        );
        ensure!(
            self.remote_port > 0 && self.local_port > 0,
            "Ports must be between 1 and 65535."
        );
        ensure!(
            !self.host.address.is_empty() && self.host.port > 0,
            "The source SSH terminal is no longer available."
        );
        self.name = self.name.trim().to_owned();
        ensure!(
            self.name.len() <= 256 && !self.name.chars().any(char::is_control),
            "Use a name of at most 256 bytes without control characters."
        );
        if self.name.is_empty() {
            self.name = format!("{} · {}", self.host.label, self.remote_port);
        }
        ensure!(
            !others.iter().any(|r| r.id != self.id
                && same_server(&r.host, &self.host)
                && r.remote_port == self.remote_port),
            "This server already has a rule for this remote port. Edit it or choose another port."
        );
        Ok(())
    }
}
fn same_server(a: &Host, b: &Host) -> bool {
    a.port == b.port
        && a.address
            .trim_end_matches('.')
            .eq_ignore_ascii_case(b.address.trim_end_matches('.'))
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Stopped,
    CheckingNetwork,
    WaitingForNetwork,
    Queued,
    Starting,
    Running,
    Retrying,
    Stopping,
    Attention,
    Failed,
}
impl State {
    pub fn label(self) -> &'static str {
        match self {
            Self::CheckingNetwork => "Checking network…",
            Self::WaitingForNetwork => "Waiting for network",
            Self::Queued => "Queued for connection",
            Self::Stopped => "Stopped",
            Self::Starting => "Connecting…",
            Self::Running => "Tunnel established",
            Self::Retrying => "Retrying…",
            Self::Stopping => "Stopping…",
            Self::Attention => "Needs attention",
            Self::Failed => "Connection failed",
        }
    }
    pub fn active(self) -> bool {
        matches!(
            self,
            Self::CheckingNetwork
                | Self::WaitingForNetwork
                | Self::Queued
                | Self::Starting
                | Self::Running
                | Self::Retrying
                | Self::Stopping
        )
    }
}
#[derive(Clone)]
pub struct View {
    pub rule: Rule,
    pub state: State,
    pub error: String,
    pub last_failure: String,
    pub attempt: usize,
    pub last_attempt: Option<std::time::SystemTime>,
    pub retry_at: Option<Instant>,
    pub service: Option<bool>,
    pub inspection: Option<Inspection>,
    pub busy: bool,
}
impl View {
    pub fn status(&self) -> String {
        if self.state == State::Retrying {
            if let Some(deadline) = self.retry_at {
                return format!(
                    "Retrying in {}s",
                    deadline
                        .saturating_duration_since(Instant::now())
                        .as_secs_f64()
                        .ceil() as u64
                );
            }
        }
        self.state.label().to_string()
    }
}
struct Worker {
    cancel: Arc<AtomicBool>,
    events: mpsc::Receiver<Event>,
}
enum Event {
    Attempted(std::time::SystemTime),
    Status(State, String),
    Done,
}
struct Entry {
    view: View,
    worker: Option<Worker>,
    pending: Option<ConfigMap>,
    control: Option<ControlJob>,
    probe: Option<mpsc::Receiver<bool>>,
    cancelled_for_network: bool,
    stable_since: Option<Instant>,
    ownership_file: std::path::PathBuf,
}
impl Drop for Entry {
    fn drop(&mut self) {
        if let Some(w) = &self.worker {
            w.cancel.store(true, Ordering::Relaxed);
        }
    }
}
pub struct Manager {
    entries: Vec<Entry>,
    pub load_error: String,
    loaded: bool,
    resumed: bool,
    routes: forward_paths::Routes,
    next: usize,
    data_path: std::path::PathBuf,
    storage_lock: Option<std::fs::File>,
}
impl Default for Manager {
    fn default() -> Self {
        Self {
            entries: vec![],
            load_error: String::new(),
            loaded: false,
            resumed: false,
            routes: Default::default(),
            next: 0,
            data_path: path(),
            storage_lock: None,
        }
    }
}
pub fn manager() -> &'static Mutex<Manager> {
    static M: OnceLock<Mutex<Manager>> = OnceLock::new();
    M.get_or_init(|| Mutex::new(Manager::default()))
}
fn path() -> std::path::PathBuf {
    HostStore::path().with_file_name("forwarding.json")
}
impl Manager {
    pub fn load(&mut self) {
        if self.loaded {
            return;
        }
        self.loaded = true;
        let result = (|| -> anyhow::Result<Vec<Rule>> {
            let dir = self.data_path.parent().context("Invalid forwarding path")?;
            std::fs::create_dir_all(dir)?;
            let lock = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(self.data_path.with_extension("lock"))?;
            lock.try_lock().context("Forwarding is managed by another TermViAI instance; close it before restoring tunnels")?;
            self.storage_lock = Some(lock);
            match std::fs::File::open(&self.data_path) {
                Ok(f) => {
                    let rules: Vec<Rule> = serde_json::from_reader(f)?;
                    ensure!(rules.len() <= 512, "Too many forwarding rules");
                    let mut valid = vec![];
                    for mut r in rules {
                        ensure!(
                            !valid.iter().any(|v: &Rule| v.id == r.id),
                            "Duplicate rule ID"
                        );
                        r.validate(&valid)?;
                        valid.push(r);
                    }
                    Ok(valid)
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(vec![]),
                Err(e) => Err(e.into()),
            }
        })();
        match result {
            Ok(rules) => {
                self.entries = rules
                    .into_iter()
                    .map(|r| entry_at(r, &self.data_path))
                    .collect()
            }
            Err(e) => {
                self.load_error =
                    format!("Could not load forwarding.json; preserved without changes: {e:#}")
            }
        }
    }
    fn persist(&self, rules: &[Rule]) -> anyhow::Result<()> {
        ensure!(self.load_error.is_empty(), "{}", self.load_error);
        let path = &self.data_path;
        let dir = path.parent().unwrap();
        std::fs::create_dir_all(dir)?;
        let mut f = tempfile::NamedTempFile::new_in(dir)?;
        serde_json::to_writer_pretty(&mut f, rules)?;
        f.write_all(b"\n")?;
        f.as_file().sync_all()?;
        f.persist(path).map_err(|e| e.error)?;
        Ok(())
    }
    pub fn views(&mut self) -> Vec<View> {
        self.load();
        if !self.resumed && self.load_error.is_empty() {
            self.resumed = true;
            let wanted: Vec<_> = self
                .entries
                .iter()
                .filter(|e| e.view.rule.desired)
                .map(|e| e.view.rule.id.clone())
                .collect();
            let store = HostStore::load();
            for id in wanted {
                let result = match &store {
                    Ok(store) => self.start(&id, store),
                    Err(error) => Err(anyhow::anyhow!(
                        "Cannot restore host credentials: {error:#}"
                    )),
                };
                if let Err(error) = result {
                    if let Some(e) = self.entries.iter_mut().find(|e| e.view.rule.id == id) {
                        e.view.state = State::Attention;
                        e.view.error = format!("{error:#}");
                    }
                }
            }
        }
        self.tick();
        self.entries.iter().map(|e| e.view.clone()).collect()
    }
    pub fn save(&mut self, mut rule: Rule) -> anyhow::Result<String> {
        self.load();
        let mut rules: Vec<_> = self.entries.iter().map(|e| e.view.rule.clone()).collect();
        rule.desired = rules
            .iter()
            .find(|r| r.id == rule.id)
            .is_some_and(|r| r.desired);
        rule.validate(&rules)?;
        let current = self.entries.iter().find(|e| e.view.rule.id == rule.id);
        let connection_changed = current.is_some_and(|e| {
            let old = &e.view.rule;
            old.host != rule.host
                || old.remote_port != rule.remote_port
                || old.local_port != rule.local_port
                || old.network != rule.network
        });
        let cancel_retry = current.is_some_and(|e| {
            e.view.rule.retry
                && !rule.retry
                && matches!(
                    e.view.state,
                    State::Retrying
                        | State::WaitingForNetwork
                        | State::Queued
                        | State::CheckingNetwork
                )
        });
        if cancel_retry {
            rule.desired = false;
        }
        let new_config = if rule.desired && connection_changed {
            Some(connection_config(&rule.host, &HostStore::load()?)?)
        } else {
            None
        };
        if let Some(r) = rules.iter_mut().find(|r| r.id == rule.id) {
            *r = rule.clone()
        } else {
            ensure!(rules.len() < 512, "Maximum 512 rules");
            rules.push(rule.clone())
        }
        self.persist(&rules)?;
        let id = rule.id.clone();
        if let Some(e) = self.entries.iter_mut().find(|e| e.view.rule.id == id) {
            if connection_changed || cancel_retry {
                if let Some(w) = &e.worker {
                    w.cancel.store(true, Ordering::Relaxed);
                }
                e.cancelled_for_network = false;
                e.pending = new_config;
                e.control = None;
                e.probe = None;
                e.stable_since = None;
                e.view.last_failure.clear();
                e.view.last_attempt = None;
                e.view.service = None;
                e.view.busy = false;
                e.view.inspection = None;
                e.view.retry_at = None;
                e.view.attempt = 0;
                e.view.error.clear();
                e.view.state = if e.pending.is_some() {
                    State::CheckingNetwork
                } else {
                    State::Stopped
                };
                self.routes.forget(&rule.host.address);
            }
            e.view.rule = rule
        } else {
            self.entries.push(entry_at(rule, &self.data_path))
        }
        Ok(id)
    }
    pub fn delete(&mut self, id: &str) -> anyhow::Result<()> {
        let rules: Vec<_> = self
            .entries
            .iter()
            .filter(|e| e.view.rule.id != id)
            .map(|e| e.view.rule.clone())
            .collect();
        self.persist(&rules)?;
        self.entries.retain(|e| e.view.rule.id != id);
        Ok(())
    }
    fn set_desired(&mut self, id: &str, desired: bool) -> anyhow::Result<()> {
        let mut rules: Vec<_> = self.entries.iter().map(|e| e.view.rule.clone()).collect();
        let rule = rules
            .iter_mut()
            .find(|r| r.id == id)
            .context("Rule no longer exists")?;
        rule.desired = desired;
        self.persist(&rules)?;
        self.entries
            .iter_mut()
            .find(|e| e.view.rule.id == id)
            .unwrap()
            .view
            .rule
            .desired = desired;
        Ok(())
    }
    pub fn start(&mut self, id: &str, store: &HostStore) -> anyhow::Result<()> {
        self.set_desired(id, true)?;
        let e = self
            .entries
            .iter_mut()
            .find(|e| e.view.rule.id == id)
            .context("Rule no longer exists")?;
        e.control = None;
        e.view.busy = false;
        let cfg = connection_config(&e.view.rule.host, store)?;
        if let Some(w) = &e.worker {
            w.cancel.store(true, Ordering::Relaxed);
        }
        self.routes.forget(&e.view.rule.host.address);
        e.cancelled_for_network = false;
        e.pending = Some(cfg);
        e.view.state = State::CheckingNetwork;
        e.view.attempt = 0;
        e.view.retry_at = None;
        e.view.error.clear();
        e.stable_since = None;
        Ok(())
    }
    pub fn retry_waiting(&mut self, id: &str, store: &HostStore) -> anyhow::Result<()> {
        if self.entries.iter().any(|e| {
            e.view.rule.id == id
                && e.view.rule.desired
                && matches!(e.view.state, State::WaitingForNetwork | State::Retrying)
        }) {
            self.start(id, store)?;
        }
        Ok(())
    }
    pub fn stop(&mut self, id: &str) -> anyhow::Result<()> {
        let persisted = self.set_desired(id, false);
        self.stop_runtime(id);
        persisted.context("Tunnel stopped, but its stop preference could not be saved")
    }
    fn stop_runtime(&mut self, id: &str) {
        if let Some(e) = self.entries.iter_mut().find(|e| e.view.rule.id == id) {
            e.cancelled_for_network = false;
            e.pending = None;
            e.view.retry_at = None;
            e.stable_since = None;
            e.control = None;
            e.view.busy = false;
            if let Some(w) = &e.worker {
                w.cancel.store(true, Ordering::Relaxed);
                e.view.state = State::Stopping;
            } else {
                e.view.state = State::Stopped;
            }
            e.view.error.clear();
        }
    }
    pub fn tick(&mut self) {
        let now = Instant::now();
        self.routes.tick(now);
        self.drain(now);
        let routes = &mut self.routes;
        schedule(
            &mut self.entries,
            &mut self.next,
            now,
            |host| routes.get(host, now),
            launch,
        );
    }
    fn drain(&mut self, now: Instant) {
        for e in &mut self.entries {
            if let Some(rx) = &e.probe {
                if let Ok(value) = rx.try_recv() {
                    e.view.service = Some(value);
                    e.probe = None;
                    e.view.busy = false;
                }
            }
            if let Some(job) = &e.control {
                match job.rx.try_recv() {
                    Ok(result) => {
                        let job = e.control.take().unwrap();
                        e.view.busy = false;
                        match result {
                            Ok(info) => {
                                let restart = info.released.then(|| job.restart.clone()).flatten();
                                e.view.inspection = Some(info);
                                if let Some(cfg) = restart {
                                    e.pending = Some(cfg);
                                    e.view.state = State::CheckingNetwork;
                                    e.view.retry_at = None;
                                }
                            }
                            Err(err) => {
                                e.view.error = format!("{err:#}");
                                e.view.inspection = None;
                            }
                        }
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        e.control = None;
                        e.view.busy = false;
                        e.view.error = "Inspection worker ended".into();
                    }
                    Err(mpsc::TryRecvError::Empty) => (),
                }
            }
            if e.stable_since.is_some_and(|since| {
                now.saturating_duration_since(since) >= Duration::from_secs(30)
            }) {
                e.view.attempt = 0;
                e.stable_since = None;
            }
            let mut done = false;
            if let Some(w) = &e.worker {
                loop {
                    match w.events.try_recv() {
                        Ok(Event::Attempted(at)) if !w.cancel.load(Ordering::Relaxed) => {
                            e.view.last_attempt = Some(at);
                        }
                        Ok(Event::Status(s, msg))
                            if !w.cancel.load(Ordering::Relaxed)
                                || (s == State::Attention && e.cancelled_for_network) =>
                        {
                            if s == State::WaitingForNetwork {
                                self.routes.forget(&e.view.rule.host.address);
                                e.view.retry_at = None;
                            }
                            e.view.state = s;
                            if matches!(s, State::Attention | State::Failed) {
                                e.view.last_failure.clone_from(&msg);
                            }
                            if !msg.is_empty() || s == State::Running {
                                e.view.error = msg;
                            }
                            if s == State::Running {
                                e.stable_since = Some(now);
                            }
                            if s == State::Attention {
                                e.pending = None;
                            }
                        }
                        Ok(Event::Status(..) | Event::Attempted(_)) => (),
                        Ok(Event::Done) | Err(mpsc::TryRecvError::Disconnected) => {
                            done = true;
                            break;
                        }
                        Err(mpsc::TryRecvError::Empty) => break,
                    }
                }
            }
            if done {
                let cancelled = e
                    .worker
                    .take()
                    .is_some_and(|w| w.cancel.load(Ordering::Relaxed));
                e.stable_since = None;
                if cancelled {
                    if e.pending.is_none() && e.view.state == State::Stopping {
                        e.view.state = State::Stopped;
                    }
                } else if e.pending.is_some() && e.view.state != State::WaitingForNetwork {
                    if e.view.rule.retry {
                        let delay = retry_delay(e.view.attempt);
                        let jitter = e
                            .view
                            .rule
                            .id
                            .bytes()
                            .fold(0u64, |h, b| h.wrapping_mul(31) + b as u64)
                            % 251;
                        e.view.retry_at = Some(now + delay - Duration::from_millis(jitter));
                        e.view.attempt = e.view.attempt.saturating_add(1);
                        e.view.state = State::Retrying;
                    } else {
                        e.pending = None;
                        e.view.state = State::Failed;
                    }
                } else if e.view.state == State::WaitingForNetwork && !e.view.rule.retry {
                    e.pending = None;
                    e.view.state = State::Failed;
                }
            }
        }
    }
    pub fn probe(&mut self, id: &str) {
        if let Some(e) = self.entries.iter_mut().find(|e| e.view.rule.id == id) {
            if e.probe.is_some() || e.control.is_some() {
                return;
            }
            let port = e.view.rule.local_port;
            let (tx, rx) = mpsc::channel();
            e.probe = Some(rx);
            e.view.busy = true;
            std::thread::spawn(move || {
                let ok = TcpStream::connect_timeout(
                    &SocketAddr::from(([127, 0, 0, 1], port)),
                    Duration::from_secs(2),
                )
                .is_ok();
                tx.send(ok).ok();
            });
        }
    }
}

fn entry_at(rule: Rule, data_path: &std::path::Path) -> Entry {
    let ownership_file = data_path
        .parent()
        .unwrap()
        .join("forwarding-owners")
        .join(format!("{}.json", rule.id));
    Entry {
        ownership_file,
        cancelled_for_network: false,
        stable_since: None,
        view: View {
            rule,
            state: State::Stopped,
            error: String::new(),
            last_failure: String::new(),
            attempt: 0,
            last_attempt: None,
            retry_at: None,
            service: None,
            inspection: None,
            busy: false,
        },
        worker: None,
        pending: None,
        control: None,
        probe: None,
    }
}
pub fn connection_config(host: &Host, store: &HostStore) -> anyhow::Result<ConfigMap> {
    let mut h = host.clone();
    h.resolve_identity(store)?;
    // Config::new supplies defaults, but intentionally never loads config files.
    let mut cfg = wezterm_ssh::Config::new().for_host(&h.address);
    cfg.insert("termviai_no_ssh_config".into(), "yes".into());
    cfg.insert("hostname".into(), h.address.clone());
    cfg.insert("port".into(), h.port.to_string());
    if !h.username.is_empty() {
        cfg.insert("user".into(), h.username.clone());
    }
    cfg.insert("wezterm_ssh_backend".into(), "libssh".into());
    cfg.insert("connecttimeout".into(), "10".into());
    cfg.insert("serveraliveinterval".into(), "15".into());
    let mode = match h.authentication {
        Some(crate::hosts::Authentication::Key) => "key",
        Some(crate::hosts::Authentication::Password) => "password",
        Some(crate::hosts::Authentication::Identity) => "identity",
        None => "",
    };
    if !mode.is_empty() {
        cfg.insert("termviai_auth_mode".into(), mode.into());
    }
    if !h.identity_file.is_empty() {
        cfg.insert(
            if mode == "key" {
                "termviai_identity_file"
            } else {
                "identityfile"
            }
            .into(),
            h.identity_file.clone(),
        );
    }
    if !h.credential_ref().is_empty() {
        cfg.insert("termviai_credential".into(), h.credential_ref().into());
    }
    Ok(cfg)
}
#[derive(Debug, thiserror::Error)]
#[error("Authentication or host verification requires attention")]
struct Attention;
#[derive(Debug, thiserror::Error)]
#[error("No network path to the SSH host")]
struct NoNetworkPath;
fn connect(
    cfg: ConfigMap,
    cancel: &AtomicBool,
    events: Option<&mpsc::Sender<Event>>,
) -> anyhow::Result<Session> {
    ensure!(!cancel.load(Ordering::Relaxed), "Cancelled");
    if let Some(host) = cfg.get("hostname") {
        if wezterm_ssh::forward_network::path_to(host) == NetworkPath::Offline {
            return Err(NoNetworkPath.into());
        }
    }
    ensure!(!cancel.load(Ordering::Relaxed), "Cancelled");
    if let Some(tx) = events {
        let _ = tx.send(Event::Attempted(std::time::SystemTime::now()));
    }
    let (session, events) = Session::connect(cfg)?;
    let until = Instant::now() + Duration::from_secs(25);
    loop {
        ensure!(!cancel.load(Ordering::Relaxed), "Cancelled");
        ensure!(Instant::now() < until, "SSH authentication timed out");
        match events.try_recv() {
            Ok(SessionEvent::Authenticated) => return Ok(session),
            Ok(SessionEvent::Banner(_)) => (),
            Ok(SessionEvent::HostVerify(request)) => {
                request.try_answer(false).ok();
                return Err(anyhow::anyhow!("Verify this host in its terminal before starting forwarding.").context(Attention));
            }
            Ok(SessionEvent::Authenticate(_)) => return Err(anyhow::anyhow!("Authentication is required. Save this host's password / Identity, or unlock its key in the SSH agent, then retry.").context(Attention)),
            Ok(SessionEvent::HostVerificationFailed(error)) => return Err(anyhow::anyhow!("{error}").context(Attention)),
            Ok(SessionEvent::AuthenticationFailed(error)) => return Err(anyhow::anyhow!("{error}").context(Attention)),
            Ok(SessionEvent::Error(error)) => anyhow::bail!("{error}"),
            Err(smol::channel::TryRecvError::Closed) => anyhow::bail!("SSH connection closed"),
            Err(smol::channel::TryRecvError::Empty) => std::thread::sleep(Duration::from_millis(25)),
        }
    }
}
fn retry_delay(attempt: usize) -> Duration {
    const SECONDS: [u64; 6] = [1, 2, 4, 8, 16, 30];
    Duration::from_secs(SECONDS[attempt.min(SECONDS.len() - 1)])
}
fn needs_attention(error: &anyhow::Error) -> bool {
    let message = format!("{error:#}");
    error.downcast_ref::<Attention>().is_some()
        || error
            .downcast_ref::<wezterm_ssh::RemoteListenerRefused>()
            .is_some()
        || [
            "The selected SSH key was not accepted",
            "unhandled auth case",
            "No authentication methods",
            "Authentication failed",
            "Unable to authenticate",
        ]
        .iter()
        .any(|reason| message.contains(reason))
}
// The record is learned over the same SSH transport before it can listen.
// No PID is inferred from a port, username or saved Host label.
#[derive(Serialize, Deserialize)]
struct Ownership {
    host: String,
    ssh_port: u16,
    remote_port: u16,
    bind: String,
    owner: Owner,
}
impl Ownership {
    fn matches(&self, rule: &Rule) -> bool {
        self.host == rule.host.address
            && self.ssh_port == rule.host.port
            && self.remote_port == rule.remote_port
            && self.bind == rule.bind()
    }
}
fn read_ownership(file: &std::path::Path, rule: &Rule) -> anyhow::Result<Option<Owner>> {
    match std::fs::File::open(file) {
        Ok(f) => {
            let record: Ownership =
                serde_json::from_reader(f).context("Cannot read forwarding ownership record")?;
            ensure!(
                record.owner.pid > 1 && record.owner.eligible && !record.owner.token.is_empty(),
                "Invalid forwarding ownership identity"
            );
            Ok(record.matches(rule).then_some(record.owner))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}
fn save_ownership(file: &std::path::Path, rule: &Rule, owner: Option<Owner>) -> anyhow::Result<()> {
    if let Some(owner) = owner {
        ensure!(
            owner.pid > 1 && owner.eligible && !owner.token.is_empty(),
            "Invalid SSH ownership record"
        );
        let record = Ownership {
            host: rule.host.address.clone(),
            ssh_port: rule.host.port,
            remote_port: rule.remote_port,
            bind: rule.bind().into(),
            owner,
        };
        let dir = file.parent().context("Invalid ownership path")?;
        std::fs::create_dir_all(dir)?;
        let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
        serde_json::to_writer(&mut tmp, &record)?;
        tmp.as_file().sync_all()?;
        tmp.persist(file).map_err(|e| e.error)?;
    } else {
        match std::fs::remove_file(file) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}
fn ownership_setup(
    rule: Rule,
    file: std::path::PathBuf,
    warning: Arc<Mutex<String>>,
) -> anyhow::Result<wezterm_ssh::RemoteForwardSetup> {
    Ok(wezterm_ssh::RemoteForwardSetup {
        command: remote_command(serde_json::json!({"register":true}))?,
        commit: Box::new(move |response| {
            let owner = response.and_then(|s| {
                let data: serde_json::Value = serde_json::from_str(&s)?;
                if let Some(error) = data.get("error").and_then(|v| v.as_str()) {
                    anyhow::bail!("{error}")
                }
                Ok(serde_json::from_value::<Owner>(data["session"].clone())?)
            });
            let owner = match owner {
                Ok(owner) => Some(owner),
                Err(error) => {
                    *warning.lock().unwrap() = format!("Automatic stale-port recovery unavailable: {error:#}. The tunnel can still run.");
                    None
                }
            };
            let registered = owner.is_some();
            save_ownership(&file, &rule, owner).map_err(|e| e.context(Attention))?;
            Ok(registered)
        }),
    })
}
fn schedule(
    entries: &mut [Entry],
    next: &mut usize,
    now: Instant,
    mut path: impl FnMut(&str) -> Option<NetworkPath>,
    mut start: impl FnMut(&mut Entry, ConfigMap),
) {
    let mut endpoints = std::collections::HashSet::new();
    let mut connecting = 0;
    let endpoint = |r: &Rule| {
        (
            r.host.address.trim_end_matches('.').to_ascii_lowercase(),
            r.host.port,
        )
    };
    for e in entries.iter() {
        if e.worker.is_some() && e.view.state != State::Running {
            connecting += 1;
            endpoints.insert(endpoint(&e.view.rule));
        }
    }
    let count = entries.len();
    let first = *next;
    for offset in 0..count {
        let index = (first + offset) % count;
        let e = &mut entries[index];
        if !e.view.rule.desired || e.pending.is_none() || e.view.state == State::Running {
            continue;
        }
        let route = path(&e.view.rule.host.address);
        if route == Some(NetworkPath::Offline) {
            if let Some(w) = &e.worker {
                if !w.cancel.load(Ordering::Relaxed) {
                    e.cancelled_for_network = true;
                }
                w.cancel.store(true, Ordering::Relaxed);
            }
            e.view.retry_at = None;
            if e.view.rule.retry {
                e.view.state = State::WaitingForNetwork;
            } else {
                e.view.state = State::Failed;
                e.view.error = "No network path to the SSH host".into();
                e.pending = None;
            }
            continue;
        }
        if e.worker.is_some()
            || route.is_none()
            || e.view.retry_at.is_some_and(|deadline| now < deadline)
        {
            continue;
        }
        e.view.state = State::Queued;
        if connecting >= 4 || !endpoints.insert(endpoint(&e.view.rule)) {
            continue;
        }
        connecting += 1;
        e.view.retry_at = None;
        *next = (index + 1) % count;
        start(e, e.pending.as_ref().unwrap().clone());
    }
}

fn launch(e: &mut Entry, cfg: ConfigMap) {
    let cancel = Arc::new(AtomicBool::new(false));
    let stop = cancel.clone();
    let (tx, rx) = mpsc::channel();
    let rule = e.view.rule.clone();
    let ownership_file = e.ownership_file.clone();
    e.view.state = State::Starting;
    e.view.service = None;
    e.cancelled_for_network = false;
    e.worker = Some(Worker { cancel, events: rx });
    std::thread::spawn(move || {
        if !stop.load(Ordering::Relaxed) {
            let result = (|| -> anyhow::Result<()> {
                if let Some(owner) =
                    read_ownership(&ownership_file, &rule).map_err(|e| e.context(Attention))?
                {
                    tx.send(Event::Status(
                        State::Starting,
                        "Checking the previous TermViAI SSH session…".into(),
                    ))
                    .ok();
                    inspect(
                        cfg.clone(),
                        rule.clone(),
                        Some(owner),
                        true,
                        &stop,
                        Some(&tx),
                    )?;
                }
                ensure!(!stop.load(Ordering::Relaxed), "Cancelled");
                let session = connect(cfg.clone(), &stop, Some(&tx))?;
                let warning = Arc::new(Mutex::new(String::new()));
                let setup = ownership_setup(rule.clone(), ownership_file.clone(), warning.clone())?;
                let (ready, rx) = mpsc::channel();
                let fallback_ready = ready.clone();
                let request = wezterm_ssh::RemoteForward {
                    bind: rule.bind().into(),
                    remote_port: rule.remote_port,
                    local_port: rule.local_port,
                    cancel: stop.clone(),
                    ready,
                    setup: Some(setup),
                };
                let outcome = smol::block_on(async {
                    let run = async {
                        let result = session.run_remote_forward(request).await;
                        if result.as_ref().err().is_some_and(|e| {
                            e.downcast_ref::<wezterm_ssh::RemoteForwardSetupUnavailable>()
                                .is_some()
                        }) && !stop.load(Ordering::Relaxed)
                        {
                            // A rejected exec request can leave a sticky libssh
                            // error. Keep ordinary forwarding on exec-restricted
                            // or non-Python hosts working on a fresh transport.
                            let fresh = connect(cfg.clone(), &stop, Some(&tx))?;
                            fresh
                                .run_remote_forward(wezterm_ssh::RemoteForward {
                                    bind: rule.bind().into(),
                                    remote_port: rule.remote_port,
                                    local_port: rule.local_port,
                                    cancel: stop.clone(),
                                    ready: fallback_ready,
                                    setup: None,
                                })
                                .await
                        } else {
                            result
                        }
                    };
                    let notify = async {
                        loop {
                            if rx.try_recv().is_ok() {
                                tx.send(Event::Status(
                                    State::Running,
                                    warning.lock().unwrap().clone(),
                                ))
                                .ok();
                                smol::future::pending::<()>().await;
                            }
                            smol::Timer::after(Duration::from_millis(20)).await;
                        }
                    };
                    smol::future::or(run, async {
                        notify.await;
                        Ok(())
                    })
                    .await
                });
                outcome
            })();
            if !stop.load(Ordering::Relaxed) || result.as_ref().err().is_some_and(needs_attention) {
                let error = result
                    .err()
                    .unwrap_or_else(|| anyhow::anyhow!("SSH forwarding ended"));
                let state = if error.downcast_ref::<NoNetworkPath>().is_some() {
                    State::WaitingForNetwork
                } else if needs_attention(&error) {
                    State::Attention
                } else {
                    State::Failed
                };
                tx.send(Event::Status(state, format!("{error:#}"))).ok();
            }
        }
        tx.send(Event::Done).ok();
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    fn rule() -> Rule {
        Rule::new(Host {
            label: "209".into(),
            address: "192.168.25.209".into(),
            port: 22,
            username: "root".into(),
            ..Host::default()
        })
    }
    #[test]
    fn overlap_is_server_scoped_not_username_scoped() {
        let mut a = rule();
        a.name = "A".into();
        let mut b = rule();
        b.host.username = "other".into();
        b.network = true;
        assert!(b.validate(&[a.clone()]).is_err());
        b.host.port = 2222;
        assert!(b.validate(&[a]).is_ok());
    }
    #[test]
    fn explicit_config_does_not_inherit_user_config() {
        let cfg = connection_config(&rule().host, &HostStore::default()).unwrap();
        assert_eq!(cfg["hostname"], "192.168.25.209");
        assert_eq!(cfg["termviai_no_ssh_config"], "yes");
        assert!(!cfg.contains_key("proxycommand"));
    }
    #[test]
    fn network_retries_continue_at_the_capped_delay() {
        assert_eq!(
            (0..9)
                .map(retry_delay)
                .map(|d| d.as_secs())
                .collect::<Vec<_>>(),
            [1, 2, 4, 8, 16, 30, 30, 30, 30]
        );
        assert_eq!(retry_delay(usize::MAX), Duration::from_secs(30));
    }
    #[test]
    fn worker_retries_eight_dropped_connections_and_can_stop_during_backoff() {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let mut rule = rule();
        rule.host.address = "127.0.0.1".into();
        rule.host.port = listener.local_addr().unwrap().port();
        let cfg = connection_config(&rule.host, &HostStore::default()).unwrap();
        let dir = tempfile::tempdir().unwrap();
        rule.desired = true;
        let id = rule.id.clone();
        let mut entry = entry_at(rule, &dir.path().join("forwarding.json"));
        entry.pending = Some(cfg);
        let mut m = Manager {
            entries: vec![entry],
            ..Default::default()
        };
        let mut dropped = 0;
        let until = Instant::now() + Duration::from_secs(20);
        let mut now = Instant::now();
        while m.entries[0].view.attempt < 8 {
            assert!(Instant::now() < until, "timed out after {dropped} sockets");
            m.drain(now);
            if m.entries[0].view.attempt == 8 {
                break;
            }
            schedule(
                &mut m.entries,
                &mut m.next,
                now,
                |_| Some(NetworkPath::Available),
                launch,
            );
            if let Ok((socket, _)) = listener.accept() {
                drop(socket);
                dropped += 1;
            }
            now += Duration::from_secs(30);
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(dropped, 8);
        assert_eq!(m.entries[0].view.state, State::Retrying);
        m.stop_runtime(&id);
        schedule(
            &mut m.entries,
            &mut m.next,
            now + Duration::from_secs(60),
            |_| Some(NetworkPath::Available),
            |_, _| panic!("stopped service retried"),
        );
    }
    fn pending_entry(address: &str) -> Entry {
        let mut r = rule();
        r.host.address = address.into();
        r.desired = true;
        let mut entry = entry_at(r, &path());
        entry.pending = Some(ConfigMap::new());
        entry
    }
    fn simulated_worker(e: &mut Entry) -> mpsc::Sender<Event> {
        let (tx, events) = mpsc::channel();
        e.worker = Some(Worker {
            cancel: Arc::new(false.into()),
            events,
        });
        e.view.state = State::Starting;
        tx
    }
    #[test]
    fn offline_ten_minutes_then_recovery_respects_stop_and_single_attempt() {
        let mut m = Manager {
            entries: vec![pending_entry("a"), pending_entry("b")],
            ..Default::default()
        };
        let now = Instant::now();
        for t in 0..=600 {
            schedule(
                &mut m.entries,
                &mut m.next,
                now + Duration::from_secs(t),
                |_| Some(NetworkPath::Offline),
                |_, _| panic!("attempt while offline"),
            );
        }
        assert!(m
            .entries
            .iter()
            .all(|e| e.view.state == State::WaitingForNetwork));
        let stopped = m.entries[1].view.rule.id.clone();
        m.stop_runtime(&stopped);
        let mut attempts = 0;
        let mut workers = vec![];
        for _ in 0..10 {
            schedule(
                &mut m.entries,
                &mut m.next,
                now + Duration::from_secs(601),
                |_| Some(NetworkPath::Available),
                |e, _| {
                    assert_ne!(e.view.rule.id, stopped);
                    attempts += 1;
                    let tx = simulated_worker(e);
                    tx.send(Event::Status(State::Running, String::new()))
                        .unwrap();
                    workers.push(tx);
                },
            );
            m.drain(now + Duration::from_secs(601));
        }
        assert_eq!(attempts, 1);
        assert_eq!(m.entries[0].view.state, State::Running);
        assert_eq!(m.entries[1].view.state, State::Stopped);
    }
    #[test]
    fn recovery_limits_connections_and_isolates_unreachable_targets() {
        let hosts = ["vpn", "lan", "lan", "a", "b", "c", "d"];
        let mut m = Manager {
            entries: hosts.iter().map(|h| pending_entry(h)).collect(),
            ..Default::default()
        };
        let mut launched = vec![];
        let now = Instant::now();
        for _ in 0..3 {
            schedule(
                &mut m.entries,
                &mut m.next,
                now,
                |h| {
                    Some(if h == "vpn" {
                        NetworkPath::Offline
                    } else {
                        NetworkPath::Available
                    })
                },
                |e, _| launched.push((e.view.rule.host.address.clone(), simulated_worker(e))),
            );
        }
        assert_eq!(launched.len(), 4);
        assert_eq!(launched.iter().filter(|(name, _)| name == "lan").count(), 1);
        launched[0]
            .1
            .send(Event::Status(State::Running, String::new()))
            .unwrap();
        m.drain(now);
        schedule(
            &mut m.entries,
            &mut m.next,
            now,
            |_| Some(NetworkPath::Available),
            |e, _| launched.push((e.view.rule.host.address.clone(), simulated_worker(e))),
        );
        assert_eq!(launched.len(), 5);
    }
    #[test]
    fn offline_does_not_wake_manual_only_or_attention_rules() {
        let mut entries = vec![pending_entry("manual"), pending_entry("rejected")];
        entries[0].view.rule.retry = false;
        entries[1].view.state = State::Attention;
        entries[1].pending = None;
        let mut next = 0;
        for path in [NetworkPath::Offline, NetworkPath::Available] {
            schedule(
                &mut entries,
                &mut next,
                Instant::now(),
                |_| Some(path),
                |_, _| panic!("unexpected automatic retry"),
            );
        }
        assert_eq!(entries[0].view.state, State::Failed);
        assert_eq!(entries[1].view.state, State::Attention);
    }
    #[test]
    fn disabling_reconnect_cancels_waiting_but_keeps_established_tunnel() {
        let dir = tempfile::tempdir().unwrap();
        let mut m = Manager {
            data_path: dir.path().join("forwarding.json"),
            ..Default::default()
        };
        m.load();
        let mut e = pending_entry("lan");
        let tx = simulated_worker(&mut e);
        e.view.state = State::Running;
        m.entries.push(e);
        let mut edit = m.entries[0].view.rule.clone();
        edit.retry = false;
        m.save(edit).unwrap();
        assert!(!m.entries[0]
            .worker
            .as_ref()
            .unwrap()
            .cancel
            .load(Ordering::Relaxed));
        tx.send(Event::Status(State::Failed, "connection lost".into()))
            .unwrap();
        tx.send(Event::Done).unwrap();
        m.drain(Instant::now());
        assert_eq!(m.entries[0].view.state, State::Failed);
        assert!(m.entries[0].pending.is_none());
        let mut waiting = pending_entry("vpn");
        waiting.view.state = State::WaitingForNetwork;
        let mut edit = waiting.view.rule.clone();
        edit.retry = false;
        m.entries.push(waiting);
        m.save(edit).unwrap();
        assert!(!m.entries[1].view.rule.desired);
        assert!(m.entries[1].pending.is_none());
    }
    #[test]
    fn network_cancellation_preserves_confirmed_auth_failure() {
        let mut m = Manager::default();
        let mut e = pending_entry("vpn");
        let tx = simulated_worker(&mut e);
        m.entries.push(e);
        schedule(
            &mut m.entries,
            &mut m.next,
            Instant::now(),
            |_| Some(NetworkPath::Offline),
            |_, _| unreachable!(),
        );
        tx.send(Event::Status(
            State::Attention,
            "Authentication rejected".into(),
        ))
        .unwrap();
        tx.send(Event::Done).unwrap();
        m.drain(Instant::now());
        assert_eq!(m.entries[0].view.state, State::Attention);
        assert!(m.entries[0].pending.is_none());
    }
    #[cfg(windows)]
    #[test]
    fn windows_observer_preserves_ipv4_and_ipv6_loopback() {
        wezterm_ssh::forward_network::revision();
        for addr in ["127.0.0.1", "[::1]"] {
            assert_eq!(
                wezterm_ssh::forward_network::path_to(addr),
                NetworkPath::Available
            );
        }
    }

    #[test]
    fn persistence_excludes_runtime() {
        let json = serde_json::to_string(&rule()).unwrap();
        assert!(!json.contains("password"));
        assert!(json.contains("\"desired\":false"));
        assert!(!json.contains("running"));
    }
    #[test]
    fn stop_cancels_pending_restart_and_stale_results() {
        let mut e = entry_at(rule(), &path());
        let (tx, rx) = mpsc::channel();
        e.worker = Some(Worker {
            cancel: Arc::new(AtomicBool::new(false)),
            events: rx,
        });
        e.pending = Some(ConfigMap::new());
        let id = e.view.rule.id.clone();
        let mut m = Manager {
            entries: vec![e],
            loaded: true,
            ..Manager::default()
        };
        m.stop_runtime(&id);
        tx.send(Event::Status(State::Running, String::new()))
            .unwrap();
        tx.send(Event::Done).unwrap();
        m.tick();
        assert_eq!(m.entries[0].view.state, State::Stopped);
        assert!(m.entries[0].pending.is_none());
    }
    #[test]
    fn desired_state_survives_restart_and_manual_stop_persists() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("forwarding.json");
        let mut m = Manager {
            data_path: file.clone(),
            ..Manager::default()
        };
        m.load();
        let r = rule();
        let id = r.id.clone();
        m.save(r).unwrap();
        m.set_desired(&id, true).unwrap();
        drop(m);
        let mut m = Manager {
            data_path: file.clone(),
            ..Manager::default()
        };
        m.load();
        assert!(m.entries[0].view.rule.desired);
        m.stop(&id).unwrap();
        drop(m);
        let mut m = Manager {
            data_path: file,
            ..Manager::default()
        };
        assert!(!m.views()[0].rule.desired);
        assert!(m.entries[0].worker.is_none());
        assert!(m.resumed);
    }
    #[test]
    fn another_local_instance_cannot_recover_or_rewrite_owned_rules() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("forwarding.json");
        let mut first = Manager {
            data_path: file.clone(),
            ..Manager::default()
        };
        first.load();
        first.save(rule()).unwrap();
        let mut second = Manager {
            data_path: file.clone(),
            ..Manager::default()
        };
        assert!(second.views().is_empty());
        assert!(second.load_error.contains("another TermViAI instance"));
        assert!(second.save(rule()).is_err());
        drop(first);
        drop(second);
        let mut next = Manager {
            data_path: file,
            ..Manager::default()
        };
        next.load();
        assert_eq!(next.entries.len(), 1);
        assert!(next.load_error.is_empty());
    }
    #[test]
    fn ownership_is_scoped_to_destination_and_legacy_intent_is_unknown() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("owner.json");
        let mut r = rule();
        let owner = Owner {
            pid: 42,
            token: "boot:123".into(),
            command: "sshd: fixture@notty".into(),
            eligible: true,
        };
        save_ownership(&file, &r, Some(owner.clone())).unwrap();
        assert_eq!(read_ownership(&file, &r).unwrap(), Some(owner));
        r.remote_port += 1;
        assert!(read_ownership(&file, &r).unwrap().is_none());
        r.remote_port -= 1;
        r.host.address = "other-host".into();
        assert!(read_ownership(&file, &r).unwrap().is_none());
        let mut legacy = serde_json::to_value(rule()).unwrap();
        legacy.as_object_mut().unwrap().remove("desired");
        assert!(!serde_json::from_value::<Rule>(legacy).unwrap().desired);
        std::fs::write(&file, b"broken").unwrap();
        assert!(read_ownership(&file, &r).is_err());
    }
    #[test]
    fn registration_is_committed_before_listener_and_failure_blocks_listen() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("record.json");
        let r = rule();
        let warning = Arc::new(Mutex::new(String::new()));
        let setup = ownership_setup(r.clone(), file.clone(), warning.clone()).unwrap();
        (setup.commit)(Ok(
            r#"{"session":{"pid":42,"token":"boot:1","command":"sshd: test","eligible":true}}"#
                .into(),
        ))
        .unwrap();
        assert_eq!(read_ownership(&file, &r).unwrap().unwrap().pid, 42);
        let setup = ownership_setup(r.clone(), file.clone(), warning.clone()).unwrap();
        (setup.commit)(Err(anyhow::anyhow!("Python unavailable"))).unwrap();
        assert!(!file.exists());
        assert!(warning.lock().unwrap().contains("unavailable"));
        let blocker = dir.path().join("not-a-directory");
        std::fs::write(&blocker, b"file").unwrap();
        let setup = ownership_setup(r, blocker.join("record.json"), warning).unwrap();
        assert!((setup.commit)(Ok(
            r#"{"session":{"pid":42,"token":"boot:1","command":"sshd: test","eligible":true}}"#
                .into()
        ))
        .is_err());
    }
    #[test]
    fn startup_restore_does_not_require_a_terminal_and_is_attempted_once() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("forwarding.json");
        let mut r = rule();
        r.desired = true;
        r.host.authentication = Some(crate::hosts::Authentication::Identity);
        r.host.identity_id = "missing-test-identity-0cbb62d7".into();
        std::fs::write(&file, serde_json::to_vec(&vec![r]).unwrap()).unwrap();
        let mut m = Manager {
            data_path: file,
            ..Manager::default()
        };
        let views = m.views();
        assert_eq!(views[0].state, State::Attention);
        assert!(views[0].rule.desired);
        assert!(m.entries[0].worker.is_none()); // missing identity fails before any SSH connection
        m.entries[0].view.error = "already attempted".into();
        assert_eq!(m.views()[0].error, "already attempted");
    }
    fn fixture_config() -> (u16, ConfigMap) {
        let port: u16 = std::env::var("TERMVIAI_TEST_SSH_PORT")
            .unwrap()
            .parse()
            .unwrap();
        let mut cfg = wezterm_ssh::Config::new().for_host("127.0.0.1");
        for (k, v) in [
            ("hostname", "127.0.0.1".into()),
            ("port", port.to_string()),
            ("user", std::env::var("TERMVIAI_TEST_SSH_USER").unwrap()),
            (
                "termviai_identity_file",
                std::env::var("TERMVIAI_TEST_SSH_KEY").unwrap(),
            ),
            (
                "userknownhostsfile",
                std::env::var("TERMVIAI_TEST_SSH_KNOWN").unwrap(),
            ),
            ("termviai_auth_mode", "key".into()),
            ("termviai_no_ssh_config", "yes".into()),
            ("wezterm_ssh_backend", "libssh".into()),
            ("connecttimeout", "5".into()),
        ] {
            cfg.insert(k.into(), v);
        }
        (port, cfg)
    }

    #[test]
    #[ignore = "requires disposable loopback SSH fixture"]
    fn recovered_route_establishes_real_listener_and_manual_stop_survives_recovery() {
        let (port, cfg) = fixture_config();
        let dir = tempfile::tempdir().unwrap();
        let mut m = Manager {
            data_path: dir.path().join("forwarding.json"),
            ..Default::default()
        };
        m.load();
        let mut r = rule();
        r.host.address = "127.0.0.1".into();
        r.host.port = port;
        r.remote_port = std::env::var("TERMVIAI_TEST_REMOTE_PORT")
            .unwrap()
            .parse()
            .unwrap();
        let service = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        r.local_port = service.local_addr().unwrap().port();
        let remote = SocketAddr::from(([127, 0, 0, 1], r.remote_port));
        let id = m.save(r).unwrap();
        m.set_desired(&id, true).unwrap();
        m.entries[0].pending = Some(cfg);
        let mut now = Instant::now();
        for _ in 0..20 {
            schedule(
                &mut m.entries,
                &mut m.next,
                now,
                |_| Some(NetworkPath::Offline),
                |_, _| panic!("SSH attempted while offline"),
            );
            now += Duration::from_secs(30);
        }
        assert!(m.entries[0].view.last_attempt.is_none());
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut attempts = 0;
        while m.entries[0].view.state != State::Running {
            m.drain(now);
            schedule(
                &mut m.entries,
                &mut m.next,
                now,
                |_| Some(NetworkPath::Available),
                |e, cfg| {
                    attempts += 1;
                    launch(e, cfg);
                },
            );
            assert!(
                Instant::now() < deadline
                    && !matches!(
                        m.entries[0].view.state,
                        State::Attention | State::Failed | State::Retrying
                    ),
                "{:?} {}",
                m.entries[0].view.state,
                m.entries[0].view.error
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(attempts, 1);
        // The SSH fixture and the native client share loopback reachability.
        // Exercise both directions through the acknowledged remote listener.
        use std::io::Read;
        let timeout = Duration::from_secs(8);
        let until = Instant::now() + timeout;
        let client = std::thread::spawn(move || {
            let mut socket = loop {
                if let Ok(s) = TcpStream::connect_timeout(&remote, Duration::from_millis(250)) {
                    break s;
                }
                assert!(Instant::now() < until, "remote listener not reachable");
                std::thread::sleep(Duration::from_millis(50));
            };
            socket.set_read_timeout(Some(timeout)).unwrap();
            socket.write_all(b"ping").unwrap();
            let mut reply = [0; 4];
            socket.read_exact(&mut reply).unwrap();
            assert_eq!(&reply, b"pong");
        });
        service.set_nonblocking(true).unwrap();
        let mut incoming = loop {
            match service.accept() {
                Ok((socket, _)) => break socket,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => (),
                Err(e) => panic!("local listener failed: {e}"),
            }
            assert!(
                Instant::now() < until,
                "no forwarded connection reached the local service"
            );
            std::thread::sleep(Duration::from_millis(20));
        };
        incoming.set_nonblocking(false).unwrap();
        incoming.set_read_timeout(Some(timeout)).unwrap();
        let mut payload = [0; 4];
        incoming.read_exact(&mut payload).unwrap();
        assert_eq!(&payload, b"ping");
        incoming.write_all(b"pong").unwrap();
        client.join().unwrap();
        m.stop(&id).unwrap();
        let until = Instant::now() + Duration::from_secs(5);
        while m.entries[0].worker.is_some() {
            assert!(Instant::now() < until);
            m.drain(now);
            std::thread::sleep(Duration::from_millis(20));
        }
        for path in [NetworkPath::Offline, NetworkPath::Available] {
            schedule(
                &mut m.entries,
                &mut m.next,
                now,
                |_| Some(path),
                |_, _| panic!("manual stop lost"),
            );
        }
        assert_eq!(m.entries[0].view.state, State::Stopped);
        assert!(!m.entries[0].view.rule.desired);
    }

    #[test]
    #[ignore = "requires disposable loopback SSH fixture; never use production hosts"]
    fn owned_stale_listener_recovers_without_user_interaction() {
        let (port, mut cfg) = fixture_config();
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("forwarding.json");
        let mut r = rule();
        r.host.address = "127.0.0.1".into();
        r.host.port = port;
        r.remote_port = std::env::var("TERMVIAI_TEST_REMOTE_PORT")
            .unwrap()
            .parse::<u16>()
            .unwrap()
            + 1;
        r.desired = true;
        let e = entry_at(r.clone(), &file);
        let owner_file = e.ownership_file.clone();
        let old_cancel = Arc::new(AtomicBool::new(false));
        struct Cancel(Arc<AtomicBool>);
        impl Drop for Cancel {
            fn drop(&mut self) {
                self.0.store(true, Ordering::Relaxed);
            }
        }
        let _cleanup = Cancel(old_cancel.clone());
        let old = connect(cfg.clone(), &old_cancel, None).unwrap();
        let warning = Arc::new(Mutex::new(String::new()));
        let setup = ownership_setup(r.clone(), owner_file.clone(), warning.clone()).unwrap();
        let (ready, rx) = mpsc::channel();
        let stop = old_cancel.clone();
        let old_rule = r.clone();
        let old_thread = std::thread::spawn(move || {
            smol::block_on(old.run_remote_forward(wezterm_ssh::RemoteForward {
                bind: old_rule.bind().into(),
                remote_port: old_rule.remote_port,
                local_port: old_rule.local_port,
                cancel: stop,
                ready,
                setup: Some(setup),
            }))
        });
        rx.recv_timeout(Duration::from_secs(30)).unwrap();
        assert!(
            warning.lock().unwrap().is_empty(),
            "{}",
            warning.lock().unwrap()
        );
        let original = read_ownership(&owner_file, &r).unwrap().unwrap();
        // An unrelated/reused identity must remain untouched even in automatic mode.
        let mut wrong = original.clone();
        wrong.token = "another-boot:1".into();
        assert!(inspect(
            cfg.clone(),
            r.clone(),
            Some(wrong),
            true,
            &AtomicBool::new(false),
            None
        )
        .is_err());
        let before = inspect(
            cfg.clone(),
            r.clone(),
            None,
            false,
            &AtomicBool::new(false),
            None,
        )
        .unwrap();
        assert_eq!(before.owner.unwrap().pid, original.pid);
        // Model a new process restoring persisted data while the old remote listener lingers.
        let mut replacement = entry_at(r.clone(), &file);
        launch(&mut replacement, cfg.clone());
        let until = Instant::now() + Duration::from_secs(45);
        loop {
            match replacement
                .worker
                .as_ref()
                .unwrap()
                .events
                .recv_timeout(Duration::from_secs(1))
            {
                Ok(Event::Status(State::Running, warning)) => {
                    assert!(warning.is_empty(), "{}", warning);
                    break;
                }
                Ok(Event::Status(State::Attention | State::Failed, error)) => {
                    panic!("recovery: {}", error)
                }
                Ok(Event::Done) => panic!("recovery worker ended before running"),
                _ => (),
            }
            assert!(Instant::now() < until, "automatic recovery timeout");
        }
        assert!(old_thread.join().unwrap().is_err());
        let current = read_ownership(&owner_file, &r).unwrap().unwrap();
        assert!(current.pid != original.pid || current.token != original.token);
        let live = inspect(
            cfg.clone(),
            r.clone(),
            None,
            false,
            &AtomicBool::new(false),
            None,
        )
        .unwrap();
        assert_eq!(live.owner.unwrap().pid, current.pid);
        replacement
            .worker
            .as_ref()
            .unwrap()
            .cancel
            .store(true, Ordering::Relaxed);
        loop {
            if matches!(
                replacement
                    .worker
                    .as_ref()
                    .unwrap()
                    .events
                    .recv_timeout(Duration::from_secs(10))
                    .unwrap(),
                Event::Done
            ) {
                break;
            }
        }
        assert!(
            !inspect(
                cfg.clone(),
                r.clone(),
                None,
                false,
                &AtomicBool::new(false),
                None
            )
            .unwrap()
            .occupied
        );
        if let Ok(port) = std::env::var("TERMVIAI_TEST_NO_EXEC_PORT") {
            // A second disposable sshd rejects exec but permits TCP forwarding.
            // Verify the manager reconnects on a fresh transport without a claim.
            let mut r = r;
            r.host.port = port.parse().unwrap();
            cfg.insert("port".into(), port);
            let fallback_dir = tempfile::tempdir().unwrap();
            let mut e = entry_at(r, &fallback_dir.path().join("forwarding.json"));
            launch(&mut e, cfg);
            let until = Instant::now() + Duration::from_secs(30);
            loop {
                match e
                    .worker
                    .as_ref()
                    .unwrap()
                    .events
                    .recv_timeout(Duration::from_secs(1))
                {
                    Ok(Event::Status(State::Running, warning)) => {
                        assert!(warning.contains("recovery unavailable"));
                        break;
                    }
                    Ok(Event::Status(State::Attention | State::Failed, error)) => {
                        panic!("fallback: {}", error)
                    }
                    Ok(Event::Done) => panic!("fallback ended before running"),
                    _ => (),
                }
                assert!(
                    Instant::now() < until,
                    "ordinary-forwarding fallback timeout"
                );
            }
            assert!(!e.ownership_file.exists());
            e.worker
                .as_ref()
                .unwrap()
                .cancel
                .store(true, Ordering::Relaxed);
            while !matches!(
                e.worker
                    .as_ref()
                    .unwrap()
                    .events
                    .recv_timeout(Duration::from_secs(10))
                    .unwrap(),
                Event::Done
            ) {}
        }
    }
    #[test]
    fn authentication_rejections_need_attention_but_network_errors_retry() {
        assert!(needs_attention(&anyhow::anyhow!(
            "The selected SSH key was not accepted: Denied"
        )));
        assert!(needs_attention(&anyhow::anyhow!(
            "unhandled auth case; methods=PUBLIC_KEY"
        )));
        assert!(!needs_attention(&anyhow::anyhow!(
            "Connection reset by peer"
        )));
    }
    #[test]
    fn control_response_waits_for_complete_record_across_ssh_reads() {
        let data = b"banner\nTERMVIAI_FORWARD_V1={\"released\":false,\"occupied\":false,\"controllable\":false,\"reason\":\"empty\",\"owner\":null}\n";
        for end in 0..data.len() {
            assert!(control_result(&data[..end]).unwrap().is_none());
        }
        assert!(!control_result(data).unwrap().unwrap().occupied);
        assert!(
            control_result(b"TERMVIAI_FORWARD_V1={\"error\":\"permission denied\"}\n").is_err()
        );
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Owner {
    pub pid: u32,
    pub token: String,
    pub command: String,
    pub eligible: bool,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Inspection {
    pub released: bool,
    pub occupied: bool,
    pub controllable: bool,
    pub reason: String,
    pub owner: Option<Owner>,
}
struct ControlJob {
    cancel: Arc<AtomicBool>,
    rx: mpsc::Receiver<anyhow::Result<Inspection>>,
    restart: Option<ConfigMap>,
}
impl Drop for ControlJob {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
fn control_command(port: u16, target: Option<Owner>, automatic: bool) -> anyhow::Result<String> {
    remote_command(serde_json::json!({"port":port,"target":target,"automatic":automatic}))
}
fn remote_command(request: serde_json::Value) -> anyhow::Result<String> {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD;
    let script=format!("ns={{'__name__':'forward_helpers'}};exec(__import__('base64').b64decode('{}'),ns);globals().update({{k:v for k,v in ns.items() if not k.startswith('__')}});exec(__import__('base64').b64decode('{}'))",b64.encode(include_str!("monitor_collect.py")),b64.encode(include_str!("forward_control.py")));
    Ok(format!(
        "python3 -c 'import base64;exec(base64.b64decode(\"{}\"))' '{}'",
        b64.encode(script),
        b64.encode(serde_json::to_vec(&request)?)
    ))
}
fn control_result(stdout: &[u8]) -> anyhow::Result<Option<Inspection>> {
    // A single SSH read may end in the middle of a JSON record.
    for line in stdout
        .split_inclusive(|b| *b == b'\n')
        .filter(|line| line.ends_with(b"\n"))
    {
        if let Some(record) = line.strip_prefix(b"TERMVIAI_FORWARD_V1=") {
            let data: serde_json::Value = serde_json::from_slice(record)?;
            if let Some(error) = data.get("error").and_then(|v| v.as_str()) {
                anyhow::bail!("{error}");
            }
            return Ok(Some(serde_json::from_value(data)?));
        }
    }
    Ok(None)
}
fn inspect(
    cfg: ConfigMap,
    rule: Rule,
    target: Option<Owner>,
    automatic: bool,
    cancel: &AtomicBool,
    events: Option<&mpsc::Sender<Event>>,
) -> anyhow::Result<Inspection> {
    use portable_pty::{Child, ChildKiller};
    use std::io::Read;
    let session = connect(cfg, cancel, events)?;
    ensure!(!cancel.load(Ordering::Relaxed), "Cancelled");
    let command = control_command(rule.remote_port, target, automatic)?;
    let mut exec = smol::block_on(smol::future::or(session.exec(&command, None), async {
        smol::Timer::after(Duration::from_secs(12)).await;
        anyhow::bail!("SSH inspection request timed out")
    }))?;
    exec.stdout.set_non_blocking(true)?;
    exec.stderr.set_non_blocking(true)?;
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut stdout = vec![];
    let mut stderr = vec![];
    loop {
        if cancel.load(Ordering::Relaxed) || Instant::now() > deadline {
            exec.child.kill().ok();
            anyhow::bail!("Inspection interrupted; if termination was requested, its result is uncertain. Inspect again before retrying.")
        }
        let mut eof = true;
        for (fd, data) in [
            (&mut exec.stdout, &mut stdout),
            (&mut exec.stderr, &mut stderr),
        ] {
            let mut buf = [0; 8192];
            match fd.read(&mut buf) {
                Ok(n) => {
                    data.extend_from_slice(&buf[..n]);
                    eof &= n == 0;
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => eof = false,
                Err(e) => return Err(e.into()),
            }
            ensure!(
                data.len() < 512000,
                "Remote inspection output exceeds limit"
            );
        }
        if let Some(result) =
            control_result(&stdout).map_err(|e| if automatic { e.context(Attention) } else { e })?
        {
            return Ok(result);
        }
        if exec.child.try_wait()?.is_some() && eof {
            anyhow::bail!(
                "Remote inspection ended: {}",
                String::from_utf8_lossy(&stderr)
            );
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}
impl Manager {
    pub fn inspect(
        &mut self,
        id: &str,
        store: &HostStore,
        target: Option<Owner>,
    ) -> anyhow::Result<()> {
        let e = self
            .entries
            .iter_mut()
            .find(|e| e.view.rule.id == id)
            .context("Rule removed")?;
        ensure!(e.control.is_none(), "An inspection is already in progress");
        ensure!(
            !e.view.state.active(),
            "Stop this tunnel before inspecting its listener"
        );
        if let Some(target) = &target {
            let info = e
                .view
                .inspection
                .as_ref()
                .filter(|i| i.controllable)
                .context("Inspect the listener first")?;
            ensure!(
                info.owner.as_ref() == Some(target),
                "Listener inspection changed; confirm again"
            );
        }
        if target.is_some() {
            self.set_desired(id, true)?;
        }
        let e = self
            .entries
            .iter_mut()
            .find(|e| e.view.rule.id == id)
            .unwrap();
        let cfg = connection_config(&e.view.rule.host, store)?;
        let restart = target.is_some().then(|| cfg.clone());
        let rule = e.view.rule.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        let (tx, rx) = mpsc::channel();
        e.view.busy = true;
        e.view.error.clear();
        e.control = Some(ControlJob {
            cancel,
            rx,
            restart,
        });
        std::thread::spawn(move || {
            let result = inspect(cfg, rule, target, false, &stop, None);
            tx.send(result).ok();
        });
        Ok(())
    }
}
