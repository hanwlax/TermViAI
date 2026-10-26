//! Cache shaping, not pixels or geometry. Resizing changes clipping/layout but
//! not the glyph sequence of an unchanged label. Fallback resolution invalidates
//! via shape_generation; font IDs separate DPI, size, weight and configuration.
use lfucache::LfuCache;
use std::rc::Rc;
use wezterm_font::shaper::GlyphInfo;
use wezterm_font::{LoadedFont, LoadedFontId};

pub(super) struct UiTextCache {
    generation: usize,
    entries: LfuCache<(LoadedFontId, String), Rc<Vec<GlyphInfo>>>,
}
impl UiTextCache {
    pub fn new(config: &config::ConfigHandle) -> Self {
        Self {
            generation: 0,
            entries: LfuCache::new("gui.ui_shape.hit", "gui.ui_shape.miss", |_| 512, config),
        }
    }
    pub fn shape<F: FnOnce() + Send + 'static>(
        &mut self,
        font: &LoadedFont,
        text: &str,
        generation: usize,
        completion: F,
    ) -> anyhow::Result<Rc<Vec<GlyphInfo>>> {
        if self.generation != generation {
            self.entries.clear();
            self.generation = generation;
        }
        let key = (font.id(), text.to_owned());
        if let Some(value) = self.entries.get(&key) {
            return Ok(Rc::clone(value));
        }
        let _timing = ::window::resize_diagnostics::Span::new("ui_shape_miss");
        let shaped = Rc::new(font.shape(
            text,
            completion,
            crate::customglyph::BlockKey::filter_out_synthetic,
            None,
            wezterm_bidi::Direction::LeftToRight,
            None,
            None,
        )?);
        // Very long unique commands should not displace the persistent labels.
        if text.len() <= 2048 {
            self.entries.put(key, Rc::clone(&shaped));
        }
        Ok(shaped)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cached_glyphs_match_shaping_and_invalidate_for_fallback_and_dpi() {
        let config = config::ConfigHandle::default_config();
        let fonts = wezterm_font::FontConfiguration::new(Some(config.clone()), 96).unwrap();
        let mut cache = UiTextCache::new(&config);
        for text in [
            "Monitor",
            "NPU 0 · Die 1",
            "Working directory: /workspace",
            "进程详情",
            "e\u{301}",
        ] {
            let font = fonts.ui_font(13, false).unwrap();
            let a = cache.shape(&font, text, 0, || {}).unwrap();
            let b = cache.shape(&font, text, 0, || {}).unwrap();
            assert!(Rc::ptr_eq(&a, &b));
            assert_eq!(
                *a,
                font.shape(
                    text,
                    || {},
                    crate::customglyph::BlockKey::filter_out_synthetic,
                    None,
                    wezterm_bidi::Direction::LeftToRight,
                    None,
                    None
                )
                .unwrap()
            );
            let c = cache.shape(&font, text, 1, || {}).unwrap();
            assert!(!Rc::ptr_eq(&a, &c));
            let bold = fonts.ui_font(13, true).unwrap();
            assert!(!Rc::ptr_eq(
                &c,
                &cache.shape(&bold, text, 1, || {}).unwrap()
            ));
        }
        let a = cache
            .shape(&fonts.ui_font(13, false).unwrap(), "Monitor", 1, || {})
            .unwrap();
        fonts.change_scaling(1., 144);
        let b = cache
            .shape(&fonts.ui_font(13, false).unwrap(), "Monitor", 1, || {})
            .unwrap();
        assert!(!Rc::ptr_eq(&a, &b));
    }
}
