//! Terminal font preferences and pane-local, non-persistent session zoom.
//!
//! Split rectangles remain in the window's layout grid. Each pane has its own
//! font grid inside that rectangle; rendering, PTY size, mouse and IME all use
//! the same metrics. Session overrides are never serialized.
use super::TermWindow;
use crate::hosts::HostStore;
use crate::utilsprites::RenderMetrics;
use anyhow::{ensure, Context};
use mux::localpane::LocalPane;
use mux::pane::PaneId;
use mux::Mux;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::rc::Rc;
use wezterm_font::FontConfiguration;
use wezterm_term::ClickPosition;
use window::WindowOps;

pub(super) const MIN_FONT_SIZE: f64 = 6.0;
pub(super) const MAX_FONT_SIZE: f64 = 48.0;
pub(super) const DEFAULT_SSH_KEEPALIVE_INTERVAL: u64 = 30;
pub(super) const MAX_SSH_KEEPALIVE_INTERVAL: u64 = 86_400;

#[derive(Default, Serialize, Deserialize)]
struct Preferences {
    terminal_font_size: Option<f64>,
    ssh_keepalive_interval_seconds: Option<u64>,
    #[serde(flatten)]
    other: serde_json::Map<String, serde_json::Value>,
}

impl Preferences {
    fn load(path: &Path) -> anyhow::Result<Self> {
        match std::fs::File::open(path) {
            Ok(file) => {
                serde_json::from_reader(file).context("Could not read terminal preferences")
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(err) => Err(err).context("Could not open terminal preferences"),
        }
    }

    fn save(&self, path: &Path) -> anyhow::Result<()> {
        let dir = path.parent().context("Missing preferences directory")?;
        std::fs::create_dir_all(dir)?;
        let mut file = tempfile::NamedTempFile::new_in(dir)?;
        serde_json::to_writer_pretty(&mut file, self)?;
        file.write_all(b"\n")?;
        file.as_file().sync_all()?;
        file.persist(path).map_err(|err| err.error)?;
        Ok(())
    }
}

fn valid_size(size: f64) -> bool {
    size.is_finite() && (MIN_FONT_SIZE..=MAX_FONT_SIZE).contains(&size)
}

fn stepped_size(size: f64, delta: i16) -> f64 {
    (size + f64::from(delta.signum())).clamp(MIN_FONT_SIZE, MAX_FONT_SIZE)
}

#[derive(Clone)]
pub(super) struct PaneFont {
    pub fonts: Rc<FontConfiguration>,
    pub metrics: RenderMetrics,
    size: f64,
    dpi: usize,
    generation: usize,
}

pub(super) struct TermviaiFonts {
    global_size: Option<f64>,
    ssh_keepalive_interval: u64,
    session_sizes: HashMap<PaneId, f64>,
    contexts: HashMap<PaneId, PaneFont>,
}

impl Default for TermviaiFonts {
    fn default() -> Self {
        Self {
            global_size: None,
            ssh_keepalive_interval: DEFAULT_SSH_KEEPALIVE_INTERVAL,
            session_sizes: HashMap::new(),
            contexts: HashMap::new(),
        }
    }
}

impl TermviaiFonts {
    pub fn load() -> Self {
        let path = HostStore::path().with_file_name("settings.json");
        let (global_size, ssh_keepalive_interval) = match Preferences::load(&path) {
            Ok(prefs) => (
                prefs.terminal_font_size.filter(|size| valid_size(*size)),
                prefs
                    .ssh_keepalive_interval_seconds
                    .filter(|interval| *interval <= MAX_SSH_KEEPALIVE_INTERVAL)
                    .unwrap_or(DEFAULT_SSH_KEEPALIVE_INTERVAL),
            ),
            Err(err) => {
                log::error!(
                    "{:#}; preserving preferences and using the configured font",
                    err
                );
                (None, DEFAULT_SSH_KEEPALIVE_INTERVAL)
            }
        };
        Self {
            global_size,
            ssh_keepalive_interval,
            ..Self::default()
        }
    }

    pub fn global_size(&self, configured: f64) -> f64 {
        self.global_size.unwrap_or(configured)
    }

    pub fn ssh_keepalive_interval(&self) -> u64 {
        self.ssh_keepalive_interval
    }
}

pub(super) fn session_visible_rows(
    viewport_rows: usize,
    cell_height: isize,
    visible_height: Option<f32>,
) -> usize {
    visible_height
        .map(|height| (height.max(0.0) / cell_height.max(1) as f32).floor() as usize)
        .unwrap_or(viewport_rows)
        .min(viewport_rows)
}

pub(super) fn session_cell_position(
    local_x: isize,
    local_y: isize,
    cell_width: isize,
    cell_height: isize,
    mouse_grabbed: bool,
) -> ClickPosition {
    let cell_width = cell_width.max(1);
    let cell_height = cell_height.max(1);
    let col = local_x.max(0) as f32 / cell_width as f32;
    ClickPosition {
        column: if mouse_grabbed {
            col.trunc()
        } else {
            col.round()
        } as usize,
        row: (local_y.max(0) / cell_height) as i64,
        x_pixel_offset: if local_x < 0 {
            local_x
        } else {
            local_x % cell_width
        },
        y_pixel_offset: if local_y < 0 {
            local_y
        } else {
            local_y % cell_height
        },
    }
}

impl TermWindow {
    pub(super) fn termviai_global_font_size(&self) -> f64 {
        self.termviai_fonts.global_size(self.config.font_size)
    }

    pub(super) fn termviai_ssh_keepalive_interval(&self) -> u64 {
        self.termviai_fonts.ssh_keepalive_interval()
    }

    pub(super) fn termviai_set_preferences(
        &mut self,
        size: f64,
        ssh_keepalive_interval: u64,
    ) -> anyhow::Result<()> {
        ensure!(valid_size(size), "Font size must be between 6 and 48 pt.");
        ensure!(
            ssh_keepalive_interval <= MAX_SSH_KEEPALIVE_INTERVAL,
            "SSH keepalive interval must be between 0 and 86400 seconds."
        );
        // Read before replacing so a malformed file is preserved and unrelated
        // future settings survive this update.
        let path = HostStore::path().with_file_name("settings.json");
        let mut prefs = Preferences::load(&path)?;
        prefs.terminal_font_size = Some(size);
        prefs.ssh_keepalive_interval_seconds = Some(ssh_keepalive_interval);
        prefs.save(&path)?;
        self.termviai_apply_preferences(size, ssh_keepalive_interval);
        for other in crate::frontend::front_end().gui_windows() {
            let current_id = self.mux_window_id;
            other
                .window
                .notify(super::TermWindowNotif::Apply(Box::new(move |window| {
                    if window.config.termviai_ui && window.mux_window_id != current_id {
                        window.termviai_apply_preferences(size, ssh_keepalive_interval);
                    }
                })));
        }
        Ok(())
    }

    fn termviai_apply_preferences(&mut self, size: f64, ssh_keepalive_interval: u64) {
        self.termviai_fonts.global_size = Some(size);
        self.termviai_fonts.ssh_keepalive_interval = ssh_keepalive_interval;
        let dimensions = self.dimensions;
        self.apply_scale_change(&dimensions, size / self.config.font_size);
        if let Some(window) = self.window.clone() {
            self.apply_dimensions(&dimensions, None, &window);
            self.termviai_sync_terminal_layout();
            window.invalidate();
        }
    }

    /// Copy/search overlays inherit their underlying session font.
    pub(super) fn termviai_font_source_pane(&self, pane_id: PaneId) -> PaneId {
        self.pane_state
            .borrow()
            .iter()
            .find_map(|(source, state)| {
                state
                    .overlay
                    .as_ref()
                    .filter(|overlay| overlay.pane.pane_id() == pane_id)
                    .map(|_| *source)
            })
            .unwrap_or(pane_id)
    }

    pub(super) fn termviai_pane_font(&mut self, pane_id: PaneId) -> anyhow::Result<Option<PaneFont>> {
        let pane_id = self.termviai_font_source_pane(pane_id);
        if !self.config.termviai_ui {
            return Ok(None);
        }
        let Some(size) = self.termviai_fonts.session_sizes.get(&pane_id).copied() else {
            return Ok(None);
        };
        let dpi = self.dimensions.dpi;
        let generation = self.config.generation();
        if let Some(context) = self.termviai_fonts.contexts.get(&pane_id) {
            if context.size == size && context.dpi == dpi && context.generation == generation {
                return Ok(Some(context.clone()));
            }
        }
        let fonts = Rc::new(FontConfiguration::new(Some(self.config.clone()), dpi)?);
        fonts.change_scaling(size / self.config.font_size, dpi);
        let metrics = RenderMetrics::new(&fonts)?;
        let context = PaneFont {
            fonts,
            metrics,
            size,
            dpi,
            generation,
        };
        self.termviai_fonts.contexts.insert(pane_id, context.clone());
        Ok(Some(context))
    }

    pub(super) fn termviai_pane_metrics(&self, pane_id: PaneId) -> RenderMetrics {
        let pane_id = self.termviai_font_source_pane(pane_id);
        self.termviai_fonts
            .contexts
            .get(&pane_id)
            .map(|context| context.metrics)
            .unwrap_or(self.render_metrics)
    }

    pub(super) fn termviai_adjust_session_font(&mut self, delta: i16) -> anyhow::Result<()> {
        let pane = self
            .get_active_pane_or_overlay()
            .context("No active terminal")?;
        ensure!(
            pane.downcast_ref::<LocalPane>().is_some(),
            "Session font zoom requires a directly connected terminal."
        );
        let pane_id = pane.pane_id();
        let prior = self
            .termviai_fonts
            .session_sizes
            .get(&pane_id)
            .copied()
            .unwrap_or(self.config.font_size * self.fonts.get_font_scale());
        let next = stepped_size(prior, delta);
        if next == prior {
            return Ok(());
        }
        self.termviai_fonts.session_sizes.insert(pane_id, next);
        if let Err(err) = self.termviai_pane_font(pane_id) {
            self.termviai_fonts.session_sizes.insert(pane_id, prior);
            return Err(err);
        }
        self.quad_generation += 1;
        self.termviai_sync_pane_font_sizes();
        self.resize_overlays();
        if let Some(window) = &self.window {
            window.invalidate();
        }
        Ok(())
    }

    /// Called after mux layout updates, including split/zoom/tab moves. Only
    /// overridden panes resize; unchanged grids do not send SSH window changes.
    pub(super) fn termviai_sync_pane_font_sizes(&mut self) {
        if !self.config.termviai_ui || self.termviai_fonts.session_sizes.is_empty() {
            return;
        }
        let mux = Mux::get();
        self.termviai_fonts
            .session_sizes
            .retain(|id, _| mux.get_pane(*id).is_some());
        let sessions = &self.termviai_fonts.session_sizes;
        self.termviai_fonts
            .contexts
            .retain(|id, _| sessions.contains_key(id));
        let tabs = mux
            .get_window(self.mux_window_id)
            .map(|window| window.iter_tabs().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        for tab in tabs {
            for pos in tab.iter_panes() {
                let id = pos.pane.pane_id();
                let context = match self.termviai_pane_font(id) {
                    Ok(Some(context)) => context,
                    Ok(None) => continue,
                    Err(err) => {
                        log::error!("Session font: {:#}", err);
                        continue;
                    }
                };
                tab.set_pane_cell_size(
                    id,
                    Some((
                        context.metrics.cell_size.width.max(1) as usize,
                        context.metrics.cell_size.height.max(1) as usize,
                    )),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn session_steps_are_bounded_and_never_saved() {
        assert_eq!(stepped_size(12.0, 1), 13.0);
        assert_eq!(stepped_size(12.0, -1), 11.0);
        assert_eq!(stepped_size(6.0, -1), 6.0);
        assert_eq!(stepped_size(48.0, 1), 48.0);
        assert!(!valid_size(f64::NAN));
        let mut state = TermviaiFonts::default();
        state.session_sizes.insert(42, 18.0);
        assert_eq!(state.global_size(12.0), 12.0);
        assert!(!serde_json::to_string(&Preferences::default())
            .unwrap()
            .contains("18"));
    }
    #[test]
    fn animated_height_never_draws_a_partial_terminal_row() {
        assert_eq!(session_visible_rows(24, 20, None), 24);
        assert_eq!(session_visible_rows(24, 20, Some(399.9)), 19);
        assert_eq!(session_visible_rows(24, 20, Some(400.0)), 20);
        assert_eq!(session_visible_rows(24, 20, Some(480.0)), 24);
        assert_eq!(session_visible_rows(24, 20, Some(1000.0)), 24);
        assert_eq!(session_visible_rows(24, 20, Some(-1.0)), 0);
        assert_eq!(session_visible_rows(16, 30, Some(400.0)), 13);
    }

    #[test]
    fn pointer_coordinates_follow_the_independent_font_grid() {
        let small = session_cell_position(145, 95, 10, 20, true);
        assert_eq!((small.column, small.row), (14, 4));
        let large = session_cell_position(145, 95, 15, 30, true);
        assert_eq!((large.column, large.row), (9, 3));
        assert_eq!((large.x_pixel_offset, large.y_pixel_offset), (10, 5));
        assert_eq!(session_cell_position(145, 95, 15, 30, false).column, 10);
        let outside = session_cell_position(-8, -12, 15, 30, false);
        assert_eq!((outside.column, outside.row), (0, 0));
        assert_eq!((outside.x_pixel_offset, outside.y_pixel_offset), (-8, -12));
    }

    #[test]
    fn identical_text_in_different_fonts_cannot_share_a_shape_cache_entry() {
        let style = config::TextStyle::default();
        let small = crate::shapecache::BorrowedShapeCacheKey {
            font_id: 1,
            style: &style,
            text: "same",
        };
        let large = crate::shapecache::BorrowedShapeCacheKey {
            font_id: 2,
            style: &style,
            text: "same",
        };
        assert_ne!(small, large);
        assert_ne!(small.to_owned(), large.to_owned());
    }

    #[test]
    fn settings_round_trip_preserves_other_preferences_and_corruption() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{"terminal_font_size":14.5,"appearance":"mocha"}"#).unwrap();
        let mut prefs = Preferences::load(&path).unwrap();
        assert_eq!(prefs.terminal_font_size, Some(14.5));
        prefs.terminal_font_size = Some(18.0);
        prefs.ssh_keepalive_interval_seconds = Some(45);
        prefs.save(&path).unwrap();
        let loaded = Preferences::load(&path).unwrap();
        assert_eq!(loaded.terminal_font_size, Some(18.0));
        assert_eq!(loaded.ssh_keepalive_interval_seconds, Some(45));
        assert_eq!(loaded.other["appearance"], "mocha");
        std::fs::write(&path, "broken").unwrap();
        assert!(Preferences::load(&path).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "broken");
    }

    #[test]
    fn keepalive_defaults_to_thirty_seconds_and_accepts_disabled() {
        assert_eq!(
            TermviaiFonts::default().ssh_keepalive_interval(),
            DEFAULT_SSH_KEEPALIVE_INTERVAL
        );
        assert!(DEFAULT_SSH_KEEPALIVE_INTERVAL <= MAX_SSH_KEEPALIVE_INTERVAL);
    }
}
