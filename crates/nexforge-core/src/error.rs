//! Errors for the core. Every `Display` is a plain sentence a player can act on;
//! the UI and CLI show it as-is.

use std::path::PathBuf;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Could not {action} {path}: {source}")]
    Io {
        action: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("{path} is not valid: {message}")]
    BadStateFile { path: PathBuf, message: String },

    #[error("Nexforge doesn't connect to {host}.")]
    HostNotAllowed { host: String },

    #[error("{host} could not be reached: {message}")]
    Network { host: String, message: String },

    #[error("{host} returned error {status}. Try again later.")]
    Http {
        host: String,
        path: String,
        status: u16,
    },

    #[error("{source_name} rate limit reached, try again at {reset_at}.")]
    RateLimited {
        source_name: String,
        reset_at: String,
    },

    #[error("{host} sent a reply Nexforge couldn't read: {message}")]
    BadResponse { host: String, message: String },

    #[error("The download was damaged. Try again.")]
    ChecksumMismatch,

    #[error("The downloaded archive is not a usable addon: {0}")]
    BadArchive(String),

    #[error("{0} isn't installed.")]
    UnknownAddon(String),

    #[error("{0}")]
    NotManaged(String),

    #[error("{folder} belongs to {owner}. Remove {owner} first.")]
    FolderOwned { folder: String, owner: String },

    #[error("{0} is already in your AddOns folder. Refresh, then choose Identify source on it.")]
    FolderUnmanaged(String),

    #[error("{name} is already installed from {from}. To get it from {to} instead, choose Change source on it.")]
    AlreadyInstalled {
        name: String,
        from: String,
        to: String,
    },

    #[error("{0} is pinned. Unpin it before updating.")]
    Pinned(String),

    #[error("No release of {name} is listed for {flavour}.")]
    NoRelease { name: String, flavour: String },

    #[error("There's no earlier version of {0} to roll back to.")]
    NoBackup(String),

    #[error("{0}")]
    Unsupported(String),

    #[error("The installation could not be found: {0}")]
    NoInstallation(String),

    #[error("Couldn't reach Windows Credential Manager: {0}")]
    Keyring(String),

    #[error("{0}")]
    Other(String),

    /// Fixed in Settings under Sources; the CLI adds its own hint.
    #[error("CurseForge needs an API key.")]
    CurseForgeKeyNeeded,
    #[error("GitHub refused the token. Replace or remove it in Settings.")]
    GitHubTokenRejected,

    #[error("CurseForge refused the API key.")]
    CurseForgeKeyRejected,
    /// CurseForge answers bursts of searches with 403 (its rate limit).
    #[error("CurseForge is limiting searches. Try again in a minute.")]
    CurseForgeSearchBlocked,
    #[error("CurseForge is busy. Try again in a minute, or paste the addon's Project ID.")]
    CurseForgeNeedsProjectId,
}

impl Error {
    pub fn io(action: &'static str, path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Error::Io {
            action,
            path: path.into(),
            source,
        }
    }
}

/// Shorthand for mapping `std::io::Error` with context.
pub(crate) trait IoContext<T> {
    fn ctx(self, action: &'static str, path: impl Into<PathBuf>) -> Result<T>;
}

impl<T> IoContext<T> for std::result::Result<T, std::io::Error> {
    fn ctx(self, action: &'static str, path: impl Into<PathBuf>) -> Result<T> {
        self.map_err(|e| Error::io(action, path, e))
    }
}
