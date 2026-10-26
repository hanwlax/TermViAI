//! Isolated SSH exec sampling. Never reuses a terminal channel or its input stream.
use crate::hosts::{Host, HostStore};
use anyhow::{bail, ensure, Context};
use base64::Engine;
use filedescriptor::FileDescriptor;
use portable_pty::Child;
#[cfg(test)]
use portable_pty::ChildKiller;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};
use wezterm_ssh::{ConfigMap, Session, SessionEvent};

#[derive(Clone, Debug, Deserialize)]
pub struct Die {
    pub npu: u32,
    pub die: u32,
    pub used_mb: Option<f64>,
    pub total_mb: Option<f64>,
    pub core: Option<f64>,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Process {
    pub npu: u32,
    pub die: u32,
    pub pid: u32,
    pub name: String,
    pub memory_mb: Option<f64>,
    pub token: String,
    pub command: String,
    #[serde(default)]
    pub cwd: String,
    pub owner: String,
    pub controllable: bool,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Snapshot {
    pub dies: Vec<Die>,
    pub processes: Vec<Process>,
    pub processes_available: bool,
    pub stop_reason: String,
}
impl Snapshot {
    pub fn cards(&self) -> Vec<u32> {
        self.dies
            .iter()
            .map(|d| d.npu)
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect()
    }
    pub fn processes(&self, npu: Option<u32>, die: Option<u32>) -> Vec<Process> {
        let mut unique = BTreeMap::new();
        for p in self
            .processes
            .iter()
            .filter(|p| npu.map_or(true, |n| n == p.npu) && die.map_or(true, |d| d == p.die))
        {
            unique.entry(p.pid).or_insert_with(|| p.clone());
        }
        unique.into_values().collect()
    }
    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            !self.dies.is_empty() && self.dies.len() <= 512,
            "Invalid device topology"
        );
        ensure!(self.processes.len() <= 10000, "Too many process rows");
        let ids: std::collections::HashSet<_> = self.dies.iter().map(|d| (d.npu, d.die)).collect();
        ensure!(ids.len() == self.dies.len(), "Duplicate device ids");
        for d in &self.dies {
            ensure!(
                [d.used_mb, d.total_mb, d.core]
                    .iter()
                    .flatten()
                    .all(|v| v.is_finite() && *v >= 0.),
                "Invalid metric"
            );
            ensure!(d.core.map_or(true, |c| c <= 100.), "Invalid utilization");
        }
        for p in &self.processes {
            ensure!(
                ids.contains(&(p.npu, p.die)) && p.pid > 1,
                "Invalid process device / PID"
            );
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct Target {
    pub pid: u32,
    pub token: String,
}
#[derive(Clone, Debug, Deserialize)]
pub struct StopResult {
    pub pid: u32,
    pub status: String,
}
#[derive(Deserialize)]
struct Response {
    snapshot: Option<Snapshot>,
    results: Option<Vec<StopResult>>,
    error: Option<String>,
}
fn response(text: &str) -> anyhow::Result<Response> {
    let line = text
        .lines()
        .rev()
        .find_map(|l| l.strip_prefix("TERMVIAI_MONITOR_V1="))
        .context(
            "No monitor data returned. The remote host requires Linux, python3 and npu-smi.",
        )?;
    let result: Response = serde_json::from_str(line).context("Invalid monitor response")?;
    if let Some(e) = &result.error {
        bail!("{}", e);
    }
    if let Some(s) = &result.snapshot {
        s.validate()?;
    }
    ensure!(
        result.snapshot.is_some() || result.results.is_some(),
        "Empty monitor response"
    );
    Ok(result)
}
fn command(targets: Option<&[Target]>) -> anyhow::Result<String> {
    let request = if let Some(targets) = targets {
        ensure!(
            !targets.is_empty() && targets.iter().all(|t| t.pid > 1 && !t.token.is_empty()),
            "Invalid process selection"
        );
        serde_json::json!({"targets":targets})
    } else {
        serde_json::json!({})
    };
    let b64 = base64::engine::general_purpose::STANDARD;
    Ok(format!(
        "python3 -c 'import base64;exec(base64.b64decode(\"{}\"))' '{}'",
        b64.encode(include_str!("monitor_collect.py")),
        b64.encode(serde_json::to_vec(&request)?)
    ))
}
fn connect(config: ConfigMap, cancel: &AtomicBool) -> anyhow::Result<Session> {
    let (session, events) = Session::connect(config)?;
    let deadline = Instant::now() + Duration::from_secs(25);
    loop {
        ensure!(!cancel.load(Ordering::Relaxed), "Monitoring stopped");
        ensure!(Instant::now() < deadline, "SSH authentication timed out");
        match events.try_recv() {
            Ok(SessionEvent::Authenticated) => return Ok(session),
            Ok(SessionEvent::Banner(_)) => (),
            Ok(SessionEvent::HostVerify(event)) => {
                let message = event.message.clone();
                event.try_answer(false).ok();
                bail!("{} Open this Host once from Hosts to verify its fingerprint, then retry monitoring.", message);
            }
            Ok(SessionEvent::Authenticate(_)) => bail!("Interactive authentication is required. Save a password / Identity or unlock the key in your SSH agent, then retry."),
            Ok(SessionEvent::HostVerificationFailed(e)) => bail!("{}", e),
            Ok(SessionEvent::Error(e)) => bail!("{}", e),
            Err(smol::channel::TryRecvError::Closed) => bail!("SSH authentication ended"),
            Err(smol::channel::TryRecvError::Empty) => std::thread::sleep(Duration::from_millis(40)),
        }
    }
}
fn execute(session: &Session, command: &str, cancel: &AtomicBool) -> anyhow::Result<Response> {
    let exec = smol::block_on(smol::future::or(session.exec(command, None), async {
        smol::Timer::after(Duration::from_secs(15)).await;
        bail!("SSH exec timed out")
    }))?;
    collect_output(exec.stdin, exec.stdout, exec.stderr, exec.child, cancel)
}

fn collect_output(
    _stdin: FileDescriptor,
    mut stdout: FileDescriptor,
    mut stderr: FileDescriptor,
    mut child: impl Child,
    cancel: &AtomicBool,
) -> anyhow::Result<Response> {
    // Keep stdin alive until output and exit status have been collected.
    // SessionInner treats local stdin EOF as channel.close(), not SSH send_eof().
    // The python -c collector never reads stdin, so no half-close is needed here.
    stdout.set_non_blocking(true)?;
    stderr.set_non_blocking(true)?;
    let deadline = Instant::now() + Duration::from_secs(25);
    let mut out = Vec::new();
    let mut err = Vec::new();
    let mut eof = [false; 2];
    loop {
        if cancel.load(Ordering::Relaxed) || Instant::now() > deadline {
            child.kill().ok();
            bail!("Sampling cancelled or timed out");
        }
        for (i, (fd, data)) in
            IntoIterator::into_iter([(&mut stdout, &mut out), (&mut stderr, &mut err)]).enumerate()
        {
            let mut buf = [0u8; 8192];
            match fd.read(&mut buf) {
                Ok(0) => eof[i] = true,
                Ok(n) => {
                    data.extend_from_slice(&buf[..n]);
                    ensure!(data.len() <= 4_000_000, "Remote output exceeds limit");
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => (),
                Err(e) => return Err(e.into()),
            }
        }
        if eof.iter().all(|v| *v) {
            if let Some(status) = child.try_wait()? {
                ensure!(
                    status.success(),
                    "Remote collector failed (exit {}): {}",
                    status.exit_code(),
                    if err.is_empty() {
                        "SSH command ended without an error message; retry and check the remote Python environment.".into()
                    } else {
                        String::from_utf8_lossy(&err)
                    }
                );
                return response(&String::from_utf8_lossy(&out));
            }
        }
        std::thread::sleep(Duration::from_millis(15));
    }
}
struct Completed {
    response: anyhow::Result<Response>,
    session: Option<Session>,
}
pub struct Entry {
    pub host: Host,
    pub paused: bool,
    pub snapshot: Option<Snapshot>,
    pub error: String,
    pub updated: Option<Instant>,
    pub results: Vec<StopResult>,
    pub stopping: bool,
    pending_stop: Option<Vec<Target>>,
    job_stop: bool,
    session: Option<Session>,
    rx: Option<mpsc::Receiver<Completed>>,
    cancel: Arc<AtomicBool>,
    next: Instant,
}
impl Drop for Entry {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
impl Entry {
    pub fn busy(&self) -> bool {
        self.rx.is_some()
    }
    pub fn live(&self) -> bool {
        !self.paused
            && self.error.is_empty()
            && self
                .updated
                .is_some_and(|t| t.elapsed() < Duration::from_secs(30))
            && !self.stopping
    }
    pub fn status(&self) -> &str {
        if self.stopping {
            "Ending processes"
        } else if self.paused {
            "Paused"
        } else if !self.error.is_empty() {
            "Unavailable"
        } else if self.snapshot.is_some() {
            "Collecting"
        } else {
            "Connecting"
        }
    }
    fn spawn(&mut self, config: ConfigMap, targets: Option<Vec<Target>>) {
        let (tx, rx) = mpsc::sync_channel(1);
        let old = self.session.take();
        let cancel = self.cancel.clone();
        self.stopping = targets.is_some();
        self.job_stop = targets.is_some();
        self.rx = Some(rx);
        std::thread::spawn(move || {
            let mut session = None;
            let response = (|| {
                let s = if let Some(s) = old {
                    s
                } else {
                    connect(config, &cancel)?
                };
                let r = execute(&s, &command(targets.as_deref())?, &cancel)?;
                session = Some(s);
                Ok(r)
            })();
            tx.send(Completed { response, session }).ok();
        });
    }
}
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct Preferences {
    hosts: Vec<Host>,
    interval: u64,
}
pub struct Monitor {
    pub entries: Vec<Entry>,
    pub interval: u64,
    pub load_error: String,
}
impl Default for Monitor {
    fn default() -> Self {
        Self {
            entries: vec![],
            interval: 2,
            load_error: String::new(),
        }
    }
}
impl Monitor {
    pub fn load() -> Self {
        let mut m = Self::default();
        match std::fs::read(HostStore::path().with_file_name("monitor.json")) {
            Ok(data) => match serde_json::from_slice::<Preferences>(&data) {
                Ok(p) => {
                    m.interval = if [2, 5, 10].contains(&p.interval) {
                        p.interval
                    } else {
                        2
                    };
                    for h in p.hosts {
                        m.add(h);
                    }
                }
                Err(e) => {
                    m.load_error = format!(
                        "Monitor preferences could not be read; original file preserved: {e}"
                    )
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => m.load_error = e.to_string(),
        }
        m
    }
    pub fn save(&self) -> anyhow::Result<()> {
        ensure!(self.load_error.is_empty(), "{}", self.load_error);
        let path = HostStore::path().with_file_name("monitor.json");
        std::fs::create_dir_all(path.parent().unwrap())?;
        let mut temp = tempfile::NamedTempFile::new_in(path.parent().unwrap())?;
        serde_json::to_writer_pretty(
            &mut temp,
            &Preferences {
                hosts: self.entries.iter().map(|e| e.host.clone()).collect(),
                interval: self.interval,
            },
        )?;
        temp.as_file().sync_all()?;
        temp.persist(&path).map_err(|e| e.error)?;
        Ok(())
    }
    pub fn contains(&self, h: &Host) -> bool {
        self.entries.iter().any(|e| same_host(&e.host, h))
    }
    pub fn add(&mut self, host: Host) {
        if !self.contains(&host) {
            self.entries.push(Entry {
                host,
                paused: false,
                snapshot: None,
                error: String::new(),
                updated: None,
                results: vec![],
                stopping: false,
                pending_stop: None,
                job_stop: false,
                session: None,
                rx: None,
                cancel: Arc::new(AtomicBool::new(false)),
                next: Instant::now(),
            });
        }
    }
    pub fn retry(&mut self, id: usize) {
        if let Some(e) = self.entries.get_mut(id) {
            e.error.clear();
            e.paused = false;
            e.next = Instant::now();
        }
    }
    pub fn tick(&mut self, store: &HostStore, config: impl Fn(&Host) -> anyhow::Result<ConfigMap>) {
        for e in &mut self.entries {
            if let Some(rx) = &e.rx {
                let done = match rx.try_recv() {
                    Ok(v) => Some(v),
                    Err(mpsc::TryRecvError::Disconnected) => Some(Completed {
                        response: Err(anyhow::anyhow!("Monitor worker ended")),
                        session: None,
                    }),
                    Err(mpsc::TryRecvError::Empty) => None,
                };
                if let Some(done) = done {
                    e.rx = None;
                    e.session = done.session;
                    let stopping = e.job_stop;
                    e.job_stop = false;
                    e.stopping = e.pending_stop.is_some();
                    match done.response {
                        Ok(r) => {
                            if let Some(s) = r.snapshot {
                                if s.processes_available {
                                    for result in &mut e.results {
                                        if result.status == "Exit requested; waiting for process"
                                            && !s.processes.iter().any(|p| p.pid == result.pid)
                                        {
                                            result.status = "No longer reported by NPU".into();
                                        }
                                    }
                                }
                                e.snapshot = Some(s);
                                e.updated = Some(Instant::now());
                            }
                            if let Some(r) = r.results {
                                e.results = r;
                            }
                            e.error.clear();
                        }
                        Err(err) => {
                            e.error = format!("{err:#}");
                            e.pending_stop = None;
                            e.stopping = false;
                        }
                    }
                    e.next = Instant::now()
                        + if stopping {
                            Duration::ZERO
                        } else {
                            Duration::from_secs(self.interval)
                        };
                }
            }
            if !e.busy() {
                if let Some(targets) = e.pending_stop.take() {
                    let verified = e.snapshot.as_ref().is_some_and(|s| {
                        targets.iter().all(|t| {
                            s.processes
                                .iter()
                                .any(|p| p.pid == t.pid && p.token == t.token && p.controllable)
                        })
                    });
                    if !e.paused && e.error.is_empty() && e.session.is_some() && verified {
                        e.spawn(ConfigMap::default(), Some(targets));
                    } else {
                        e.stopping = false;
                        e.error =
                            "Process or connection changed before execution; request cancelled."
                                .into();
                    }
                }
            }
            if !e.paused && !e.busy() && e.error.is_empty() && Instant::now() >= e.next {
                let cfg = (|| {
                    let mut h=store.hosts.iter().find(|h|same_host(h,&e.host)).context("Host changed or was removed. Remove this monitor and add the current Host.")?.clone();
                    h.resolve_identity(store)?;
                    config(&h)
                })();
                match cfg {
                    Ok(cfg) => e.spawn(cfg, None),
                    Err(err) => e.error = format!("{err:#}"),
                }
            }
        }
    }
    pub fn stop(&mut self, id: usize, targets: Vec<Target>) -> anyhow::Result<()> {
        let e = self.entries.get_mut(id).context("Host removed")?;
        ensure!(
            e.live(),
            "Resume sampling and wait for current data before retrying."
        );
        let s = e.snapshot.as_ref().context("No sample")?;
        ensure!(
            !targets.is_empty()
                && targets.iter().all(|t| s
                    .processes
                    .iter()
                    .any(|p| p.pid == t.pid && p.token == t.token && p.controllable)),
            "Process changed or permission unavailable. Refresh before retrying."
        );
        ensure!(
            e.session.is_some() || e.busy(),
            "SSH connection is unavailable"
        );
        e.results.clear();
        if e.busy() {
            e.pending_stop = Some(targets);
            e.stopping = true;
        } else {
            e.spawn(ConfigMap::default(), Some(targets));
        }
        Ok(())
    }
}
pub fn same_host(a: &Host, b: &Host) -> bool {
    a.label == b.label && a.address == b.address && a.port == b.port && a.username == b.username
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Debug)]
    struct CollectorChild {
        stdin_peer: FileDescriptor,
        status: portable_pty::ExitStatus,
    }
    impl ChildKiller for CollectorChild {
        fn kill(&mut self) -> std::io::Result<()> {
            Ok(())
        }
        fn clone_killer(&self) -> Box<dyn ChildKiller + Send + Sync> {
            panic!("unused in collector test")
        }
    }
    impl Child for CollectorChild {
        fn try_wait(&mut self) -> std::io::Result<Option<portable_pty::ExitStatus>> {
            // Match the SSH bridge: EOF here would close the remote command.
            let mut byte = [0];
            assert_eq!(
                self.stdin_peer.read(&mut byte).unwrap_err().kind(),
                std::io::ErrorKind::WouldBlock,
                "collector closed stdin before receiving command completion"
            );
            Ok(Some(self.status.clone()))
        }
        fn wait(&mut self) -> std::io::Result<portable_pty::ExitStatus> {
            Ok(self.status.clone())
        }
        fn process_id(&self) -> Option<u32> {
            None
        }
        #[cfg(windows)]
        fn as_raw_handle(&self) -> Option<std::os::windows::io::RawHandle> {
            None
        }
    }
    fn collector_output(output: &[u8], error: &[u8], exit: u32) -> anyhow::Result<Response> {
        use std::io::Write;
        let (stdin, mut stdin_peer) = filedescriptor::socketpair().unwrap();
        stdin_peer.set_non_blocking(true).unwrap();
        let (mut output_peer, stdout) = filedescriptor::socketpair().unwrap();
        let (mut error_peer, stderr) = filedescriptor::socketpair().unwrap();
        output_peer.write_all(output).unwrap();
        error_peer.write_all(error).unwrap();
        drop(output_peer);
        drop(error_peer);
        collect_output(
            stdin,
            stdout,
            stderr,
            CollectorChild {
                stdin_peer,
                status: portable_pty::ExitStatus::with_exit_code(exit),
            },
            &AtomicBool::new(false),
        )
    }
    #[test]
    fn collector_keeps_stdin_open_until_output_and_exit_arrive() {
        let r = collector_output(b"TERMVIAI_MONITOR_V1={\"results\":[]}\n", b"", 0).unwrap();
        assert!(r.results.unwrap().is_empty());
    }
    #[test]
    fn collector_failure_retains_remote_stderr_and_exit_code() {
        let e = collector_output(b"", b"python3: not found", 127)
            .err()
            .unwrap()
            .to_string();
        assert!(e.contains("exit 127"));
        assert!(e.contains("python3: not found"));
        let e = collector_output(b"", b"", 1).err().unwrap().to_string();
        assert!(e.contains("exit 1"));
        assert!(e.contains("without an error message"));
    }
    #[test]
    fn deduplicates_processes_by_pid_across_dies() {
        let s = fixture();
        assert_eq!(s.processes(None, None).len(), 1);
        assert_eq!(s.processes(Some(4), Some(2)).len(), 1);
        assert!(s.processes(Some(0), None).is_empty());
        s.validate().unwrap();
    }
    fn fixture() -> Snapshot {
        serde_json::from_value(serde_json::json!({"dies":[{"npu":4,"die":2,"used_mb":1,"total_mb":65536,"core":2},{"npu":4,"die":3,"used_mb":1,"total_mb":65536,"core":2}],"processes":[{"npu":4,"die":2,"pid":21,"name":"python","memory_mb":1,"token":"boot:4","command":"cmd","owner":"u","controllable":true},{"npu":4,"die":3,"pid":21,"name":"python","memory_mb":1,"token":"boot:4","command":"cmd","owner":"u","controllable":true}],"processes_available":true,"stop_reason":""})).unwrap()
    }
    #[test]
    fn missing_or_invalid_payload_is_not_idle() {
        assert!(response("banner only").is_err());
        assert!(response("TERMVIAI_MONITOR_V1={}").is_err());
        assert!(response("TERMVIAI_MONITOR_V1={\"error\":\"no permission\"}").is_err());
        let mut s = fixture();
        s.dies[0].core = Some(101.);
        assert!(s.validate().is_err());
    }
    #[test]
    fn command_never_interpolates_process_text() {
        let c = command(Some(&[Target {
            pid: 23,
            token: "'; touch /tmp/injection; '".into(),
        }]))
        .unwrap();
        assert!(!c.contains("touch /tmp"));
        assert!(command(Some(&[Target {
            pid: 0,
            token: "x".into()
        }]))
        .is_err());
    }
    #[test]
    fn removing_monitor_preserves_host_and_duplicate_exclusion() {
        let h = Host {
            label: "test".into(),
            address: "example.invalid".into(),
            port: 22,
            ..Default::default()
        };
        let mut m = Monitor::default();
        m.add(h.clone());
        m.add(h.clone());
        assert_eq!(m.entries.len(), 1);
        m.entries.clear();
        assert_eq!(h.address, "example.invalid");
    }
    #[test]
    fn queued_stop_is_cancelled_when_sample_identity_changes() {
        let mut m = Monitor::default();
        m.add(Host::default());
        let e = &mut m.entries[0];
        e.snapshot = Some(fixture());
        e.updated = Some(Instant::now());
        let (tx, rx) = mpsc::channel();
        e.rx = Some(rx);
        m.stop(
            0,
            vec![Target {
                pid: 21,
                token: "boot:4".into(),
            }],
        )
        .unwrap();
        assert!(m.entries[0].stopping);
        let mut fresh = fixture();
        fresh
            .processes
            .iter_mut()
            .for_each(|p| p.token = "boot:new".into());
        tx.send(Completed {
            response: Ok(Response {
                snapshot: Some(fresh),
                results: None,
                error: None,
            }),
            session: None,
        })
        .unwrap();
        m.tick(&HostStore::default(), |_| {
            panic!("cancelled stop must not reconnect")
        });
        assert!(!m.entries[0].stopping);
        assert!(m.entries[0].error.contains("cancelled"));
    }
    #[test]
    fn dropping_entry_cancels_outstanding_work() {
        let mut m = Monitor::default();
        m.add(Host::default());
        let flag = m.entries[0].cancel.clone();
        m.entries.clear();
        assert!(flag.load(Ordering::Relaxed));
    }
}
