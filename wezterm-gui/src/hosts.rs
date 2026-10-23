//! Local host metadata. Authentication stays in the existing SSH transport.
use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Authentication {
    #[default]
    Password,
    Key,
    Identity,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Host {
    pub label: String,
    pub address: String,
    pub port: u16,
    pub username: String,
    pub group: String,
    pub group_id: String,
    pub identity_file: String,
    pub identity_id: String,
    // Selected Keychain entry; explicit Key mode stores either this label or a path.
    pub key_label: String,
    // None preserves legacy/quick-connect SSH behavior until explicitly edited.
    pub authentication: Option<Authentication>,
    pub credential_id: String,
    // An opaque session reference may be persisted; its password only lives in memory.
    pub session_credential: String,
}

impl Host {
    pub fn validate(&mut self) -> anyhow::Result<()> {
        self.label = self.label.trim().to_owned();
        self.address = self.address.trim().to_owned();
        self.username = self.username.trim().to_owned();
        self.group = self.group.trim().to_owned();
        self.identity_file = self.identity_file.trim().to_owned();
        if self.address.is_empty()
            || self
                .address
                .chars()
                .any(|c| c.is_whitespace() || c.is_control())
            || self.address.contains("://")
            || self.address.contains('@')
            || self.address.starts_with('-')
        {
            bail!("Enter a hostname or IP address, without a protocol or username.");
        }
        if self.address.contains(':') || self.address.contains('[') || self.address.contains(']') {
            let address = self
                .address
                .strip_prefix('[')
                .and_then(|s| s.strip_suffix(']'))
                .unwrap_or(&self.address);
            address
                .parse::<std::net::Ipv6Addr>()
                .context("Enter a valid IPv6 address; put the SSH port in the Port field.")?;
            self.address = address.to_owned();
        }
        if self.port == 0 {
            bail!("Port must be between 1 and 65535.");
        }
        if self
            .username
            .chars()
            .any(|c| c.is_whitespace() || c.is_control())
        {
            bail!("Username cannot contain spaces or control characters.");
        }
        if [&self.label, &self.group, &self.identity_file]
            .iter()
            .any(|s| s.chars().any(char::is_control))
        {
            bail!("Fields cannot contain control characters.");
        }
        if self.label.is_empty() {
            self.label = self.address.clone();
        }
        if self.key_label.is_empty()
            && !self.identity_file.is_empty()
            && !Path::new(&self.identity_file).is_file()
        {
            bail!("The private key file does not exist on this computer.");
        }
        Ok(())
    }

    pub fn credential_ref(&self) -> &str {
        if !self.session_credential.is_empty() {
            &self.session_credential
        } else {
            &self.credential_id
        }
    }

    pub fn own_credential_ref(&self) -> &str {
        // beta.4 stored the selected Identity's reference on the Host as well.
        let reference = self.credential_ref();
        if !self.identity_id.is_empty() && reference == self.identity_id {
            ""
        } else {
            reference
        }
    }

    pub fn authentication_mode(&self) -> Authentication {
        self.authentication.unwrap_or_else(|| {
            if !self.identity_id.is_empty() {
                Authentication::Identity
            } else if !self.key_label.is_empty() || !self.identity_file.is_empty() {
                Authentication::Key
            } else {
                Authentication::Password
            }
        })
    }

    pub fn resolve_identity(&mut self, store: &HostStore) -> anyhow::Result<()> {
        match self.authentication {
            Some(Authentication::Password) => {
                self.identity_id.clear();
                self.identity_file.clear();
                self.key_label.clear();
            }
            Some(Authentication::Key) => {
                self.identity_id.clear();
                self.credential_id.clear();
                self.session_credential.clear();
                anyhow::ensure!(
                    !self.key_label.is_empty() || !self.identity_file.is_empty(),
                    "Choose a private key."
                );
            }
            Some(Authentication::Identity) => {
                self.identity_file.clear();
                self.key_label.clear();
                self.credential_id.clear();
                self.session_credential.clear();
                anyhow::ensure!(
                    !self.identity_id.is_empty(),
                    "Choose an identity from Keychain."
                );
            }
            None => {}
        }
        if !self.identity_id.is_empty() {
            let identity = store
                .identities
                .iter()
                .find(|i| i.id == self.identity_id)
                .context(
                "This identity is no longer in Keychain. Edit the host to choose authentication.",
            )?;
            self.username = identity.username.clone();
            self.credential_id = identity.id.clone();
            self.session_credential.clear();
        }
        if !self.key_label.is_empty() {
            self.identity_file = store
                .keys
                .iter()
                .find(|key| key.label == self.key_label)
                .context("This key is no longer in Keychain. Edit the host to choose a key.")?
                .path
                .clone();
            anyhow::ensure!(
                Path::new(&self.identity_file).is_file(),
                "The selected Keychain key file does not exist on this computer."
            );
        }
        Ok(())
    }

    pub fn endpoint(&self) -> String {
        let addr = self.address.trim_start_matches('[').trim_end_matches(']');
        if addr.contains(':') {
            format!("[{}]:{}", addr, self.port)
        } else {
            format!("{}:{}", addr, self.port)
        }
    }

    pub fn quick_connect(text: &str) -> anyhow::Result<Self> {
        let text = text.trim().strip_prefix("ssh ").unwrap_or(text.trim());
        if text.contains("://") || text.split_whitespace().count() != 1 {
            bail!("Use user@hostname or user@hostname:port for SSH.");
        }
        let (username, endpoint) = text.rsplit_once('@').unwrap_or(("", text));
        let (address, port) = if endpoint.starts_with('[') {
            let end = endpoint
                .find(']')
                .context("IPv6 address needs a closing ].")?;
            endpoint[1..end]
                .parse::<std::net::Ipv6Addr>()
                .context("Brackets must contain a valid IPv6 address.")?;
            let suffix = &endpoint[end + 1..];
            let port = if suffix.is_empty() {
                22
            } else {
                suffix
                    .strip_prefix(':')
                    .context("Use [IPv6]:port.")?
                    .parse::<u16>()
                    .context("Invalid SSH port.")?
            };
            (endpoint[1..end].to_owned(), port)
        } else if endpoint.matches(':').count() == 1 {
            let (addr, port) = endpoint.rsplit_once(':').unwrap();
            (
                addr.to_owned(),
                port.parse::<u16>().context("Invalid SSH port.")?,
            )
        } else {
            (endpoint.to_owned(), 22)
        };
        let mut host = Self {
            address,
            port,
            username: username.to_owned(),
            ..Default::default()
        };
        host.validate()?;
        Ok(host)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct KeyFile {
    pub label: String,
    pub path: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Identity {
    pub id: String,
    pub label: String,
    pub username: String,
}

pub fn new_credential_id() -> String {
    format!("{:032x}", fastrand::u128(..))
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostGroup {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub parent_id: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct HostStore {
    pub groups: Vec<HostGroup>,
    pub hosts: Vec<Host>,
    pub keys: Vec<KeyFile>,
    pub identities: Vec<Identity>,
}

impl HostStore {
    /// Include old text-only groups without changing Host snapshots or credentials.
    fn prepare_groups(&mut self) -> anyhow::Result<()> {
        let mut ids = std::collections::HashSet::new();
        for group in &self.groups {
            anyhow::ensure!(
                !group.id.is_empty() && ids.insert(group.id.clone()),
                "The group library has duplicate or empty IDs."
            );
            anyhow::ensure!(
                !group.name.trim().is_empty() && !group.name.chars().any(char::is_control),
                "The group library contains an invalid name."
            );
        }
        for group in &self.groups {
            let mut seen = std::collections::HashSet::new();
            let mut id = group.id.as_str();
            while !id.is_empty() {
                anyhow::ensure!(
                    seen.insert(id),
                    "The group library contains a parent cycle."
                );
                let parent = self
                    .groups
                    .iter()
                    .find(|g| g.id == id)
                    .context("A parent group is missing.")?;
                id = &parent.parent_id;
            }
        }
        for host in &self.hosts {
            if !host.group_id.is_empty() {
                anyhow::ensure!(
                    ids.contains(&host.group_id),
                    "A Host refers to a missing group."
                );
            } else if !host.group.is_empty()
                && !self
                    .groups
                    .iter()
                    .any(|g| g.parent_id.is_empty() && g.name == host.group)
            {
                // Deterministic across reloads until the next library save.
                let mut id = format!(
                    "legacy-{}",
                    host.group
                        .as_bytes()
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect::<String>()
                );
                while ids.contains(&id) {
                    id.push('_');
                }
                ids.insert(id.clone());
                self.groups.push(HostGroup {
                    id,
                    name: host.group.clone(),
                    parent_id: String::new(),
                });
            }
        }
        Ok(())
    }

    pub fn group(&self, id: &str) -> Option<&HostGroup> {
        self.groups.iter().find(|g| g.id == id)
    }

    pub fn group_trail(&self, id: &str) -> Vec<&HostGroup> {
        let mut result = vec![];
        let mut id = id;
        // Libraries are validated at load/save. The bound also makes temporary
        // in-memory edits safe to display without recursive traversal.
        for _ in 0..self.groups.len() {
            let Some(group) = self.group(id) else { break };
            result.push(group);
            id = &group.parent_id;
        }
        result.reverse();
        result
    }

    pub fn group_path(&self, id: &str) -> String {
        self.group_trail(id)
            .iter()
            .map(|g| g.name.as_str())
            .collect::<Vec<_>>()
            .join(" / ")
    }

    pub fn child_groups(&self, parent: &str) -> Vec<&HostGroup> {
        let mut result: Vec<_> = self
            .groups
            .iter()
            .filter(|g| g.parent_id == parent)
            .collect();
        result.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then(a.name.cmp(&b.name))
        });
        result
    }

    pub fn host_group_id<'a>(&'a self, host: &'a Host) -> &'a str {
        if !host.group_id.is_empty() {
            &host.group_id
        } else {
            self.groups
                .iter()
                .find(|g| g.parent_id.is_empty() && g.name == host.group)
                .map(|g| g.id.as_str())
                .unwrap_or("")
        }
    }

    pub fn host_in_group(&self, host: &Host, group: &str) -> bool {
        group.is_empty()
            || self
                .group_trail(self.host_group_id(host))
                .iter()
                .any(|g| g.id == group)
    }

    pub fn add_group(&mut self, name: &str, parent: &str) -> anyhow::Result<String> {
        let name = name.trim();
        anyhow::ensure!(!name.is_empty(), "Enter a group name.");
        anyhow::ensure!(
            name.chars().count() <= 60 && !name.chars().any(|c| c.is_control() || c == '/'),
            "Use up to 60 characters without / or control characters."
        );
        anyhow::ensure!(
            parent.is_empty() || self.group(parent).is_some(),
            "The parent group changed. Reopen New Group."
        );
        anyhow::ensure!(
            !self
                .groups
                .iter()
                .any(|g| g.parent_id == parent && g.name.to_lowercase() == name.to_lowercase()),
            "A group with this name already exists here."
        );
        let id = format!("group-{}", new_credential_id());
        self.groups.push(HostGroup {
            id: id.clone(),
            name: name.into(),
            parent_id: parent.into(),
        });
        Ok(id)
    }

    /// The existing Host Group input accepts the displayed path. Exact legacy
    /// labels (including slashes) continue to resolve to their original group.
    pub fn assign_host_group(
        &mut self,
        host: &mut Host,
        path: &str,
        original: Option<&Host>,
    ) -> anyhow::Result<()> {
        let path = path.trim();
        let original_id = original.map(|h| self.host_group_id(h)).unwrap_or("");
        let id = if path.is_empty() {
            String::new()
        } else if !original_id.is_empty() && self.group_path(original_id) == path {
            original_id.to_owned()
        } else {
            let matches: Vec<_> = self
                .groups
                .iter()
                .filter(|g| self.group_path(&g.id) == path)
                .map(|g| g.id.clone())
                .collect();
            anyhow::ensure!(
                matches.len() <= 1,
                "This group path is ambiguous. Keep the original group or choose another path."
            );
            if let Some(id) = matches.into_iter().next() {
                id
            } else {
                let mut parent = String::new();
                for name in path.split('/').map(str::trim) {
                    let existing = self
                        .groups
                        .iter()
                        .find(|g| {
                            g.parent_id == parent && g.name.to_lowercase() == name.to_lowercase()
                        })
                        .map(|g| g.id.clone());
                    parent = match existing {
                        Some(id) => id,
                        None => self.add_group(name, &parent)?,
                    };
                }
                parent
            }
        };
        host.group = self.group_path(&id);
        host.group_id = id;
        Ok(())
    }

    /// Delete only the container. Promote direct children; keep every Host and
    /// its stable credential references, including legacy text-only memberships.
    pub fn remove_group(&mut self, original: &HostGroup) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.group(&original.id) == Some(original),
            "This group changed or was deleted. Reopen its menu and try again."
        );
        let children = self.child_groups(&original.id);
        let mut names: std::collections::HashSet<String> = self
            .child_groups(&original.parent_id)
            .iter()
            .filter(|g| g.id != original.id)
            .map(|g| g.name.to_lowercase())
            .collect();
        for child in children {
            anyhow::ensure!(names.insert(child.name.to_lowercase()),
                "Cannot move subgroup '{}' to the parent: that name already exists there. Resolve the duplicate group first.", child.name);
        }
        let memberships: Vec<String> = self
            .hosts
            .iter()
            .map(|h| self.host_group_id(h).to_owned())
            .collect();
        self.groups.retain(|g| g.id != original.id);
        for group in &mut self.groups {
            if group.parent_id == original.id {
                group.parent_id = original.parent_id.clone();
            }
        }
        let assignments: Vec<_> = memberships
            .into_iter()
            .map(|id| {
                let id = if id == original.id {
                    original.parent_id.clone()
                } else {
                    id
                };
                let path = self.group_path(&id);
                (id, path)
            })
            .collect();
        for (host, (id, path)) in self.hosts.iter_mut().zip(assignments) {
            host.group_id = id;
            host.group = path;
        }
        Ok(())
    }

    /// Validate the entire captured selection before moving anything. This is
    /// a metadata move, not a duplicate or a change to live SSH sessions.
    pub fn move_hosts_to_group(&mut self, originals: &[Host], target: &str) -> anyhow::Result<()> {
        anyhow::ensure!(
            target.is_empty() || self.group(target).is_some(),
            "The destination group was deleted. Reopen Add Existing Hosts."
        );
        anyhow::ensure!(!originals.is_empty(), "Select at least one Host.");
        let mut indices = std::collections::HashSet::new();
        for host in originals {
            let index = self
                .hosts
                .iter()
                .enumerate()
                .find(|(index, current)| !indices.contains(index) && *current == host)
                .map(|(index, _)| index)
                .context("A selected Host changed or was deleted. Reopen Add Existing Hosts.")?;
            indices.insert(index);
        }
        let path = self.group_path(target);
        for index in indices {
            self.hosts[index].group_id = target.to_owned();
            self.hosts[index].group = path.clone();
        }
        Ok(())
    }

    /// Remove the exact entry captured by the confirmation, not its current index.
    pub fn remove_host(&mut self, original: &Host) -> anyhow::Result<()> {
        let index = self
            .hosts
            .iter()
            .position(|host| host == original)
            .context("This host changed or was deleted. Reopen its menu and try again.")?;
        self.hosts.remove(index);
        // Saved workspaces, recent connections and duplicates can still own the
        // credential reference. Removing a library entry does not revoke it.
        Ok(())
    }

    /// Duplicate against a freshly loaded library, preserving all connection fields.
    pub fn duplicate_host(&mut self, original: &Host) -> anyhow::Result<Host> {
        anyhow::ensure!(
            self.hosts.contains(original),
            "This host changed. Reopen its menu and try again."
        );
        let mut copy = original.clone();
        let base = format!("{}-copy", original.label);
        copy.label = base.clone();
        let mut suffix = 2;
        while self.hosts.iter().any(|host| host.label == copy.label) {
            copy.label = format!("{base}-{suffix}");
            suffix += 1;
        }
        self.hosts.push(copy.clone());
        Ok(copy)
    }

    pub fn path() -> PathBuf {
        std::env::var_os("TERMVIAI_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| config::DATA_DIR.join("termviai"))
            .join("hosts.json")
    }
    pub fn load() -> anyhow::Result<Self> {
        Self::load_from(&Self::path())
    }
    pub(crate) fn load_from(path: &Path) -> anyhow::Result<Self> {
        match std::fs::File::open(path) {
            Ok(f) => {
                let mut store: Self = serde_json::from_reader(f).context(
                    "Could not read the local host library. The file has been preserved.",
                )?;
                store.prepare_groups()?;
                Ok(store)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e).context("Could not open the local host library."),
        }
    }
    pub fn save(&self) -> anyhow::Result<()> {
        self.save_to(&Self::path())
    }
    pub(crate) fn save_to(&self, path: &Path) -> anyhow::Result<()> {
        let mut normalized = self.clone();
        normalized.prepare_groups()?;
        let dir = path.parent().context("Missing library directory")?;
        std::fs::create_dir_all(dir)?;
        let mut file = tempfile::NamedTempFile::new_in(dir)?;
        serde_json::to_writer_pretty(&mut file, &normalized)?;
        file.write_all(b"\n")?;
        file.as_file().sync_all()?;
        file.persist(path)
            .map_err(|e| e.error)
            .context("Could not save the local host library.")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deleting_group_promotes_children_and_preserves_legacy_hosts_and_credentials() {
        let mut store = HostStore::default();
        let parent = store.add_group("Parent", "").unwrap();
        let child = store.add_group("Child", &parent).unwrap();
        let deep = store.add_group("Deep", &child).unwrap();
        for (group, group_id, credential) in [
            ("Parent", "", "old-password"),
            ("Parent / Child", child.as_str(), "key-reference"),
            ("Parent / Child / Deep", deep.as_str(), "identity-reference"),
        ] {
            store.hosts.push(Host {
                label: credential.into(),
                address: "example.org".into(),
                port: 22,
                group: group.into(),
                group_id: group_id.into(),
                credential_id: credential.into(),
                ..Default::default()
            });
        }
        let original = store.group(&parent).unwrap().clone();
        store.remove_group(&original).unwrap();
        assert_eq!(store.hosts.len(), 3);
        assert_eq!(store.hosts[0].group, "");
        assert_eq!(store.hosts[0].group_id, "");
        assert_eq!(store.hosts[0].credential_id, "old-password");
        assert_eq!(store.group(&child).unwrap().parent_id, "");
        assert_eq!(store.hosts[1].group, "Child");
        assert_eq!(store.hosts[2].group, "Child / Deep");
        assert_eq!(store.hosts[2].group_id, deep);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hosts.json");
        store.save_to(&path).unwrap();
        let loaded = HostStore::load_from(&path).unwrap();
        assert!(loaded.group(&parent).is_none());
        assert_eq!(loaded.hosts, store.hosts);
        assert!(store.remove_group(&original).is_err());
    }

    #[test]
    fn deleting_group_rejects_stale_target_and_promotion_name_conflicts_atomically() {
        let mut store = HostStore::default();
        let root = store.add_group("Root", "").unwrap();
        let parent = store.add_group("Parent", &root).unwrap();
        store.add_group("Child", &parent).unwrap();
        store.add_group("child", &root).unwrap();
        let original = store.group(&parent).unwrap().clone();
        let before = serde_json::to_value(&store).unwrap();
        assert!(store.remove_group(&original).is_err());
        assert_eq!(serde_json::to_value(&store).unwrap(), before);
        let mut stale = original;
        stale.parent_id.clear();
        assert!(store.remove_group(&stale).is_err());
        assert_eq!(serde_json::to_value(&store).unwrap(), before);
    }

    #[test]
    fn moving_hosts_validates_full_selection_and_changes_only_membership() {
        let mut store = HostStore::default();
        let a = store.add_group("A", "").unwrap();
        let b = store.add_group("B", "").unwrap();
        for n in 0..3 {
            store.hosts.push(Host {
                label: format!("Host {n}"),
                address: format!("host-{n}"),
                port: 22,
                group_id: a.clone(),
                group: "A".into(),
                credential_id: format!("credential-{n}"),
                ..Default::default()
            });
        }
        let selected = store.hosts[..2].to_vec();
        let mut stale = selected.clone();
        stale[1].label = "Changed".into();
        let before = store.hosts.clone();
        assert!(store.move_hosts_to_group(&stale, &b).is_err());
        assert_eq!(store.hosts, before);
        assert!(store.move_hosts_to_group(&selected, "deleted").is_err());
        store.move_hosts_to_group(&selected, &b).unwrap();
        for (i, host) in store.hosts[..2].iter().enumerate() {
            let mut expected = selected[i].clone();
            expected.group_id = b.clone();
            expected.group = "B".into();
            assert_eq!(*host, expected);
        }
        assert_eq!(store.hosts[2], before[2]);
        let selected = store.hosts[..2].to_vec();
        store.move_hosts_to_group(&selected, "").unwrap();
        assert!(store.hosts[0].group.is_empty());
    }

    #[test]
    fn nested_groups_include_descendants_without_name_prefix_matching() {
        let mut store = HostStore::default();
        let parent = store.add_group("PortForwarding", "").unwrap();
        let child = store.add_group("218", &parent).unwrap();
        let deep = store.add_group("Docker", &child).unwrap();
        let other = store.add_group("PortForwarding-copy", "").unwrap();
        let other_child = store.add_group("218", &other).unwrap();
        assert!(store.add_group("  218  ", &parent).is_err());
        assert!(store.add_group("portforwarding", "").is_err());
        for bad in ["", "bad/name", "bad\nname"] {
            assert!(store.add_group(bad, "").is_err());
        }
        assert!(store.add_group("child", "missing").is_err());
        for (name, group) in [
            ("direct", &parent),
            ("child", &child),
            ("deep", &deep),
            ("elsewhere", &other_child),
        ] {
            store.hosts.push(Host {
                label: name.into(),
                group_id: group.clone(),
                ..Default::default()
            });
        }
        assert_eq!(
            store
                .hosts
                .iter()
                .filter(|h| store.host_in_group(h, &parent))
                .count(),
            3
        );
        assert_eq!(
            store
                .hosts
                .iter()
                .filter(|h| store.host_in_group(h, &child))
                .count(),
            2
        );
        assert_eq!(
            store
                .hosts
                .iter()
                .filter(|h| store.host_in_group(h, ""))
                .count(),
            4
        );
        assert_eq!(store.group_path(&deep), "PortForwarding / 218 / Docker");
        assert_eq!(store.child_groups(&parent).len(), 1);
        let mut host = Host::default();
        store
            .assign_host_group(&mut host, "PortForwarding / 218", None)
            .unwrap();
        assert_eq!(host.group_id, child);
        store.assign_host_group(&mut host, "", None).unwrap();
        assert!(host.group.is_empty() && host.group_id.is_empty());
    }

    #[test]
    fn empty_groups_and_legacy_labels_survive_reload_without_changing_hosts() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hosts.json");
        let legacy = r#"{"hosts":[{"label":"one","group":"Production"},{"label":"two","group":"literal/name"}]}"#;
        std::fs::write(&path, legacy).unwrap();
        let mut store = HostStore::load_from(&path).unwrap();
        let original = store.hosts.clone();
        assert_eq!(store.child_groups("").len(), 2);
        assert!(store.groups.iter().all(|g| g.parent_id.is_empty()));
        assert_eq!(store.groups, HostStore::load_from(&path).unwrap().groups);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), legacy);
        let empty = store.add_group("Empty", "").unwrap();
        store.save_to(&path).unwrap();
        let loaded = HostStore::load_from(&path).unwrap();
        assert_eq!(loaded.hosts, original);
        assert!(loaded.group(&empty).is_some());
        assert_eq!(
            loaded.group_path(loaded.host_group_id(&loaded.hosts[1])),
            "literal/name"
        );
        let mut edited = loaded.hosts[1].clone();
        store
            .assign_host_group(&mut edited, "literal/name", Some(&loaded.hosts[1]))
            .unwrap();
        assert_eq!(edited.group_id, loaded.host_group_id(&loaded.hosts[1]));
    }

    #[test]
    fn invalid_group_graphs_never_overwrite_the_library() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hosts.json");
        std::fs::write(&path, "preserve me").unwrap();
        let mut store = HostStore::default();
        let a = store.add_group("A", "").unwrap();
        let b = store.add_group("B", &a).unwrap();
        store.groups[0].parent_id = b;
        assert!(store.save_to(&path).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "preserve me");
        std::fs::write(&path, serde_json::to_string(&store).unwrap()).unwrap();
        assert!(HostStore::load_from(&path).is_err());
        store.groups[0].parent_id = "missing".into();
        assert!(store.save_to(&path).is_err());
        store.groups[0].parent_id.clear();
        store.hosts.push(Host {
            group_id: "missing".into(),
            ..Default::default()
        });
        assert!(store.save_to(&path).is_err());
    }

    #[test]
    fn legacy_libraries_and_identity_references_round_trip_without_passwords() {
        let mut store: HostStore = serde_json::from_str(
            r#"{"hosts":[{"label":"old","address":"example.org","port":22}],"keys":[]}"#,
        )
        .unwrap();
        assert!(store.identities.is_empty());
        assert!(store.hosts[0].credential_ref().is_empty());
        store.identities.push(Identity {
            id: "opaque-ref".into(),
            label: "Operations".into(),
            username: "deploy".into(),
        });
        store.hosts[0].identity_id = "opaque-ref".into();
        let mut host = store.hosts[0].clone();
        host.resolve_identity(&store).unwrap();
        assert_eq!(host.username, "deploy");
        assert_eq!(host.credential_ref(), "opaque-ref");
        let json = serde_json::to_string(&store).unwrap();
        assert!(!json.contains("password"));
        assert_eq!(
            serde_json::from_str::<HostStore>(&json).unwrap().identities,
            store.identities
        );
        host.identity_id = "missing".into();
        assert!(host.resolve_identity(&store).is_err());
    }

    #[test]
    fn keychain_overrides_only_at_connection_and_preserves_manual_fallbacks() {
        let key = tempfile::NamedTempFile::new().unwrap();
        let manual = Host {
            username: "manual-user".into(),
            credential_id: "manual-secret-ref".into(),
            identity_file: "unused-manual-key".into(),
            identity_id: "identity-ref".into(),
            key_label: "Selected key".into(),
            address: "example.invalid".into(),
            port: 22,
            ..Default::default()
        };
        let store = HostStore {
            keys: vec![KeyFile {
                label: "Selected key".into(),
                path: key.path().to_string_lossy().into_owned(),
            }],
            identities: vec![Identity {
                id: "identity-ref".into(),
                label: "Ops".into(),
                username: "selected-user".into(),
            }],
            ..Default::default()
        };
        let mut connection = manual.clone();
        connection.resolve_identity(&store).unwrap();
        connection.validate().unwrap();
        assert_eq!(connection.username, "selected-user");
        assert_eq!(connection.credential_ref(), "identity-ref");
        assert_eq!(connection.identity_file, store.keys[0].path);
        assert_eq!(manual.username, "manual-user");
        assert_eq!(manual.own_credential_ref(), "manual-secret-ref");
        let mut unlinked = manual.clone();
        unlinked.identity_id.clear();
        unlinked.key_label.clear();
        unlinked.resolve_identity(&store).unwrap();
        assert_eq!(unlinked.username, manual.username);
        assert_eq!(unlinked.credential_ref(), manual.credential_ref());
        assert_eq!(unlinked.identity_file, manual.identity_file);
        let mut missing = manual.clone();
        missing.key_label = "deleted".into();
        assert!(missing.resolve_identity(&store).is_err());
        let mut legacy = manual;
        legacy.credential_id = legacy.identity_id.clone();
        assert!(legacy.own_credential_ref().is_empty());
    }

    #[test]
    fn explicit_authentication_ignores_inactive_credentials_and_legacy_still_infers() {
        let key = tempfile::NamedTempFile::new().unwrap();
        let store = HostStore {
            keys: vec![KeyFile {
                label: "saved-key".into(),
                path: key.path().to_string_lossy().into(),
            }],
            identities: vec![Identity {
                id: "identity".into(),
                label: "Ops".into(),
                username: "deploy".into(),
            }],
            ..Default::default()
        };
        let stale = Host {
            username: "manual".into(),
            identity_id: "identity".into(),
            key_label: "saved-key".into(),
            identity_file: "missing/inactive/key".into(),
            credential_id: "host-password".into(),
            ..Default::default()
        };
        assert_eq!(stale.authentication_mode(), Authentication::Identity);
        for mode in [
            Authentication::Password,
            Authentication::Key,
            Authentication::Identity,
        ] {
            let mut host = stale.clone();
            host.authentication = Some(mode);
            host.resolve_identity(&store).unwrap();
            match mode {
                Authentication::Password => {
                    assert_eq!(host.username, "manual");
                    assert_eq!(host.credential_ref(), "host-password");
                    assert!(
                        host.identity_id.is_empty()
                            && host.identity_file.is_empty()
                            && host.key_label.is_empty()
                    );
                }
                Authentication::Key => {
                    assert_eq!(host.username, "manual");
                    assert_eq!(host.identity_file, store.keys[0].path);
                    assert!(host.identity_id.is_empty() && host.credential_ref().is_empty());
                }
                Authentication::Identity => {
                    assert_eq!(host.username, "deploy");
                    assert_eq!(host.credential_ref(), "identity");
                    assert!(host.identity_file.is_empty() && host.key_label.is_empty());
                }
            }
            let json = serde_json::to_string(&host).unwrap();
            assert_eq!(serde_json::from_str::<Host>(&json).unwrap(), host);
        }
        for mode in [Authentication::Key, Authentication::Identity] {
            let mut missing = Host {
                authentication: Some(mode),
                ..Default::default()
            };
            assert!(missing.resolve_identity(&store).is_err());
        }
        let legacy: Host = serde_json::from_str(r#"{"identity_file":"key.pem"}"#).unwrap();
        assert_eq!(legacy.authentication, None);
        assert_eq!(legacy.authentication_mode(), Authentication::Key);
        assert_eq!(
            Host::default().authentication_mode(),
            Authentication::Password
        );
    }

    #[test]
    fn duplicate_preserves_connection_fields_and_avoids_label_collisions() {
        let original = Host {
            label: "开发".into(),
            address: "192.0.2.1".into(),
            port: 2222,
            username: "alice".into(),
            group: "Production".into(),
            identity_file: "C:/keys/id_ed25519".into(),
            ..Default::default()
        };
        let mut store = HostStore {
            hosts: vec![original.clone()],
            ..Default::default()
        };
        let copy = store.duplicate_host(&original).unwrap();
        assert_eq!(copy.label, "开发-copy");
        let mut expected = original.clone();
        expected.label = copy.label.clone();
        assert_eq!(copy, expected);
        assert_eq!(
            store.duplicate_host(&original).unwrap().label,
            "开发-copy-2"
        );
        assert_eq!(store.hosts[0], original);
        let path = tempfile::tempdir().unwrap();
        store.save_to(&path.path().join("hosts.json")).unwrap();
        let loaded = HostStore::load_from(&path.path().join("hosts.json")).unwrap();
        assert_eq!(loaded.hosts, store.hosts);
        let mut stale = original;
        stale.port = 22;
        assert!(store.duplicate_host(&stale).is_err());
        assert_eq!(store.hosts.len(), 3);
    }

    #[test]
    fn ssh_addresses() {
        let host = Host::quick_connect("ssh alice@[2001:db8::1]:2200").unwrap();
        assert_eq!(host.endpoint(), "[2001:db8::1]:2200");
        assert_eq!(host.username, "alice");
        assert_eq!(Host::quick_connect("example.org").unwrap().port, 22);
        for bad in [
            "telnet://server",
            "ssh://server",
            "server:0",
            "server:65536",
            "-oProxyCommand=x",
            "server command",
            "[not-ipv6]:22",
            "[2001:db8::1]:bad",
        ] {
            assert!(Host::quick_connect(bad).is_err(), "{}", bad);
        }
    }
    #[test]
    fn library_round_trip_and_corruption() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("hosts.json");
        let mut store = HostStore::load_from(&path).unwrap();
        let mut h = Host::quick_connect("用户@example.org:2222").unwrap();
        h.label = "开发服务器".into();
        h.group = "开发".into();
        store.hosts.push(h);
        store.save_to(&path).unwrap();
        let mut read = HostStore::load_from(&path).unwrap();
        assert_eq!(read.hosts[0].label, "开发服务器");
        read.hosts.push(Host::quick_connect("other").unwrap());
        read.save_to(&path).unwrap();
        assert_eq!(HostStore::load_from(&path).unwrap().hosts.len(), 2);
        std::fs::write(&path, b"broken json").unwrap();
        assert!(HostStore::load_from(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"broken json");
    }
}
