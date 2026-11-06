//! Where Nexforge keeps its own files (never inside the game folder, except
//! `addons.toml` next to the addons and a transient swap folder during installs).

use crate::error::{Error, Result};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct AppPaths {
    root: PathBuf,
}

impl AppPaths {
    /// Data dir from `NEXFORGE_HOME`, else the local (non-roaming) app-data
    /// dir: `%LOCALAPPDATA%\Nexabyte\Nexforge` on Windows. Local, because
    /// kept versions are large and should not roam with the profile.
    pub fn discover() -> Result<AppPaths> {
        if let Some(home) = std::env::var_os("NEXFORGE_HOME") {
            return Ok(AppPaths::at(home));
        }
        let dirs =
            directories::ProjectDirs::from("uk", "Nexabyte", "Nexforge").ok_or_else(|| {
                Error::Other("Could not find a home folder to keep Nexforge's settings in.".into())
            })?;
        // On Windows `data_local_dir` is `…\Local\Nexabyte\Nexforge\data`;
        // drop the trailing `data` so the folder is simply `…\Nexforge`.
        let local = dirs.data_local_dir();
        let root = if cfg!(windows) && local.ends_with("data") {
            local.parent().unwrap_or(local)
        } else {
            local
        };
        Ok(AppPaths::at(root))
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
    /// Browse index (SQLite). Safe to delete.
    pub fn catalogue(&self) -> PathBuf {
        self.root.join("catalogue.db")
    }
    /// Addon icons and screenshots. Safe to delete.
    pub fn image_cache(&self) -> PathBuf {
        self.root.join("cache").join("images")
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

/// Held while one process (the app or the CLI) changes an installation, so
/// two never interleave load → change → save of `addons.toml`, or swap the
/// same folders. An OS file lock: it is released if the process dies.
#[derive(Debug)]
pub struct InstallLock {
    _file: std::fs::File,
}

impl AppPaths {
    fn lock_file(&self, installation: &str) -> Result<std::fs::File> {
        let dir = self.root.join("locks");
        std::fs::create_dir_all(&dir).map_err(|e| Error::io("create", &dir, e))?;
        let p = dir.join(format!("{}.lock", sanitize(installation)));
        std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&p)
            .map_err(|e| Error::io("open", &p, e))
    }

    /// Wait (up to a few seconds) for the lock on `installation`.
    pub fn lock(&self, installation: &str) -> Result<InstallLock> {
        let file = self.lock_file(installation)?;
        for _ in 0..50 {
            match file.try_lock() {
                Ok(()) => return Ok(InstallLock { _file: file }),
                Err(std::fs::TryLockError::WouldBlock) => {
                    std::thread::sleep(std::time::Duration::from_millis(100))
                }
                Err(std::fs::TryLockError::Error(e)) => {
                    return Err(Error::io("lock", self.root.join("locks"), e))
                }
            }
        }
        Err(Error::Other(
            "Another Nexforge window or command is changing these addons. Try again when it has finished.".into(),
        ))
    }

    /// The lock on `installation` if it is free now, without waiting.
    pub fn try_lock(&self, installation: &str) -> Option<InstallLock> {
        let file = self.lock_file(installation).ok()?;
        file.try_lock().ok().map(|_| InstallLock { _file: file })
    }

    /// Tidy the data folder: rotate `network.log` past 5 MB (one old copy
    /// is kept), drop cached responses and images unused for 30 days, and
    /// clear downloads and staging left by a crash more than a day ago.
    /// Never touches backups, settings or anything in the game folder.
    pub fn housekeeping(&self) {
        const LOG_MAX: u64 = 5 * 1024 * 1024;
        let log = self.network_log();
        if std::fs::metadata(&log).is_ok_and(|m| m.len() > LOG_MAX) {
            let _ = std::fs::rename(&log, log.with_extension("log.1"));
        }
        let day = std::time::Duration::from_secs(24 * 3600);
        prune_older_than(&self.http_cache(), 30 * day);
        prune_older_than(&self.image_cache(), 30 * day);
        prune_older_than(&self.tmp(), day);
        prune_older_than(&self.staging(), day);
    }
}

/// Remove entries of `dir` (files or whole folders) not modified for `age`.
fn prune_older_than(dir: &Path, age: std::time::Duration) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let old = e
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|a| a > age);
        if !old {
            continue;
        }
        let p = e.path();
        if p.is_dir() {
            let _ = std::fs::remove_dir_all(&p);
        } else {
            let _ = std::fs::remove_file(&p);
        }
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
    fn lock_is_exclusive_and_released() {
        let dir = tempfile::tempdir().unwrap();
        let p = super::AppPaths::at(dir.path());
        let held = p.lock("forever").unwrap();
        assert!(p.try_lock("forever").is_none(), "second taker waits");
        assert!(
            p.try_lock("classic-era").is_some(),
            "other installs are free"
        );
        drop(held);
        assert!(p.try_lock("forever").is_some(), "released on drop");
    }

    #[test]
    fn housekeeping_rotates_the_log_and_keeps_recent_files() {
        let dir = tempfile::tempdir().unwrap();
        let p = super::AppPaths::at(dir.path());
        std::fs::write(p.network_log(), vec![b'x'; 6 * 1024 * 1024]).unwrap();
        std::fs::create_dir_all(p.tmp()).unwrap();
        std::fs::write(p.tmp().join("fresh.zip"), b"x").unwrap();
        p.housekeeping();
        assert!(!p.network_log().exists());
        assert!(p.network_log().with_extension("log.1").exists());
        assert!(p.tmp().join("fresh.zip").exists(), "recent downloads stay");
    }

    #[test]
    fn sanitize_versions() {
        assert_eq!(
            super::sanitize("bundle/v12.5.0+v1.0.10"),
            "bundle_v12.5.0+v1.0.10"
        );
        assert_eq!(super::sanitize(".."), "_");
    }
}
