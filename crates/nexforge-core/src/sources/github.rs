//! GitHub Releases. No key needed; an optional personal access token raises the
//! rate limit from 60 to 5000 requests an hour.

use super::{AddonInfo, CatalogueEntry, Channel, Release, Source, SourceId};
use crate::error::{Error, Result};
use crate::flavour::Flavour;
use crate::net::{Fetch, Get};
use crate::toc::parse_github_repo;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures_util::{stream, StreamExt};
use regex::Regex;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

pub const API: &str = "https://api.github.com";

/// How many of the newest releases get their `release.json` read. Older
/// releases fall back to asset-name matching, which keeps a first check to a
/// handful of requests against the 60/hour unauthenticated limit.
const RELEASE_JSON_DEPTH: usize = 3;

// ---- flavour labels: the one place GitHub's names map to `Flavour` ----

/// What a GitHub flavour label means to us.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Label {
    Ours(Flavour),
    /// A real game flavour we do not support (wrath, cata, mists …).
    Other,
    Unknown,
}

/// `release.json` `metadata[].flavor` values (BigWigsMods packager).
pub fn map_flavor(flavor: &str) -> Label {
    match flavor.trim().to_ascii_lowercase().as_str() {
        "forever" | "camelot" => Label::Ours(Flavour::Forever),
        "classic" | "vanilla" => Label::Ours(Flavour::ClassicEra),
        "bcc" | "tbc" => Label::Ours(Flavour::Anniversary),
        "mainline" | "retail" => Label::Ours(Flavour::Retail),
        "wrath" | "wotlk" | "wotlkc" | "cata" | "mists" | "mop" | "titan" => Label::Other,
        _ => Label::Unknown,
    }
}

/// Asset-name suffix (`-forever.zip`, `_camelot.zip`, `-classic.zip` …).
pub fn asset_label(name: &str) -> Label {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"(?i)[-_]([a-z]+)\.zip$").unwrap());
    match re.captures(name) {
        Some(c) => map_flavor(&c[1]),
        None => Label::Unknown,
    }
}

// ---- wire types ----

#[derive(Debug, Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    published_at: Option<DateTime<Utc>>,
    body: Option<String>,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Debug, Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: u64,
    digest: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ReleaseJson {
    #[serde(default)]
    releases: Vec<RjEntry>,
}

#[derive(Debug, Deserialize)]
struct RjEntry {
    filename: String,
    #[serde(default)]
    nolib: bool,
    version: Option<String>,
    #[serde(default)]
    metadata: Vec<RjMeta>,
}

#[derive(Debug, Deserialize)]
struct RjMeta {
    flavor: String,
}

#[derive(Debug, Deserialize)]
struct GhRepo {
    name: String,
    full_name: String,
    description: Option<String>,
    html_url: String,
    owner: GhOwner,
}

#[derive(Debug, Deserialize)]
struct GhOwner {
    login: String,
    avatar_url: Option<String>,
}

fn bad(message: impl ToString) -> Error {
    Error::BadResponse {
        host: "api.github.com".into(),
        message: message.to_string(),
    }
}

fn channel_of(r: &GhRelease) -> Channel {
    if r.tag_name.to_ascii_lowercase().contains("alpha") {
        Channel::Alpha
    } else if r.prerelease {
        Channel::Beta
    } else {
        Channel::Stable
    }
}

fn base_release(r: &GhRelease, a: &GhAsset, version: String, flavours: Vec<Flavour>) -> Release {
    Release {
        version,
        download_url: a.browser_download_url.clone(),
        file_name: Some(a.name.clone()),
        published_at: r.published_at,
        size: (a.size > 0).then_some(a.size),
        channel: channel_of(r),
        flavours,
        changelog: r.body.clone().filter(|b| !b.trim().is_empty()),
        sha256: a
            .digest
            .as_deref()
            .and_then(|d| d.strip_prefix("sha256:"))
            .map(str::to_ascii_lowercase),
        md5: None,
    }
}

/// Releases from one GitHub release using its `release.json`.
fn from_release_json(r: &GhRelease, rj: &ReleaseJson) -> Vec<Release> {
    let mut out = Vec::new();
    for e in rj.releases.iter().filter(|e| !e.nolib) {
        let Some(asset) = r.assets.iter().find(|a| a.name == e.filename) else {
            continue;
        };
        let mut flavours: Vec<Flavour> = e
            .metadata
            .iter()
            .filter_map(|m| match map_flavor(&m.flavor) {
                Label::Ours(f) => Some(f),
                _ => None,
            })
            .collect();
        flavours.sort();
        flavours.dedup();
        if flavours.is_empty() && !e.metadata.is_empty() {
            // Built only for flavours we do not support.
            continue;
        }
        let version = e.version.clone().unwrap_or_else(|| r.tag_name.clone());
        out.push(base_release(r, asset, version, flavours));
    }
    out
}

/// Releases from one GitHub release by asset-name suffix.
fn from_asset_names(r: &GhRelease) -> Vec<Release> {
    r.assets
        .iter()
        .filter(|a| a.name.to_ascii_lowercase().ends_with(".zip"))
        .filter(|a| !a.name.to_ascii_lowercase().contains("-nolib"))
        .filter_map(|a| {
            let flavours = match asset_label(&a.name) {
                Label::Ours(f) => vec![f],
                Label::Other => return None,
                Label::Unknown => vec![],
            };
            Some(base_release(r, a, r.tag_name.clone(), flavours))
        })
        .collect()
}

/// The repo owner's avatar, served by avatars.githubusercontent.com without
/// an API call (an image host on the allowlist).
pub fn avatar_url(repo: &str) -> Option<String> {
    let owner = repo.split('#').next()?.split('/').next()?;
    (!owner.is_empty()).then(|| format!("https://avatars.githubusercontent.com/{owner}?s=96"))
}

pub struct GitHub {
    fetch: Arc<dyn Fetch>,
    token: Option<String>,
}

impl GitHub {
    pub fn new(fetch: Arc<dyn Fetch>, token: Option<String>) -> Self {
        GitHub { fetch, token }
    }

    fn api(&self, path: &str, reason: String) -> Get {
        let mut g = Get::new(format!("{API}{path}"), reason)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .fresh_for(Duration::from_secs(600));
        if let Some(t) = &self.token {
            g = g.header("Authorization", format!("Bearer {t}"));
        }
        g
    }

    /// Split a source id into `(owner/repo, tag prefix)`. `owner/repo#tag`
    /// means only releases tagged `tag/…` (one repo, several addons).
    fn check_id(id: &str) -> Result<(&str, Option<&str>)> {
        let (repo, tag) = match id.split_once('#') {
            Some((r, t)) if !t.is_empty() && !t.contains('/') => (r, Some(t)),
            Some(_) => ("", None),
            None => (id, None),
        };
        if parse_github_repo(repo).as_deref() == Some(repo) {
            Ok((repo, tag))
        } else {
            Err(Error::Other(format!(
                "\"{id}\" is not a GitHub repository. Use owner/repo."
            )))
        }
    }
}

/// Keep only releases tagged `tag/…`, and show their version without the prefix.
fn tagged(releases: Vec<GhRelease>, tag: Option<&str>) -> Vec<GhRelease> {
    let Some(tag) = tag else { return releases };
    let prefix = format!("{tag}/");
    releases
        .into_iter()
        .filter_map(|mut r| {
            let v = r.tag_name.strip_prefix(&prefix)?.to_string();
            r.tag_name = v;
            Some(r)
        })
        .collect()
}

#[async_trait]
impl Source for GitHub {
    fn id(&self) -> SourceId {
        SourceId::Github
    }
    fn needs_key(&self) -> bool {
        false
    }

    /// GitHub has no addon index; search the bundled one (`data/github-index.toml`).
    async fn search(&self, q: &str, _flavour: Flavour) -> Result<Vec<CatalogueEntry>> {
        Ok(crate::known::search(q)
            .into_iter()
            .map(|k| CatalogueEntry {
                source: SourceId::Github,
                source_id: k.source_id(),
                name: k.name.clone(),
                authors: k
                    .repo
                    .split('/')
                    .next()
                    .map(str::to_string)
                    .into_iter()
                    .collect(),
                summary: None,
                flavours: k.flavours.clone(),
                downloads: None,
                downloads_monthly: None,
                updated_at: None,
                icon_url: avatar_url(&k.repo),
            })
            .collect())
    }

    /// The bundled index is the whole GitHub catalogue.
    async fn catalogue(&self) -> Result<Vec<CatalogueEntry>> {
        self.search("", Flavour::Forever).await
    }

    async fn resolve(&self, source_id: &str) -> Result<AddonInfo> {
        let (repo, _) = Self::check_id(source_id)?;
        let known = crate::known::by_source_id(source_id);
        let text = self
            .fetch
            .get_text(self.api(
                &format!("/repos/{repo}"),
                format!("repo details for {repo}"),
            ))
            .await?;
        let r: GhRepo = serde_json::from_str(&text).map_err(bad)?;
        let _ = r.full_name;
        Ok(AddonInfo {
            // A repo that ships several addons is named after the repo; the
            // index knows which one this is.
            name: known.map(|k| k.name.clone()).unwrap_or(r.name),
            authors: vec![r.owner.login],
            // The owner's avatar (avatars.githubusercontent.com) is the icon;
            // GitHub has no addon screenshots.
            icon_url: r.owner.avatar_url,
            screenshots: vec![],
            summary: r.description,
            url: Some(r.html_url),
        })
    }

    async fn all_releases(&self, source_id: &str) -> Result<Vec<Release>> {
        let (repo, tag) = Self::check_id(source_id)?;
        // A shared repo interleaves its addons' releases; read further back.
        let per_page = if tag.is_some() { 100 } else { 10 };
        let text = self
            .fetch
            .get_text(self.api(
                &format!("/repos/{repo}/releases?per_page={per_page}"),
                format!("releases for {repo}"),
            ))
            .await?;
        let releases: Vec<GhRelease> = serde_json::from_str(&text).map_err(bad)?;
        let releases = tagged(releases, tag);
        let hint = crate::known::by_source_id(source_id)
            .map(|k| k.flavours.clone())
            .unwrap_or_default();
        let mut out = Vec::new();
        let mut json_reads = 0;
        for r in releases.iter().filter(|r| !r.draft) {
            let rj_asset = r.assets.iter().find(|a| a.name == "release.json");
            let mut done = false;
            if let (Some(a), true) = (rj_asset, json_reads < RELEASE_JSON_DEPTH) {
                json_reads += 1;
                let get = Get::new(
                    a.browser_download_url.clone(),
                    format!("release.json for {repo} {}", r.tag_name),
                )
                // A tag's release.json does not change.
                .fresh_for(Duration::from_secs(30 * 24 * 3600));
                if let Ok(body) = self.fetch.get_text(get).await {
                    if let Ok(rj) = serde_json::from_str::<ReleaseJson>(&body) {
                        out.extend(from_release_json(r, &rj));
                        done = true;
                    }
                }
            }
            if !done {
                out.extend(from_asset_names(r));
            }
        }
        // Releases that say nothing about flavour take the index's hint (the
        // addon's own TOC `## Interface`).
        if !hint.is_empty() {
            for r in out.iter_mut().filter(|r| r.flavours.is_empty()) {
                r.flavours = hint.clone();
            }
        }
        Ok(out)
    }

    /// One request per repo (cached 10 minutes), at most 4 at a time. A rate
    /// limit fails the whole batch so the UI can say when to retry; any other
    /// per-repo failure just leaves that repo out.
    async fn batch_latest(
        &self,
        ids: &[String],
        flavour: Flavour,
        channel: Channel,
    ) -> Result<HashMap<String, Release>> {
        let results: Vec<(String, Result<Vec<Release>>)> = stream::iter(ids.iter().cloned())
            .map(|id| async move {
                let r = self.releases(&id, flavour).await;
                (id, r)
            })
            .buffer_unordered(4)
            .collect()
            .await;
        let mut out = HashMap::new();
        for (id, r) in results {
            match r {
                Ok(rels) => {
                    if let Some(rel) = super::pick_release(&rels, flavour, channel) {
                        out.insert(id, rel.clone());
                    }
                }
                Err(e @ Error::RateLimited { .. }) => return Err(e),
                Err(_) => {}
            }
        }
        Ok(out)
    }

    async fn resolve_url(&self, url: &str) -> Result<Option<String>> {
        if !url.to_ascii_lowercase().contains("github.com/") {
            return Ok(None);
        }
        let Some(repo) = parse_github_repo(url) else {
            return Ok(None);
        };
        // A repo that ships several addons (one tag prefix each) is never
        // installed whole: use the tag in the URL (`…/releases/tag/spoken/v3.8.0`),
        // else the repo's first index entry.
        let shared: Vec<_> = crate::known::github_index()
            .iter()
            .filter(|k| k.repo.eq_ignore_ascii_case(&repo) && k.tag.is_some())
            .collect();
        if shared.is_empty() {
            return Ok(Some(repo));
        }
        let lower = url.to_ascii_lowercase();
        let pick = shared
            .iter()
            .find(|k| {
                let t = k.tag.as_deref().unwrap_or_default().to_ascii_lowercase();
                lower.contains(&format!("/tag/{t}/")) || lower.contains(&format!("/tag/{t}%2f"))
            })
            .unwrap_or(&shared[0]);
        Ok(Some(pick.source_id()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels() {
        assert_eq!(
            asset_label("Foo-1.0-forever.zip"),
            Label::Ours(Flavour::Forever)
        );
        assert_eq!(
            asset_label("Foo_1.0_Camelot.zip"),
            Label::Ours(Flavour::Forever)
        );
        assert_eq!(
            asset_label("Foo-1.0-classic.zip"),
            Label::Ours(Flavour::ClassicEra)
        );
        assert_eq!(
            asset_label("Foo-1.0_tbc.zip"),
            Label::Ours(Flavour::Anniversary)
        );
        assert_eq!(asset_label("Foo-1.0-wrath.zip"), Label::Other);
        assert_eq!(asset_label("Foo-v1.0.zip"), Label::Unknown);
        assert_eq!(map_flavor("mainline"), Label::Ours(Flavour::Retail));
    }
}
