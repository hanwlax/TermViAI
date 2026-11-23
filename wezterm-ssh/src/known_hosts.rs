//! TermViAI's private endpoint-to-SHA256 pins. Other SSH clients keep using OpenSSH verification.
use anyhow::{ensure, Context};
use base64::engine::general_purpose::STANDARD_NO_PAD;
use base64::Engine;
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};

pub const CONFIG_KEY: &str = "termviai_known_hosts";

/// The exact private-store entry an unattended SSH handshake is waiting for.
#[derive(Clone, Debug, thiserror::Error)]
#[error("Verify the SSH fingerprint for {endpoint} in its terminal.")]
pub struct PendingHostKey {
    pub path: PathBuf,
    pub endpoint: String,
    pub fingerprint: String,
}

/// Read each store at most once per scheduling pass, and only when a task is waiting.
#[derive(Default)]
pub struct TrustSnapshot {
    stores: BTreeMap<PathBuf, BTreeMap<String, String>>,
}
impl TrustSnapshot {
    pub fn trusts(&mut self, key: &PendingHostKey) -> bool {
        let pins = self
            .stores
            .entry(key.path.clone())
            .or_insert_with(|| load(&key.path).unwrap_or_default());
        pins.get(&key.endpoint) == Some(&key.fingerprint)
    }
}

pub fn load(path: &Path) -> anyhow::Result<BTreeMap<String, String>> {
    match File::open(path) {
        Ok(file) => {
            serde_json::from_reader(file).with_context(|| format!("Reading {}", path.display()))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(e) => Err(e).with_context(|| format!("Opening {}", path.display())),
    }
}

fn change(
    path: &Path,
    endpoint: &str,
    expected: Option<&str>,
    new: Option<&str>,
) -> anyhow::Result<()> {
    let dir = path.parent().context("Invalid known hosts path")?;
    fs::create_dir_all(dir)?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path.with_extension("lock"))?;
    lock.lock()?;
    let mut pins = load(path)?;
    let current = pins.get(endpoint).map(String::as_str);
    if current == new {
        return Ok(());
    }
    ensure!(
        current == expected,
        "The saved key for {endpoint} changed. Review it again."
    );
    if let Some(new) = new {
        pins.insert(endpoint.to_owned(), new.to_owned());
    } else {
        pins.remove(endpoint);
    }
    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    serde_json::to_writer_pretty(&mut tmp, &pins)?;
    tmp.as_file().sync_all()?;
    tmp.persist(path)?;
    Ok(())
}

pub fn remove(path: &Path, endpoint: &str, fingerprint: &str) -> anyhow::Result<()> {
    change(path, endpoint, Some(fingerprint), None)
}

pub(crate) fn verify(
    path: &Path,
    host: &str,
    port: u16,
    digest: &[u8],
    ask: impl FnOnce(String, bool, PendingHostKey) -> anyhow::Result<bool>,
) -> anyhow::Result<()> {
    ensure!(digest.len() == 32, "Missing SHA256 server key fingerprint");
    let endpoint = match host.parse::<std::net::Ipv6Addr>() {
        Ok(ip) => format!("[{ip}]:{port}"),
        Err(_) => format!("{}:{port}", host.to_ascii_lowercase()),
    };
    let fingerprint = format!("SHA256:{}", STANDARD_NO_PAD.encode(digest));
    let pins = load(path)?;
    let previous = pins.get(&endpoint).map(String::as_str);
    if previous == Some(fingerprint.as_str()) {
        return Ok(());
    }
    let message = match previous {
        Some(old) => format!("Host key changed for {endpoint}.\nSaved fingerprint: {old}\nPresented fingerprint: {fingerprint}"),
        None => format!("First connection to {endpoint}.\nPresented fingerprint: {fingerprint}"),
    };
    ensure!(
        ask(
            message,
            previous.is_some(),
            PendingHostKey {
                path: path.to_owned(),
                endpoint: endpoint.clone(),
                fingerprint: fingerprint.clone()
            }
        )?,
        "SSH host key was not trusted"
    );
    change(path, &endpoint, previous, Some(&fingerprint))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn first_use_rotation_rejection_and_stale_decisions() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test with spaces").join("known_hosts.json");
        let accept = |_: String, _: bool, _: PendingHostKey| Ok(true);
        verify(&file, "HOST", 22, &[1; 32], |_, changed, _| {
            assert!(!changed);
            Ok(true)
        })
        .unwrap();
        verify(&file, "host", 22, &[1; 32], |_, _, _| {
            panic!("unexpected prompt")
        })
        .unwrap();
        let old = load(&file).unwrap()["host:22"].clone();
        assert!(verify(&file, "host", 22, &[2; 32], |message, changed, _| {
            assert!(changed && message.contains(&old));
            Ok(false)
        })
        .is_err());
        assert_eq!(load(&file).unwrap()["host:22"], old);
        verify(&file, "host", 2222, &[1; 32], accept).unwrap();
        assert!(verify(&file, "host", 22, &[3; 32], |_, _, _| {
            verify(&file, "host", 22, &[2; 32], accept)?;
            Ok(true)
        })
        .is_err());
        assert!(remove(&file, "host:22", &old).is_err());
        let pins = load(&file).unwrap();
        remove(&file, "host:22", &pins["host:22"]).unwrap();
        assert_eq!(load(&file).unwrap().len(), 1);
        fs::write(&file, b"{").unwrap();
        assert!(verify(&file, "host", 22, &[1; 32], accept).is_err());
        assert_eq!(fs::read(&file).unwrap(), b"{");
    }
}
