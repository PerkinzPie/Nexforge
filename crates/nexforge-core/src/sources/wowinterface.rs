//! WoWInterface (MMOUI API v3). No key needed.

use super::{AddonInfo, CatalogueEntry, Channel, Release, Source, SourceId};
use crate::error::{Error, Result};
use crate::flavour::Flavour;
use crate::net::{Fetch, Get};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

pub const API: &str = "https://api.mmoui.com/v3/game/WOW";

// ---- flavour labels: the one place WoWInterface's names map to `Flavour` ----

/// Map a `UICompatibility[].version` to a flavour. 1.60 is Forever; 1.13–1.15
/// are Classic Era builds; 2.5 is TBC/Anniversary; 12.x is Retail.
pub fn map_compat(version: &str) -> Option<Flavour> {
    let mut it = version.trim().split('.').map(|p| p.parse::<u32>().ok());
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

// ---- lenient JSON access (MMOUI mixes strings and numbers) ----

fn s(v: &Value, k: &str) -> Option<String> {
    match v.get(k)? {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn n(v: &Value, k: &str) -> Option<u64> {
    match v.get(k)? {
        Value::Number(n) => n.as_u64(),
        Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

fn date_ms(v: &Value, k: &str) -> Option<DateTime<Utc>> {
    DateTime::from_timestamp_millis(n(v, k)? as i64)
}

/// Image URLs in a filelist entry (`UIIMG_Thumbs` small, `UIIMGs` full size).
fn images(v: &Value, k: &str) -> Vec<String> {
    v.get(k)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .filter(|u| !u.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// `(flavours, said_anything)` from a filelist entry.
fn compat(v: &Value) -> (Vec<Flavour>, bool) {
    let list = v
        .get("UICompatibility")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut out: Vec<Flavour> = list
        .iter()
        .filter_map(|c| c.get("version").and_then(Value::as_str))
        .filter_map(map_compat)
        .collect();
    out.sort();
    out.dedup();
    (out, !list.is_empty())
}

/// A filelist entry as a catalogue entry.
fn entry(e: &Value) -> Option<CatalogueEntry> {
    Some(CatalogueEntry {
        source: SourceId::Wowinterface,
        source_id: s(e, "UID")?,
        name: s(e, "UIName")?,
        authors: s(e, "UIAuthorName").into_iter().collect(),
        summary: None,
        flavours: compat(e).0,
        downloads: n(e, "UIDownloadTotal"),
        downloads_monthly: n(e, "UIDownloadMonthly"),
        updated_at: date_ms(e, "UIDate"),
        icon_url: images(e, "UIIMG_Thumbs").into_iter().next(),
    })
}

fn bad(message: impl ToString) -> Error {
    Error::BadResponse {
        host: "api.mmoui.com".into(),
        message: message.to_string(),
    }
}

fn check_id(id: &str) -> Result<&str> {
    if !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()) {
        Ok(id)
    } else {
        Err(Error::Other(format!(
            "\"{id}\" is not a WoWInterface addon id (it should be a number)."
        )))
    }
}

pub struct WowInterface {
    fetch: Arc<dyn Fetch>,
}

impl WowInterface {
    pub fn new(fetch: Arc<dyn Fetch>) -> Self {
        WowInterface { fetch }
    }

    /// Every addon on the site (~2 MB, cached for a day).
    pub async fn filelist(&self) -> Result<Vec<Value>> {
        let text = self
            .fetch
            .get_text(
                Get::new(format!("{API}/filelist.json"), "WoWInterface addon list")
                    .fresh_for(Duration::from_secs(24 * 3600)),
            )
            .await?;
        serde_json::from_str(&text).map_err(bad)
    }

    async fn details(&self, id: &str) -> Result<Value> {
        let id = check_id(id)?;
        let text = self
            .fetch
            .get_text(
                Get::new(
                    format!("{API}/filedetails/{id}.json"),
                    format!("details for WoWInterface addon {id}"),
                )
                .fresh_for(Duration::from_secs(600)),
            )
            .await?;
        let v: Value = serde_json::from_str(&text).map_err(bad)?;
        match v {
            Value::Array(mut a) if !a.is_empty() => Ok(a.swap_remove(0)),
            Value::Object(_) => Ok(v),
            _ => Err(Error::NotManaged(format!(
                "WoWInterface has no addon with id {id}."
            ))),
        }
    }
}

#[async_trait]
impl Source for WowInterface {
    fn id(&self) -> SourceId {
        SourceId::Wowinterface
    }
    fn needs_key(&self) -> bool {
        false
    }

    async fn search(&self, q: &str, _flavour: Flavour) -> Result<Vec<CatalogueEntry>> {
        let q = q.to_lowercase();
        let mut hits: Vec<CatalogueEntry> = self
            .filelist()
            .await?
            .iter()
            .filter(|e| {
                s(e, "UIName")
                    .map(|n| n.to_lowercase().contains(&q))
                    .unwrap_or(false)
            })
            // Entries for other flavours stay listed; the UI offers
            // "Install anyway" for them.
            .filter_map(entry)
            .collect();
        hits.sort_by_key(|a| std::cmp::Reverse(a.downloads));
        hits.truncate(50);
        Ok(hits)
    }

    /// The whole filelist (about 8,000 addons, one cached request).
    async fn catalogue(&self) -> Result<Vec<CatalogueEntry>> {
        Ok(self.filelist().await?.iter().filter_map(entry).collect())
    }

    async fn resolve(&self, source_id: &str) -> Result<AddonInfo> {
        let d = self.details(source_id).await?;
        // Images are only in the (cached) filelist; the first thumbnail is the icon.
        let entry = self.filelist().await.ok().and_then(|list| {
            list.into_iter()
                .find(|e| s(e, "UID").as_deref() == Some(source_id))
        });
        let (icon_url, screenshots) = match &entry {
            Some(e) => (
                images(e, "UIIMG_Thumbs").into_iter().next(),
                images(e, "UIIMGs"),
            ),
            None => (None, vec![]),
        };
        Ok(AddonInfo {
            name: s(&d, "UIName").unwrap_or_else(|| source_id.to_string()),
            authors: s(&d, "UIAuthorName").into_iter().collect(),
            icon_url,
            screenshots,
            summary: s(&d, "UIDescription").map(|t| t.chars().take(300).collect()),
            url: Some(format!(
                "https://www.wowinterface.com/downloads/info{source_id}"
            )),
        })
    }

    async fn all_releases(&self, source_id: &str) -> Result<Vec<Release>> {
        let d = self.details(source_id).await?;
        // filedetails has no compatibility list; the (cached) filelist does.
        let flavours = match self.filelist().await {
            Ok(list) => list
                .iter()
                .find(|e| s(e, "UID").as_deref() == Some(source_id))
                .map(|e| compat(e).0)
                .unwrap_or_default(),
            Err(_) => vec![],
        };
        let Some(url) = s(&d, "UIDownload") else {
            return Ok(vec![]);
        };
        Ok(vec![Release {
            version: s(&d, "UIVersion").unwrap_or_default(),
            download_url: url,
            file_name: s(&d, "UIFileName"),
            published_at: date_ms(&d, "UIDate"),
            size: None,
            channel: Channel::Stable,
            flavours,
            changelog: s(&d, "UIChangeLog"),
            sha256: None,
            md5: s(&d, "UIMD5").map(|m| m.to_ascii_lowercase()),
        }])
    }

    async fn resolve_url(&self, url: &str) -> Result<Option<String>> {
        static RE: OnceLock<Regex> = OnceLock::new();
        let re = RE.get_or_init(|| {
            Regex::new(r"(?i)wowinterface\.com/downloads/(?:info|download|landing\.php\?fileid=|fileinfo\.php\?id=)(\d+)")
                .unwrap()
        });
        Ok(re.captures(url).map(|c| c[1].to_string()))
    }

    /// One filelist call answers every id. `download_url` is left empty;
    /// fetch [`Source::releases`] before downloading.
    async fn batch_latest(
        &self,
        ids: &[String],
        flavour: Flavour,
        _channel: Channel,
    ) -> Result<HashMap<String, Release>> {
        let list = self.filelist().await?;
        let by_id: HashMap<String, &Value> = list
            .iter()
            .filter_map(|e| Some((s(e, "UID")?, e)))
            .collect();
        let mut out = HashMap::new();
        for id in ids {
            let Some(e) = by_id.get(id) else { continue };
            let (flavours, _) = compat(e);
            if !flavours.is_empty() && !flavours.contains(&flavour) {
                continue;
            }
            out.insert(
                id.clone(),
                Release {
                    version: s(e, "UIVersion").unwrap_or_default(),
                    download_url: String::new(),
                    file_name: None,
                    published_at: date_ms(e, "UIDate"),
                    size: None,
                    channel: Channel::Stable,
                    flavours,
                    changelog: None,
                    sha256: None,
                    md5: None,
                },
            );
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compat_labels() {
        assert_eq!(map_compat("1.60.1"), Some(Flavour::Forever));
        assert_eq!(map_compat("1.15.7"), Some(Flavour::ClassicEra));
        assert_eq!(map_compat("2.5.4"), Some(Flavour::Anniversary));
        assert_eq!(map_compat("12.1.0"), Some(Flavour::Retail));
        assert_eq!(map_compat("4.4.2"), None);
        assert_eq!(map_compat("1.12.1"), None);
    }
}
