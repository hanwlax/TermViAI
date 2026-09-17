//! Geometry shared by terminal-tab docking hit testing and its drop preview.

use mux::tab::{SplitDirection, SplitRequest, SplitSize};

/// Coordinates are logical pixels, before applying the window's DPI scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DockSide {
    Left,
    Right,
    Top,
    Bottom,
}

impl Rect {
    /// Half-open bounds keep adjacent panes from claiming the same point.
    pub fn contains(self, x: f32, y: f32) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && self.w.is_finite()
            && self.h.is_finite()
            && self.w > 0.0
            && self.h > 0.0
            && x >= self.x
            && y >= self.y
            && x < self.x + self.w
            && y < self.y + self.h
    }

    /// Compare distances relative to each axis so a wide or tall pane still
    /// offers useful targets on all four sides. Ties prefer horizontal splits.
    pub fn dock_side(self, x: f32, y: f32) -> Option<DockSide> {
        if !self.contains(x, y) {
            return None;
        }
        let u = (x - self.x) / self.w;
        let v = (y - self.y) / self.h;
        let horizontal = if u <= 0.5 {
            (u, DockSide::Left)
        } else {
            (1.0 - u, DockSide::Right)
        };
        let vertical = if v <= 0.5 {
            (v, DockSide::Top)
        } else {
            (1.0 - v, DockSide::Bottom)
        };
        Some(if horizontal.0 <= vertical.0 {
            horizontal.1
        } else {
            vertical.1
        })
    }

    /// The half occupied by the moved pane; decorative insets belong to paint.
    pub fn preview(self, side: DockSide) -> Self {
        match side {
            DockSide::Left => Self {
                w: self.w * 0.5,
                ..self
            },
            DockSide::Right => Self {
                x: self.x + self.w * 0.5,
                w: self.w * 0.5,
                ..self
            },
            DockSide::Top => Self {
                h: self.h * 0.5,
                ..self
            },
            DockSide::Bottom => Self {
                y: self.y + self.h * 0.5,
                h: self.h * 0.5,
                ..self
            },
        }
    }
}

impl DockSide {
    pub fn split_request(self) -> SplitRequest {
        SplitRequest {
            direction: match self {
                Self::Left | Self::Right => SplitDirection::Horizontal,
                Self::Top | Self::Bottom => SplitDirection::Vertical,
            },
            target_is_second: matches!(self, Self::Right | Self::Bottom),
            top_level: false,
            size: SplitSize::Percent(50),
        }
    }
}

/// Small pointer jitter remains a tab click. Use logical coordinates so the
/// drag threshold remains six pixels at every display scale.
pub(super) fn drag_threshold_reached(start: (f32, f32), current: (f32, f32)) -> bool {
    let dx = current.0 - start.0;
    let dy = current.1 - start.1;
    dx.is_finite() && dy.is_finite() && dx * dx + dy * dy >= 36.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pane() -> Rect {
        Rect {
            x: 220.0,
            y: 96.0,
            w: 800.0,
            h: 400.0,
        }
    }

    #[test]
    fn docking_bounds_do_not_claim_neighboring_panes() {
        let rect = pane();
        assert_eq!(rect.dock_side(220.0, 296.0), Some(DockSide::Left));
        assert_eq!(rect.dock_side(620.0, 96.0), Some(DockSide::Top));
        assert_eq!(rect.dock_side(1019.0, 296.0), Some(DockSide::Right));
        assert_eq!(rect.dock_side(620.0, 495.0), Some(DockSide::Bottom));
        for point in [
            (219.0, 296.0),
            (620.0, 95.0),
            (1020.0, 296.0),
            (620.0, 496.0),
            (f32::NAN, 296.0),
            (620.0, f32::INFINITY),
        ] {
            assert_eq!(rect.dock_side(point.0, point.1), None);
        }
        assert_eq!(Rect { w: 0.0, ..rect }.dock_side(220.0, 296.0), None);
        assert_eq!(Rect { h: -1.0, ..rect }.dock_side(220.0, 296.0), None);
    }

    #[test]
    fn normalized_edges_and_preview_agree_with_split_placement() {
        for (w, h) in [(1200.0, 160.0), (160.0, 1200.0)] {
            let rect = Rect { w, h, ..pane() };
            for (u, v, side) in [
                (0.1, 0.4, DockSide::Left),
                (0.9, 0.4, DockSide::Right),
                (0.4, 0.1, DockSide::Top),
                (0.4, 0.9, DockSide::Bottom),
            ] {
                let x = rect.x + w * u;
                let y = rect.y + h * v;
                assert_eq!(rect.dock_side(x, y), Some(side));
                assert!(rect.preview(side).contains(x, y));
                let preview = rect.preview(side);
                let request = side.split_request();
                assert_eq!(request.size, SplitSize::Percent(50));
                assert!(!request.top_level);
                match side {
                    DockSide::Left | DockSide::Right => {
                        assert_eq!(request.direction, SplitDirection::Horizontal);
                        assert_eq!(preview.w, w * 0.5);
                        assert_eq!(preview.h, h);
                    }
                    DockSide::Top | DockSide::Bottom => {
                        assert_eq!(request.direction, SplitDirection::Vertical);
                        assert_eq!(preview.w, w);
                        assert_eq!(preview.h, h * 0.5);
                    }
                }
                assert_eq!(
                    request.target_is_second,
                    matches!(side, DockSide::Right | DockSide::Bottom)
                );
            }
        }
        assert_eq!(pane().dock_side(620.0, 296.0), Some(DockSide::Left));
    }

    #[test]
    fn click_jitter_and_diagonal_drags_use_the_same_threshold() {
        let start = (220.0, 26.0);
        assert!(!drag_threshold_reached(start, start));
        assert!(!drag_threshold_reached(start, (225.9, 26.0)));
        assert!(!drag_threshold_reached(start, (224.0, 30.0)));
        assert!(drag_threshold_reached(start, (226.0, 26.0)));
        assert!(drag_threshold_reached(start, (220.0, 20.0)));
        assert!(drag_threshold_reached(start, (225.0, 30.0)));
        assert!(!drag_threshold_reached(start, (f32::NAN, 30.0)));
    }
}
