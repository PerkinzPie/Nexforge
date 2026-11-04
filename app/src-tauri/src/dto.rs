//! What the webview receives. Plain data, camelCase, nothing it could use to
//! reach the network.

use chrono::{DateTime, Utc};
use nexforge_core::manager::Status;
use nexforge_core::sources::CatalogueEntry;
use nexforge_core::state::AddonEntry;
use nexforge_core::{Flavour, Installation, Release, SourceId};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallationDto {
    pub id: String,
    pub flavour: String,
    pub label: String,
    pub short: String,
    pub path: String,
    pub detected: bool,
}

impl From<&Installation> for InstallationDto {
    fn from(i: &Installation) -> Self {
        InstallationDto {
            id: i.id.clone(),
            flavour: i.flavour.as_str().into(),
            label: i.flavour.label().into(),
            short: i.flavour.short_label().into(),
            path: i.path.display().to_string(),
            detected: i.addons_dir().is_dir(),
        }
    }
}

/// Web page for an addon on its host.
pub fn website(source: Option<SourceId>, id: &str) -> Option<String> {
    let id = id.trim();
    if id.is_empty() {
        return None;
    }
    Some(match source? {
        SourceId::Github => {
            let repo = id.split('#').next().unwrap_or(id);
            format!("https://github.com/{repo}")
        }
        SourceId::Wowinterface => format!("https://www.wowinterface.com/downloads/info{id}"),
        SourceId::Curseforge => format!("https://www.curseforge.com/projects/{id}"),
        SourceId::Wago => format!("https://addons.wago.io/addons/{id}"),
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddonDto {
    pub id: String,
    pub name: String,
    pub author: Option<String>,
    pub folders: Vec<String>,
    pub source: Option<String>,
    pub source_label: String,
    pub source_id: String,
    pub version: String,
    pub pinned: bool,
    pub skipped_version: String,
    pub flavour_mismatch: bool,
    pub managed: bool,
    pub website: Option<String>,
    /// Icon to show (GitHub owner avatar); None means a monogram tile.
    pub icon_url: Option<String>,
    /// IDs from the TOCs, for "Identify source" suggestions.
    pub known_ids: Vec<(String, String)>,
    /// Kept versions it can roll back to, newest first (not the installed one).
    pub kept: Vec<String>,
}

impl From<&AddonEntry> for AddonDto {
    fn from(a: &AddonEntry) -> Self {
        let known_ids = SourceId::PREFERENCE
            .iter()
            .filter_map(|s| Some((s.as_str().to_string(), a.ids.get(*s)?.to_string())))
            .collect();
        AddonDto {
            id: a.id.clone(),
            name: a.name.clone(),
            author: a.author.clone(),
            folders: a.folders.clone(),
            source: a.source.map(|s| s.as_str().into()),
            source_label: a.source.map(|s| s.label()).unwrap_or("Unknown").into(),
            source_id: a.source_id.clone(),
            version: a.installed_version.clone(),
            pinned: a.pinned,
            skipped_version: a.skipped_version.clone(),
            flavour_mismatch: a.flavour_mismatch,
            managed: a.is_managed(),
            website: website(a.source, &a.source_id),
            icon_url: match a.source {
                Some(SourceId::Github) => nexforge_core::sources::github::avatar_url(&a.source_id),
                _ => None,
            },
            known_ids,
            kept: vec![],
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LastRun {
    pub at: DateTime<Utc>,
    pub count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub installations: Vec<InstallationDto>,
    pub current: Option<String>,
    pub flavour: String,
    pub flavour_label: String,
    pub flavour_short: String,
    pub addons: Vec<AddonDto>,
    pub state_exists: bool,
    pub addons_dir_exists: bool,
    /// True when this snapshot just adopted the AddOns folder (first open).
    pub adopted_now: bool,
    /// No config existed before this launch: show the welcome screen.
    pub first_run: bool,
    pub last_run: Option<LastRun>,
    pub settings: SettingsDto,
    /// Sources usable right now (enabled, and keyed where needed).
    pub sources_on: Vec<String>,
}

impl Snapshot {
    pub fn empty(installations: Vec<InstallationDto>, settings: SettingsDto) -> Snapshot {
        Snapshot {
            installations,
            current: None,
            flavour: Flavour::Forever.as_str().into(),
            flavour_label: Flavour::Forever.label().into(),
            flavour_short: Flavour::Forever.short_label().into(),
            addons: vec![],
            state_exists: false,
            addons_dir_exists: false,
            adopted_now: false,
            first_run: false,
            last_run: None,
            settings,
            sources_on: vec![],
        }
    }
}

/// One line of markdown or BBCode as plain text: `[text](url)` keeps its
/// text, tags like `[b]` and HTML line breaks are dropped.
fn clean_markup(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find('[') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        match (after.find("]("), after.find(']')) {
            (Some(j), _) if after[j + 2..].contains(')') => {
                out.push_str(&after[..j]);
                let close = after[j + 2..].find(')').unwrap_or(0);
                rest = &after[j + 2 + close + 1..];
            }
            (_, Some(j)) => {
                // BBCode tag like [b] or [/url]: drop it.
                rest = &after[j + 1..];
            }
            _ => {
                out.push('[');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out.replace("**", "")
        .replace('`', "")
        .replace("<br />", " ")
        .replace("<br>", " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// A long description as plain paragraphs (headings and blank lines dropped).
pub fn paragraphs(text: Option<&str>, max: usize) -> Vec<String> {
    let Some(text) = text else {
        return vec![];
    };
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| {
            let l = l
                .strip_prefix("- ")
                .or_else(|| l.strip_prefix("* "))
                .or_else(|| l.strip_prefix("[*]"))
                .unwrap_or(l);
            clean_markup(l)
        })
        .filter(|l| !l.is_empty())
        .take(max)
        .collect()
}

/// First few release-note bullets, as plain text.
pub fn note_lines(changelog: Option<&str>, max: usize) -> Vec<String> {
    let Some(text) = changelog else {
        return vec![];
    };
    let clean = clean_markup;
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let bullets: Vec<String> = lines
        .iter()
        .filter_map(|l| {
            l.strip_prefix("- ")
                .or_else(|| l.strip_prefix("* "))
                .or_else(|| l.strip_prefix("• "))
                .or_else(|| l.strip_prefix("[*]"))
        })
        .map(clean)
        .filter(|l| !l.is_empty())
        .take(max)
        .collect();
    if !bullets.is_empty() {
        return bullets;
    }
    lines
        .iter()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| clean(l))
        .filter(|l| !l.is_empty())
        .take(max)
        .collect()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseDto {
    pub version: String,
    pub published_at: Option<DateTime<Utc>>,
    pub size: Option<u64>,
    pub channel: String,
    pub flavour_unknown: bool,
    pub notes: Vec<String>,
    pub has_more_notes: bool,
}

impl From<&Release> for ReleaseDto {
    fn from(r: &Release) -> Self {
        let notes = note_lines(r.changelog.as_deref(), 3);
        let all = note_lines(r.changelog.as_deref(), 4);
        ReleaseDto {
            version: r.version.clone(),
            published_at: r.published_at,
            size: r.size,
            channel: format!("{:?}", r.channel).to_lowercase(),
            flavour_unknown: r.flavour_unknown(),
            has_more_notes: all.len() > notes.len(),
            notes,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusDto {
    pub id: String,
    /// up_to_date | available | pinned | skipped | no_build | not_managed | unavailable | failed
    pub kind: String,
    pub release: Option<ReleaseDto>,
    pub message: Option<String>,
}

impl StatusDto {
    pub fn from_status(id: &str, s: &Status) -> StatusDto {
        let (kind, release, message) = match s {
            Status::UpToDate => ("up_to_date", None, None),
            Status::Available(r) => ("available", Some(r.into()), None),
            Status::Pinned(r) => ("pinned", r.as_ref().map(Into::into), None),
            Status::Skipped(r) => ("skipped", Some(r.into()), None),
            Status::NoFlavourBuild => ("no_build", None, None),
            Status::NotManaged => ("not_managed", None, None),
            Status::SourceUnavailable(src) => (
                "unavailable",
                None,
                Some(match src {
                    SourceId::Curseforge => "CurseForge needs an API key (Settings).".to_string(),
                    SourceId::Wago => "Wago needs a paid key; not supported.".to_string(),
                    s => format!("{} is turned off in Settings.", s.label()),
                }),
            ),
            Status::Failed(e) => ("failed", None, Some(e.clone())),
        };
        StatusDto {
            id: id.into(),
            kind: kind.into(),
            release,
            message,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceErrorDto {
    pub source: String,
    pub source_label: String,
    pub message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckDto {
    pub checked_at: DateTime<Utc>,
    pub results: Vec<StatusDto>,
    pub source_errors: Vec<SourceErrorDto>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProgressDto {
    pub id: String,
    pub done: u64,
    pub total: Option<u64>,
    pub percent: Option<u8>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RollbackDto {
    pub from: String,
    pub to: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentDto {
    pub id: String,
    pub name: String,
    pub source: Option<String>,
    pub from: String,
    pub to: String,
    pub at: DateTime<Utc>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HitDto {
    pub source: String,
    pub source_label: String,
    pub source_id: String,
    pub spec: String,
    pub name: String,
    pub authors: Vec<String>,
    pub summary: Option<String>,
    pub flavours: Vec<String>,
    pub for_flavour: bool,
    pub flavour_unknown: bool,
    pub downloads: Option<u64>,
    pub updated_at: Option<DateTime<Utc>>,
    pub icon_url: Option<String>,
    pub installed: bool,
    pub also_on: Vec<String>,
}

impl HitDto {
    pub fn new(h: CatalogueEntry, flavour: Flavour, installed: bool) -> HitDto {
        HitDto {
            source: h.source.as_str().into(),
            source_label: h.source.label().into(),
            spec: format!("{}:{}", h.source.as_str(), h.source_id),
            source_id: h.source_id,
            name: h.name,
            authors: h.authors,
            summary: h.summary,
            for_flavour: h.flavours.contains(&flavour),
            flavour_unknown: h.flavours.is_empty(),
            flavours: h
                .flavours
                .iter()
                .map(|f| f.short_label().to_string())
                .collect(),
            downloads: h.downloads,
            updated_at: h.updated_at,
            icon_url: h.icon_url,
            installed,
            also_on: vec![],
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewVersionDto {
    pub version: String,
    pub url: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdoptedDto {
    pub installation: InstallationDto,
    pub addons: usize,
    pub managed: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotesDto {
    pub lines: Vec<String>,
    pub has_more: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogueDto {
    pub total: u64,
    pub refreshed_at: Option<DateTime<Utc>>,
    pub errors: Vec<SourceErrorDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchDto {
    pub hits: Vec<HitDto>,
    pub errors: Vec<SourceErrorDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InfoDto {
    pub name: String,
    pub authors: Vec<String>,
    pub icon_url: Option<String>,
    pub summary: Option<String>,
    pub url: Option<String>,
}

/// Everything the details view shows that is not in local state.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetailsDto {
    pub name: String,
    pub authors: Vec<String>,
    pub icon_url: Option<String>,
    pub screenshots: Vec<String>,
    /// Same order as `screenshots`; empty means use the full images.
    pub screenshot_thumbs: Vec<String>,
    pub summary: Option<String>,
    pub about: Vec<String>,
    pub url: Option<String>,
    pub releases: Vec<HistoryDto>,
}

/// One release in the details view's version history.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryDto {
    pub version: String,
    pub published_at: Option<DateTime<Utc>>,
    pub size: Option<u64>,
    pub channel: String,
    pub flavours: Vec<String>,
    pub notes: Vec<String>,
}

impl From<&Release> for HistoryDto {
    fn from(r: &Release) -> Self {
        HistoryDto {
            version: r.version.clone(),
            published_at: r.published_at,
            size: r.size,
            channel: format!("{:?}", r.channel).to_lowercase(),
            flavours: r
                .flavours
                .iter()
                .map(|f| f.short_label().to_string())
                .collect(),
            notes: note_lines(r.changelog.as_deref(), 6),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsDto {
    pub check_on_launch: bool,
    pub auto_apply: bool,
    pub keep_previous_versions: usize,
    pub self_update_check: bool,
    pub github: bool,
    pub wowinterface: bool,
    pub curseforge: bool,
    pub key_github: bool,
    pub key_curseforge: bool,
    pub key_wago: bool,
    pub data_dir: String,
    pub network_log: String,
    pub version: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsInput {
    pub check_on_launch: bool,
    pub auto_apply: bool,
    pub keep_previous_versions: usize,
    pub self_update_check: bool,
    pub github: bool,
    pub wowinterface: bool,
    pub curseforge: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes() {
        let md = "# Questie\n\n## [v12.5.1](https://x)\n- Forever: added [dungeon](https://x) quests\n- **Fixed** tracker\n- Three\n- Four\n";
        assert_eq!(
            note_lines(Some(md), 3),
            vec!["Forever: added dungeon quests", "Fixed tracker", "Three"]
        );
        assert_eq!(
            note_lines(Some("[b]Hello[/b] world"), 3),
            vec!["Hello world"]
        );
        // WoWInterface changelogs are BBCode lists.
        assert_eq!(
            note_lines(
                Some("[list]\n[*]Fixed bars\n[*][b]New[/b] raid\n[/list]"),
                3
            ),
            vec!["Fixed bars", "New raid"]
        );
        assert!(note_lines(None, 3).is_empty());
    }

    #[test]
    fn websites() {
        assert_eq!(
            website(Some(SourceId::Github), "rusty-key/spoken-wow#spoken").as_deref(),
            Some("https://github.com/rusty-key/spoken-wow")
        );
        assert_eq!(website(None, "x"), None);
    }
}
