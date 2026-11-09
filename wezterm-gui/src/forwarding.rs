//! Application-owned reverse tunnels. Only opaque Host credential references are persisted.
use crate::hosts::{new_credential_id, Host, HostStore};
use anyhow::{ensure, Context};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::net::{SocketAddr, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
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
            Self::Starting | Self::Running | Self::Retrying | Self::Stopping
        )
    }
}
#[derive(Clone)]
pub struct View {
    pub rule: Rule,
    pub state: State,
    pub error: String,
    pub attempt: usize,
    pub service: Option<bool>,
    pub inspection: Option<Inspection>,
    pub busy: bool,
}
struct Worker {
    cancel: Arc<AtomicBool>,
    events: mpsc::Receiver<Event>,
}
enum Event {
    Status(State, String, usize),
    Done,
}
struct Entry {
    view: View,
    worker: Option<Worker>,
    pending: Option<ConfigMap>,
    control: Option<ControlJob>,
    probe: Option<mpsc::Receiver<bool>>,
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
        if let Some(r) = rules.iter_mut().find(|r| r.id == rule.id) {
            *r = rule.clone()
        } else {
            ensure!(rules.len() < 512, "Maximum 512 rules");
            rules.push(rule.clone())
        }
        self.persist(&rules)?;
        let id = rule.id.clone();
        if let Some(e) = self.entries.iter_mut().find(|e| e.view.rule.id == id) {
            if let Some(w) = &e.worker {
                w.cancel.store(true, Ordering::Relaxed);
            }
            e.pending = None;
            e.control = None;
            e.view.busy = false;
            e.view.inspection = None;
            e.probe = None;
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
            e.pending = Some(cfg);
            e.view.state = State::Stopping;
        } else {
            launch(e, cfg)
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
            e.pending = None;
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
                                    launch(e, cfg)
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
            let mut done = false;
            if let Some(w) = &e.worker {
                loop {
                    match w.events.try_recv() {
                        Ok(Event::Status(s, msg, n)) if !w.cancel.load(Ordering::Relaxed) => {
                            e.view.state = s;
                            e.view.error = msg;
                            e.view.attempt = n;
                        }
                        Ok(Event::Status(..)) => (),
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
                if let Some(cfg) = e.pending.take() {
                    launch(e, cfg)
                } else if cancelled {
                    e.view.state = State::Stopped;
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
        view: View {
            rule,
            state: State::Stopped,
            error: String::new(),
            attempt: 0,
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
fn connect(cfg: ConfigMap, cancel: &AtomicBool) -> anyhow::Result<Session> {
    let (session, events) = Session::connect(cfg)?;
    let until = Instant::now() + Duration::from_secs(25);
    loop {
        ensure!(!cancel.load(Ordering::Relaxed), "Cancelled");
        ensure!(Instant::now() < until, "SSH authentication timed out");
        match events.try_recv(){
        Ok(SessionEvent::Authenticated)=>return Ok(session),Ok(SessionEvent::Banner(_))=>(),
        Ok(SessionEvent::HostVerify(e))=>{e.try_answer(false).ok();return Err(anyhow::anyhow!("Verify this host in its terminal before starting forwarding.").context(Attention));},
        Ok(SessionEvent::Authenticate(_))=>return Err(anyhow::anyhow!("Authentication is required. Save this host's password / Identity, or unlock its key in the SSH agent, then retry.").context(Attention)),
        Ok(SessionEvent::HostVerificationFailed(e))=>return Err(anyhow::anyhow!("{e}").context(Attention)),
        Ok(SessionEvent::Error(e) | SessionEvent::AuthenticationFailed(e)) =>anyhow::bail!("{e}"),Err(smol::channel::TryRecvError::Closed)=>anyhow::bail!("SSH connection closed"),Err(smol::channel::TryRecvError::Empty)=>std::thread::sleep(Duration::from_millis(25)),
    }
    }
}
fn retry_delay(attempt: usize) -> Option<Duration> {
    [1, 2, 4, 8, 16, 30]
        .get(attempt)
        .map(|s| Duration::from_secs(*s))
}
fn needs_attention(error: &anyhow::Error) -> bool {
    let message = format!("{error:#}");
    error.downcast_ref::<Attention>().is_some()
        || [
            "Remote listener refused",
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
fn launch(e: &mut Entry, cfg: ConfigMap) {
    let cancel = Arc::new(AtomicBool::new(false));
    let stop = cancel.clone();
    let (tx, rx) = mpsc::channel();
    let rule = e.view.rule.clone();
    let ownership_file = e.ownership_file.clone();
    e.view.state = State::Starting;
    e.view.error.clear();
    e.view.attempt = 0;
    e.view.service = None;
    e.worker = Some(Worker { cancel, events: rx });
    std::thread::spawn(move || {
        for attempt in 0..=6 {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            tx.send(Event::Status(State::Starting, String::new(), attempt))
                .ok();
            let result = (|| -> anyhow::Result<()> {
                if let Some(owner) =
                    read_ownership(&ownership_file, &rule).map_err(|e| e.context(Attention))?
                {
                    tx.send(Event::Status(
                        State::Starting,
                        "Checking the previous TermViAI SSH session…".into(),
                        attempt,
                    ))
                    .ok();
                    inspect(cfg.clone(), rule.clone(), Some(owner), true, &stop)?;
                }
                ensure!(!stop.load(Ordering::Relaxed), "Cancelled");
                let session = connect(cfg.clone(), &stop)?;
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
                smol::block_on(async {
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
                            let fresh = connect(cfg.clone(), &stop)?;
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
                                    attempt,
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
                })
            })();
            if stop.load(Ordering::Relaxed) {
                break;
            }
            let error = result
                .err()
                .unwrap_or_else(|| anyhow::anyhow!("SSH forwarding ended"));
            let message = format!("{error:#}");
            if needs_attention(&error) {
                tx.send(Event::Status(State::Attention, message, attempt))
                    .ok();
                break;
            }
            if let Some(delay) = retry_delay(attempt).filter(|_| rule.retry) {
                tx.send(Event::Status(
                    State::Retrying,
                    format!(
                        "{message}\nRetry {} / 6 in {}s",
                        attempt + 1,
                        delay.as_secs()
                    ),
                    attempt + 1,
                ))
                .ok();
                let until = Instant::now() + delay;
                while Instant::now() < until && !stop.load(Ordering::Relaxed) {
                    std::thread::sleep(Duration::from_millis(25));
                }
            } else {
                tx.send(Event::Status(State::Failed, message, attempt)).ok();
                break;
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
    fn retry_is_bounded_and_persistence_excludes_runtime() {
        assert_eq!(
            (0..7)
                .filter_map(retry_delay)
                .map(|d| d.as_secs())
                .collect::<Vec<_>>(),
            [1, 2, 4, 8, 16, 30]
        );
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
        tx.send(Event::Status(State::Running, String::new(), 0))
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
    #[test]
    #[ignore = "requires disposable loopback SSH fixture; never use production hosts"]
    fn owned_stale_listener_recovers_without_user_interaction() {
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
        let old = connect(cfg.clone(), &old_cancel).unwrap();
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
            &AtomicBool::new(false)
        )
        .is_err());
        let before = inspect(cfg.clone(), r.clone(), None, false, &AtomicBool::new(false)).unwrap();
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
                Ok(Event::Status(State::Running, warning, _)) => {
                    assert!(warning.is_empty(), "{}", warning);
                    break;
                }
                Ok(Event::Status(State::Attention | State::Failed, error, _)) => {
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
        let live = inspect(cfg.clone(), r.clone(), None, false, &AtomicBool::new(false)).unwrap();
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
            !inspect(cfg.clone(), r.clone(), None, false, &AtomicBool::new(false))
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
                    Ok(Event::Status(State::Running, warning, _)) => {
                        assert!(warning.contains("recovery unavailable"));
                        break;
                    }
                    Ok(Event::Status(State::Attention | State::Failed, error, _)) => {
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
) -> anyhow::Result<Inspection> {
    use portable_pty::{Child, ChildKiller};
    use std::io::Read;
    let session = connect(cfg, cancel)?;
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
            let result = inspect(cfg, rule, target, false, &stop);
            tx.send(result).ok();
        });
        Ok(())
    }
}
