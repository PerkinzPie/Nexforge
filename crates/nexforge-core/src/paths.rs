//! Where Nexforge keeps its own files (never inside the game folder, except
//! `addons.toml` next to the addons and a transient swap folder during installs).

use crate::error::{Error, Result};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct AppPaths {
    root: PathBuf,
}

impl AppPaths {
    /// Data dir from `NEXFORGE_HOME`, else the OS app-data dir
    /// (`%APPDATA%\Nexabyte\Nexforge` on Windows).
    pub fn discover() -> Result<AppPaths> {
        if let Some(home) = std::env::var_os("NEXFORGE_HOME") {
            return Ok(AppPaths::at(home));
        }
        let dirs =
            directories::ProjectDirs::from("uk", "Nexabyte", "Nexforge").ok_or_else(|| {
                Error::Other("Could not find a home folder to keep Nexforge's settings in.".into())
            })?;
        Ok(AppPaths::at(dirs.config_dir()))
    }

    pub fn at(root: impl Into<PathBuf>) -> AppPaths {
        AppPaths { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn config_file(&self) -> PathBuf {
        self.root.join("config.toml")
    }
    pub fn network_log(&self) -> PathBuf {
        self.root.join("network.log")
    }
    /// HTTP response cache (ETags). Safe to delete.
    pub fn http_cache(&self) -> PathBuf {
        self.root.join("cache").join("http")
    }
    pub fn tmp(&self) -> PathBuf {
        self.root.join("tmp")
    }
    pub fn staging(&self) -> PathBuf {
        self.root.join("staging")
    }
    pub fn backups(&self, installation: &str, addon: &str) -> PathBuf {
        self.root
            .join("backups")
            .join(sanitize(installation))
            .join(sanitize(addon))
    }
}

/// Make a string safe to use as one path component on every OS.
pub fn sanitize(s: &str) -> String {
    let out: String = s
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let out = out.trim_matches(['.', ' ']).to_string();
    if out.is_empty() {
        "_".into()
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn sanitize_versions() {
        assert_eq!(
            super::sanitize("bundle/v12.5.0+v1.0.10"),
            "bundle_v12.5.0+v1.0.10"
        );
        assert_eq!(super::sanitize(".."), "_");
    }
}
