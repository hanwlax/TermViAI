//! Local host metadata. Authentication stays in the existing SSH transport.
use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Host {
    pub label: String,
    pub address: String,
    pub port: u16,
    pub username: String,
    pub group: String,
    pub identity_file: String,
    pub identity_id: String,
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
        if !self.identity_file.is_empty() && !Path::new(&self.identity_file).is_file() {
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

    pub fn resolve_identity(&mut self, store: &HostStore) -> anyhow::Result<()> {
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
            self.identity_file.clear();
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

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct HostStore {
    pub hosts: Vec<Host>,
    pub keys: Vec<KeyFile>,
    pub identities: Vec<Identity>,
}

impl HostStore {
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
            Ok(f) => serde_json::from_reader(f)
                .context("Could not read the local host library. The file has been preserved."),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e).context("Could not open the local host library."),
        }
    }
    pub fn save(&self) -> anyhow::Result<()> {
        self.save_to(&Self::path())
    }
    pub(crate) fn save_to(&self, path: &Path) -> anyhow::Result<()> {
        let dir = path.parent().context("Missing library directory")?;
        std::fs::create_dir_all(dir)?;
        let mut file = tempfile::NamedTempFile::new_in(dir)?;
        serde_json::to_writer_pretty(&mut file, self)?;
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
