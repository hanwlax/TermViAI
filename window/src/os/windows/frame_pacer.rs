//! Start-to-start pacing: slow rendering consumes the budget instead of adding
//! another whole frame delay. Win32 timers wake the modal size/move loop too.
use std::time::{Duration, Instant};
#[derive(Default)]
pub(super) struct FramePacer {
    last_start: Option<Instant>,
    pending_resize: Option<Instant>,
}
impl FramePacer {
    pub fn delay(&self, now: Instant, fps: u64) -> Duration {
        let interval = Duration::from_secs_f64(1. / fps.clamp(1, 1000) as f64);
        self.last_start.map_or(Duration::ZERO, |at| {
            interval.saturating_sub(now.saturating_duration_since(at))
        })
    }
    pub fn start(&mut self, now: Instant) {
        self.last_start = Some(now);
    }
    pub fn request_resize(&mut self, now: Instant) {
        // Keep the oldest unpainted request to measure queueing latency.
        self.pending_resize.get_or_insert(now);
    }
    pub fn has_resize(&self) -> bool {
        self.pending_resize.is_some()
    }
    pub fn take_resize(&mut self) -> Option<Instant> {
        self.pending_resize.take()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn work_time_counts_toward_frame_budget() {
        let mut p = FramePacer::default();
        let at = Instant::now();
        assert!(p.delay(at, 60).is_zero());
        p.start(at);
        let remaining = p.delay(at + Duration::from_millis(12), 60);
        assert!(remaining > Duration::from_millis(4) && remaining < Duration::from_millis(5));
        assert!(p.delay(at + Duration::from_millis(30), 60).is_zero());
        assert!(p.delay(at, 0) <= Duration::from_secs(1));
    }
    #[test]
    fn resize_messages_coalesce_and_are_consumed_once() {
        let mut p = FramePacer::default();
        let at = Instant::now();
        for n in 0..100 {
            p.request_resize(at + Duration::from_micros(n));
        }
        assert!(p.has_resize());
        assert_eq!(p.take_resize(), Some(at));
        assert!(!p.has_resize());
        assert_eq!(p.take_resize(), None);
        p.request_resize(at + Duration::from_millis(50));
        assert_eq!(p.take_resize(), Some(at + Duration::from_millis(50)));
    }
}
