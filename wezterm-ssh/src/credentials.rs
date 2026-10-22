//! Opaque credential references; passwords never enter SSH options or JSON.
use anyhow::Context;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

fn session_store() -> &'static Mutex<HashMap<String, String>> {
    static STORE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn save(id: &str, password: &str, persistent: bool) -> anyhow::Result<()> {
    anyhow::ensure!(!id.is_empty() && !password.is_empty(), "Enter a password.");
    if persistent {
        platform::save(id, password)?;
    } else {
        session_store()
            .lock()
            .unwrap()
            .insert(id.into(), password.into());
    }
    Ok(())
}

pub fn load(id: &str) -> anyhow::Result<Option<String>> {
    if id.is_empty() {
        return Ok(None);
    }
    if let Some(value) = session_store().lock().unwrap().get(id) {
        return Ok(Some(value.clone()));
    }
    if id.starts_with("session-") {
        return Ok(None);
    }
    platform::load(id)
}

pub fn delete(id: &str) -> anyhow::Result<()> {
    session_store().lock().unwrap().remove(id);
    platform::delete(id)
}

#[cfg(windows)]
mod platform {
    use super::*;
    use windows_sys::Win32::Security::Credentials::*;
    fn target(id: &str) -> Vec<u16> {
        format!("TermViAI/password/{id}")
            .encode_utf16()
            .chain(Some(0))
            .collect()
    }
    pub fn save(id: &str, password: &str) -> anyhow::Result<()> {
        let mut target = target(id);
        let mut bytes = password.as_bytes().to_vec();
        anyhow::ensure!(
            bytes.len() <= 2560,
            "Password exceeds Windows credential storage limit."
        );
        let entry = CREDENTIALW {
            Type: CRED_TYPE_GENERIC,
            TargetName: target.as_mut_ptr(),
            CredentialBlobSize: bytes.len() as u32,
            CredentialBlob: bytes.as_mut_ptr(),
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            ..unsafe { std::mem::zeroed() }
        };
        let ok = unsafe { CredWriteW(&entry, 0) };
        bytes.fill(0);
        if ok == 0 {
            return Err(std::io::Error::last_os_error())
                .context("Could not save password in Windows Credential Manager");
        }
        Ok(())
    }
    pub fn load(id: &str) -> anyhow::Result<Option<String>> {
        let mut entry = std::ptr::null_mut();
        if unsafe { CredReadW(target(id).as_ptr(), CRED_TYPE_GENERIC, 0, &mut entry) } == 0 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() == Some(1168) {
                return Ok(None);
            }
            return Err(error).context("Could not read Windows credential");
        }
        // CredFree must run even if the stored data cannot be decoded.
        let result = unsafe {
            let len = (*entry).CredentialBlobSize as usize;
            let bytes = if len == 0 {
                &[][..]
            } else {
                std::slice::from_raw_parts((*entry).CredentialBlob, len)
            };
            let result = std::str::from_utf8(bytes)
                .map(str::to_owned)
                .context("Invalid stored credential encoding");
            if len > 0 {
                std::ptr::write_bytes((*entry).CredentialBlob, 0, len);
            }
            CredFree(entry.cast());
            result
        };
        result.map(Some)
    }
    pub fn delete(id: &str) -> anyhow::Result<()> {
        if unsafe { CredDeleteW(target(id).as_ptr(), CRED_TYPE_GENERIC, 0) } == 0 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() != Some(1168) {
                return Err(error).context("Could not remove Windows credential");
            }
        }
        Ok(())
    }
}
#[cfg(not(windows))]
mod platform {
    pub fn save(_: &str, _: &str) -> anyhow::Result<()> {
        anyhow::bail!("Remembering passwords is supported on Windows.")
    }
    pub fn load(_: &str) -> anyhow::Result<Option<String>> {
        Ok(None)
    }
    pub fn delete(_: &str) -> anyhow::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    #[test]
    fn windows_vault_roundtrip_replace_and_delete() {
        let id = format!(
            "test-vault-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        assert!(super::load(&id).unwrap().is_none());
        struct Cleanup(String);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = super::delete(&self.0);
            }
        }
        let _cleanup = Cleanup(id.clone());
        super::save(&id, "synthetic 密碼", true).unwrap();
        assert_eq!(super::load(&id).unwrap().as_deref(), Some("synthetic 密碼"));
        super::save(&id, "replacement", true).unwrap();
        assert_eq!(super::load(&id).unwrap().as_deref(), Some("replacement"));
        super::delete(&id).unwrap();
        assert!(super::load(&id).unwrap().is_none());
    }

    #[test]
    fn session_password_roundtrip() {
        let id = format!("test-session-{}", std::process::id());
        super::save(&id, "密碼 !", false).unwrap();
        assert_eq!(super::load(&id).unwrap().as_deref(), Some("密碼 !"));
        super::delete(&id).unwrap();
        assert!(super::load(&id).unwrap().is_none());
    }
}
