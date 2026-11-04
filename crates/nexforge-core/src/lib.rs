//! Nexforge core: installations, TOC parsing, reconcile, state files,
//! sources, the install engine and the network layer. No UI dependencies.

pub mod archive;
pub mod catalogue;
pub mod error;
pub mod flavour;
pub mod install;
pub mod installations;
pub mod keys;
pub mod known;
pub mod manager;
pub mod net;
pub mod paths;
pub mod reconcile;
pub mod self_update;
pub mod sources;
pub mod state;
pub mod toc;
pub mod version;

pub use error::{Error, Result};
pub use flavour::Flavour;
pub use installations::Installation;
pub use manager::{Manager, SourceKeys, Status};
pub use paths::AppPaths;
pub use sources::{Channel, Release, Source, SourceId};
pub use state::{AddonEntry, AddonsFile, Config};
