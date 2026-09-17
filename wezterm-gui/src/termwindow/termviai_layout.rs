//! Shared geometry for session headers and the collapsible broadcast tray.
//!
//! The mux keeps outer split rectangles. Its optional header inset reduces PTY
//! dimensions; the GUI maps those same rows out of rendering, IME and mouse
//! coordinates. The tray and available terminal area share the same animated
//! reservation; PTY grids only change when a cell boundary is crossed.

use super::termviai_dock::Rect;
use super::termviai_motion::{frame_interval, Tween};
use super::TermWindow;
use mux::localpane::LocalPane;
use mux::pane::PaneId;
use mux::tab::{PositionedPane, Tab};
use mux::Mux;
use std::collections::HashMap;
use std::time::{Duration, Instant};

const HEADER_HEIGHT: f32 = 32.0;
/// A single visual gutter in logical pixels. Mux separators occupy one cell,
/// but a terminal cell is taller than it is wide; drawing that cell verbatim
/// made horizontal gaps much larger than vertical ones.
const PANE_GUTTER: f32 = 12.0;
pub(super) const BROADCAST_TRAY_HEIGHT: f32 = 76.0;
pub(super) const BROADCAST_SURFACE_TOP_INSET: f32 = 8.0;
/// Breathing room between the terminal grid and its pane frame. Keeping this
/// independent from the cell aspect ratio makes all four edges feel balanced.
const PANE_CONTENT_PADDING: f32 = 10.0;

#[derive(Default)]
pub(super) struct TermviaiLayout {
    reserved_tray_pixels: usize,
    pane_frames: HashMap<PaneId, PaneFrameMotion>,
}

struct PaneFrameMotion {
    from: Rect,
    target: Rect,
    displayed: Rect,
    header_height: f32,
    motion: Tween,
}

impl PaneFrameMotion {
    fn sample(&self, now: Instant) -> Rect {
        let p = self.motion.value(now);
        let mix = |a, b| a + (b - a) * p;
        Rect {
            x: mix(self.from.x, self.target.x),
            y: mix(self.from.y, self.target.y),
            w: mix(self.from.w, self.target.w),
            h: mix(self.from.h, self.target.h),
        }
    }

    fn update(&mut self, target: Rect, animate: bool, now: Instant) {
        if self.target != target {
            self.from = self.sample(now);
            self.target = target;
            self.motion = Tween::new(if animate { 0. } else { 1. });
            if animate {
                // Smooth cell-sized layout increments without delaying the
                // terminal resize itself or scaling the rendered glyphs.
                self.motion.set_target(1., now, Duration::from_millis(90));
            }
        }
        self.displayed = self.sample(now);
    }
}

impl TermviaiLayout {
    fn sync_frames(&mut self, headers: &[PaneHeader], animate: bool, now: Instant) {
        self.pane_frames
            .retain(|id, _| headers.iter().any(|h| h.pane_id == *id));
        for header in headers {
            let motion =
                self.pane_frames
                    .entry(header.pane_id)
                    .or_insert_with(|| PaneFrameMotion {
                        from: header.frame,
                        target: header.frame,
                        displayed: header.frame,
                        header_height: header.rect.h,
                        motion: Tween::new(1.),
                    });
            motion.header_height = header.rect.h;
            motion.update(header.frame, animate, now);
        }
    }

    fn animating(&self, now: Instant) -> bool {
        self.pane_frames
            .values()
            .any(|frame| frame.motion.is_active(now))
    }
    /// The animation can settle between the layout pass and chrome paint. In
    /// that case the chrome has no active tween left to request the last frame.
    /// Ask for one follow-up so the next layout pass releases the old PTY inset.
    fn settled_frame_due(
        &self,
        desired_pixels: usize,
        now: Instant,
        max_fps: u64,
    ) -> Option<Instant> {
        (self.reserved_tray_pixels != desired_pixels
            || self
                .pane_frames
                .values()
                .any(|frame| frame.displayed != frame.target))
        .then(|| now + frame_interval(max_fps))
    }
}

pub(super) struct PaneHeader {
    pub pane_id: PaneId,
    pub is_active: bool,
    pub rect: Rect,
    pub frame: Rect,
}

fn header_rows(dpi: f32, cell_height: usize) -> usize {
    (HEADER_HEIGHT * dpi / 96.0 / cell_height.max(1) as f32).ceil() as usize
}

fn content_inset(rows: usize, requested: usize) -> usize {
    requested.min(rows.saturating_sub(1))
}

/// Keep the lower pane frame visually aligned with the broadcast surface.
/// The PTY remains cell-aligned; only its surrounding frame absorbs the
/// otherwise variable partial-row remainder at the bottom of the window.
fn workspace_frame_bottom(canvas_bottom: f32, tray_progress: f32) -> f32 {
    let expanded_shift = BROADCAST_TRAY_HEIGHT - BROADCAST_SURFACE_TOP_INSET;
    canvas_bottom - PANE_GUTTER - expanded_shift * tray_progress.clamp(0., 1.)
}

fn pane_frame(
    left: usize,
    top: usize,
    width: usize,
    height: usize,
    columns: usize,
    rows: usize,
    origin_x: f32,
    origin_y: f32,
    cell_width: f32,
    cell_height: f32,
    scale: f32,
) -> Rect {
    let x = (origin_x + left as f32 * cell_width) / scale;
    let y = (origin_y + top as f32 * cell_height) / scale;
    let right = x + width as f32 * cell_width / scale;
    let bottom = y + height as f32 * cell_height / scale;
    let horizontal_adjust = (cell_width / scale - PANE_GUTTER) / 2.;
    let vertical_adjust = (cell_height / scale - PANE_GUTTER) / 2.;

    // Internal edges meet around the middle of the reserved separator cell.
    // A negative adjustment intentionally widens a separator whose cell is
    // narrower than PANE_GUTTER. Outer edges retain a small canvas inset.
    let frame_left = if left > 0 {
        x - horizontal_adjust
    } else {
        x + 2.
    };
    let frame_top = if top > 0 { y - vertical_adjust } else { y + 2. };
    let frame_right = if left + width < columns {
        right + horizontal_adjust
    } else {
        right - 2.
    };
    let frame_bottom = if top + height < rows {
        bottom + vertical_adjust
    } else {
        bottom - 2.
    };
    Rect {
        x: frame_left,
        y: frame_top,
        w: (frame_right - frame_left).max(1.),
        h: (frame_bottom - frame_top).max(1.),
    }
}

impl TermWindow {
    pub(super) fn termviai_tray_pixel_height(&self) -> usize {
        if self.config.termviai_ui {
            self.termviai_layout.reserved_tray_pixels
        } else {
            0
        }
    }

    fn termviai_pane_header_rows(&self) -> usize {
        if self.config.termviai_ui {
            header_rows(
                self.dimensions.dpi as f32,
                self.render_metrics.cell_size.height as usize,
            )
        } else {
            0
        }
    }

    /// Called before rendering and safe to call after a visibility change.
    /// Unchanged geometry never triggers a PTY resize.
    pub(super) fn termviai_sync_terminal_layout(&mut self) {
        let mux = Mux::get();
        let tabs = mux
            .get_window(self.mux_window_id)
            .map(|window| window.iter_tabs().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        let rows = self.termviai_pane_header_rows();
        let scale = self.dimensions.dpi as f32 / 96.;
        let content_padding = (PANE_CONTENT_PADDING * scale).round() as usize;
        let mut changed_headers = false;
        for tab in tabs {
            // Direct SSH sessions are LocalPane PTYs too. ClientDomain and tmux
            // panes mirror a remote layout: shrinking their PTYs as if they were
            // local would feed the reduced dimensions back into the remote tree
            // and subtract the header again on every resync.
            let panes = tab.iter_panes_ignoring_zoom();
            let direct_pty_tab = !panes.is_empty()
                && panes
                    .iter()
                    .all(|pane| pane.pane.downcast_ref::<LocalPane>().is_some());
            let inset_rows = if direct_pty_tab { rows } else { 0 };
            let padding = if direct_pty_tab {
                (content_padding, content_padding)
            } else {
                (0, 0)
            };
            tab.set_proportional_resize(inset_rows > 0);
            changed_headers |=
                tab.pane_header_rows() != inset_rows || tab.pane_content_padding() != padding;
            tab.set_pane_chrome(inset_rows, padding);
        }
        if changed_headers {
            self.resize_overlays();
        }

        let tray_pixels = self.termviai_desired_tray_pixels();
        if self.termviai_layout.reserved_tray_pixels != tray_pixels {
            self.termviai_layout.reserved_tray_pixels = tray_pixels;
            if let Some(window) = self.window.clone() {
                self.apply_dimensions(&self.dimensions.clone(), None, &window);
            }
        }
        self.termviai_sync_pane_font_sizes();
        let now = Instant::now();
        let animate =
            self.termviai_ui.broadcast_tray_animating(now) || self.termviai_layout.animating(now);
        let headers = self.termviai_raw_pane_headers();
        self.termviai_layout.sync_frames(&headers, animate, now);
    }

    /// Snapshot from the same layout pass as the rendered header/frame. All
    /// coordinates remain pixel translations; terminal glyphs never stretch.
    pub(super) fn termviai_pane_visual_offset(&self, pane_id: PaneId) -> (f32, f32) {
        let scale = self.dimensions.dpi as f32 / 96.;
        let padding = self.termviai_pane_content_padding_pixels(pane_id);
        self.termviai_layout
            .pane_frames
            .get(&pane_id)
            .map(|frame| {
                (
                    (frame.displayed.x - frame.target.x) * scale + padding,
                    (frame.displayed.y - frame.target.y) * scale + padding,
                )
            })
            .unwrap_or((0., 0.))
    }

    pub(super) fn termviai_pane_content_padding_pixels(&self, pane_id: PaneId) -> f32 {
        if self.config.termviai_ui && self.termviai_layout.pane_frames.contains_key(&pane_id) {
            PANE_CONTENT_PADDING * self.dimensions.dpi as f32 / 96.
        } else {
            0.
        }
    }

    pub(super) fn termviai_pane_visual_content_height(&self, pane_id: PaneId) -> Option<f32> {
        let padding = self.termviai_pane_content_padding_pixels(pane_id);
        self.termviai_layout
            .pane_frames
            .get(&pane_id)
            .and_then(|frame| {
                (frame.displayed != frame.target).then(|| {
                    (frame.displayed.h - frame.header_height).max(0.) * self.dimensions.dpi as f32
                        / 96.
                        - padding * 2.
                })
            })
            .map(|height| height.max(0.))
    }

    pub(super) fn termviai_pane_geometry_in_motion(&self) -> bool {
        self.termviai_layout
            .pane_frames
            .values()
            .any(|frame| frame.displayed != frame.target)
    }

    fn termviai_desired_tray_pixels(&self) -> usize {
        if self.config.termviai_ui {
            (self.termviai_ui.broadcast_tray_height().max(0.) * self.dimensions.dpi as f32 / 96.)
                .round() as usize
        } else {
            0
        }
    }

    /// Run after chrome paint as well as the earlier layout sync. Do not resize
    /// here: all quads and IME coordinates in this frame must share one geometry.
    pub(super) fn termviai_finish_terminal_layout_frame(&self) {
        let next_due = self.termviai_layout.settled_frame_due(
            self.termviai_desired_tray_pixels(),
            Instant::now(),
            self.config.max_fps as u64,
        );
        self.update_next_frame_time(next_due);
    }

    /// `get_pos_panes_for_tab` uses this after substituting pane overlays.
    /// Full-tab overlays retain their full-canvas layout.
    pub(super) fn termviai_content_panes(
        &self,
        tab: &Tab,
        mut panes: Vec<PositionedPane>,
    ) -> Vec<PositionedPane> {
        // Use the tab's actual mux reservation, including when a pane has been
        // replaced with a copy/search overlay. Unsupported remote tabs reserve 0.
        let requested = tab.pane_header_rows();
        if requested == 0 {
            return panes;
        }
        let cell_height = self.render_metrics.cell_size.height as usize;
        for pane in &mut panes {
            let inset = content_inset(pane.height, requested);
            pane.top += inset;
            pane.height -= inset;
            pane.pixel_height = pane.height * cell_height;
        }
        panes
    }

    /// Outer pane-header bounds in the logical coordinate system used by TermViAI
    /// hit targets: pixels / scale, with the OS top border already removed.
    pub(super) fn termviai_pane_headers(&self) -> Vec<PaneHeader> {
        let mut headers = self.termviai_raw_pane_headers();
        for header in &mut headers {
            if let Some(frame) = self.termviai_layout.pane_frames.get(&header.pane_id) {
                if frame.target == header.frame {
                    header.rect.x += frame.displayed.x - frame.target.x;
                    header.rect.y += frame.displayed.y - frame.target.y;
                    header.rect.w += frame.displayed.w - frame.target.w;
                    header.frame = frame.displayed;
                }
            }
        }
        headers
    }

    fn termviai_raw_pane_headers(&self) -> Vec<PaneHeader> {
        if !self.config.termviai_ui || self.termviai_library_active() {
            return vec![];
        }
        let mux = Mux::get();
        let Some(tab) = mux.get_active_tab_for_window(self.mux_window_id) else {
            return vec![];
        };
        if self.tab_state(tab.tab_id()).overlay.is_some() {
            return vec![];
        }
        let scale = self.dimensions.dpi as f32 / 96.0;
        let (padding_left, padding_top) = self.padding_left_top();
        let border = self.get_os_border();
        let tab_height = if self.show_tab_bar {
            self.tab_bar_pixel_height().unwrap_or(0.0)
        } else {
            0.0
        };
        let cell_w = self.render_metrics.cell_size.width as f32;
        let cell_h = self.render_metrics.cell_size.height as f32;
        let requested = tab.pane_header_rows();
        let tab_dimensions = tab.get_size();
        let canvas_bottom = (self.dimensions.pixel_height as f32
            - border.top.get() as f32
            - border.bottom.get() as f32)
            / scale;
        let tray_progress = self.termviai_ui.broadcast_tray_height() / BROADCAST_TRAY_HEIGHT;
        let outer_bottom = workspace_frame_bottom(canvas_bottom, tray_progress);
        let origin_x = padding_left + border.left.get() as f32;
        let origin_y = tab_height + padding_top;
        tab.iter_panes()
            .into_iter()
            .filter_map(|pane| {
                let inset = content_inset(pane.height, requested);
                (inset > 0).then(|| {
                    let mut frame = pane_frame(
                        pane.left,
                        pane.top,
                        pane.width,
                        pane.height,
                        tab_dimensions.cols,
                        tab_dimensions.rows,
                        origin_x,
                        origin_y,
                        cell_w,
                        cell_h,
                        scale,
                    );
                    if pane.top + pane.height >= tab_dimensions.rows {
                        frame.h = (outer_bottom - frame.y).max(1.);
                    }
                    PaneHeader {
                        pane_id: pane.pane.pane_id(),
                        is_active: pane.is_active,
                        rect: Rect {
                            x: (padding_left
                                + border.left.get() as f32
                                + pane.left as f32 * cell_w)
                                / scale,
                            y: (tab_height + padding_top + pane.top as f32 * cell_h) / scale,
                            w: pane.width as f32 * cell_w / scale,
                            h: inset as f32 * cell_h / scale,
                        },
                        frame,
                    }
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapse_finishing_during_paint_gets_one_layout_followup() {
        use super::super::termviai_motion::Tween;
        use std::time::Duration;

        let start = Instant::now();
        let duration = Duration::from_millis(200);
        let mut motion = Tween::new(1.0);
        motion.set_target(0.0, start, duration);
        let mut layout = TermviaiLayout {
            reserved_tray_pixels: 76,
            ..Default::default()
        };

        // The layout pass ran immediately before the transition completed.
        let layout_time = start + duration - Duration::from_millis(1);
        assert!(motion.is_active(layout_time));
        assert!(layout.settled_frame_due(76, layout_time, 60).is_none());

        // Chrome paints after it completes, so its tween requests no more frames.
        let paint_time = start + duration;
        assert_eq!(motion.value(paint_time), 0.0);
        assert!(!motion.is_active(paint_time));
        let next_frame = layout.settled_frame_due(0, paint_time, 60).unwrap();
        assert!(next_frame > paint_time);

        // The follow-up applies zero reservation, then scheduling goes idle.
        layout.reserved_tray_pixels = 0;
        assert!(layout.settled_frame_due(0, next_frame, 60).is_none());
    }

    #[test]
    fn unchanged_tray_layout_does_not_schedule_idle_frames() {
        let now = Instant::now();
        for pixels in [0, 76, 114, 152] {
            let layout = TermviaiLayout {
                reserved_tray_pixels: pixels,
                ..Default::default()
            };
            assert!(layout.settled_frame_due(pixels, now, 60).is_none());
        }
        // A page change can also remove the tray during chrome paint.
        let layout = TermviaiLayout {
            reserved_tray_pixels: 114,
            ..Default::default()
        };
        assert!(layout.settled_frame_due(0, now, 30).is_some());
    }

    #[test]
    fn pane_motion_smooths_grid_steps_and_retargets_without_jumping() {
        let now = Instant::now();
        let initial = Rect {
            x: 220.,
            y: 200.,
            w: 500.,
            h: 200.,
        };
        let mut frame = PaneFrameMotion {
            from: initial,
            target: initial,
            displayed: initial,
            header_height: 44.,
            motion: Tween::new(1.),
        };
        let first = Rect {
            y: 222.,
            h: 222.,
            ..initial
        };
        frame.update(first, true, now);
        assert_eq!(frame.displayed, initial);
        let middle = now + Duration::from_millis(45);
        frame.update(first, true, middle);
        assert!(frame.displayed.y > initial.y && frame.displayed.y < first.y);
        let visible = frame.displayed;
        let second = Rect {
            y: 244.,
            h: 244.,
            ..initial
        };
        frame.update(second, true, middle);
        assert_eq!(frame.displayed, visible);
        let mut layout = TermviaiLayout::default();
        layout.pane_frames.insert(7, frame);
        // Even if the tween expires after layout and before chrome paint,
        // the unsampled final position still requests a follow-up frame.
        let end = middle + Duration::from_millis(100);
        assert!(layout.settled_frame_due(0, end, 60).is_some());
        layout
            .pane_frames
            .get_mut(&7)
            .unwrap()
            .update(second, false, end);
        assert_eq!(layout.pane_frames[&7].displayed, second);
        assert!(layout.settled_frame_due(0, end, 60).is_none());
    }

    #[test]
    fn header_reservation_tracks_dpi_and_font_size() {
        assert_eq!(header_rows(96.0, 20), 2);
        assert_eq!(header_rows(144.0, 30), 2);
        assert_eq!(header_rows(192.0, 20), 4);
        assert_eq!(header_rows(96.0, 40), 1);
    }

    #[test]
    fn tiny_panes_always_retain_a_terminal_row() {
        assert_eq!(content_inset(0, 2), 0);
        assert_eq!(content_inset(1, 2), 0);
        assert_eq!(content_inset(2, 2), 1);
        assert_eq!(content_inset(24, 2), 2);
        assert_eq!(content_inset(24, 0), 0);
    }

    #[test]
    fn visual_gutters_ignore_terminal_cell_aspect_ratio() {
        let top_left = pane_frame(0, 0, 49, 19, 100, 40, 220., 52., 10., 20., 1.);
        let top_right = pane_frame(50, 0, 50, 19, 100, 40, 220., 52., 10., 20., 1.);
        let bottom_left = pane_frame(0, 20, 49, 20, 100, 40, 220., 52., 10., 20., 1.);
        let horizontal = bottom_left.y - (top_left.y + top_left.h);
        let vertical = top_right.x - (top_left.x + top_left.w);
        assert!((horizontal - PANE_GUTTER).abs() < f32::EPSILON);
        assert!((vertical - PANE_GUTTER).abs() < f32::EPSILON);
    }

    #[test]
    fn workspace_bottom_gutter_matches_the_broadcast_surface() {
        let canvas_bottom = 1000.;
        let collapsed_bottom = workspace_frame_bottom(canvas_bottom, 0.);
        assert_eq!(canvas_bottom - collapsed_bottom, PANE_GUTTER);

        let expanded_bottom = workspace_frame_bottom(canvas_bottom, 1.);
        let surface_top =
            canvas_bottom - BROADCAST_TRAY_HEIGHT + BROADCAST_SURFACE_TOP_INSET;
        assert_eq!(surface_top - expanded_bottom, PANE_GUTTER);
    }
}
