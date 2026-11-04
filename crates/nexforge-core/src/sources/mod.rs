//! Addon hosts. One [`Source`] trait; each host is one file that also owns the
//! mapping from that host's flavour labels to [`Flavour`].

pub mod curseforge;
pub mod github;
pub mod wowinterface;

use crate::error::Result;
use crate::flavour::Flavour;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceId {
    Github,
    Wowinterface,
    Wago,
    Curseforge,
}

impl SourceId {
    /// Preference when several hosts can serve one addon: keys and
    /// author opt-outs make CurseForge the least reliable.
    pub const PREFERENCE: [SourceId; 4] = [
        SourceId::Wago,
        SourceId::Github,
        SourceId::Wowinterface,
        SourceId::Curseforge,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            SourceId::Github => "github",
            SourceId::Wowinterface => "wowinterface",
            SourceId::Wago => "wago",
            SourceId::Curseforge => "curseforge",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SourceId::Github => "GitHub",
            SourceId::Wowinterface => "WoWInterface",
            SourceId::Wago => "Wago",
            SourceId::Curseforge => "CurseForge",
        }
    }
}

impl fmt::Display for SourceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for SourceId {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "github" | "gh" => Ok(SourceId::Github),
            "wowinterface" | "wowi" => Ok(SourceId::Wowinterface),
            "wago" => Ok(SourceId::Wago),
            "curseforge" | "curse" | "cf" => Ok(SourceId::Curseforge),
            _ => Err(format!(
                "\"{s}\" is not a source. Use github, wowinterface, wago or curseforge."
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    #[default]
    Stable,
    Beta,
    Alpha,
}

impl Channel {
    /// Would a user on `self` take a release on `other`?
    pub fn accepts(self, other: Channel) -> bool {
        other <= self
    }
}

impl FromStr for Channel {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "stable" | "release" => Ok(Channel::Stable),
            "beta" => Ok(Channel::Beta),
            "alpha" => Ok(Channel::Alpha),
            _ => Err(format!(
                "\"{s}\" is not a channel. Use stable, beta or alpha."
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Release {
    pub version: String,
    /// May be empty in [`Source::batch_latest`] results from hosts whose bulk
    /// listing has no link; call [`Source::releases`] before downloading.
    pub download_url: String,
    pub file_name: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
    pub size: Option<u64>,
    pub channel: Channel,
    /// Empty means the host did not say ("flavour unknown").
    pub flavours: Vec<Flavour>,
    pub changelog: Option<String>,
    pub sha256: Option<String>,
    /// WoWInterface publishes MD5 rather than SHA-256.
    pub md5: Option<String>,
    /// The host's id for this file (CurseForge), for fetching its changelog.
    #[serde(default)]
    pub file_id: Option<String>,
}

impl Release {
    pub fn supports(&self, f: Flavour) -> bool {
        self.flavours.contains(&f)
    }
    pub fn flavour_unknown(&self) -> bool {
        self.flavours.is_empty()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddonInfo {
    pub name: String,
    pub authors: Vec<String>,
    /// Fetch with [`crate::net::Fetch::get_image`].
    pub icon_url: Option<String>,
    /// Full-size screenshots, if the host lists any.
    #[serde(default)]
    pub screenshots: Vec<String>,
    /// Small versions of `screenshots`, same order; empty if the host has none.
    #[serde(default)]
    pub screenshot_thumbs: Vec<String>,
    pub summary: Option<String>,
    /// The host's long description (may hold markup), for the details view.
    #[serde(default)]
    pub description: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogueEntry {
    pub source: SourceId,
    pub source_id: String,
    pub name: String,
    pub authors: Vec<String>,
    pub summary: Option<String>,
    pub flavours: Vec<Flavour>,
    pub downloads: Option<u64>,
    /// Downloads in the last month, where the host says (WoWInterface).
    #[serde(default)]
    pub downloads_monthly: Option<u64>,
    pub updated_at: Option<DateTime<Utc>>,
    pub icon_url: Option<String>,
}

#[async_trait]
pub trait Source: Send + Sync {
    fn id(&self) -> SourceId;
    fn needs_key(&self) -> bool;

    async fn search(&self, q: &str, flavour: Flavour) -> Result<Vec<CatalogueEntry>>;
    async fn resolve(&self, source_id: &str) -> Result<AddonInfo>;

    /// Every release the host lists, newest first, any flavour.
    async fn all_releases(&self, source_id: &str) -> Result<Vec<Release>>;

    /// Newest first, filtered to releases that list `flavour` or do not say
    /// (flavour-unknown). Empty means "No <flavour> build listed".
    async fn releases(&self, source_id: &str, flavour: Flavour) -> Result<Vec<Release>> {
        Ok(self
            .all_releases(source_id)
            .await?
            .into_iter()
            .filter(|r| r.supports(flavour) || r.flavour_unknown())
            .collect())
    }

    /// Every addon the host can list in one go, for the local catalogue.
    /// Hosts without a bulk listing return nothing and are searched live.
    async fn catalogue(&self) -> Result<Vec<CatalogueEntry>> {
        Ok(vec![])
    }

    /// Whether to search the host live even when the catalogue holds some of
    /// its addons (its catalogue is a sample, not the whole host).
    fn live_search(&self) -> bool {
        false
    }

    /// Release notes for `release`. Most hosts include them with the
    /// release; CurseForge has a separate endpoint.
    async fn changelog(&self, _source_id: &str, release: &Release) -> Result<Option<String>> {
        Ok(release.changelog.clone())
    }

    /// "Install from URL": map a web address on this host to a source id.
    async fn resolve_url(&self, url: &str) -> Result<Option<String>>;

    /// Latest release per id for update checks. Sources with a bulk endpoint
    /// override this; the default asks per id.
    async fn batch_latest(
        &self,
        ids: &[String],
        flavour: Flavour,
        channel: Channel,
    ) -> Result<HashMap<String, Release>> {
        let mut out = HashMap::new();
        for id in ids {
            let rels = self.releases(id, flavour).await?;
            if let Some(r) = pick_release(&rels, flavour, channel) {
                out.insert(id.clone(), r.clone());
            }
        }
        Ok(out)
    }
}

/// Newest release acceptable on `channel`, preferring one that names
/// `flavour` over a flavour-unknown one of the same version.
pub fn pick_release(releases: &[Release], flavour: Flavour, channel: Channel) -> Option<&Release> {
    let ok: Vec<&Release> = releases
        .iter()
        .filter(|r| channel.accepts(r.channel))
        .collect();
    let first = ok.first()?;
    ok.iter()
        .find(|r| r.version == first.version && r.supports(flavour))
        .copied()
        .or(Some(*first))
}

/// Parse `"source:id"` (e.g. `github:Questie/Questie`, `wowi:22095`).
pub fn parse_spec(spec: &str) -> Option<(SourceId, String)> {
    let (s, id) = spec.split_once(':')?;
    if s.contains('/') {
        return None;
    }
    let source = s.parse().ok()?;
    let id = id.trim();
    (!id.is_empty()).then(|| (source, id.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rel(v: &str, ch: Channel, fl: &[Flavour]) -> Release {
        Release {
            version: v.into(),
            download_url: format!("https://github.com/{v}"),
            file_name: None,
            published_at: None,
            size: None,
            channel: ch,
            flavours: fl.to_vec(),
            changelog: None,
            sha256: None,
            md5: None,
            file_id: None,
        }
    }

    #[test]
    fn picks_newest_on_channel_and_flavour() {
        let rels = vec![
            rel("3", Channel::Beta, &[Flavour::Forever]),
            rel("2", Channel::Stable, &[]),
            rel("2", Channel::Stable, &[Flavour::Forever]),
            rel("1", Channel::Stable, &[Flavour::Forever]),
        ];
        let r = pick_release(&rels, Flavour::Forever, Channel::Stable).unwrap();
        assert_eq!((r.version.as_str(), r.flavours.len()), ("2", 1));
        let r = pick_release(&rels, Flavour::Forever, Channel::Alpha).unwrap();
        assert_eq!(r.version, "3");
    }

    #[test]
    fn specs() {
        assert_eq!(
            parse_spec("github:Questie/Questie"),
            Some((SourceId::Github, "Questie/Questie".into()))
        );
        assert_eq!(
            parse_spec("wowi:22095"),
            Some((SourceId::Wowinterface, "22095".into()))
        );
        assert_eq!(parse_spec("https://github.com/a/b"), None);
    }
}
