//! The bundled GitHub index (`data/github-index.toml`): addons whose releases
//! are published on GitHub, keyed by the host IDs their TOCs carry. It lets
//! reconcile, search and install use GitHub (no key) for addons whose TOCs
//! only name CurseForge or Wago.

use crate::flavour::Flavour;
use crate::toc::Toc;
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct KnownAddon {
    pub name: String,
    /// `owner/repo`.
    pub repo: String,
    /// For repos that release several things: only releases whose tag starts
    /// with `<tag>/` belong to this addon (Spoken: `spoken/v3.8.0`,
    /// `quests-audio/v2.5.0` …).
    pub tag: Option<String>,
    /// The addon's main folder in AddOns, when known.
    pub folder: Option<String>,
    pub curseforge: Option<String>,
    pub wago: Option<String>,
    pub wowinterface: Option<String>,
    /// Flavours to assume when a release does not say (no release.json and
    /// no flavour suffix), taken from the addon's own TOC `## Interface`.
    #[serde(default)]
    pub flavours: Vec<Flavour>,
}

#[derive(Deserialize)]
struct IndexFile {
    addon: Vec<KnownAddon>,
}

const INDEX: &str = include_str!("../data/github-index.toml");

/// Every entry in the bundled index.
pub fn github_index() -> &'static [KnownAddon] {
    static PARSED: OnceLock<Vec<KnownAddon>> = OnceLock::new();
    PARSED.get_or_init(|| {
        toml::from_str::<IndexFile>(INDEX)
            .expect("data/github-index.toml is checked by tests")
            .addon
    })
}

fn same(a: &Option<String>, b: &Option<String>) -> bool {
    matches!((a, b), (Some(x), Some(y)) if x == y)
}

fn conflicts(a: &Option<String>, b: &Option<String>) -> bool {
    matches!((a, b), (Some(x), Some(y)) if x != y)
}

impl KnownAddon {
    /// The GitHub source id: `owner/repo`, or `owner/repo#tag`.
    pub fn source_id(&self) -> String {
        match &self.tag {
            Some(t) => format!("{}#{t}", self.repo),
            None => self.repo.clone(),
        }
    }

    fn folder_is(&self, folder: &str) -> bool {
        self.folder
            .as_deref()
            .is_some_and(|f| f.eq_ignore_ascii_case(folder))
    }

    fn shares_id(&self, toc: &Toc) -> bool {
        same(&self.curseforge, &toc.curse_id)
            || same(&self.wago, &toc.wago_id)
            || same(&self.wowinterface, &toc.wowi_id)
    }

    fn contradicts(&self, toc: &Toc) -> bool {
        conflicts(&self.curseforge, &toc.curse_id)
            || conflicts(&self.wago, &toc.wago_id)
            || conflicts(&self.wowinterface, &toc.wowi_id)
    }
}

/// The index entry for an installed folder: by a shared host ID first, else by
/// folder name when the TOC's IDs do not point at a different addon.
pub fn match_folder(folder: &str, toc: &Toc) -> Option<&'static KnownAddon> {
    let idx = github_index();
    idx.iter().find(|k| k.shares_id(toc)).or_else(|| {
        idx.iter()
            .find(|k| k.folder_is(folder) && !k.contradicts(toc))
    })
}

/// The entry for a GitHub source id (`owner/repo` or `owner/repo#tag`).
pub fn by_source_id(source_id: &str) -> Option<&'static KnownAddon> {
    github_index()
        .iter()
        .find(|k| k.source_id().eq_ignore_ascii_case(source_id))
}

/// Look an addon up by name, folder or repo (`questie`, `DBM-Core`,
/// `Questie/Questie`), ignoring case and spaces.
pub fn find(query: &str) -> Option<&'static KnownAddon> {
    let norm = |s: &str| {
        s.chars()
            .filter(|c| c.is_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect::<String>()
    };
    let q = norm(query);
    if q.is_empty() {
        return None;
    }
    github_index().iter().find(|k| {
        norm(&k.name) == q
            || k.folder.as_deref().is_some_and(|f| norm(f) == q)
            || k.source_id().eq_ignore_ascii_case(query.trim())
            || (k.tag.is_none() && k.repo.split_once('/').is_some_and(|(_, r)| norm(r) == q))
    })
}

/// Entries whose name, folder or repo contains `query`.
pub fn search(query: &str) -> Vec<&'static KnownAddon> {
    let q = query.trim().to_lowercase();
    github_index()
        .iter()
        .filter(|k| {
            q.is_empty()
                || k.name.to_lowercase().contains(&q)
                || k.folder
                    .as_deref()
                    .is_some_and(|f| f.to_lowercase().contains(&q))
                || k.repo.to_lowercase().contains(&q)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::toc::parse_github_repo;

    #[test]
    fn index_is_well_formed() {
        let idx = github_index();
        assert!(idx.len() >= 25);
        for k in idx {
            assert_eq!(
                parse_github_repo(&k.repo).as_deref(),
                Some(k.repo.as_str()),
                "{} has a bad repo",
                k.name
            );
            // Install/search-only entries (a tagged pack whose folder is not
            // confirmed yet) are allowed; everything else must be matchable.
            assert!(
                k.curseforge.is_some()
                    || k.wago.is_some()
                    || k.wowinterface.is_some()
                    || k.folder.is_some()
                    || k.tag.is_some(),
                "{} has nothing to match an installed addon on",
                k.name
            );
            if let Some(f) = &k.folder {
                let dupes = idx.iter().filter(|o| o.folder_is(f)).count();
                assert_eq!(dupes, 1, "{f} folder listed twice");
            }
            let same_id = idx
                .iter()
                .filter(|o| o.source_id().eq_ignore_ascii_case(&k.source_id()))
                .count();
            assert_eq!(same_id, 1, "{} listed twice", k.source_id());
            assert_eq!(by_source_id(&k.source_id()), Some(k));
        }
    }

    #[test]
    fn matches_by_id_then_folder() {
        // Questie's TOC names only CurseForge and Wago.
        let t = Toc::parse("## X-Curse-Project-ID: 334372\n## X-Wago-ID: qv634BKb\n");
        assert_eq!(match_folder("Questie", &t).unwrap().repo, "Questie/Questie");
        // Same IDs under another folder name still match.
        assert_eq!(
            match_folder("Whatever", &t).unwrap().repo,
            "Questie/Questie"
        );
        // No IDs: folder name is enough.
        assert_eq!(
            match_folder("DBM-Core", &Toc::default()).unwrap().repo,
            "DeadlyBossMods/DeadlyBossMods"
        );
        // A folder with the right name but someone else's IDs is not a match.
        let other = Toc::parse("## X-Curse-Project-ID: 1\n");
        assert!(match_folder("Questie", &other).is_none());
        // Spoken's voice packs are their own entries.
        assert_eq!(
            match_folder("SpokenBossesAudio", &Toc::default())
                .unwrap()
                .tag
                .as_deref(),
            Some("bosses-audio")
        );
    }

    #[test]
    fn finds_by_name() {
        assert_eq!(find("questie").unwrap().repo, "Questie/Questie");
        assert_eq!(
            find("Deadly Boss Mods").unwrap().folder.as_deref(),
            Some("DBM-Core")
        );
        assert_eq!(find("dbm-core").unwrap().name, "Deadly Boss Mods");
        assert_eq!(
            find("spoken").unwrap().source_id(),
            "rusty-key/spoken-wow#spoken"
        );
        assert_eq!(
            find("Spoken Quests Audio").unwrap().source_id(),
            "rusty-key/spoken-wow#quests-audio"
        );
        // A shared repo name never resolves on its own.
        assert!(find("spoken-wow").is_none());
        assert_eq!(
            find("WeakAuras2").unwrap().folder.as_deref(),
            Some("WeakAuras")
        );
        assert_eq!(
            find("Plater-Nameplates").unwrap().folder.as_deref(),
            Some("Plater")
        );
        assert!(find("not an addon").is_none());
        assert!(search("bigwigs").len() >= 5);
    }
}
