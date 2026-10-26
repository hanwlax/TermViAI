//! Opt-in, bounded resize timings. No terminal text, hostnames or credentials.
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{mpsc, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const LIMIT: usize = 8192;
#[derive(Default)]
struct Samples {
    values: Vec<f64>,
    count: usize,
    total: f64,
    max: f64,
}
impl Samples {
    fn add(&mut self, duration: Duration) {
        let ms = duration.as_secs_f64() * 1000.;
        self.count += 1;
        self.total += ms;
        self.max = self.max.max(ms);
        if self.values.len() < LIMIT {
            self.values.push(ms);
        }
    }
    fn percentile(&mut self, percent: usize) -> f64 {
        self.values.sort_by(f64::total_cmp);
        if self.values.is_empty() {
            return 0.;
        }
        self.values[((self.values.len() * percent + 99) / 100).saturating_sub(1)]
    }
}
struct Session {
    id: u128,
    started: Instant,
    last_frame: Option<Instant>,
    pending_resize: Option<Instant>,
    stages: BTreeMap<&'static str, Samples>,
}
thread_local! { static SESSION: RefCell<Option<Session>> = const { RefCell::new(None) }; }
static WRITER: OnceLock<Option<mpsc::SyncSender<Session>>> = OnceLock::new();
fn writer() -> Option<&'static mpsc::SyncSender<Session>> {
    WRITER
        .get_or_init(|| {
            let path = PathBuf::from(std::env::var_os("TERMVIAI_RESIZE_TRACE")?);
            if path.as_os_str().is_empty() {
                return None;
            }
            let (tx, rx) = mpsc::sync_channel::<Session>(8);
            std::thread::Builder::new()
                .name("resize-timings".into())
                .spawn(move || {
                    for mut session in rx {
                        let result = (|| -> std::io::Result<()> {
                            let mut file = std::fs::OpenOptions::new()
                                .create(true)
                                .append(true)
                                .open(&path)?;
                            if file.metadata()?.len() == 0 {
                                writeln!(
                                    file,
                                    "session,stage,count,sampled,p50_ms,p95_ms,max_ms,total_ms"
                                )?;
                            }
                            for (stage, samples) in &mut session.stages {
                                let p50 = samples.percentile(50);
                                let p95 = samples.percentile(95);
                                writeln!(
                                    file,
                                    "{},{},{},{},{:.3},{:.3},{:.3},{:.3}",
                                    session.id,
                                    stage,
                                    samples.count,
                                    samples.values.len(),
                                    p50,
                                    p95,
                                    samples.max,
                                    samples.total
                                )?;
                            }
                            Ok(())
                        })();
                        if let Err(err) = result {
                            log::warn!("resize timings: {}", err);
                        }
                    }
                })
                .ok()?;
            Some(tx)
        })
        .as_ref()
}
pub fn begin() {
    if writer().is_some() {
        SESSION.with(|s| {
            *s.borrow_mut() = Some(Session {
                id: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_micros(),
                started: Instant::now(),
                last_frame: None,
                pending_resize: None,
                stages: BTreeMap::new(),
            })
        });
    }
}
pub fn end() {
    SESSION.with(|s| {
        if let Some(mut session) = s.borrow_mut().take() {
            session
                .stages
                .entry("session")
                .or_default()
                .add(session.started.elapsed());
            if let Some(writer) = writer() {
                let _ = writer.try_send(session);
            }
        }
    });
}
pub fn record(stage: &'static str, duration: Duration) {
    SESSION.with(|s| {
        if let Some(s) = s.borrow_mut().as_mut() {
            s.stages.entry(stage).or_default().add(duration);
        }
    });
}
pub fn resize_requested() {
    SESSION.with(|s| {
        if let Some(s) = s.borrow_mut().as_mut() {
            s.pending_resize.get_or_insert_with(Instant::now);
            s.stages
                .entry("resize_event")
                .or_default()
                .add(Duration::ZERO);
        }
    });
}
pub fn presented() {
    SESSION.with(|s| {
        if let Some(s) = s.borrow_mut().as_mut() {
            let now = Instant::now();
            if let Some(requested) = s.pending_resize.take() {
                s.stages
                    .entry("resize_to_present_return")
                    .or_default()
                    .add(now.duration_since(requested));
            }
            if let Some(last) = s.last_frame.replace(now) {
                s.stages
                    .entry("present_interval")
                    .or_default()
                    .add(now.duration_since(last));
            }
        }
    });
}
pub struct Span {
    stage: &'static str,
    started: Option<Instant>,
}
impl Span {
    pub fn new(stage: &'static str) -> Self {
        Self {
            stage,
            started: SESSION.with(|s| s.borrow().as_ref().map(|_| Instant::now())),
        }
    }
}
impl Drop for Span {
    fn drop(&mut self) {
        if let Some(at) = self.started {
            record(self.stage, at.elapsed());
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn trace_writes_a_completed_session_without_ui_or_sensitive_content() {
        let path =
            std::env::temp_dir().join(format!("termviai-trace-test-{}.csv", std::process::id()));
        let previous = std::env::var_os("TERMVIAI_RESIZE_TRACE");
        std::env::set_var("TERMVIAI_RESIZE_TRACE", &path);
        begin();
        resize_requested();
        record("ui_layout", Duration::from_millis(3));
        presented();
        end();
        match previous {
            Some(v) => std::env::set_var("TERMVIAI_RESIZE_TRACE", v),
            None => std::env::remove_var("TERMVIAI_RESIZE_TRACE"),
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let csv = std::fs::read_to_string(&path).unwrap_or_default();
            if csv.contains(",ui_layout,1,1,3.000,3.000,3.000,3.000") {
                assert!(
                    csv.starts_with("session,stage,count,sampled,p50_ms,p95_ms,max_ms,total_ms")
                );
                assert!(csv.contains(",resize_to_present_return,"));
                assert!(csv.contains(",resize_event,1,"));
                std::fs::remove_file(&path).unwrap();
                break;
            }
            assert!(
                Instant::now() < deadline,
                "trace writer did not finish: {}",
                csv
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn samples_are_bounded_but_totals_include_all_frames() {
        let mut s = Samples::default();
        for _ in 0..LIMIT + 1 {
            s.add(Duration::from_millis(10));
        }
        s.add(Duration::from_millis(100));
        assert_eq!(s.values.len(), LIMIT);
        assert_eq!(s.count, LIMIT + 2);
        assert_eq!(s.max, 100.);
        assert_eq!(s.percentile(95), 10.);
        assert_eq!(s.total, (LIMIT + 1) as f64 * 10. + 100.);
    }
}
