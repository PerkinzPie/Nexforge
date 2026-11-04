//! Plain-file state: `addons.toml` per installation and `config.toml` for settings.

use crate::error::{Error, IoContext, Result};
use crate::flavour::Flavour;
use crate::sources::{Channel, SourceId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const SCHEMA: u32 = 1;

/// IDs found in an addon's TOCs. Kept even for sources that are not in use, so
/// enabling Wago or CurseForge later can re-home an addon without a rescan.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnownIds {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub github: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wowinterface: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wago: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub curseforge: Option<String>,
}

impl KnownIds {
    pub fn is_empty(&self) -> bool {
        self.github.is_none()
            && self.wowinterface.is_none()
            && self.wago.is_none()
            && self.curseforge.is_none()
    }

    pub fn get(&self, source: SourceId) -> Option<&str> {
        match source {
            SourceId::Github => self.github.as_deref(),
            SourceId::Wowinterface => self.wowinterface.as_deref(),
            SourceId::Wago => self.wago.as_deref(),
            SourceId::Curseforge => self.curseforge.as_deref(),
        }
    }

    pub fn set(&mut self, source: SourceId, id: String) {
        let slot = match source {
            SourceId::Github => &mut self.github,
            SourceId::Wowinterface => &mut self.wowinterface,
            SourceId::Wago => &mut self.wago,
            SourceId::Curseforge => &mut self.curseforge,
        };
        slot.get_or_insert(id);
    }
}

fn is_false(b: &bool) -> bool {
    !*b
}
fn is_stable(c: &Channel) -> bool {
    *c == Channel::Stable
}

/// One addon in `addons.toml`. An entry without `source` was found during
/// reconcile but is not managed yet ("Identify source" in the UI).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddonEntry {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceId>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source_id: String,
    #[serde(default)]
    pub installed_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub folders: Vec<String>,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub skipped_version: String,
    #[serde(default, skip_serializing_if = "is_stable")]
    pub channel: Channel,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// True when the release installed was not listed for this flavour
    /// ("Install anyway"), or the TOCs say the addon does not load here.
    #[serde(default, skip_serializing_if = "is_false")]
    pub flavour_mismatch: bool,
    #[serde(default, skip_serializing_if = "KnownIds::is_empty")]
    pub ids: KnownIds,
}

impl AddonEntry {
    pub fn is_managed(&self) -> bool {
        self.source.is_some() && !self.source_id.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddonsFile {
    pub schema: u32,
    pub flavour: Flavour,
    #[serde(default, rename = "addon")]
    pub addons: Vec<AddonEntry>,
}

impl AddonsFile {
    pub fn new(flavour: Flavour) -> Self {
        AddonsFile {
            schema: SCHEMA,
            flavour,
            addons: Vec::new(),
        }
    }

    /// Load, or start empty if the file does not exist yet.
    pub fn load_or_new(path: &Path, flavour: Flavour) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::new(flavour));
        }
        let file: AddonsFile = read_toml(path)?;
        if file.schema > SCHEMA {
            return Err(Error::BadStateFile {
                path: path.into(),
                message: format!(
                    "it was written by a newer Nexforge (schema {}). Update Nexforge to read it.",
                    file.schema
                ),
            });
        }
        Ok(file)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let header = "# Written by Nexforge. Safe to read; edit with care.\n\
                      # One [[addon]] per addon. Entries without `source` were found in AddOns but are not managed yet.\n\n";
        write_toml(path, self, header)
    }

    pub fn get(&self, id: &str) -> Option<&AddonEntry> {
        self.addons.iter().find(|a| a.id.eq_ignore_ascii_case(id))
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut AddonEntry> {
        self.addons
            .iter_mut()
            .find(|a| a.id.eq_ignore_ascii_case(id))
    }

    pub fn require_mut(&mut self, id: &str) -> Result<&mut AddonEntry> {
        self.get_mut(id)
            .ok_or_else(|| Error::UnknownAddon(id.into()))
    }

    /// The entry that owns `folder`, if any (case-insensitive, as Windows is).
    pub fn owner_of(&self, folder: &str) -> Option<&AddonEntry> {
        self.addons
            .iter()
            .find(|a| a.folders.iter().any(|f| f.eq_ignore_ascii_case(folder)))
    }

    /// A slug not used by any other entry.
    pub fn unique_id(&self, name: &str) -> String {
        let base = slugify(name);
        let mut id = base.clone();
        let mut n = 2;
        while self.get(&id).is_some() {
            id = format!("{base}-{n}");
            n += 1;
        }
        id
    }
}

/// `"BigWigs [Classic]"` → `"bigwigs-classic"`.
pub fn slugify(name: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in name.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    let out = out.trim_end_matches('-').to_string();
    if out.is_empty() {
        "addon".into()
    } else {
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallationConfig {
    pub id: String,
    pub flavour: Flavour,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourcesConfig {
    #[serde(default = "yes")]
    pub github: bool,
    #[serde(default = "yes")]
    pub wowinterface: bool,
    #[serde(default = "yes")]
    pub wago: bool,
    #[serde(default)]
    pub curseforge: bool,
}

fn yes() -> bool {
    true
}
fn three() -> usize {
    3
}
fn schema() -> u32 {
    SCHEMA
}

impl Default for SourcesConfig {
    fn default() -> Self {
        SourcesConfig {
            github: true,
            wowinterface: true,
            wago: true,
            curseforge: false,
        }
    }
}

impl SourcesConfig {
    pub fn enabled(&self, s: SourceId) -> bool {
        match s {
            SourceId::Github => self.github,
            SourceId::Wowinterface => self.wowinterface,
            SourceId::Wago => self.wago,
            SourceId::Curseforge => self.curseforge,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "schema")]
    pub schema: u32,
    #[serde(default = "yes")]
    pub check_on_launch: bool,
    #[serde(default)]
    pub auto_apply: bool,
    #[serde(default = "three")]
    pub keep_previous_versions: usize,
    #[serde(default)]
    pub self_update_check: bool,
    #[serde(default, rename = "installation")]
    pub installations: Vec<InstallationConfig>,
    #[serde(default)]
    pub sources: SourcesConfig,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            schema: SCHEMA,
            check_on_launch: true,
            auto_apply: false,
            keep_previous_versions: 3,
            self_update_check: false,
            installations: Vec::new(),
            sources: SourcesConfig::default(),
        }
    }
}

impl Config {
    pub fn load_or_default(path: &Path) -> Result<Config> {
        if path.exists() {
            read_toml(path)
        } else {
            Ok(Config::default())
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        write_toml(path, self, "# Nexforge settings.\n\n")
    }
}

fn read_toml<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let text = std::fs::read_to_string(path).ctx("read", path)?;
    toml::from_str(&text).map_err(|e| Error::BadStateFile {
        path: path.into(),
        message: e.message().to_string(),
    })
}

/// Write via a temp file and rename, so a crash never leaves half a file.
fn write_toml<T: Serialize>(path: &Path, value: &T, header: &str) -> Result<()> {
    let body = toml::to_string_pretty(value).map_err(|e| Error::Other(e.to_string()))?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).ctx("create", dir)?;
    }
    let tmp: PathBuf = path.with_extension("toml.tmp");
    std::fs::write(&tmp, format!("{header}{body}")).ctx("write", &tmp)?;
    std::fs::rename(&tmp, path).ctx("replace", path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_spec_example() {
        let text = r#"
schema = 1
flavour = "forever"

[[addon]]
id = "questie"
name = "Questie"
source = "github"
source_id = "Questie/Questie"
installed_version = "10.4.2"
installed_at = "2026-11-04T21:14:08Z"
folders = ["Questie", "Questie-Core", "Questie-Lib", "Questie-Loc"]
pinned = false
skipped_version = ""
channel = "stable"
"#;
        let f: AddonsFile = toml::from_str(text).unwrap();
        assert_eq!(f.flavour, Flavour::Forever);
        assert_eq!(f.addons[0].source, Some(SourceId::Github));
        assert_eq!(f.addons[0].folders.len(), 4);
        let again: AddonsFile = toml::from_str(&toml::to_string_pretty(&f).unwrap()).unwrap();
        assert_eq!(again, f);
    }

    #[test]
    fn config_defaults_and_paths() {
        let c: Config = toml::from_str(
            "schema = 1\n[[installation]]\nid = \"forever\"\nflavour = \"forever\"\npath = 'C:\\Games\\World of Warcraft\\_forever_'\n",
        )
        .unwrap();
        assert_eq!(c.keep_previous_versions, 3);
        assert!(c.sources.github && !c.sources.curseforge);
        // Compare as text: a Windows path is one component on other systems.
        assert!(c.installations[0]
            .path
            .to_string_lossy()
            .ends_with(r"World of Warcraft\_forever_"));
    }

    #[test]
    fn slugs() {
        assert_eq!(slugify("BigWigs [Classic]"), "bigwigs-classic");
        assert_eq!(
            slugify("Details!: Tiny Threat (plugin)"),
            "details-tiny-threat-plugin"
        );
        let mut f = AddonsFile::new(Flavour::Forever);
        f.addons.push(AddonEntry {
            id: "foo".into(),
            name: "Foo".into(),
            source: None,
            source_id: String::new(),
            installed_version: String::new(),
            installed_at: None,
            folders: vec!["Foo".into()],
            pinned: false,
            skipped_version: String::new(),
            channel: Channel::Stable,
            author: None,
            flavour_mismatch: false,
            ids: KnownIds::default(),
        });
        assert_eq!(f.unique_id("Foo"), "foo-2");
        assert_eq!(f.owner_of("FOO").unwrap().id, "foo");
    }
}
