use crate::{hosts::HostStore, terminal_keys::ShortcutMap, theme::Theme};
use anyhow::{Context, ensure};
use serde::{Deserialize, Serialize};
use std::{
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub terminal_font_size: Option<f64>,
    pub ssh_keepalive_interval_seconds: Option<u64>,
    pub theme: Theme,
    pub confirm_close_tabs: Option<bool>,
    pub audible_bell: Option<bool>,
    pub npu_monitor_enabled: bool,
    pub keyboard_shortcuts: ShortcutMap,
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
            npu_monitor_enabled: true,
            keyboard_shortcuts: Default::default(),
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
    fn path() -> PathBuf {
        HostStore::path().with_file_name("settings.json")
    }
    pub fn load() -> anyhow::Result<Self> {
        Self::load_at(&Self::path())
    }
    fn load_at(path: &Path) -> anyhow::Result<Self> {
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
        Self::update_at(&Self::path(), |current| {
            current.terminal_font_size = self.terminal_font_size;
            current.ssh_keepalive_interval_seconds = self.ssh_keepalive_interval_seconds;
            current.theme = self.theme;
            current.confirm_close_tabs = self.confirm_close_tabs;
            current.audible_bell = self.audible_bell;
            current.npu_monitor_enabled = self.npu_monitor_enabled;
            Ok(())
        })
    }

    pub fn save_shortcuts(&mut self, draft: &ShortcutMap) -> anyhow::Result<()> {
        self.save_shortcuts_at(&Self::path(), draft)
    }

    fn save_shortcuts_at(&mut self, path: &Path, draft: &ShortcutMap) -> anyhow::Result<()> {
        Self::update_at(path, |current| {
            ensure!(
                current.keyboard_shortcuts == self.keyboard_shortcuts,
                "Bindings changed in another window. Reopen Keyboard shortcuts to load them."
            );
            current.keyboard_shortcuts = draft.clone();
            Ok(())
        })?;
        self.keyboard_shortcuts = draft.clone();
        Ok(())
    }

    fn update_at(
        path: &Path,
        change: impl FnOnce(&mut Self) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        let dir = path.parent().context("Missing settings directory")?;
        std::fs::create_dir_all(dir)?;
        let lock = std::fs::File::options()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path.with_extension("lock"))?;
        lock.lock()?;
        let mut current = Self::load_at(path)?;
        change(&mut current)?;
        let mut file = tempfile::NamedTempFile::new_in(dir)?;
        serde_json::to_writer_pretty(&mut file, &current)?;
        file.write_all(b"\n")?;
        file.as_file().sync_all()?;
        file.persist(path).map_err(|e| e.error)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_binding_survives_a_fresh_process_and_restoring_defaults() {
        use crate::terminal_keys::{ROWS, Shortcut};
        const CHILD: &str = "TERMVIAI_SHORTCUT_RELOAD_TEST";
        if std::env::var_os(CHILD).is_some() {
            let mut settings = Settings::load().unwrap();
            assert!(settings.bell());
            assert_eq!(
                settings.keyboard_shortcuts.resolve("alt-k", true),
                Some(Shortcut::Copy)
            );
            assert_eq!(
                settings.keyboard_shortcuts.resolve("ctrl-shift-c", true),
                None
            );
            assert_eq!(settings.keyboard_shortcuts.resolve("ctrl-c", true), None);
            settings.save_shortcuts(&ShortcutMap::default()).unwrap();
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{"audible_bell":true,"extra_field":17}"#).unwrap();
        let mut settings = Settings::load_at(&path).unwrap();
        let mut draft = settings.keyboard_shortcuts.clone();
        let slot = ROWS
            .iter()
            .flat_map(|r| r.slots)
            .find(|s| s.action == Shortcut::Copy)
            .unwrap();
        draft.assign(slot, "alt-k").unwrap();
        assert_eq!(settings.keyboard_shortcuts.resolve("alt-k", true), None);
        settings.save_shortcuts_at(&path, &draft).unwrap();
        assert_eq!(settings.keyboard_shortcuts, draft);
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "settings::tests::saved_binding_survives_a_fresh_process_and_restoring_defaults",
            ])
            .env(CHILD, "1")
            .env("TERMVIAI_DATA_DIR", dir.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        let restored = Settings::load_at(&path).unwrap();
        assert_eq!(
            restored.keyboard_shortcuts.resolve("ctrl-shift-c", true),
            Some(Shortcut::Copy)
        );
        assert_eq!(restored.other["extra_field"], 17);
        assert!(settings.save_shortcuts_at(&path, &draft).is_err());
        assert_eq!(
            Settings::load_at(&path).unwrap().keyboard_shortcuts,
            ShortcutMap::default()
        );
    }

    #[test]
    fn npu_monitor_defaults_on_for_new_and_existing_profiles_and_remembers_opt_out() {
        assert!(Settings::default().npu_monitor_enabled);
        let mut settings: Settings = serde_json::from_str(r#"{"audible_bell":true}"#).unwrap();
        assert!(settings.npu_monitor_enabled);
        for enabled in [false, true] {
            settings.npu_monitor_enabled = enabled;
            let restored: Settings =
                serde_json::from_slice(&serde_json::to_vec(&settings).unwrap()).unwrap();
            assert_eq!(restored.npu_monitor_enabled, enabled);
            assert!(restored.bell());
        }
    }
}
