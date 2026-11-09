//! Per-pane overlay scrollbars. Positions are stable rows; only the fractional
//! row is applied at quad submission, keeping cached text crisp and reusable.
use super::termviai_motion::{frame_interval, Tween};
use super::TermWindow;
use mux::pane::{Pane, PaneId};
use mux::renderable::RenderableDimensions;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use wezterm_term::StableRowIndex;
use window::{CursorIcon, MouseEvent, MouseEventKind, MousePress, WindowOps};

const HIDE_AFTER: Duration = Duration::from_millis(1200);
const FADE: Duration = Duration::from_millis(180);

#[derive(Clone, Copy, Debug)]
pub(super) struct Geometry {
    pub left: f32,
    pub x: f32,
    pub y: f32,
    pub height: f32,
    pub thumb_y: f32,
    pub thumb_height: f32,
}
impl Geometry {
    pub(super) fn new(
        left: f32,
        x: f32,
        y: f32,
        height: f32,
        rows: usize,
        low: isize,
        high: isize,
        position: f64,
    ) -> Self {
        let height = height.max(0.);
        let span = high.saturating_sub(low).max(0) as f64;
        let thumb_height = (height as f64 * rows as f64 / (span + rows.max(1) as f64))
            .max(32.)
            .min(height as f64) as f32;
        let ratio = if span > 0. {
            ((position - low as f64) / span).clamp(0., 1.)
        } else {
            1.
        };
        Self {
            left,
            x,
            y,
            height,
            thumb_y: y + ratio as f32 * (height - thumb_height),
            thumb_height,
        }
    }
    fn contains(self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + 14. && y >= self.y && y < self.y + self.height
    }
    fn row_at(self, y: f32, grab: f32, low: isize, high: isize) -> f64 {
        let travel = self.height - self.thumb_height;
        if travel <= 0. {
            return high as f64;
        }
        let ratio = ((y - grab - self.y) / travel).clamp(0., 1.) as f64;
        low as f64 + high.saturating_sub(low).max(0) as f64 * ratio
    }
}

struct State {
    position: f64,
    target: f64,
    low: isize,
    high: isize,
    last_viewport: Option<StableRowIndex>,
    updated: Instant,
    reveal_until: Instant,
    opacity: Tween,
    width: Tween,
    hovered: bool,
}
impl State {
    fn new(viewport: Option<isize>, dims: RenderableDimensions, now: Instant) -> Self {
        let p = viewport
            .unwrap_or(dims.physical_top)
            .clamp(dims.scrollback_top, dims.physical_top) as f64;
        Self {
            position: p,
            target: p,
            low: dims.scrollback_top,
            high: dims.physical_top,
            last_viewport: viewport,
            updated: now,
            reveal_until: now,
            opacity: Tween::new(0.),
            width: Tween::new(4.),
            hovered: false,
        }
    }
    fn active(&self) -> bool {
        self.position != self.target
    }
    fn touch(&mut self, now: Instant) {
        self.reveal_until = now + HIDE_AFTER;
    }
    fn sample(&mut self, now: Instant) {
        let dt = now.saturating_duration_since(self.updated).as_secs_f64();
        self.updated = now;
        // Same 45 ms exponential response as the approved interactive design.
        self.position += (self.target - self.position) * (1. - (-dt / 0.045).exp());
        if (self.position - self.target).abs() < 0.002 {
            self.position = self.target;
        }
    }
    fn sync(&mut self, viewport: Option<isize>, dims: RenderableDimensions, now: Instant) {
        self.low = dims.scrollback_top;
        self.high = dims.physical_top.max(self.low);
        if viewport != self.last_viewport || (viewport.is_none() && !self.active()) {
            self.position = viewport.unwrap_or(self.high) as f64;
            self.target = self.position;
        }
        self.position = self.position.clamp(self.low as f64, self.high as f64);
        self.target = self.target.clamp(self.low as f64, self.high as f64);
        self.sample(now);
    }
    fn move_by(&mut self, rows: f64, now: Instant) {
        self.sample(now);
        if rows * (self.target - self.position) < 0. {
            self.target = self.position;
        }
        self.target = (self.target + rows).clamp(self.low as f64, self.high as f64);
        // Finish wheel motion on an exact terminal row.
        self.target = self.target.round();
        self.touch(now);
    }
    fn jump(&mut self, row: f64, now: Instant) {
        self.position = row.clamp(self.low as f64, self.high as f64);
        self.target = self.position;
        self.updated = now;
        self.touch(now);
    }
    fn viewport(&self) -> Option<isize> {
        if self.position >= self.high as f64 {
            None
        } else {
            Some(self.position.floor() as isize)
        }
    }
    fn appearance(&mut self, now: Instant, dragging: bool) -> (f32, f32, Option<Instant>) {
        let visible = self.hovered || dragging || self.active() || now < self.reveal_until;
        self.opacity
            .set_target(if visible { 1. } else { 0. }, now, FADE);
        self.width.set_target(
            if self.hovered || dragging { 7. } else { 4. },
            now,
            Duration::from_millis(120),
        );
        let due = if self.active() || self.opacity.is_active(now) || self.width.is_active(now) {
            Some(now)
        } else if visible && !self.hovered && !dragging {
            Some(self.reveal_until)
        } else {
            None
        };
        (self.opacity.value(now), self.width.value(now), due)
    }
}
#[derive(Default)]
pub(super) struct Scrollbars {
    panes: HashMap<PaneId, State>,
    drag: Option<(PaneId, f32)>,
    track_pressed: bool,
}
pub(super) struct Visual {
    pub geometry: Geometry,
    pub opacity: f32,
    pub width: f32,
    pub emphasis: bool,
    pub label: Option<String>,
}

impl TermWindow {
    fn scrollbar_pane_allowed(&self, pane: &dyn Pane) -> bool {
        !pane.is_alt_screen_active()
            && self.termviai_font_source_pane(pane.pane_id()) == pane.pane_id()
            && self.pane_state(pane.pane_id()).overlay.is_none()
    }
    fn scrollbar_geometry(&self, id: PaneId, position: f64) -> Option<Geometry> {
        let pos = self
            .get_panes_to_render()
            .into_iter()
            .find(|p| p.pane.pane_id() == id)?;
        if !self.scrollbar_pane_allowed(pos.pane.as_ref()) {
            return None;
        }
        let dims = pos.pane.get_dimensions();
        if dims.physical_top <= dims.scrollback_top {
            return None;
        }
        let scale = self.dimensions.dpi as f32 / 96.;
        let (pad_x, pad_y) = self.padding_left_top();
        let border = self.get_os_border();
        let tab = if self.show_tab_bar {
            self.tab_bar_pixel_height().unwrap_or(0.)
        } else {
            0.
        };
        let (dx, dy) = self.termviai_pane_visual_offset(id);
        let padding = self.termviai_pane_content_padding_pixels(id);
        let right = pad_x
            + border.left.get() as f32
            + dx
            + pos.left as f32 * self.render_metrics.cell_size.width as f32
            + dims.pixel_width as f32
            + padding;
        let top = pad_y + tab + dy + pos.top as f32 * self.render_metrics.cell_size.height as f32
            - padding;
        let height = self
            .termviai_pane_visual_content_height(id)
            .unwrap_or(dims.pixel_height as f32)
            + padding * 2.;
        if height < 16. * scale || dims.pixel_width < (20. * scale) as usize {
            return None;
        }
        Some(Geometry::new(
            (right - dims.pixel_width as f32 - padding * 2.) / scale,
            right / scale - 16.,
            top / scale + 4.,
            height / scale - 8.,
            dims.viewport_rows,
            dims.scrollback_top,
            dims.physical_top,
            position,
        ))
    }
    pub(super) fn termviai_scrollbar_dragging(&self) -> bool {
        self.termviai_scrollbars.drag.is_some() || self.termviai_scrollbars.track_pressed
    }
    pub(super) fn termviai_scrollbar_cancel(&mut self) {
        if let Some((id, _)) = self.termviai_scrollbars.drag.take() {
            if let Some(s) = self.termviai_scrollbars.panes.get_mut(&id) {
                s.jump(s.position.round(), Instant::now());
                s.last_viewport = s.viewport();
                let next = s.last_viewport;
                self.pane_state(id).viewport = next;
            }
        }
        self.termviai_scrollbars.track_pressed = false;
        self.termviai_scrollbar_unhover();
    }
    pub(super) fn termviai_scrollbar_unhover(&mut self) {
        let now = Instant::now();
        for s in self.termviai_scrollbars.panes.values_mut() {
            if s.hovered {
                s.hovered = false;
                s.touch(now);
            }
        }
    }
    pub(super) fn termviai_scrollbar_external(
        &mut self,
        id: PaneId,
        viewport: Option<isize>,
        dims: RenderableDimensions,
    ) {
        if !self.config.termviai_ui {
            return;
        }
        let now = Instant::now();
        let mut state = State::new(viewport, dims, now);
        let mut changed = self.get_viewport(id) != viewport;
        if let Some(old) = self.termviai_scrollbars.panes.remove(&id) {
            changed |= old.active() || old.position.fract() != 0.;
            state.reveal_until = old.reveal_until;
            state.opacity = old.opacity;
            state.width = old.width;
            state.hovered = old.hovered;
        }
        // Typing while already following live output is not a scroll gesture.
        if changed {
            state.touch(now);
        }
        self.termviai_scrollbars.panes.insert(id, state);
    }
    pub(super) fn termviai_smooth_wheel(&mut self, pane: &Arc<dyn Pane>, rows: isize) -> bool {
        if !self.termviai_scrollbars_allowed() || !self.scrollbar_pane_allowed(pane.as_ref()) {
            return false;
        }
        let id = pane.pane_id();
        let dims = pane.get_dimensions();
        let viewport = self.get_viewport(id);
        let now = Instant::now();
        let s = self
            .termviai_scrollbars
            .panes
            .entry(id)
            .or_insert_with(|| State::new(viewport, dims, now));
        s.sync(viewport, dims, now);
        s.move_by(rows as f64, now);
        if let Some(win) = &self.window {
            win.invalidate();
        }
        true
    }
    /// Called before terminal rendering, never while holding pane lines.
    pub(super) fn termviai_scrollbar_frame(&mut self) {
        if !self.config.termviai_ui {
            return;
        }
        let panes = self.get_panes_to_render();
        let allowed = self.termviai_scrollbars_allowed();
        let now = Instant::now();
        self.termviai_scrollbars
            .panes
            .retain(|id, _| panes.iter().any(|p| p.pane.pane_id() == *id));
        for p in panes {
            let id = p.pane.pane_id();
            if !self.scrollbar_pane_allowed(p.pane.as_ref()) {
                self.termviai_scrollbars.panes.remove(&id);
                continue;
            }
            let dims = p.pane.get_dimensions();
            let viewport = self.get_viewport(id);
            let state = self
                .termviai_scrollbars
                .panes
                .entry(id)
                .or_insert_with(|| State::new(viewport, dims, now));
            state.sync(viewport, dims, now);
            if !allowed {
                if state.active() || state.position.fract() != 0. {
                    state.jump(state.position.round(), now);
                }
                state.hovered = false;
            }
            state.last_viewport = state.viewport();
            let next = state.last_viewport;
            let active = state.active();
            self.pane_state(id).viewport = next;
            if active {
                self.update_next_frame_time(Some(now + frame_interval(self.config.max_fps as u64)));
            }
        }
        if !allowed
            || self
                .termviai_scrollbars
                .drag
                .is_some_and(|(id, _)| !self.termviai_scrollbars.panes.contains_key(&id))
        {
            self.termviai_scrollbar_cancel();
        }
    }
    pub(super) fn termviai_scroll_fraction(&self, id: PaneId) -> f32 {
        if !self.config.termviai_ui {
            return 0.;
        }
        self.termviai_scrollbars
            .panes
            .get(&id)
            .filter(|s| s.last_viewport == self.get_viewport(id))
            .map(|s| (s.position - s.position.floor()) as f32)
            .unwrap_or(0.)
    }
    pub(super) fn termviai_scrollbar_visuals(&mut self) -> Vec<Visual> {
        if !self.termviai_scrollbars_allowed() {
            return vec![];
        }
        let now = Instant::now();
        let geometry: Vec<_> = self
            .termviai_scrollbars
            .panes
            .iter()
            .filter_map(|(&id, s)| self.scrollbar_geometry(id, s.position).map(|g| (id, g)))
            .collect();
        let mut result = vec![];
        for (id, g) in geometry {
            let dragging = self.termviai_scrollbars.drag.is_some_and(|(p, _)| p == id);
            let s = self.termviai_scrollbars.panes.get_mut(&id).unwrap();
            let (opacity, width, due) = s.appearance(now, dragging);
            let label = dragging.then(|| {
                if s.position >= s.high as f64 {
                    "Latest output".to_string()
                } else if s.position <= s.low as f64 {
                    "Start of history".to_string()
                } else {
                    format!(
                        "History · {:.0}%",
                        (s.position - s.low as f64) / (s.high - s.low) as f64 * 100.
                    )
                }
            });
            result.push(Visual {
                geometry: g,
                opacity,
                width,
                emphasis: s.hovered || dragging,
                label,
            });
            if let Some(due) = due {
                self.update_next_frame_time(Some(if due <= now {
                    now + frame_interval(self.config.max_fps as u64)
                } else {
                    due
                }));
            }
        }
        result
    }
    pub(super) fn termviai_scrollbar_mouse(
        &mut self,
        event: &MouseEvent,
        context: &dyn WindowOps,
    ) -> bool {
        if !self.termviai_scrollbars_allowed() {
            self.termviai_scrollbar_cancel();
            return false;
        }
        let scale = self.dimensions.dpi as f32 / 96.;
        let x = event.coords.x as f32 / scale;
        let y = (event.coords.y as f32 - self.get_os_border().top.get() as f32) / scale;
        let now = Instant::now();
        if self.termviai_scrollbars.track_pressed {
            if matches!(event.kind, MouseEventKind::Release(MousePress::Left)) {
                self.termviai_scrollbars.track_pressed = false;
            }
            context.set_cursor(Some(CursorIcon::Default));
            return true;
        }
        if let Some((id, grab)) = self.termviai_scrollbars.drag {
            let g = self
                .termviai_scrollbars
                .panes
                .get(&id)
                .and_then(|s| self.scrollbar_geometry(id, s.position));
            if let Some(g) = g {
                if matches!(event.kind, MouseEventKind::Move) {
                    let s = self.termviai_scrollbars.panes.get_mut(&id).unwrap();
                    s.jump(g.row_at(y, grab, s.low, s.high), now);
                    s.last_viewport = s.viewport();
                    let next = s.last_viewport;
                    self.pane_state(id).viewport = next;
                }
                if matches!(event.kind, MouseEventKind::Release(MousePress::Left)) {
                    // Stop on a whole row for crisp stationary text.
                    let s = self.termviai_scrollbars.panes.get_mut(&id).unwrap();
                    s.jump(s.position.round(), now);
                    s.last_viewport = s.viewport();
                    let next = s.last_viewport;
                    self.pane_state(id).viewport = next;
                    self.termviai_scrollbars.drag = None;
                }
                context.set_cursor(Some(CursorIcon::Grabbing));
                context.invalidate();
                return true;
            }
            self.termviai_scrollbars.drag = None;
        }
        if self.current_mouse_capture.is_some()
            || self.dragging.is_some()
            || self.window_drag_position.is_some()
        {
            return false;
        }
        let over = self.termviai_scrollbars.panes.iter().find_map(|(&id, s)| {
            self.scrollbar_geometry(id, s.position)
                .filter(|g| g.contains(x, y))
                .map(|g| (id, g))
        });
        for (&id, s) in &mut self.termviai_scrollbars.panes {
            let hover = over.is_some_and(|(p, _)| p == id);
            if s.hovered != hover {
                s.hovered = hover;
                s.touch(now);
                context.invalidate();
            }
        }
        let Some((id, g)) = over else {
            if matches!(event.kind, MouseEventKind::Press(_)) {
                let ids: Vec<_> = self
                    .termviai_scrollbars
                    .panes
                    .iter()
                    .filter(|(_, s)| s.active() || s.position.fract() != 0.)
                    .map(|(&id, _)| id)
                    .collect();
                for id in ids {
                    let s = self.termviai_scrollbars.panes.get_mut(&id).unwrap();
                    s.jump(s.position.round(), now);
                    s.last_viewport = s.viewport();
                    let next = s.last_viewport;
                    self.pane_state(id).viewport = next;
                }
            }
            return false;
        };
        match event.kind {
            MouseEventKind::Press(MousePress::Left) => {
                let s = self.termviai_scrollbars.panes.get_mut(&id).unwrap();
                if y >= g.thumb_y && y < g.thumb_y + g.thumb_height {
                    s.jump(s.position, now);
                    self.termviai_scrollbars.drag = Some((id, y - g.thumb_y));
                } else {
                    self.termviai_scrollbars.track_pressed = true;
                    let pane = mux::Mux::get().get_pane(id);
                    if let Some(pane) = pane {
                        s.move_by(
                            if y < g.thumb_y { -1. } else { 1. }
                                * pane.get_dimensions().viewport_rows as f64
                                * 0.9,
                            now,
                        );
                    }
                }
                context.invalidate();
                context.set_cursor(Some(CursorIcon::Grab));
                true
            }
            MouseEventKind::Move => {
                context.set_cursor(Some(CursorIcon::Grab));
                true
            }
            MouseEventKind::Release(MousePress::Left) => true,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn dims() -> RenderableDimensions {
        RenderableDimensions {
            scrollback_top: 100,
            physical_top: 1100,
            viewport_rows: 30,
            scrollback_rows: 1030,
            ..Default::default()
        }
    }
    #[test]
    fn geometry_roundtrips_and_tiny_or_empty_tracks_are_finite() {
        for height in [0., 20., 200., 600.] {
            for position in [100., 600., 1100.] {
                let g = Geometry::new(0., 200., 40., height, 30, 100, 1100, position);
                assert!(g.thumb_height <= height);
                let row = g.row_at(g.thumb_y + 10., 10., 100, 1100);
                assert!(row.is_finite());
                if height > 32. {
                    assert!((row - position).abs() < 0.001);
                }
                assert!(g.row_at(-1000., 0., 100, 1100) >= 100.);
                assert!(g.row_at(10000., 0., 100, 1100) <= 1100.);
            }
        }
    }
    #[test]
    fn wheel_is_frame_rate_independent_reverses_and_converges() {
        for fps in [30, 60, 144] {
            let now = Instant::now();
            let mut s = State::new(None, dims(), now);
            s.move_by(-12., now);
            let mut before = s.position;
            for i in 1..=fps {
                s.sample(now + Duration::from_secs_f64(i as f64 / fps as f64));
                assert!(s.position <= before && s.position >= 1088.);
                before = s.position;
            }
            assert_eq!(s.position, 1088.);
            assert!(!s.active());
            s.move_by(-30., now + Duration::from_secs(2));
            s.sample(now + Duration::from_millis(2020));
            let shown = s.position;
            s.move_by(6., now + Duration::from_millis(2020));
            assert_eq!(s.target, (shown + 6.).round());
        }
    }
    #[test]
    fn follows_live_output_but_preserves_history_and_external_jumps() {
        let now = Instant::now();
        let mut s = State::new(None, dims(), now);
        let mut d = dims();
        d.physical_top += 20;
        s.sync(None, d, now);
        assert_eq!(s.position, 1120.);
        s.jump(300.25, now);
        s.last_viewport = s.viewport();
        d.physical_top += 20;
        s.sync(Some(300), d, now);
        assert_eq!(s.position, 300.25);
        d.scrollback_top = 400;
        s.sync(Some(300), d, now);
        assert_eq!(s.position, 400.);
        s.last_viewport = Some(400);
        s.sync(None, d, now);
        assert_eq!(s.position, 1140.);
    }
    #[test]
    fn four_panes_keep_independent_positions_and_resize_preserves_anchor() {
        let now = Instant::now();
        let mut panes: Vec<_> = (0..4).map(|_| State::new(None, dims(), now)).collect();
        panes[2].move_by(-80., now);
        panes[2].sample(now + Duration::from_millis(35));
        panes[2].last_viewport = panes[2].viewport();
        let anchor = panes[2].position;
        let viewport = panes[2].last_viewport;
        let mut resized = dims();
        resized.viewport_rows = 12;
        panes[2].sync(viewport, resized, now + Duration::from_millis(35));
        assert_eq!(panes[2].position, anchor);
        for i in [0, 1, 3] {
            assert_eq!(panes[i].viewport(), None);
            assert!(!panes[i].active());
        }
        panes[2].jump(457.6, now + Duration::from_millis(40));
        assert!(!panes[2].active());
        assert_eq!(panes[2].viewport(), Some(457));
        let settled = panes[2].position.round();
        panes[2].jump(settled, now + Duration::from_millis(41));
        assert_eq!(panes[2].position, 458.);
        panes[2].jump(1100., now + Duration::from_millis(42));
        assert_eq!(panes[2].viewport(), None);
    }

    #[test]
    fn hidden_until_activity_then_fades_and_stays_during_drag() {
        let now = Instant::now();
        let mut s = State::new(None, dims(), now);
        assert_eq!(s.appearance(now, false).0, 0.);
        s.touch(now);
        s.appearance(now, false);
        assert_eq!(s.appearance(now + FADE, false).0, 1.);
        s.appearance(now + HIDE_AFTER, false);
        assert_eq!(s.appearance(now + HIDE_AFTER + FADE, false).0, 0.);
        s.appearance(now + Duration::from_secs(3), true);
        assert_eq!(s.appearance(now + Duration::from_secs(4), true).0, 1.);
    }
}
