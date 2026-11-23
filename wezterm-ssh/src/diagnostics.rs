//! Opt-in, bounded connection diagnostics. Never log terminal data or ConfigMap.
use crate::ConfigMap;
use chrono::{SecondsFormat, Utc};
use serde_json::{json, Value};
use socket2::Socket;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant, SystemTime};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);
static RUN_ID: LazyLock<String> =
    LazyLock::new(|| format!("{}-{}", std::process::id(), Utc::now().timestamp_millis()));
// Serializes append/rotation across all connections in this process. Different
// processes use different files, so an already-running app cannot rotate ours.
static WRITER: Mutex<()> = Mutex::new(());
const MAX_BYTES: u64 = 4 * 1024 * 1024;
const BACKUPS: usize = 3;

/// A lightweight observer; holding it does not keep an SSH connection alive.
#[derive(Clone, Default)]
pub struct ConnectionDiagnostics(Option<Arc<Mutex<State>>>);

struct State {
    path: PathBuf,
    context: Value,
    started: Instant,
    last_read: Option<Instant>,
    last_write: Option<Instant>,
    last_keepalive: Option<Instant>,
    keepalive_attempts: u64,
    keepalive_successes: u64,
    last_keepalive_at: Option<String>,
    last_keepalive_success_at: Option<String>,
    last_keepalive_error: Option<String>,
    write_failed: bool,
}

fn timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

impl ConnectionDiagnostics {
    pub(crate) fn new(config: &ConfigMap) -> Self {
        let Some(directory) = config.get("termviai_diagnostics_dir") else {
            return Self::default();
        };
        let path = Path::new(directory).join(format!("ssh-{}.jsonl", &*RUN_ID));
        let this = Self(Some(Arc::new(Mutex::new(State {
            path,
            context: json!({
                "connection_id": format!("{}-{}", &*RUN_ID, NEXT_ID.fetch_add(1, Ordering::Relaxed)),
                "pid": std::process::id(),
                "host": config.get("hostname"),
                "configured_port": config.get("port").and_then(|v| v.parse::<u16>().ok()).unwrap_or(22),
                "backend": config.get("wezterm_ssh_backend"),
                "proxy": config.get("proxycommand").is_some_and(|v| v != "none"),
                "keepalive_interval_seconds": config.get("serveraliveinterval").and_then(|v| v.parse::<u64>().ok()).unwrap_or(0),
                "keepalive_mode": "not_started",
                "local_address": null, "local_port": null,
                "peer_address": null, "peer_port": null,
            }),
            started: Instant::now(),
            last_read: None,
            last_write: None,
            last_keepalive: None,
            keepalive_attempts: 0,
            keepalive_successes: 0,
            last_keepalive_at: None,
            last_keepalive_success_at: None,
            last_keepalive_error: None,
            write_failed: false,
        }))));
        this.event("connection_start", json!({}));
        this
    }

    pub(crate) fn socket_connected(&self, socket: &Socket) {
        self.update(|s| {
            if let Some(addr) = socket.local_addr().ok().and_then(|a| a.as_socket()) {
                s.context["local_address"] = json!(addr.ip().to_string());
                s.context["local_port"] = json!(addr.port());
            }
            if let Some(addr) = socket.peer_addr().ok().and_then(|a| a.as_socket()) {
                s.context["peer_address"] = json!(addr.ip().to_string());
                s.context["peer_port"] = json!(addr.port());
            }
        });
        self.event("socket_connected", json!({}));
    }

    pub(crate) fn keepalive_mode(&self, mode: &str, interval_seconds: u64) {
        self.update(|s| {
            s.context["keepalive_mode"] = json!(mode);
            s.context["keepalive_interval_seconds"] = json!(interval_seconds);
        });
        self.event("keepalive_configured", json!({}));
    }

    pub(crate) fn keepalive(&self, error: Option<String>) {
        let at = timestamp();
        self.update(|s| {
            s.last_keepalive = Some(Instant::now());
            s.keepalive_attempts += 1;
            s.last_keepalive_at = Some(at.clone());
            if error.is_none() {
                s.keepalive_successes += 1;
                s.last_keepalive_success_at = Some(at);
            }
            s.last_keepalive_error = error.clone();
        });
        self.event("keepalive_send", json!({"error": error}));
    }

    // These are SSH channel payload timestamps, not TCP ACKs or heartbeat replies.
    pub(crate) fn activity(&self, read: bool) {
        self.update(|s| {
            if read {
                s.last_read = Some(Instant::now());
            } else {
                s.last_write = Some(Instant::now());
            }
        });
    }

    /// Associate the application's terminal with this transport without its data.
    pub fn terminal_started(&self, session_id: usize) {
        self.update(|s| s.context["terminal_session_id"] = json!(session_id));
        self.event("terminal_connected", json!({}));
    }

    /// Record when a terminal observes EOF, an error, cancellation, or child exit.
    /// `error` must describe the I/O failure, never terminal input/output.
    pub fn terminal_ended(&self, reason: &str, error: Option<&str>) {
        self.event("terminal_ended", json!({"reason": reason, "error": error}));
    }

    fn update(&self, f: impl FnOnce(&mut State)) {
        if let Some(state) = &self.0 {
            if let Ok(mut state) = state.lock() {
                f(&mut state);
            }
        }
    }

    pub(crate) fn event(&self, event: &str, detail: Value) {
        self.update(|s| {
            let mut record = s.context.clone();
            record["timestamp"] = json!(timestamp());
            record["event"] = json!(event);
            record["elapsed_ms"] = json!(s.started.elapsed().as_millis() as u64);
            record["channel_read_age_ms"] =
                json!(s.last_read.map(|t| t.elapsed().as_millis() as u64));
            record["channel_write_age_ms"] =
                json!(s.last_write.map(|t| t.elapsed().as_millis() as u64));
            record["keepalive_age_ms"] =
                json!(s.last_keepalive.map(|t| t.elapsed().as_millis() as u64));
            record["keepalive_attempts"] = json!(s.keepalive_attempts);
            record["keepalive_send_successes"] = json!(s.keepalive_successes);
            record["last_keepalive_at"] = json!(s.last_keepalive_at);
            record["last_keepalive_success_at"] = json!(s.last_keepalive_success_at);
            record["last_keepalive_error"] = json!(s.last_keepalive_error);
            record["detail"] = detail;
            if let Err(error) = append(&s.path, &record, MAX_BYTES) {
                if !s.write_failed {
                    log::warn!("Cannot write SSH diagnostics {}: {error}", s.path.display());
                }
                s.write_failed = true;
            } else {
                s.write_failed = false;
            }
        });
    }
}

fn backup(path: &Path, n: usize) -> PathBuf {
    path.with_extension(format!("jsonl.{n}"))
}

fn append(path: &Path, record: &Value, limit: u64) -> io::Result<()> {
    let _guard = WRITER
        .lock()
        .map_err(|_| io::Error::other("SSH log lock poisoned"))?;
    let directory = path
        .parent()
        .ok_or_else(|| io::Error::other("Missing log directory"))?;
    fs::create_dir_all(directory)?;
    let mut bytes = serde_json::to_vec(record)?;
    bytes.push(b'\n');
    if let Ok(meta) = fs::metadata(path) {
        if meta.len() + bytes.len() as u64 > limit {
            for n in (1..=BACKUPS).rev() {
                let source = if n == 1 {
                    path.to_owned()
                } else {
                    backup(path, n - 1)
                };
                let dest = backup(path, n);
                if dest.exists() {
                    fs::remove_file(&dest)?;
                }
                if source.exists() {
                    fs::rename(source, dest)?;
                }
            }
        }
    } else {
        // Prune only our stale logs. Each process has its own filename; actively
        // written logs have a recent mtime. No persistent file handles are held.
        for entry in fs::read_dir(directory)?.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("ssh-")
                && (name.ends_with(".jsonl")
                    || (1..=BACKUPS).any(|n| name.ends_with(&format!(".jsonl.{n}"))))
            {
                if let Ok(meta) = entry.metadata() {
                    if meta.is_file()
                        && meta
                            .modified()
                            .ok()
                            .and_then(|t| SystemTime::now().duration_since(t).ok())
                            .is_some_and(|age| age > Duration::from_secs(7 * 86400))
                    {
                        let _ = fs::remove_file(entry.path());
                    }
                }
            }
        }
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(&bytes)?;
    file.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{TcpListener, TcpStream};

    #[test]
    fn records_endpoints_keepalives_and_terminal_failure_without_payloads() {
        let dir = assert_fs::TempDir::new().unwrap();
        let mut config = ConfigMap::new();
        config.insert(
            "termviai_diagnostics_dir".into(),
            dir.path().display().to_string(),
        );
        config.insert("hostname".into(), "fixture".into());
        config.insert("password".into(), "never-log-this".into());
        let d = ConnectionDiagnostics::new(&config);
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let stream = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let local_port = stream.local_addr().unwrap().port();
        d.socket_connected(&Socket::from(stream));
        d.terminal_started(209);
        d.keepalive_mode("ssh_ignore_no_reply", 30);
        d.keepalive(None);
        d.keepalive(Some("connection reset".into()));
        d.terminal_ended(
            "reader_error",
            Some("Connection reset by peer (os error 104)"),
        );
        let path = d.0.as_ref().unwrap().lock().unwrap().path.clone();
        let text = fs::read_to_string(path).unwrap();
        assert!(!text.contains("never-log-this"));
        let records: Vec<Value> = text
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        let last = records.last().unwrap();
        assert_eq!(last["local_port"], local_port);
        assert_eq!(last["peer_port"], listener.local_addr().unwrap().port());
        assert_eq!(last["terminal_session_id"], 209);
        assert_eq!(last["keepalive_attempts"], 2);
        assert_eq!(last["keepalive_send_successes"], 1);
        assert_eq!(last["last_keepalive_error"], "connection reset");
        assert!(last["last_keepalive_success_at"].is_string());
        assert!(chrono::DateTime::parse_from_rfc3339(last["timestamp"].as_str().unwrap()).is_ok());
        assert_eq!(last["detail"]["reason"], "reader_error");
        assert!(records
            .iter()
            .all(|r| r["connection_id"] == last["connection_id"]));
    }

    #[test]
    fn rotation_keeps_complete_records_and_recent_failure() {
        let dir = assert_fs::TempDir::new().unwrap();
        let path = dir.path().join("ssh-test.jsonl");
        for n in 0..30 {
            append(&path, &json!({"event": "failure", "n": n}), 100).unwrap();
        }
        assert!(backup(&path, BACKUPS).exists());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), BACKUPS + 1);
        for entry in fs::read_dir(dir.path()).unwrap() {
            let text = fs::read_to_string(entry.unwrap().path()).unwrap();
            assert!(text.ends_with('\n'));
            for line in text.lines() {
                serde_json::from_str::<Value>(line).unwrap();
            }
        }
        assert!(fs::read_to_string(path).unwrap().contains("\"n\":29"));
    }

    #[test]
    fn unwritable_destination_does_not_break_a_session() {
        let file = assert_fs::NamedTempFile::new("not-a-directory").unwrap();
        fs::write(file.path(), "occupied").unwrap();
        let mut config = ConfigMap::new();
        config.insert(
            "termviai_diagnostics_dir".into(),
            file.path().display().to_string(),
        );
        let d = ConnectionDiagnostics::new(&config);
        d.keepalive(None);
        d.terminal_ended("reader_eof", None);
        assert!(d.0.unwrap().lock().unwrap().write_failed);
    }
}
