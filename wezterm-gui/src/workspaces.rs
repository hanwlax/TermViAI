//! Local SSH workspace shortcuts. These contain connection metadata, never credentials.
use crate::hosts::Host;
use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const SCHEMA_VERSION: u32 = 1;
const MAX_RECENT: usize = 30;
const MAX_HISTORY: usize = 20;
const MAX_SAVED: usize = 100;
const MAX_PANES: usize = 64;
const MAX_DEPTH: usize = 16;
// A process can have several windows, each with its own cached library.
static STORE_WRITE: Mutex<()> = Mutex::new(());

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SplitAxis {
    /// First is left, second is right.
    Horizontal,
    /// First is above, second is below.
    Vertical,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SavedLayout {
    Pane {
        host: Host,
    },
    Split {
        axis: SplitAxis,
        first_percent: u8,
        first: Box<SavedLayout>,
        second: Box<SavedLayout>,
    },
}

impl SavedLayout {
    pub fn pane_count(&self) -> usize {
        match self {
            Self::Pane { .. } => 1,
            Self::Split { first, second, .. } => first.pane_count() + second.pane_count(),
        }
    }

    pub fn hosts(&self) -> Vec<&Host> {
        let mut hosts = Vec::new();
        self.collect_hosts(&mut hosts);
        hosts
    }

    fn collect_hosts<'a>(&'a self, hosts: &mut Vec<&'a Host>) {
        match self {
            Self::Pane { host } => hosts.push(host),
            Self::Split { first, second, .. } => {
                first.collect_hosts(hosts);
                second.collect_hosts(hosts);
            }
        }
    }

    fn validate(&self, depth: usize, panes: &mut usize) -> anyhow::Result<()> {
        if depth > MAX_DEPTH {
            bail!("The workspace layout has too many nested splits.");
        }
        match self {
            Self::Pane { host } => {
                *panes += 1;
                if *panes > MAX_PANES {
                    bail!(
                        "A saved workspace can contain at most {} terminals.",
                        MAX_PANES
                    );
                }
                validate_host(host)
            }
            Self::Split {
                first_percent,
                first,
                second,
                ..
            } => {
                if !(1..=99).contains(first_percent) {
                    bail!("A workspace split must leave space for both terminals.");
                }
                first.validate(depth + 1, panes)?;
                second.validate(depth + 1, panes)
            }
        }
    }

    fn same_connections(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Pane { host: a }, Self::Pane { host: b }) => same_host(a, b),
            (
                Self::Split {
                    axis: a_axis,
                    first_percent: a_percent,
                    first: a_first,
                    second: a_second,
                },
                Self::Split {
                    axis: b_axis,
                    first_percent: b_percent,
                    first: b_first,
                    second: b_second,
                },
            ) => {
                a_axis == b_axis
                    && a_percent == b_percent
                    && a_first.same_connections(b_first)
                    && a_second.same_connections(b_second)
            }
            _ => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentConnection {
    pub host: Host,
    pub last_connected_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedWorkspace {
    pub id: String,
    pub name: String,
    pub layout: SavedLayout,
    pub updated_at_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkspaceStore {
    pub recent: Vec<RecentConnection>,
    pub history: Vec<SavedWorkspace>,
    pub saved: Vec<SavedWorkspace>,
    version: u32,
    // Retain unrelated metadata written by a compatible newer application.
    #[serde(flatten)]
    extra: BTreeMap<String, serde_json::Value>,
    #[serde(skip)]
    source: Option<PathBuf>,
}

impl Default for WorkspaceStore {
    fn default() -> Self {
        Self {
            recent: Vec::new(),
            history: Vec::new(),
            saved: Vec::new(),
            version: SCHEMA_VERSION,
            extra: BTreeMap::new(),
            source: None,
        }
    }
}

impl WorkspaceStore {
    pub fn path() -> PathBuf {
        std::env::var_os("TERMVIAI_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| config::DATA_DIR.join("termviai"))
            .join("workspaces.json")
    }

    pub fn load() -> anyhow::Result<Self> {
        Self::load_from(&Self::path())
    }

    fn load_from(path: &Path) -> anyhow::Result<Self> {
        let mut store: Self = match std::fs::File::open(path) {
            Ok(file) => serde_json::from_reader(file).context(
                "Could not read saved workspaces. The existing file has been preserved.",
            )?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(error) => return Err(error).context("Could not open saved workspaces."),
        };
        store.source = Some(path.to_owned());
        store.validate()?;
        store.sort_and_trim();
        Ok(store)
    }

    /// Record a successful SSH connection. Address/user/port/key identify an entry;
    /// changing its label updates that entry instead of creating a duplicate.
    pub fn record_recent(&mut self, host: &Host) -> anyhow::Result<()> {
        validate_host(host)?;
        self.mutate(|latest| {
            let stamp = latest.next_stamp();
            latest.recent.retain(|entry| !same_host(&entry.host, host));
            latest.recent.push(RecentConnection {
                host: host.clone(),
                last_connected_ms: stamp,
            });
            Ok(())
        })
    }

    /// History records groups only; a single host belongs in recent connections.
    pub fn record_group_history(&mut self, name: &str, layout: SavedLayout) -> anyhow::Result<()> {
        layout.validate(0, &mut 0)?;
        if layout.pane_count() < 2 {
            return Ok(());
        }
        let name = workspace_name(name)?;
        self.mutate(|latest| {
            let stamp = latest.next_stamp();
            let previous = latest
                .history
                .iter()
                .find(|entry| entry.layout.same_connections(&layout))
                .map(|entry| entry.id.clone());
            latest
                .history
                .retain(|entry| !entry.layout.same_connections(&layout));
            latest.history.push(SavedWorkspace {
                id: previous.unwrap_or_else(new_id),
                name,
                layout,
                updated_at_ms: stamp,
            });
            Ok(())
        })
    }

    /// Save a group, or update its stable ID after another Ctrl+S.
    /// Returns the ID to associate with the live tab.
    pub fn save_group(
        &mut self,
        id: Option<&str>,
        name: &str,
        layout: SavedLayout,
    ) -> anyhow::Result<String> {
        layout.validate(0, &mut 0)?;
        if layout.pane_count() < 2 {
            bail!("Save a workspace after adding at least two SSH terminals.");
        }
        let name = workspace_name(name)?;
        if id.is_some_and(|id| id.is_empty() || id.chars().any(char::is_control)) {
            bail!("The saved workspace ID is invalid.");
        }
        self.mutate(|latest| {
            let id = id.map(str::to_owned).unwrap_or_else(new_id);
            if latest.saved.len() >= MAX_SAVED && !latest.saved.iter().any(|entry| entry.id == id) {
                bail!(
                    "The saved workspace library is full ({} groups).",
                    MAX_SAVED
                );
            }
            let stamp = latest.next_stamp();
            latest.saved.retain(|entry| entry.id != id);
            latest.saved.push(SavedWorkspace {
                id: id.clone(),
                name,
                layout,
                updated_at_ms: stamp,
            });
            Ok(id)
        })
    }

    pub fn rename_saved(&mut self, id: &str, name: &str) -> anyhow::Result<()> {
        let name = workspace_name(name)?;
        self.mutate(|latest| {
            let stamp = latest.next_stamp();
            let entry = latest
                .saved
                .iter_mut()
                .find(|entry| entry.id == id)
                .context("The saved workspace no longer exists.")?;
            entry.name = name;
            entry.updated_at_ms = stamp;
            Ok(())
        })
    }

    /// Merge this cache with the current disk state. Prefer the operation methods
    /// above; they automatically persist and refresh the local cache.
    pub fn save(&mut self) -> anyhow::Result<()> {
        self.validate()?;
        let cache = self.clone();
        self.mutate(|latest| {
            for incoming in cache.recent {
                let existing = latest
                    .recent
                    .iter_mut()
                    .find(|entry| same_host(&entry.host, &incoming.host));
                match existing {
                    Some(existing) if incoming.last_connected_ms > existing.last_connected_ms => {
                        *existing = incoming;
                    }
                    None => latest.recent.push(incoming),
                    _ => {}
                }
            }
            merge_groups(&mut latest.saved, cache.saved, false);
            merge_groups(&mut latest.history, cache.history, true);
            for (key, value) in cache.extra {
                latest.extra.entry(key).or_insert(value);
            }
            Ok(())
        })
    }

    fn mutate<T>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> anyhow::Result<T>,
    ) -> anyhow::Result<T> {
        let _guard = STORE_WRITE
            .lock()
            .map_err(|_| anyhow::anyhow!("The workspace library lock is unavailable."))?;
        let path = self.source.clone().unwrap_or_else(Self::path);
        // Never overwrite a corrupt or unsupported file, including when the UI
        // recovered from a failed initial load using an empty in-memory library.
        let mut latest = Self::load_from(&path)?;
        let result = operation(&mut latest)?;
        latest.sort_and_trim();
        latest.validate()?;
        latest.write_to(&path)?;
        *self = latest;
        Ok(result)
    }

    fn write_to(&self, path: &Path) -> anyhow::Result<()> {
        let directory = path
            .parent()
            .context("Missing workspace library directory.")?;
        std::fs::create_dir_all(directory)?;
        let mut file = tempfile::NamedTempFile::new_in(directory)?;
        serde_json::to_writer_pretty(&mut file, self)?;
        file.write_all(b"\n")?;
        file.as_file().sync_all()?;
        file.persist(path)
            .map_err(|error| error.error)
            .context("Could not save the workspace library.")?;
        Ok(())
    }

    fn validate(&self) -> anyhow::Result<()> {
        if self.version != SCHEMA_VERSION {
            bail!("Unsupported workspace library version. The existing file has been preserved.");
        }
        if self.saved.len() > MAX_SAVED {
            bail!("The workspace library has too many saved groups. The file has been preserved.");
        }
        for entry in &self.recent {
            validate_host(&entry.host)?;
        }
        for entry in self.saved.iter().chain(&self.history) {
            if entry.id.is_empty() || entry.id.chars().any(char::is_control) {
                bail!("The workspace library contains an invalid ID.");
            }
            workspace_name(&entry.name)?;
            entry.layout.validate(0, &mut 0)?;
        }
        Ok(())
    }

    fn sort_and_trim(&mut self) {
        self.recent
            .sort_by_key(|entry| std::cmp::Reverse(entry.last_connected_ms));
        self.recent.truncate(MAX_RECENT);
        self.history
            .sort_by_key(|entry| std::cmp::Reverse(entry.updated_at_ms));
        self.history.truncate(MAX_HISTORY);
        self.saved
            .sort_by_key(|entry| std::cmp::Reverse(entry.updated_at_ms));
    }

    fn next_stamp(&self) -> u64 {
        let previous = self
            .recent
            .iter()
            .map(|entry| entry.last_connected_ms)
            .chain(self.history.iter().map(|entry| entry.updated_at_ms))
            .chain(self.saved.iter().map(|entry| entry.updated_at_ms))
            .max()
            .unwrap_or(0);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .min(u64::MAX as u128) as u64;
        now.max(previous.saturating_add(1))
    }
}

fn merge_groups(current: &mut Vec<SavedWorkspace>, incoming: Vec<SavedWorkspace>, history: bool) {
    for entry in incoming {
        let existing = current.iter_mut().find(|existing| {
            existing.id == entry.id || (history && existing.layout.same_connections(&entry.layout))
        });
        match existing {
            Some(existing) if entry.updated_at_ms > existing.updated_at_ms => *existing = entry,
            None => current.push(entry),
            _ => {}
        }
    }
}

fn new_id() -> String {
    format!(
        "workspace-{:016x}{:016x}",
        fastrand::u64(..),
        fastrand::u64(..)
    )
}

fn workspace_name(name: &str) -> anyhow::Result<String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 120 || name.chars().any(char::is_control) {
        bail!("Give the workspace a name of 1–120 characters, without control characters.");
    }
    Ok(name.to_owned())
}

fn validate_host(host: &Host) -> anyhow::Result<()> {
    let mut host = host.clone();
    // An unavailable removable key drive must not invalidate an entire library.
    // Actual connections revalidate the chosen key using the normal SSH flow.
    if host.identity_file.chars().any(char::is_control) {
        bail!("The SSH key path contains control characters.");
    }
    host.identity_file.clear();
    host.validate()
}

fn normalized_address(host: &Host) -> String {
    let address = host
        .address
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']');
    match address.parse::<std::net::IpAddr>() {
        Ok(address) => address.to_string(),
        Err(_) => address.to_ascii_lowercase(),
    }
}

fn same_host(a: &Host, b: &Host) -> bool {
    a.port == b.port
        && a.username == b.username
        && a.identity_file == b.identity_file
        && normalized_address(a) == normalized_address(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host(address: &str) -> Host {
        Host::quick_connect(address).unwrap()
    }

    fn group() -> SavedLayout {
        SavedLayout::Split {
            axis: SplitAxis::Horizontal,
            first_percent: 40,
            first: Box::new(SavedLayout::Pane { host: host("one") }),
            second: Box::new(SavedLayout::Split {
                axis: SplitAxis::Vertical,
                first_percent: 65,
                first: Box::new(SavedLayout::Pane {
                    host: host("alice@two:2222"),
                }),
                second: Box::new(SavedLayout::Pane {
                    host: host("[2001:db8::1]"),
                }),
            }),
        }
    }

    #[test]
    fn workspace_round_trip_preserves_nested_layout_and_stable_id() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("workspaces.json");
        let mut store = WorkspaceStore::load_from(&path).unwrap();
        let id = store.save_group(None, "开发环境", group()).unwrap();
        store.record_group_history("开发环境", group()).unwrap();
        let updated = store.save_group(Some(&id), "生产环境", group()).unwrap();
        assert_eq!(id, updated);
        let loaded = WorkspaceStore::load_from(&path).unwrap();
        assert_eq!(loaded.saved.len(), 1);
        assert_eq!(loaded.saved[0].name, "生产环境");
        assert_eq!(loaded.saved[0].layout, group());
        assert_eq!(loaded.saved[0].layout.pane_count(), 3);
        assert_eq!(loaded.saved[0].layout.hosts()[1].username, "alice");
        assert_eq!(loaded.history.len(), 1);
    }

    #[test]
    fn workspace_recents_deduplicate_identity_update_label_and_cap_entries() {
        let temporary = tempfile::tempdir().unwrap();
        let mut store =
            WorkspaceStore::load_from(&temporary.path().join("workspaces.json")).unwrap();
        for index in 0..MAX_RECENT + 5 {
            store
                .record_recent(&host(&format!("server-{}", index)))
                .unwrap();
        }
        assert_eq!(store.recent.len(), MAX_RECENT);
        let mut renamed = host("SERVER-34");
        renamed.label = "Staging".into();
        store.record_recent(&renamed).unwrap();
        assert_eq!(store.recent.len(), MAX_RECENT);
        assert_eq!(store.recent[0].host.label, "Staging");
        store.record_recent(&host("alice@SERVER-34")).unwrap();
        assert_eq!(store.recent[0].host.username, "alice");
        assert_eq!(store.recent[1].host.label, "Staging");
    }

    #[test]
    fn workspace_stale_windows_merge_without_reverting_saved_names() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("workspaces.json");
        let mut first = WorkspaceStore::load_from(&path).unwrap();
        let id = first.save_group(None, "Initial", group()).unwrap();
        let mut second = WorkspaceStore::load_from(&path).unwrap();
        first.record_recent(&host("one")).unwrap();
        second.record_recent(&host("two")).unwrap();
        first.rename_saved(&id, "Renamed").unwrap();
        second.save().unwrap();
        let loaded = WorkspaceStore::load_from(&path).unwrap();
        assert_eq!(loaded.recent.len(), 2);
        assert_eq!(loaded.saved[0].name, "Renamed");
    }

    #[test]
    fn workspace_corruption_and_future_versions_are_never_overwritten() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("workspaces.json");
        let mut store = WorkspaceStore::load_from(&path).unwrap();
        for invalid in [b"broken json".as_slice(), br#"{"version":999}"#] {
            std::fs::write(&path, invalid).unwrap();
            assert!(WorkspaceStore::load_from(&path).is_err());
            assert!(store.record_recent(&host("server")).is_err());
            assert!(store.save_group(None, "Group", group()).is_err());
            assert!(store.save().is_err());
            assert_eq!(std::fs::read(&path).unwrap(), invalid);
        }
    }

    #[test]
    fn workspace_history_deduplicates_layouts_but_keeps_ssh_credentials_distinct() {
        let temporary = tempfile::tempdir().unwrap();
        let mut store =
            WorkspaceStore::load_from(&temporary.path().join("workspaces.json")).unwrap();
        store.record_group_history("Original", group()).unwrap();
        let id = store.history[0].id.clone();
        let mut updated = group();
        if let SavedLayout::Split { first, .. } = &mut updated {
            if let SavedLayout::Pane { host } = first.as_mut() {
                host.label = "A new label".into();
            }
        }
        store
            .record_group_history("Renamed", updated.clone())
            .unwrap();
        assert_eq!(store.history.len(), 1);
        assert_eq!(store.history[0].id, id);
        assert_eq!(store.history[0].name, "Renamed");
        if let SavedLayout::Split { first, .. } = &mut updated {
            if let SavedLayout::Pane { host } = first.as_mut() {
                host.identity_file = "not-currently-mounted/id_ed25519".into();
            }
        }
        store
            .record_group_history("Different key", updated)
            .unwrap();
        assert_eq!(store.history.len(), 2);
    }

    #[test]
    fn workspace_invalid_layout_or_name_does_not_modify_library() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("workspaces.json");
        let mut store = WorkspaceStore::load_from(&path).unwrap();
        store.save_group(None, "Good", group()).unwrap();
        let before = std::fs::read(&path).unwrap();
        let mut invalid = group();
        if let SavedLayout::Split { first_percent, .. } = &mut invalid {
            *first_percent = 100;
        }
        assert!(store.save_group(None, "Invalid", invalid).is_err());
        assert!(store.save_group(None, "\n", group()).is_err());
        assert!(store
            .save_group(None, "Single", SavedLayout::Pane { host: host("one") })
            .is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }

    #[test]
    fn workspace_compatible_unknown_metadata_survives_updates() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("workspaces.json");
        std::fs::write(&path, br#"{"version":1,"future_metadata":{"keep":true}}"#).unwrap();
        let mut store = WorkspaceStore::load_from(&path).unwrap();
        store.record_recent(&host("one")).unwrap();
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(value["future_metadata"]["keep"], true);
    }
}
