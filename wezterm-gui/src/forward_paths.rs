//! Debounced Windows notifications and bounded background DNS/route snapshots.
use std::collections::{HashMap, HashSet};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use wezterm_ssh::forward_network::{self, Path};

pub(super) struct Routes {
    epoch: u64,
    revision: u64,
    settle: Instant,
    refresh: Instant,
    previous: Instant,
    cache: HashMap<String, Path>,
    pending: HashSet<String>,
    tx: mpsc::Sender<(String, u64, Path)>,
    rx: mpsc::Receiver<(String, u64, Path)>,
}
impl Default for Routes {
    fn default() -> Self {
        let now = Instant::now();
        let (tx, rx) = mpsc::channel();
        Self {
            epoch: 0,
            revision: forward_network::revision(),
            settle: now,
            refresh: now + Duration::from_secs(30),
            previous: now,
            cache: HashMap::new(),
            pending: HashSet::new(),
            tx,
            rx,
        }
    }
}
impl Routes {
    pub fn tick(&mut self, now: Instant) {
        self.observe(now, forward_network::revision());
    }
    fn observe(&mut self, now: Instant, revision: u64) {
        let changed =
            revision != self.revision || now.duration_since(self.previous) > Duration::from_secs(5);
        self.previous = now;
        if changed || now >= self.refresh {
            self.revision = revision;
            self.cache.clear();
            self.refresh = now + Duration::from_secs(30);
            if changed {
                self.epoch = self.epoch.wrapping_add(1);
                self.settle = now + Duration::from_secs(1);
            }
        }
        while let Ok((host, epoch, result)) = self.rx.try_recv() {
            self.pending.remove(&host);
            if epoch == self.epoch {
                self.cache.insert(host, result);
            }
        }
    }
    pub fn forget(&mut self, host: &str) {
        self.cache.remove(host);
    }
    pub fn get(&mut self, host: &str, now: Instant) -> Option<Path> {
        if let Some(path) = self.cache.get(host) {
            return Some(*path);
        }
        if now >= self.settle && self.pending.len() < 4 && self.pending.insert(host.to_owned()) {
            let host = host.to_owned();
            let epoch = self.epoch;
            let tx = self.tx.clone();
            std::thread::spawn(move || {
                let result = forward_network::path_to(&host);
                let _ = tx.send((host, epoch, result));
            });
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeated_events_debounce_and_old_route_answers_are_discarded() {
        let mut routes = Routes::default();
        let now = routes.previous;
        let revision = routes.revision;
        let old = routes.epoch;
        routes.cache.insert("vpn".into(), Path::Available);
        routes.observe(now, revision + 1);
        routes
            .tx
            .send(("vpn".into(), old, Path::Available))
            .unwrap();
        routes.observe(now + Duration::from_millis(500), revision + 2);
        assert_eq!(routes.get("vpn", now + Duration::from_secs(1)), None);
        assert!(routes.pending.is_empty());
        assert!(routes.cache.is_empty());
        assert_eq!(routes.settle, now + Duration::from_millis(1500));
    }
    #[test]
    fn periodic_snapshot_rechecks_offline_without_a_notification() {
        let mut routes = Routes::default();
        let now = routes.previous;
        routes.cache.insert("vpn".into(), Path::Offline);
        for seconds in 1..=30 {
            routes.observe(now + Duration::from_secs(seconds), routes.revision);
        }
        assert!(routes.cache.is_empty());
    }
}
