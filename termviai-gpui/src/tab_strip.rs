//! Geometry shared by painting, scrolling, revealing and drag insertion.
use gpui::{KeyBinding, NoAction};

pub const TAB_WIDTH: f32 = 242.;
pub const GAP: f32 = 6.;
pub const STRIDE: f32 = TAB_WIDTH + GAP;
pub const PICKER_CONTEXT: &str = "TermViAITabPicker";

pub fn bindings() -> Vec<KeyBinding> {
    let mut keys = vec![
        KeyBinding::new("ctrl-tab", NoAction, Some("TermViAI")),
        KeyBinding::new("ctrl-shift-tab", NoAction, Some("TermViAI")),
    ];
    for key in ["up", "down", "enter", "escape"] {
        // Match at Input depth: a parent-only binding loses to Input's actions.
        for context in [PICKER_CONTEXT, "TermViAITabPicker > Input"] {
            keys.push(KeyBinding::new(key, NoAction, Some(context)));
        }
    }
    keys
}

#[derive(Clone, Copy, Default, Debug)]
pub struct Metrics {
    pub overflow: bool,
    pub viewport: f32,
    pub content: f32,
}
impl Metrics {
    pub fn new(window: f32, count: usize) -> Self {
        let content = if count == 0 {
            0.
        } else {
            count as f32 * STRIDE - GAP
        };
        // Left padding 8, brand 152, add 30, drag space 26, controls 126,
        // plus four 4px gaps. Overflow adds a 56px list button, its gap,
        // and two 26px arrows with two 4px rail gaps.
        let available = (window - 358.).max(0.);
        let overflow = content > available;
        Self {
            overflow,
            content,
            viewport: if overflow {
                (available - 120.).max(1.)
            } else {
                content
            },
        }
    }
    pub fn max_offset(self) -> f32 {
        (self.content - self.viewport).max(0.)
    }
    pub fn clamp(self, offset: f32) -> f32 {
        offset.clamp(0., self.max_offset())
    }
    pub fn reveal(self, offset: f32, index: usize) -> f32 {
        let start = index as f32 * STRIDE;
        let end = start + TAB_WIDTH;
        let offset = self.clamp(offset);
        self.clamp(if start < offset {
            start
        } else if end > offset + self.viewport {
            end - self.viewport
        } else {
            offset
        })
    }
    /// Returns the tab under the pointer and which side of it receives the move.
    pub fn insertion(self, offset: f32, x: f32, count: usize) -> Option<(usize, bool)> {
        if count == 0 || x < 0. || x > self.viewport {
            return None;
        }
        let position = self.clamp(offset) + x;
        let index = ((position / STRIDE) as usize).min(count - 1);
        Some((index, position - index as f32 * STRIDE >= TAB_WIDTH / 2.))
    }
    pub fn edge(self, x: f32) -> i8 {
        if !self.overflow || x < 0. || x > self.viewport {
            0
        } else if x < 28. {
            -1
        } else if x > self.viewport - 28. {
            1
        } else {
            0
        }
    }
}

pub fn next_index(len: usize, active: Option<usize>, reverse: bool) -> Option<usize> {
    if len == 0 {
        return None;
    }
    Some(match active.filter(|i| *i < len) {
        Some(i) if reverse => (i + len - 1) % len,
        Some(i) => (i + 1) % len,
        None if reverse => len - 1,
        None => 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn overflow_preserves_tab_width_and_reserves_controls() {
        assert!(!Metrics::new(1280., 2).overflow);
        assert_eq!(Metrics::new(1280., 2).viewport, 490.);
        let m = Metrics::new(780., 16);
        assert!(m.overflow);
        assert_eq!(m.viewport, 302.);
        assert!(m.viewport >= TAB_WIDTH);
        assert_eq!(m.content, 16. * STRIDE - GAP);
        assert!(!Metrics::new(780., 0).overflow);
    }
    #[test]
    fn reveal_handles_last_first_close_and_resize_without_resetting_manual_scroll() {
        let m = Metrics::new(1280., 8);
        assert_eq!(m.reveal(0., 7), m.max_offset());
        assert_eq!(m.reveal(m.max_offset(), 0), 0.);
        assert_eq!(m.reveal(STRIDE, 2), STRIDE);
        assert_eq!(Metrics::new(1280., 2).clamp(m.max_offset()), 0.);
        let narrow = Metrics::new(780., 8);
        assert_eq!(narrow.reveal(m.max_offset(), 7), narrow.max_offset());
    }
    #[test]
    fn insertion_tracks_scroll_and_edges_do_not_leak_outside_rail() {
        let m = Metrics::new(1280., 16);
        assert_eq!(m.insertion(STRIDE * 5., 10., 16), Some((5, false)));
        assert_eq!(m.insertion(STRIDE * 5., 200., 16), Some((5, true)));
        assert_eq!(m.insertion(0., -1., 16), None);
        assert_eq!(m.edge(-1.), 0);
        assert_eq!(m.edge(10.), -1);
        assert_eq!(m.edge(m.viewport - 10.), 1);
        assert_eq!(m.edge(m.viewport + 1.), 0);
    }
    #[test]
    fn keyboard_cycles_both_directions_and_handles_no_tabs() {
        assert_eq!(next_index(8, Some(7), false), Some(0));
        assert_eq!(next_index(8, Some(0), true), Some(7));
        assert_eq!(next_index(8, None, false), Some(0));
        assert_eq!(next_index(8, None, true), Some(7));
        assert_eq!(next_index(0, None, false), None);
    }
    #[test]
    fn picker_bindings_override_input_actions_without_changing_regular_inputs() {
        use gpui::{KeyContext, Keymap, Keystroke, actions};
        actions!(tab_strip_test, [InputMove]);
        let mut map = Keymap::default();
        map.add_bindings([KeyBinding::new("down", InputMove, Some("Input"))]);
        map.add_bindings(bindings());
        let stroke = Keystroke::parse("down").unwrap();
        let contexts = ["TermViAI", PICKER_CONTEXT, "Input"].map(|s| KeyContext::parse(s).unwrap());
        assert!(
            map.bindings_for_input(&[stroke.clone()], &contexts)
                .0
                .is_empty()
        );
        assert_eq!(map.bindings_for_input(&[stroke], &contexts[2..]).0.len(), 1);
    }
}
