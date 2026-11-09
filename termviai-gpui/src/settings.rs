use crate::{hosts::HostStore, theme::Theme};
use anyhow::{Context, ensure};
use serde::{Deserialize, Serialize};
use std::io::Write;

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub terminal_font_size: Option<f64>,
    pub ssh_keepalive_interval_seconds: Option<u64>,
    pub theme: Theme,
    pub confirm_close_tabs: Option<bool>,
    pub audible_bell: Option<bool>,
    #[serde(flatten)]
    other: serde_json::Map<String, serde_json::Value>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            terminal_font_size: None,
            ssh_keepalive_interval_seconds: None,
            theme: Theme::Ink,
            confirm_close_tabs: None,
            audible_bell: None,
            other: Default::default(),
        }
    }
}
impl Settings {
    pub fn font_size(&self) -> f32 {
        self.terminal_font_size.unwrap_or(12.).clamp(6., 48.) as f32 * 96. / 72.
    }
    pub fn keepalive(&self) -> u64 {
        self.ssh_keepalive_interval_seconds.unwrap_or(30).min(86400)
    }
    pub fn confirm_close(&self) -> bool {
        self.confirm_close_tabs.unwrap_or(true)
    }
    pub fn bell(&self) -> bool {
        self.audible_bell.unwrap_or(false)
    }
    pub fn load() -> anyhow::Result<Self> {
        let path = HostStore::path().with_file_name("settings.json");
        match std::fs::read(path) {
            Ok(b) => serde_json::from_slice(&b)
                .context("Could not read settings; original file preserved"),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }
    pub fn save(&self) -> anyhow::Result<()> {
        ensure!(
            self.terminal_font_size
                .is_none_or(|v| v.is_finite() && (6. ..=48.).contains(&v)),
            "Font size must be 6–48 pt"
        );
        ensure!(
            self.keepalive() == self.ssh_keepalive_interval_seconds.unwrap_or(30),
            "Keepalive must be 0–86400 seconds"
        );
        // Merge unknown fields from the latest file, preserving other modules' settings.
        let mut current = Self::load()?;
        current.terminal_font_size = self.terminal_font_size;
        current.ssh_keepalive_interval_seconds = self.ssh_keepalive_interval_seconds;
        current.theme = self.theme;
        current.confirm_close_tabs = self.confirm_close_tabs;
        current.audible_bell = self.audible_bell;
        let path = HostStore::path().with_file_name("settings.json");
        let dir = path.parent().context("Missing settings directory")?;
        std::fs::create_dir_all(dir)?;
        let mut file = tempfile::NamedTempFile::new_in(dir)?;
        serde_json::to_writer_pretty(&mut file, &current)?;
        file.write_all(b"\n")?;
        file.as_file().sync_all()?;
        file.persist(path).map_err(|e| e.error)?;
        Ok(())
    }
}
