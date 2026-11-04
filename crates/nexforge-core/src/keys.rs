//! API keys and tokens live in the OS credential store (Windows Credential
//! Manager), never in a file. Entries are `nexforge/<name>`.

use crate::error::{Error, Result};

const SERVICE: &str = "nexforge";

/// Names Nexforge stores: `github` (optional PAT), `wago`, `curseforge`.
pub const KNOWN: &[&str] = &["github", "wago", "curseforge"];

fn entry(name: &str) -> Result<keyring::Entry> {
    keyring::Entry::new(SERVICE, name).map_err(|e| Error::Keyring(e.to_string()))
}

pub fn get(name: &str) -> Result<Option<String>> {
    match entry(name)?.get_password() {
        Ok(v) if !v.trim().is_empty() => Ok(Some(v.trim().to_string())),
        Ok(_) | Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(Error::Keyring(e.to_string())),
    }
}

pub fn set(name: &str, value: &str) -> Result<()> {
    entry(name)?
        .set_password(value.trim())
        .map_err(|e| Error::Keyring(e.to_string()))
}

pub fn delete(name: &str) -> Result<()> {
    match entry(name)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(Error::Keyring(e.to_string())),
    }
}
