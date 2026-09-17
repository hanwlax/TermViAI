//! Small elapsed-time transitions for native application chrome.
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub(super) struct Tween {
    from: f32,
    target: f32,
    started: Instant,
    duration: Duration,
}

impl Tween {
    pub(super) fn new(value: f32) -> Self {
        Self {
            from: value,
            target: value,
            started: Instant::now(),
            duration: Duration::ZERO,
        }
    }

    /// Retarget from the currently displayed value, even during a reversal.
    /// Repeated requests for the same target never restart the transition.
    pub(super) fn set_target(&mut self, target: f32, now: Instant, duration: Duration) {
        if target == self.target {
            return;
        }
        self.from = self.value(now);
        self.target = target;
        self.started = now;
        self.duration = duration;
    }

    pub(super) fn value(&self, now: Instant) -> f32 {
        if !self.is_active(now) {
            return self.target;
        }
        let progress =
            now.saturating_duration_since(self.started).as_secs_f32() / self.duration.as_secs_f32();
        let eased = 1. - (1. - progress).powi(3);
        self.from + (self.target - self.from) * eased
    }

    /// Only active transitions need another scheduled paint.
    pub(super) fn is_active(&self, now: Instant) -> bool {
        self.from != self.target && now.saturating_duration_since(self.started) < self.duration
    }
}

/// Chrome motion uses the window frame limit, not the slower terminal blink rate.
pub(super) fn frame_interval(max_fps: u64) -> Duration {
    Duration::from_secs_f64(1. / max_fps.clamp(1, 60) as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tween_reaches_endpoints_and_stops_requesting_frames() {
        let now = Instant::now();
        let duration = Duration::from_millis(220);
        let mut tween = Tween::new(0.);
        assert!(!tween.is_active(now));
        tween.set_target(1., now, duration);
        assert_eq!(tween.value(now), 0.);
        assert!(tween.is_active(now));
        assert!((tween.value(now + duration / 2) - 0.875).abs() < f32::EPSILON);
        assert_eq!(tween.value(now + duration), 1.);
        assert!(!tween.is_active(now + duration));
        assert_eq!(tween.value(now + duration * 2), 1.);
        assert!(!tween.is_active(now + duration * 2));
    }

    #[test]
    fn reversal_continues_from_the_visible_position() {
        let now = Instant::now();
        let duration = Duration::from_millis(200);
        let halfway = now + duration / 2;
        let mut tween = Tween::new(0.);
        tween.set_target(1., now, duration);
        let visible = tween.value(halfway);
        tween.set_target(0., halfway, duration);
        assert_eq!(tween.value(halfway), visible);
        let closing = tween.value(halfway + duration / 2);
        assert!(closing > 0. && closing < visible);
        assert_eq!(tween.value(halfway + duration), 0.);
        assert!(!tween.is_active(halfway + duration));
    }

    #[test]
    fn repeated_target_does_not_extend_an_animation() {
        let now = Instant::now();
        let duration = Duration::from_millis(180);
        let mut tween = Tween::new(0.);
        tween.set_target(1., now, duration);
        tween.set_target(1., now + duration / 2, duration);
        assert_eq!(tween.value(now + duration), 1.);
        assert!(!tween.is_active(now + duration));
    }

    #[test]
    fn zero_duration_settles_immediately_without_division() {
        let now = Instant::now();
        let mut tween = Tween::new(0.);
        tween.set_target(1., now, Duration::ZERO);
        assert_eq!(tween.value(now), 1.);
        assert!(!tween.is_active(now));
    }

    #[test]
    fn frame_interval_is_nonzero_and_respects_the_window_limit() {
        assert_eq!(frame_interval(0), Duration::from_secs(1));
        assert_eq!(frame_interval(1), Duration::from_secs(1));
        assert_eq!(frame_interval(30), Duration::from_secs_f64(1. / 30.));
        assert_eq!(frame_interval(60), Duration::from_secs_f64(1. / 60.));
        assert_eq!(frame_interval(u64::MAX), frame_interval(60));
    }
}
