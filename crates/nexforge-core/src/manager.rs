//! High-level operations shared by the CLI and (later) the app: update
//! checks, update, install, pin. Thin over sources + engine + state.

use crate::error::{Error, Result};
use crate::flavour::Flavour;
use crate::install::{Engine, Target};
use crate::installations::Installation;
use crate::net::{Fetch, Net, Progress};
use crate::paths::AppPaths;
use crate::sources::{
    curseforge::CurseForge, github::GitHub, parse_spec, pick_release, wowinterface::WowInterface,
    Channel, Release, Source, SourceId,
};
use crate::state::{AddonEntry, AddonsFile, Config};
use crate::version::is_newer;
use futures_util::future::join_all;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    UpToDate,
    Available(Release),
    /// Pinned; carries the newer release if there is one.
    Pinned(Option<Release>),
    /// The newest release is the one the user skipped.
    Skipped(Release),
    /// The host lists nothing for this flavour.
    NoFlavourBuild,
    /// Found in AddOns, not managed yet ("Identify source").
    NotManaged,
    /// Its source is disabled, needs a key, or is not implemented yet.
    SourceUnavailable(SourceId),
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct CheckResult {
    pub id: String,
    pub status: Status,
}

#[derive(Debug, Default)]
pub struct CheckReport {
    pub results: Vec<CheckResult>,
    /// Per-source failures, e.g. "GitHub rate limit, try again at 14:05."
    pub source_errors: Vec<(SourceId, String)>,
}

impl CheckReport {
    pub fn status_of(&self, id: &str) -> Option<&Status> {
        self.results.iter().find(|r| r.id == id).map(|r| &r.status)
    }
    pub fn available(&self) -> impl Iterator<Item = (&str, &Release)> {
        self.results.iter().filter_map(|r| match &r.status {
            Status::Available(rel) => Some((r.id.as_str(), rel)),
            _ => None,
        })
    }
}

/// Keys and tokens read from the OS credential store.
#[derive(Debug, Clone, Default)]
pub struct SourceKeys {
    /// Optional personal access token (raises GitHub's rate limit).
    pub github: Option<String>,
    /// Required for CurseForge.
    pub curseforge: Option<String>,
}

impl SourceKeys {
    pub fn from_credential_store() -> SourceKeys {
        SourceKeys {
            github: crate::keys::get("github").ok().flatten(),
            curseforge: crate::keys::get("curseforge").ok().flatten(),
        }
    }
}

pub struct Manager {
    pub paths: AppPaths,
    pub config: Config,
    fetch: Arc<dyn Fetch>,
    sources: Vec<Arc<dyn Source>>,
}

impl Manager {
    /// Real network, config from disk, keys from the credential store.
    pub fn open(paths: AppPaths) -> Result<Manager> {
        let config = Config::load_or_default(&paths.config_file())?;
        let net = Net::new(Some(paths.network_log()), Some(paths.http_cache()))?;
        Ok(Self::with_fetch(
            paths,
            config,
            Arc::new(net),
            SourceKeys::from_credential_store(),
        ))
    }

    pub fn with_fetch(
        paths: AppPaths,
        config: Config,
        fetch: Arc<dyn Fetch>,
        keys: SourceKeys,
    ) -> Manager {
        let mut sources: Vec<Arc<dyn Source>> = Vec::new();
        if config.sources.github {
            sources.push(Arc::new(GitHub::new(fetch.clone(), keys.github)));
        }
        if config.sources.wowinterface {
            sources.push(Arc::new(WowInterface::new(fetch.clone())));
        }
        // CurseForge only with a key, and only while turned on in config.
        if let (true, Some(key)) = (config.sources.curseforge, keys.curseforge) {
            sources.push(Arc::new(CurseForge::new(fetch.clone(), key)));
        }
        // Wago needs a paid key; not implemented.
        Manager {
            paths,
            config,
            fetch,
            sources,
        }
    }

    pub fn save_config(&self) -> Result<()> {
        self.config.save(&self.paths.config_file())
    }

    pub fn source(&self, id: SourceId) -> Option<&Arc<dyn Source>> {
        self.sources.iter().find(|s| s.id() == id)
    }

    /// Sources that can serve addons right now.
    pub fn available_sources(&self) -> Vec<SourceId> {
        self.sources.iter().map(|s| s.id()).collect()
    }

    pub fn engine(&self) -> Engine {
        Engine::new(
            self.fetch.clone(),
            self.paths.clone(),
            self.config.keep_previous_versions,
        )
    }

    fn require_source(&self, id: SourceId) -> Result<&Arc<dyn Source>> {
        self.source(id).ok_or_else(|| {
            Error::Unsupported(match id {
                SourceId::Curseforge => "CurseForge needs an API key. Get one at console.curseforge.com, then run `nexforge key set curseforge`.".to_string(),
                SourceId::Wago => "Wago needs a paid API key, and Nexforge does not support it yet.".to_string(),
                _ => format!("{} is turned off in Settings.", id.label()),
            })
        })
    }

    /// Check every addon for updates. One failing source never fails the rest.
    pub async fn check(&self, flavour: Flavour, state: &AddonsFile) -> CheckReport {
        let mut report = CheckReport::default();
        let mut batches: BTreeMap<(SourceId, Channel), Vec<&AddonEntry>> = BTreeMap::new();
        for e in &state.addons {
            match e.source {
                None => report.results.push(CheckResult {
                    id: e.id.clone(),
                    status: Status::NotManaged,
                }),
                Some(s) if self.source(s).is_none() => report.results.push(CheckResult {
                    id: e.id.clone(),
                    status: Status::SourceUnavailable(s),
                }),
                Some(s) => batches.entry((s, e.channel)).or_default().push(e),
            }
        }

        let jobs = batches.iter().map(|((s, ch), entries)| async move {
            let src = self.source(*s).expect("filtered above");
            let ids: Vec<String> = entries.iter().map(|e| e.source_id.clone()).collect();
            (*s, entries, src.batch_latest(&ids, flavour, *ch).await)
        });
        for (s, entries, result) in join_all(jobs).await {
            match result {
                Err(e) => {
                    let msg = e.to_string();
                    for entry in entries.iter() {
                        report.results.push(CheckResult {
                            id: entry.id.clone(),
                            status: Status::Failed(msg.clone()),
                        });
                    }
                    report.source_errors.push((s, msg));
                }
                Ok(latest) => {
                    for entry in entries.iter() {
                        let status = status_for(entry, latest.get(&entry.source_id));
                        report.results.push(CheckResult {
                            id: entry.id.clone(),
                            status,
                        });
                    }
                }
            }
        }
        let order: HashMap<&str, usize> = state
            .addons
            .iter()
            .enumerate()
            .map(|(i, a)| (a.id.as_str(), i))
            .collect();
        report
            .results
            .sort_by_key(|r| order.get(r.id.as_str()).copied().unwrap_or(usize::MAX));
        report
    }

    /// Fresh release list for an addon and the one to install, honouring
    /// its channel. `anyway` falls back to the newest release of any flavour.
    pub async fn pick(
        &self,
        entry: &AddonEntry,
        flavour: Flavour,
        anyway: bool,
    ) -> Result<(Release, bool)> {
        let source = entry
            .source
            .ok_or_else(|| Error::NotManaged(format!("{} is not managed yet. Install it from a source first (nexforge install <source:id> --as {}).", entry.name, entry.id)))?;
        let src = self.require_source(source)?;
        let rels = src.releases(&entry.source_id, flavour).await?;
        let picked = match pick_release(&rels, flavour, entry.channel) {
            Some(r) => Some((r.clone(), r.flavour_unknown())),
            None if anyway => src
                .all_releases(&entry.source_id)
                .await?
                .into_iter()
                .find(|r| entry.channel.accepts(r.channel))
                .map(|r| (r, true)),
            None => None,
        };
        if let Some((r, mismatch)) = picked {
            if r.download_url.is_empty() {
                return Err(Error::Unsupported(match source {
                    SourceId::Curseforge => format!(
                        "The author of {} does not allow downloads outside the CurseForge app, so Nexforge cannot install or update it.",
                        entry.name
                    ),
                    _ => format!("{} {} has no download link.", entry.name, r.version),
                }));
            }
            return Ok((r, mismatch));
        }
        Err(Error::NoRelease {
            name: entry.name.clone(),
            flavour: flavour.short_label().to_string(),
        })
    }

    /// Update one addon. Returns `None` when already up to date.
    pub async fn update(
        &self,
        installation: &Installation,
        state: &mut AddonsFile,
        state_path: &Path,
        id: &str,
        progress: Progress<'_>,
    ) -> Result<Option<(String, AddonEntry)>> {
        let entry = state
            .get(id)
            .cloned()
            .ok_or_else(|| Error::UnknownAddon(id.into()))?;
        if entry.pinned {
            return Err(Error::Pinned(entry.name));
        }
        let (release, mismatch) = self.pick(&entry, installation.flavour, false).await?;
        if !is_newer(&release.version, &entry.installed_version) {
            return Ok(None);
        }
        let from = entry.installed_version.clone();
        let mut next = entry.clone();
        next.flavour_mismatch = mismatch;
        let mut t = Target {
            installation,
            state,
            state_path,
        };
        let done = self
            .engine()
            .install(&mut t, next, &release, progress)
            .await?;
        Ok(Some((from, done)))
    }

    /// An icon or screenshot, from the disk cache (7 days) or an image host.
    pub async fn image(&self, url: &str) -> Result<crate::net::Image> {
        use sha2::{Digest, Sha256};
        let key: String = Sha256::digest(url.as_bytes())[..16]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let dir = self.paths.image_cache();
        let (body, kind) = (dir.join(&key), dir.join(format!("{key}.type")));
        let fresh = std::fs::metadata(&body)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age < std::time::Duration::from_secs(7 * 24 * 3600));
        if fresh {
            if let (Ok(bytes), Ok(content_type)) =
                (std::fs::read(&body), std::fs::read_to_string(&kind))
            {
                return Ok(crate::net::Image {
                    bytes,
                    content_type,
                });
            }
        }
        let img = self.fetch.get_image(url, "addon icon").await?;
        if std::fs::create_dir_all(&dir).is_ok() {
            let _ = std::fs::write(&body, &img.bytes);
            let _ = std::fs::write(&kind, &img.content_type);
        }
        Ok(img)
    }

    /// Search every enabled source. GitHub answers from the bundled index, so
    /// it never blocks on the network; a failing source is reported, not fatal.
    pub async fn search(
        &self,
        q: &str,
        flavour: Flavour,
    ) -> (Vec<crate::sources::CatalogueEntry>, Vec<(SourceId, String)>) {
        let jobs = self
            .sources
            .iter()
            .map(|s| async move { (s.id(), s.search(q, flavour).await) });
        let mut hits = Vec::new();
        let mut errors = Vec::new();
        for (id, r) in join_all(jobs).await {
            match r {
                Ok(h) => hits.extend(h),
                Err(e) => errors.push((id, e.to_string())),
            }
        }
        // Builds for this flavour first, then by downloads; GitHub index
        // entries (curated, no download counts) lead within each band.
        hits.sort_by_key(|h| {
            (
                !(h.flavours.contains(&flavour) || h.flavours.is_empty()),
                h.source != SourceId::Github,
                std::cmp::Reverse(h.downloads),
            )
        });
        (hits, errors)
    }

    /// Resolve `spec` (`source:id`, a web address, or a name from the GitHub
    /// index) to `(source, id)`.
    pub async fn resolve_spec(&self, spec: &str) -> Result<(SourceId, String)> {
        if let Some(p) = parse_spec(spec) {
            return Ok(p);
        }
        for s in &self.sources {
            if let Some(id) = s.resolve_url(spec).await? {
                return Ok((s.id(), id));
            }
        }
        if spec.to_ascii_lowercase().contains("curseforge.com/")
            && self.source(SourceId::Curseforge).is_none()
        {
            return Err(Error::Unsupported("That is a CurseForge address. CurseForge needs an API key: get one at console.curseforge.com, then run `nexforge key set curseforge`.".into()));
        }
        if let Some(k) = crate::known::find(spec) {
            return Ok((SourceId::Github, k.source_id()));
        }
        Err(Error::Other(format!(
            "Could not tell which addon \"{spec}\" is. Use its name (nexforge search <name>), a GitHub or WoWInterface address, or source:id (github:owner/repo, wowi:12345)."
        )))
    }

    /// Install an addon from a source. `adopt` names an existing entry (for
    /// example one reconcile found but could not match) to take over.
    #[allow(clippy::too_many_arguments)]
    pub async fn install(
        &self,
        installation: &Installation,
        state: &mut AddonsFile,
        state_path: &Path,
        spec: &str,
        adopt: Option<&str>,
        channel: Channel,
        anyway: bool,
        progress: Progress<'_>,
    ) -> Result<AddonEntry> {
        let (source, source_id) = self.resolve_spec(spec).await?;
        let src = self.require_source(source)?;

        let existing = match adopt {
            Some(id) => Some(
                state
                    .get(id)
                    .cloned()
                    .ok_or_else(|| Error::UnknownAddon(id.into()))?,
            ),
            None => state
                .addons
                .iter()
                .find(|e| {
                    (e.source == Some(source) && e.source_id == source_id)
                        || (!e.is_managed() && e.ids.get(source) == Some(source_id.as_str()))
                })
                .cloned(),
        };

        let mut entry = match existing {
            Some(mut e) => {
                e.source = Some(source);
                e.source_id = source_id.clone();
                e.channel = channel;
                e.ids.set(source, source_id.clone());
                e
            }
            None => {
                let info = src.resolve(&source_id).await?;
                let mut ids = crate::state::KnownIds::default();
                ids.set(source, source_id.clone());
                AddonEntry {
                    id: state.unique_id(&info.name),
                    name: info.name,
                    source: Some(source),
                    source_id: source_id.clone(),
                    installed_version: String::new(),
                    installed_at: None,
                    folders: vec![],
                    pinned: false,
                    skipped_version: String::new(),
                    channel,
                    author: info.authors.first().cloned(),
                    flavour_mismatch: false,
                    ids,
                }
            }
        };
        let (release, mismatch) = self.pick(&entry, installation.flavour, anyway).await?;
        entry.flavour_mismatch = mismatch;
        let mut t = Target {
            installation,
            state,
            state_path,
        };
        self.engine()
            .install(&mut t, entry, &release, progress)
            .await
    }
}

fn status_for(entry: &AddonEntry, latest: Option<&Release>) -> Status {
    let Some(r) = latest else {
        return if entry.pinned {
            Status::Pinned(None)
        } else {
            Status::NoFlavourBuild
        };
    };
    let newer = is_newer(&r.version, &entry.installed_version);
    if entry.pinned {
        Status::Pinned(newer.then(|| r.clone()))
    } else if !newer {
        Status::UpToDate
    } else if !entry.skipped_version.is_empty() && entry.skipped_version == r.version {
        Status::Skipped(r.clone())
    } else {
        Status::Available(r.clone())
    }
}
