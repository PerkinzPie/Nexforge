//! Find and describe game installations.

use crate::error::{Error, Result};
use crate::flavour::Flavour;
use crate::state::InstallationConfig;
use std::path::{Path, PathBuf};

/// Executables that mark a client folder. Retail and Forever ship `Wow.exe`,
/// the Classic clients ship `WowClassic.exe`.
const CLIENT_MARKERS: &[&str] = &[
    "Wow.exe",
    "WowClassic.exe",
    "World of Warcraft.app",
    "World of Warcraft Classic.app",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installation {
    pub id: String,
    pub flavour: Flavour,
    /// The client folder, e.g. `…\World of Warcraft\_forever_`.
    pub path: PathBuf,
}

impl Installation {
    pub fn addons_dir(&self) -> PathBuf {
        self.path.join("Interface").join("AddOns")
    }
    pub fn state_file(&self) -> PathBuf {
        self.addons_dir().join("addons.toml")
    }

    /// Build from a client folder, reading `.flavor.info` or the folder name.
    pub fn from_client_dir(path: &Path) -> Result<Installation> {
        let flavour = detect_flavour(path).ok_or_else(|| {
            Error::NoInstallation(format!(
                "{} does not look like a supported WoW client folder (expected _forever_, _classic_era_ or _anniversary_).",
                path.display()
            ))
        })?;
        Ok(Installation {
            id: flavour.as_str().to_string(),
            flavour,
            path: path.to_path_buf(),
        })
    }

    /// Build from an `Interface/AddOns` folder.
    pub fn from_addons_dir(addons: &Path, flavour: Option<Flavour>) -> Result<Installation> {
        let client = addons
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .unwrap_or_else(|| addons.to_path_buf());
        let flavour = match flavour.or_else(|| detect_flavour(&client)) {
            Some(f) => f,
            None => {
                return Err(Error::NoInstallation(format!(
                    "could not tell which game flavour {} belongs to. Pass --flavour.",
                    addons.display()
                )))
            }
        };
        Ok(Installation {
            id: flavour.as_str().to_string(),
            flavour,
            path: client,
        })
    }

    /// The name shown for it: "Classic Era", or "Classic Era 2" for a second
    /// client of the same flavour (id `classic-era-2`).
    pub fn label(&self) -> String {
        match self
            .id
            .strip_prefix(self.flavour.as_str())
            .and_then(|r| r.strip_prefix('-'))
        {
            Some(n) if !n.is_empty() => format!("{} {n}", self.flavour.label()),
            _ => self.flavour.label().to_string(),
        }
    }

    pub fn to_config(&self) -> InstallationConfig {
        InstallationConfig {
            id: self.id.clone(),
            flavour: self.flavour,
            path: self.path.clone(),
        }
    }

    pub fn from_config(c: &InstallationConfig) -> Installation {
        Installation {
            id: c.id.clone(),
            flavour: c.flavour,
            path: c.path.clone(),
        }
    }
}

/// Read the `product` line from `.flavor.info`. The file looks like
/// `Product Flavor!STRING:0\nwowf`.
pub fn read_flavor_info(client_dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(client_dir.join(".flavor.info")).ok()?;
    text.lines()
        .map(str::trim)
        .rfind(|l| !l.is_empty() && !l.contains('!'))
        .map(str::to_string)
}

/// `.flavor.info` first, then the folder name.
pub fn detect_flavour(client_dir: &Path) -> Option<Flavour> {
    if let Some(product) = read_flavor_info(client_dir) {
        if let Some(f) = Flavour::from_product(&product) {
            return Some(f);
        }
        // A product we know is not a v1 flavour (e.g. `wow_classic`): do not
        // guess from the folder name.
        return None;
    }
    client_dir
        .file_name()
        .and_then(|n| Flavour::from_folder_name(&n.to_string_lossy()))
}

#[derive(Debug, Clone)]
pub struct Skipped {
    pub path: PathBuf,
    pub reason: String,
}

/// Scan a `World of Warcraft` root for client folders.
pub fn detect_in(root: &Path) -> (Vec<Installation>, Vec<Skipped>) {
    let mut found = Vec::new();
    let mut skipped = Vec::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return (found, skipped);
    };
    let mut dirs: Vec<PathBuf> = entries
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| e.path())
        .filter(|p| {
            let n = p.file_name().unwrap_or_default().to_string_lossy();
            n.starts_with('_') && n.ends_with('_') && n.len() > 2
        })
        .collect();
    dirs.sort();
    for dir in dirs {
        let has_client = CLIENT_MARKERS.iter().any(|m| dir.join(m).exists())
            || dir.join(".flavor.info").exists();
        if !has_client {
            skipped.push(Skipped {
                path: dir,
                reason: "no game client in this folder".into(),
            });
            continue;
        }
        match detect_flavour(&dir) {
            Some(Flavour::Retail) => skipped.push(Skipped {
                path: dir,
                reason: "Retail is not supported yet".into(),
            }),
            Some(flavour) => found.push(Installation {
                id: flavour.as_str().to_string(),
                flavour,
                path: dir,
            }),
            None => skipped.push(Skipped {
                reason: match read_flavor_info(&dir) {
                    Some(p) => format!("game product \"{p}\" is not supported"),
                    None => "unknown client folder".into(),
                },
                path: dir,
            }),
        }
    }
    (found, skipped)
}

/// Likely `World of Warcraft` roots on this machine.
pub fn default_roots() -> Vec<PathBuf> {
    // Only Windows and macOS have known install locations; elsewhere the
    // player points Nexforge at the folder.
    #[cfg_attr(not(any(windows, target_os = "macos")), allow(unused_mut))]
    let mut out = Vec::new();
    #[cfg(windows)]
    {
        for drive in 'C'..='Z' {
            for rel in [
                r"Program Files (x86)\World of Warcraft",
                r"Program Files\World of Warcraft",
                r"World of Warcraft",
                r"Games\World of Warcraft",
            ] {
                let p = PathBuf::from(format!(r"{drive}:\{rel}"));
                if p.is_dir() {
                    out.push(p);
                }
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        let p = PathBuf::from("/Applications/World of Warcraft");
        if p.is_dir() {
            out.push(p);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_by_flavor_info_and_name() {
        let root = tempfile::tempdir().unwrap();
        let mk = |name: &str, exe: &str, product: Option<&str>| {
            let d = root.path().join(name);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join(exe), b"").unwrap();
            if let Some(p) = product {
                std::fs::write(
                    d.join(".flavor.info"),
                    format!("Product Flavor!STRING:0\n{p}\n"),
                )
                .unwrap();
            }
        };
        mk("_forever_", "Wow.exe", Some("wowf"));
        mk("_classic_era_", "WowClassic.exe", None);
        mk("_classic_", "WowClassic.exe", Some("wow_classic"));
        mk("_retail_", "Wow.exe", Some("wow"));
        std::fs::create_dir_all(root.path().join("_empty_")).unwrap();

        let (found, skipped) = detect_in(root.path());
        let flavours: Vec<_> = found.iter().map(|i| i.flavour).collect();
        assert_eq!(flavours, vec![Flavour::ClassicEra, Flavour::Forever]);
        assert_eq!(skipped.len(), 3);
        assert!(found[1]
            .state_file()
            .ends_with("Interface/AddOns/addons.toml"));
    }
}

/// Add `inst` to the configured installations (Settings → Add installation),
/// or with `replace`, point that installation at the new folder. Each client
/// folder gets its own id, so its backups and history stay its own: a second
/// Classic Era client becomes `classic-era-2` rather than replacing the
/// first. Retail is refused, as detection does.
pub fn add_to_config(
    cfg: &mut Vec<InstallationConfig>,
    mut inst: Installation,
    replace: Option<&str>,
) -> Result<Installation> {
    if inst.flavour == Flavour::Retail {
        return Err(Error::NoInstallation(
            "Retail is not supported yet. Choose a _forever_, _classic_era_ or _anniversary_ folder."
                .into(),
        ));
    }
    let same_path = |c: &InstallationConfig| {
        c.path.to_string_lossy().to_lowercase() == inst.path.to_string_lossy().to_lowercase()
    };
    if let Some(old) = replace {
        // "Change": the same installation, now in another folder.
        if let Some(c) = cfg.iter().find(|c| c.id == old) {
            if c.flavour == inst.flavour {
                inst.id = c.id.clone();
            }
        }
        cfg.retain(|c| c.id != old);
    } else if let Some(c) = cfg.iter().find(|c| same_path(c)) {
        // Already added: nothing changes.
        return Ok(Installation::from_config(c));
    }
    if cfg.iter().any(|c| c.id == inst.id) {
        let base = inst.flavour.as_str();
        let n = (2..).find(|n| !cfg.iter().any(|c| c.id == format!("{base}-{n}")));
        inst.id = format!("{base}-{}", n.unwrap_or(2));
    }
    cfg.push(inst.to_config());
    Ok(inst)
}

#[cfg(test)]
mod add_tests {
    use super::*;

    fn at(flavour: Flavour, path: &str) -> Installation {
        Installation {
            id: flavour.as_str().into(),
            flavour,
            path: PathBuf::from(path),
        }
    }

    #[test]
    fn a_second_client_of_a_flavour_gets_its_own_id() {
        let mut cfg = Vec::new();
        let a = add_to_config(
            &mut cfg,
            at(Flavour::ClassicEra, r"E:\WoW\_classic_era_"),
            None,
        )
        .unwrap();
        let b = add_to_config(
            &mut cfg,
            at(Flavour::ClassicEra, r"D:\PTR\_classic_era_"),
            None,
        )
        .unwrap();
        assert_eq!(
            (a.id.as_str(), b.id.as_str()),
            ("classic-era", "classic-era-2")
        );
        assert_eq!(cfg.len(), 2, "the first is not replaced");
        assert_eq!(b.label(), "Classic Era 2");
        // The same folder again (any case) changes nothing.
        let again = add_to_config(
            &mut cfg,
            at(Flavour::ClassicEra, r"e:\wow\_CLASSIC_ERA_"),
            None,
        )
        .unwrap();
        assert_eq!(again.id, "classic-era");
        assert_eq!(cfg.len(), 2);
        // Change keeps the id (and so its backups).
        let moved = add_to_config(
            &mut cfg,
            at(Flavour::ClassicEra, r"F:\WoW\_classic_era_"),
            Some("classic-era-2"),
        )
        .unwrap();
        assert_eq!(moved.id, "classic-era-2");
        assert_eq!(cfg.len(), 2);
        assert!(add_to_config(&mut cfg, at(Flavour::Retail, r"E:\WoW\_retail_"), None).is_err());
    }
}
