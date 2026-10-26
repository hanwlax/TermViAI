//! Small elapsed-time transitions for native application chrome.
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub(super) struct Tween {
    from: f32,
    target: f32,
    started: Instant,
    duration: Duration,
    smooth: bool,
}

impl Tween {
    pub(super) fn new(value: f32) -> Self {
        Self {
            from: value,
            target: value,
            started: Instant::now(),
            duration: Duration::ZERO,
            smooth: false,
        }
    }

    /// Retarget from the currently displayed value, even during a reversal.
    /// Repeated requests for the same target never restart the transition.
    pub(super) fn set_target(&mut self, target: f32, now: Instant, duration: Duration) {
        self.retarget(target, now, duration, false);
    }

    pub(super) fn set_target_smooth(&mut self, target: f32, now: Instant, duration: Duration) {
        self.retarget(target, now, duration, true);
    }

    fn retarget(&mut self, target: f32, now: Instant, duration: Duration, smooth: bool) {
        if target == self.target {
            return;
        }
        self.from = self.value(now);
        self.target = target;
        self.started = now;
        self.duration = duration;
        self.smooth = smooth;
    }

    pub(super) fn value(&self, now: Instant) -> f32 {
        if !self.is_active(now) {
            return self.target;
        }
        let progress =
            now.saturating_duration_since(self.started).as_secs_f32() / self.duration.as_secs_f32();
        let eased = if self.smooth {
            progress * progress * (3. - 2. * progress)
        } else {
            1. - (1. - progress).powi(3)
        };
        self.from + (self.target - self.from) * eased
    }

    /// Only active transitions need another scheduled paint.
    pub(super) fn is_active(&self, now: Instant) -> bool {
        self.from != self.target && now.saturating_duration_since(self.started) < self.duration
    }
}

/// Pixel scroll position with a critically damped response. Wheel bursts add to
/// the destination; direct thumb manipulation bypasses the animation entirely.
#[derive(Clone, Debug)]
pub(super) struct SmoothScroll {
    pub position: f32,
    pub target: f32,
    pub maximum: f32,
    pub viewport: f32,
    velocity: f32,
    updated: Instant,
}
impl Default for SmoothScroll {
    fn default() -> Self {
        Self {
            position: 0.,
            target: 0.,
            maximum: 0.,
            viewport: 0.,
            velocity: 0.,
            updated: Instant::now(),
        }
    }
}
impl SmoothScroll {
    pub fn active(&self) -> bool {
        self.position != self.target || self.velocity != 0.
    }
    pub fn sample(&mut self, now: Instant) -> f32 {
        let dt = now.saturating_duration_since(self.updated).as_secs_f32();
        self.updated = now.max(self.updated);
        if dt == 0. || !self.active() {
            return self.position;
        }
        let before = self.target - self.position;
        let displacement = -before;
        let omega = 32.;
        let decay = (-omega * dt).exp();
        let c = self.velocity + omega * displacement;
        self.position = self.target + (displacement + c * dt) * decay;
        self.velocity = (self.velocity - omega * c * dt) * decay;
        if (self.target - self.position) * before < 0.
            || ((self.target - self.position).abs() < 0.05 && self.velocity.abs() < 0.5)
        {
            self.position = self.target;
            self.velocity = 0.;
        }
        self.position = self.position.clamp(0., self.maximum);
        self.position
    }
    pub fn bounds(&mut self, viewport: f32, content: f32, now: Instant) {
        self.sample(now);
        self.viewport = viewport.max(0.);
        self.maximum = (content - self.viewport).max(0.);
        self.target = self.target.clamp(0., self.maximum);
        self.position = self.position.clamp(0., self.maximum);
        if self.position == self.target {
            self.velocity = 0.;
        }
    }
    pub fn move_by(&mut self, pixels: f32, now: Instant) {
        self.sample(now);
        // Reversing input responds from the visible position, not a backlog of
        // queued wheel ticks in the old direction.
        if pixels * (self.target - self.position) < 0. {
            self.target = self.position;
            self.velocity = 0.;
        }
        self.target = (self.target + pixels).clamp(0., self.maximum);
    }
    pub fn jump_to(&mut self, pixels: f32, now: Instant) {
        self.position = pixels.clamp(0., self.maximum);
        self.target = self.position;
        self.velocity = 0.;
        self.updated = now;
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
    fn scroll_converges_without_overshoot_at_different_frame_rates() {
        let now = Instant::now();
        for fps in [30, 60, 144] {
            let mut s = SmoothScroll::default();
            s.bounds(500., 5000., now);
            s.move_by(96., now);
            assert_eq!(s.position, 0.);
            let mut previous = 0.;
            for frame in 1..=fps {
                let position = s.sample(now + Duration::from_secs_f64(frame as f64 / fps as f64));
                assert!(position >= previous && position <= 96.);
                previous = position;
            }
            assert_eq!(s.position, 96.);
            assert!(!s.active());
        }
    }
    #[test]
    fn wheel_bursts_accumulate_and_reversing_does_not_follow_old_backlog() {
        let now = Instant::now();
        let mut s = SmoothScroll::default();
        s.bounds(500., 5000., now);
        s.move_by(96., now);
        s.move_by(96., now + Duration::from_millis(16));
        assert_eq!(s.target, 192.);
        assert!(s.position > 0. && s.position < 96.);
        let before = s.sample(now + Duration::from_millis(48));
        s.move_by(-24., now + Duration::from_millis(48));
        assert_eq!(s.position, before);
        assert_eq!(s.target, (before - 24.).max(0.));
        assert!(s.sample(now + Duration::from_millis(64)) < before);
    }
    #[test]
    fn direct_drag_cancels_animation_and_content_shrink_clamps_position() {
        let now = Instant::now();
        let mut s = SmoothScroll::default();
        s.bounds(500., 5000., now);
        s.move_by(300., now);
        s.jump_to(173.25, now + Duration::from_millis(16));
        assert_eq!(s.position, 173.25);
        assert_eq!(s.target, 173.25);
        assert!(!s.active());
        s.bounds(500., 550., now + Duration::from_millis(32));
        assert_eq!(s.position, 50.);
        assert_eq!(s.target, 50.);
        s.bounds(500., 400., now + Duration::from_millis(48));
        assert_eq!(s.position, 0.);
        assert!(!s.active());
    }

    #[test]
    fn gentle_hover_eases_in_and_out_and_reverses_without_a_jump() {
        let now = Instant::now();
        let duration = Duration::from_millis(220);
        let mut tween = Tween::new(0.);
        tween.set_target_smooth(1., now, duration);
        assert!(tween.value(now + duration / 4) < 0.25);
        assert_eq!(tween.value(now + duration / 2), 0.5);
        assert!(tween.value(now + duration * 3 / 4) > 0.75);
        let reverse = now + duration / 2;
        tween.set_target_smooth(0., reverse, duration);
        assert_eq!(tween.value(reverse), 0.5);
        assert_eq!(tween.value(reverse + duration / 2), 0.25);
        tween.set_target_smooth(0., reverse + duration / 2, duration);
        assert_eq!(tween.value(reverse + duration), 0.);
        assert!(!tween.is_active(reverse + duration));
    }
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
