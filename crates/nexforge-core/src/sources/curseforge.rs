//! CurseForge (Core API v1). Needs an API key (`x-api-key`), kept in the OS
//! credential store as `nexforge/curseforge`.
//!
//! Authors can opt out of downloads outside the CurseForge app; their files
//! come back with `downloadUrl: null`. Nexforge honours that: such files are
//! listed (so updates still show) but never downloaded, and no download URL
//! is ever built by hand.

use super::{AddonInfo, CatalogueEntry, Channel, Release, Source, SourceId};
use crate::error::{Error, Result};
use crate::flavour::Flavour;
use crate::net::{Fetch, Get, Post};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use regex::Regex;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

pub const API: &str = "https://api.curseforge.com/v1";
/// World of Warcraft's game id.
pub const GAME_ID: u32 = 1;

// ---- flavour labels: the one place CurseForge's names map to `Flavour` ----

/// `gameVersionTypeId` for a flavour. Forever's 88568 comes from WowUp's
/// provider and is unconfirmed until a real CurseForge response is recorded.
pub fn game_version_type(f: Flavour) -> u64 {
    match f {
        Flavour::Forever => 88568,
        Flavour::ClassicEra => 67408,
        Flavour::Anniversary => 73246,
        Flavour::Retail => 517,
    }
}

pub fn map_type_id(id: u64) -> Option<Flavour> {
    Flavour::ALL
        .into_iter()
        .find(|f| game_version_type(*f) == id)
}

/// Game-version strings (`1.60.1`, `1.15.7`, `2.5.4`, `12.0.0`), used only
/// when a file has no `gameVersionTypeId`.
pub fn map_game_version(v: &str) -> Option<Flavour> {
    let mut it = v.trim().split('.').map(|p| p.parse::<u32>().ok());
    let major = it.next().flatten()?;
    let minor = it.next().flatten().unwrap_or(0);
    match (major, minor) {
        (1, 60..=69) => Some(Flavour::Forever),
        (1, 13..=15) => Some(Flavour::ClassicEra),
        (2, 5) => Some(Flavour::Anniversary),
        (12, _) => Some(Flavour::Retail),
        _ => None,
    }
}

/// `releaseType`: 1 release, 2 beta, 3 alpha.
fn channel_of(release_type: u8) -> Channel {
    match release_type {
        2 => Channel::Beta,
        3 => Channel::Alpha,
        _ => Channel::Stable,
    }
}

// ---- wire types (docs.curseforge.com, checked 2026-11-04) ----

#[derive(Debug, Deserialize)]
struct Data<T> {
    data: T,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfMod {
    id: u64,
    name: String,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    download_count: Option<f64>,
    #[serde(default)]
    authors: Vec<CfAuthor>,
    #[serde(default)]
    logo: Option<CfAsset>,
    #[serde(default)]
    screenshots: Vec<CfAsset>,
    #[serde(default)]
    links: Option<CfLinks>,
    #[serde(default)]
    latest_files: Vec<CfFile>,
    #[serde(default)]
    latest_files_indexes: Vec<CfFileIndex>,
    #[serde(default)]
    date_modified: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
struct CfAuthor {
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfAsset {
    #[serde(default)]
    thumbnail_url: Option<String>,
    #[serde(default)]
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfLinks {
    #[serde(default)]
    website_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfFile {
    id: u64,
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    file_name: String,
    #[serde(default = "one")]
    release_type: u8,
    #[serde(default)]
    file_date: Option<DateTime<Utc>>,
    #[serde(default)]
    file_length: Option<u64>,
    #[serde(default)]
    download_url: Option<String>,
    #[serde(default)]
    hashes: Vec<CfHash>,
    #[serde(default)]
    sortable_game_versions: Vec<CfSortableVersion>,
    #[serde(default)]
    game_versions: Vec<String>,
    #[serde(default)]
    is_available: Option<bool>,
}

fn one() -> u8 {
    1
}

#[derive(Debug, Clone, Deserialize)]
struct CfHash {
    value: String,
    /// 1 = SHA-1, 2 = MD5.
    algo: u8,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfSortableVersion {
    #[serde(default)]
    game_version_type_id: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfFileIndex {
    #[serde(default)]
    game_version_type_id: Option<u64>,
    file_id: u64,
    #[serde(default)]
    filename: String,
    #[serde(default = "one")]
    release_type: u8,
}

/// CurseForge changelogs are HTML: list items become "- " lines, block
/// tags become line breaks, other tags are dropped, common entities decoded.
pub fn html_to_text(html: &str) -> String {
    let mut out = String::new();
    let mut rest = html;
    while let Some(i) = rest.find('<') {
        out.push_str(&rest[..i]);
        let Some(j) = rest[i..].find('>') else {
            rest = &rest[i..];
            break;
        };
        let tag = rest[i + 1..i + j].trim().to_ascii_lowercase();
        let name = tag
            .trim_start_matches('/')
            .split([' ', '/'])
            .next()
            .unwrap_or("");
        match name {
            "li" if !tag.starts_with('/') => out.push_str("\n- "),
            "br" | "p" | "div" | "ul" | "ol" | "h1" | "h2" | "h3" | "h4" => out.push('\n'),
            _ => {}
        }
        rest = &rest[i + j + 1..];
    }
    out.push_str(rest);
    let decoded = out
        .replace("&nbsp;", " ")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&");
    decoded
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && *l != "-")
        .collect::<Vec<_>>()
        .join("\n")
}

fn bad(message: impl ToString) -> Error {
    Error::BadResponse {
        host: "api.curseforge.com".into(),
        message: message.to_string(),
    }
}

fn check_id(id: &str) -> Result<u64> {
    id.trim().parse().map_err(|_| {
        Error::Other(format!(
            "\"{id}\" is not a CurseForge project id (it should be a number)."
        ))
    })
}

/// Version shown for a file: its display name, else the file name.
fn version_of(display_name: &str, file_name: &str) -> String {
    let d = display_name.trim();
    if !d.is_empty() {
        return d.to_string();
    }
    file_name
        .trim()
        .trim_end_matches(".zip")
        .trim_end_matches(".ZIP")
        .to_string()
}

fn file_flavours(f: &CfFile) -> Vec<Flavour> {
    let mut out: Vec<Flavour> = f
        .sortable_game_versions
        .iter()
        .filter_map(|s| s.game_version_type_id.and_then(map_type_id))
        .collect();
    if out.is_empty() {
        out = f
            .game_versions
            .iter()
            .filter_map(|v| map_game_version(v))
            .collect();
    }
    out.sort();
    out.dedup();
    out
}

fn to_release(f: &CfFile) -> Release {
    Release {
        version: version_of(&f.display_name, &f.file_name),
        // Null when the author opted out of third-party downloads.
        download_url: f.download_url.clone().unwrap_or_default(),
        file_name: Some(f.file_name.clone()).filter(|n| !n.is_empty()),
        published_at: f.file_date,
        size: f.file_length.filter(|n| *n > 0),
        channel: channel_of(f.release_type),
        flavours: file_flavours(f),
        changelog: None,
        sha256: None,
        md5: f
            .hashes
            .iter()
            .find(|h| h.algo == 2)
            .map(|h| h.value.to_ascii_lowercase()),
        file_id: Some(f.id.to_string()),
    }
}

pub struct CurseForge {
    fetch: Arc<dyn Fetch>,
    key: String,
}

impl CurseForge {
    pub fn new(fetch: Arc<dyn Fetch>, key: String) -> Self {
        CurseForge { fetch, key }
    }

    fn get(&self, path: &str, reason: String, fresh: Duration) -> Get {
        Get::new(format!("{API}{path}"), reason)
            .header("Accept", "application/json")
            .header("x-api-key", self.key.clone())
            .fresh_for(fresh)
    }

    /// A refused key is a setup problem the player can fix; say so.
    fn explain(e: Error) -> Error {
        match e {
            Error::Http {
                status: 401 | 403, ..
            } => Error::Other(
                "CurseForge refused the API key. Check it, or save a new one with `nexforge key set curseforge`.".into(),
            ),
            other => other,
        }
    }

    async fn get_json<T: serde::de::DeserializeOwned>(&self, req: Get) -> Result<T> {
        let text = self.fetch.get_text(req).await.map_err(Self::explain)?;
        serde_json::from_str::<Data<T>>(&text)
            .map(|d| d.data)
            .map_err(bad)
    }

    async fn get_mod(&self, id: u64) -> Result<CfMod> {
        self.get_json(self.get(
            &format!("/mods/{id}"),
            format!("details for CurseForge project {id}"),
            Duration::from_secs(600),
        ))
        .await
    }
}

#[async_trait]
impl Source for CurseForge {
    fn id(&self) -> SourceId {
        SourceId::Curseforge
    }
    fn needs_key(&self) -> bool {
        true
    }

    async fn search(&self, q: &str, flavour: Flavour) -> Result<Vec<CatalogueEntry>> {
        let path = format!(
            "/mods/search?gameId={GAME_ID}&searchFilter={}&gameVersionTypeId={}&sortField=2&sortOrder=desc&pageSize=50",
            url::form_urlencoded::byte_serialize(q.trim().as_bytes()).collect::<String>(),
            game_version_type(flavour)
        );
        let mods: Vec<CfMod> = self
            .get_json(self.get(
                &path,
                format!("CurseForge search \"{q}\""),
                Duration::from_secs(600),
            ))
            .await?;
        Ok(mods
            .into_iter()
            .map(|m| {
                let mut flavours: Vec<Flavour> = m
                    .latest_files_indexes
                    .iter()
                    .filter_map(|i| i.game_version_type_id.and_then(map_type_id))
                    .collect();
                flavours.sort();
                flavours.dedup();
                CatalogueEntry {
                    source: SourceId::Curseforge,
                    source_id: m.id.to_string(),
                    name: m.name,
                    authors: m.authors.into_iter().map(|a| a.name).collect(),
                    summary: m.summary,
                    flavours,
                    downloads: m.download_count.map(|d| d as u64),
                    downloads_monthly: None,
                    updated_at: m.date_modified,
                    icon_url: m.logo.and_then(|l| l.thumbnail_url.or(l.url)),
                }
            })
            .collect())
    }

    async fn resolve(&self, source_id: &str) -> Result<AddonInfo> {
        let m = self.get_mod(check_id(source_id)?).await?;
        // (full, thumbnail) per screenshot; the full image stands in for a missing thumbnail.
        let (screenshots, screenshot_thumbs) = m
            .screenshots
            .into_iter()
            .filter_map(|s| s.url.map(|u| (u.clone(), s.thumbnail_url.unwrap_or(u))))
            .unzip();
        Ok(AddonInfo {
            name: m.name,
            authors: m.authors.into_iter().map(|a| a.name).collect(),
            icon_url: m.logo.and_then(|l| l.thumbnail_url.or(l.url)),
            screenshots,
            screenshot_thumbs,
            summary: m.summary,
            description: None,
            url: m.links.and_then(|l| l.website_url),
        })
    }

    async fn all_releases(&self, source_id: &str) -> Result<Vec<Release>> {
        let id = check_id(source_id)?;
        let mut files: Vec<CfFile> = self
            .get_json(self.get(
                &format!("/mods/{id}/files?pageSize=50"),
                format!("files for CurseForge project {id}"),
                Duration::from_secs(600),
            ))
            .await?;
        files.retain(|f| f.is_available != Some(false));
        files.sort_by_key(|f| std::cmp::Reverse((f.file_date, f.id)));
        Ok(files.iter().map(to_release).collect())
    }

    /// `GET /mods/{id}/files/{fileId}/changelog` (HTML), as plain lines.
    async fn changelog(&self, source_id: &str, release: &Release) -> Result<Option<String>> {
        let id = check_id(source_id)?;
        let Some(file) = release.file_id.as_deref() else {
            return Ok(release.changelog.clone());
        };
        let html: String = self
            .get_json(self.get(
                &format!("/mods/{id}/files/{file}/changelog"),
                format!("changelog for CurseForge project {id}"),
                Duration::from_secs(24 * 3600),
            ))
            .await?;
        Ok(Some(html_to_text(&html)).filter(|t| !t.trim().is_empty()))
    }

    /// `curseforge.com/wow/addons/<slug>` (looked up by slug) or
    /// `curseforge.com/projects/<id>`.
    async fn resolve_url(&self, url: &str) -> Result<Option<String>> {
        static RE: OnceLock<Regex> = OnceLock::new();
        let re = RE.get_or_init(|| {
            Regex::new(r"(?i)curseforge\.com/(?:wow/addons/([a-z0-9_-]+)|projects/(\d+))").unwrap()
        });
        let Some(c) = re.captures(url) else {
            return Ok(None);
        };
        if let Some(id) = c.get(2) {
            return Ok(Some(id.as_str().to_string()));
        }
        let slug = c[1].to_ascii_lowercase();
        let mods: Vec<CfMod> = self
            .get_json(self.get(
                &format!("/mods/search?gameId={GAME_ID}&slug={slug}"),
                format!("CurseForge project for {slug}"),
                Duration::from_secs(24 * 3600),
            ))
            .await?;
        Ok(mods.first().map(|m| m.id.to_string()))
    }

    /// One `POST /mods` answers every id. Uses each project's latest files,
    /// falling back to `latestFilesIndexes` (no link: fetch
    /// [`Source::releases`] before downloading).
    async fn batch_latest(
        &self,
        ids: &[String],
        flavour: Flavour,
        channel: Channel,
    ) -> Result<HashMap<String, Release>> {
        let nums: Vec<u64> = ids.iter().filter_map(|i| i.trim().parse().ok()).collect();
        if nums.is_empty() {
            return Ok(HashMap::new());
        }
        let body = serde_json::json!({ "modIds": nums, "filterPcOnly": true }).to_string();
        let text = self
            .fetch
            .post_json(
                Post::json(format!("{API}/mods"), "CurseForge update check", body)
                    .header("Accept", "application/json")
                    .header("x-api-key", self.key.clone()),
            )
            .await
            .map_err(Self::explain)?;
        let mods: Vec<CfMod> = serde_json::from_str::<Data<Vec<CfMod>>>(&text)
            .map_err(bad)?
            .data;
        let want = game_version_type(flavour);
        let mut out = HashMap::new();
        for m in mods {
            let from_files = m
                .latest_files
                .iter()
                .filter(|f| f.is_available != Some(false))
                .filter(|f| channel.accepts(channel_of(f.release_type)))
                .filter(|f| file_flavours(f).contains(&flavour))
                .max_by_key(|f| (f.file_date, f.id))
                .map(to_release);
            let release = from_files.or_else(|| {
                m.latest_files_indexes
                    .iter()
                    .filter(|i| i.game_version_type_id == Some(want))
                    .filter(|i| channel.accepts(channel_of(i.release_type)))
                    .max_by_key(|i| i.file_id)
                    .map(|i| Release {
                        version: version_of("", &i.filename),
                        download_url: String::new(),
                        file_name: Some(i.filename.clone()),
                        published_at: None,
                        size: None,
                        channel: channel_of(i.release_type),
                        flavours: vec![flavour],
                        changelog: None,
                        sha256: None,
                        md5: None,
                        file_id: None,
                    })
            });
            if let Some(r) = release {
                out.insert(m.id.to_string(), r);
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels() {
        assert_eq!(map_type_id(88568), Some(Flavour::Forever));
        assert_eq!(map_type_id(67408), Some(Flavour::ClassicEra));
        assert_eq!(map_type_id(73246), Some(Flavour::Anniversary));
        assert_eq!(map_type_id(77522), None);
        for f in Flavour::ALL {
            assert_eq!(map_type_id(game_version_type(f)), Some(f));
        }
        assert_eq!(map_game_version("1.60.1"), Some(Flavour::Forever));
        assert_eq!(map_game_version("4.4.2"), None);
        assert_eq!(channel_of(2), Channel::Beta);
        assert_eq!(
            version_of("", "DungeonJournal-v1.4.2.zip"),
            "DungeonJournal-v1.4.2"
        );
        assert_eq!(
            html_to_text("<p>v1.4.2</p><ul><li>Fixed <b>quest</b> pins</li><li>Added &quot;Forever&quot; raids &amp; more</li></ul>"),
            "v1.4.2\n- Fixed quest pins\n- Added \"Forever\" raids & more"
        );
    }
}
