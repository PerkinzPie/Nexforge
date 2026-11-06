//! Adopt an existing AddOns folder: map folders to addons and record them in
//! `addons.toml`. Reconcile reads the disk and writes state; it never deletes
//! or moves anything in AddOns.

use crate::error::{IoContext, Result};
use crate::flavour::Flavour;
use crate::sources::{Channel, SourceId};
use crate::state::{AddonEntry, AddonsFile, KnownIds};
use crate::toc::{read_folder, AddonFolder};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

/// Folders that are never addons we manage.
pub fn is_ignored_folder(name: &str) -> bool {
    name.starts_with("Blizzard_") || name.starts_with('.')
}

#[derive(Debug, Default)]
pub struct Scan {
    pub folders: Vec<AddonFolder>,
    /// `(folder, why)` for folders we skipped.
    pub ignored: Vec<(String, String)>,
}

/// Read every addon folder in `addons_dir`.
pub fn scan(addons_dir: &Path, flavour: Flavour) -> Result<Scan> {
    let mut out = Scan::default();
    let mut entries: Vec<_> = std::fs::read_dir(addons_dir)
        .ctx("read", addons_dir)?
        .flatten()
        // Follows links: a folder shared from another client by a junction counts.
        .filter(|e| e.path().is_dir())
        .collect();
    entries.sort_by_key(|e| e.file_name().to_ascii_lowercase());
    for e in entries {
        let name = e.file_name().to_string_lossy().to_string();
        if is_ignored_folder(&name) {
            out.ignored.push((name, "Blizzard or hidden folder".into()));
            continue;
        }
        match read_folder(&e.path(), flavour) {
            Ok(Some(f)) => out.folders.push(f),
            Ok(None) => out.ignored.push((name, "no .toc file".into())),
            Err(err) => out.ignored.push((name, err.to_string())),
        }
    }
    Ok(out)
}

/// Union-find over folders. Each set may carry a label (the GitHub index
/// entry a folder matches); two sets with different labels never merge, so
/// addons the index knows as separate stay separate even when their TOCs
/// link them (`X-Part-Of`, dependencies).
struct Dsu {
    parent: Vec<usize>,
    label: Vec<Option<usize>>,
}

impl Dsu {
    fn new(labels: Vec<Option<usize>>) -> Self {
        Dsu {
            parent: (0..labels.len()).collect(),
            label: labels,
        }
    }
    fn find(&mut self, x: usize) -> usize {
        let mut r = x;
        while self.parent[r] != r {
            r = self.parent[r];
        }
        let mut x = x;
        while self.parent[x] != r {
            let next = self.parent[x];
            self.parent[x] = r;
            x = next;
        }
        r
    }
    fn union(&mut self, a: usize, b: usize) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra == rb {
            return;
        }
        match (self.label[ra], self.label[rb]) {
            (Some(x), Some(y)) if x != y => {}
            (la, lb) => {
                self.parent[rb] = ra;
                self.label[ra] = la.or(lb);
            }
        }
    }
}

fn is_sep(b: u8) -> bool {
    matches!(b, b'_' | b'-')
}

/// Do two folder names share a stem? `BigWigs` / `BigWigs_Core`,
/// `WeakAuras` / `WeakAurasOptions` and `BigWigs_Core` / `BigWigs_Options`
/// do; `Deathlog` / `DeathNotificationLib` do not.
fn shares_stem(a: &str, b: &str) -> bool {
    let (x, y) = (a.as_bytes(), b.as_bytes());
    let p = x
        .iter()
        .zip(y)
        .take_while(|(c, d)| c.eq_ignore_ascii_case(d))
        .count();
    if p < MIN_PREFIX {
        return false;
    }
    // One name is the other's whole stem (`WeakAuras` / `WeakAurasOptions`),
    // or the shared part ends at a separator (`BigWigs_` …).
    p == x.len() || p == y.len() || is_sep(x[p - 1])
}

/// `Foo_Libs` / `Foo-Data` extends `Foo`.
fn extends(name: &str, base: &str) -> bool {
    name.len() > base.len() + 1
        && name.is_char_boundary(base.len())
        && name[..base.len()].eq_ignore_ascii_case(base)
        && matches!(name.as_bytes()[base.len()], b'_' | b'-')
}

/// Shortest shared stem for dependency-based grouping of a folder that has
/// no IDs of its own. Below this, the link is only offered as a hint.
const MIN_PREFIX: usize = 3;

/// Group folders into addons. Returns groups of indices into `folders`, and
/// hints for links too weak to act on.
pub fn group(folders: &[AddonFolder]) -> (Vec<Vec<usize>>, Vec<String>) {
    let index = crate::known::github_index();
    let mut dsu = Dsu::new(
        folders
            .iter()
            .map(|f| {
                crate::known::match_folder(&f.name, &f.toc)
                    .and_then(|k| index.iter().position(|x| std::ptr::eq(x, k)))
            })
            .collect(),
    );
    let mut hints = Vec::new();
    let by_name: HashMap<String, usize> = folders
        .iter()
        .enumerate()
        .map(|(i, f)| (f.name.to_ascii_lowercase(), i))
        .collect();
    let idx = |n: &str| by_name.get(&n.trim().to_ascii_lowercase()).copied();

    // 1. Shared host IDs, X-Part-Of, X-Tukui-ProjectFolders.
    let mut keys: HashMap<String, usize> = HashMap::new();
    for (i, f) in folders.iter().enumerate() {
        let t = &f.toc;
        let mut ks = Vec::new();
        if let Some(v) = &t.curse_id {
            ks.push(format!("cf:{v}"));
        }
        if let Some(v) = &t.wago_id {
            ks.push(format!("wago:{v}"));
        }
        if let Some(v) = &t.wowi_id {
            ks.push(format!("wowi:{v}"));
        }
        if let Some(v) = &t.part_of {
            ks.push(format!("part:{}", v.to_ascii_lowercase()));
            if let Some(j) = idx(v) {
                dsu.union(j, i);
            }
        }
        for k in ks {
            match keys.get(&k) {
                Some(&j) => dsu.union(j, i),
                None => {
                    keys.insert(k, i);
                }
            }
        }
        if let Some(list) = t.fields.get("x-tukui-projectfolders") {
            for n in list.split(',') {
                if let Some(j) = idx(n) {
                    dsu.union(i, j);
                }
            }
        }
    }

    // 2. Dependencies, for folders with no IDs of their own. A folder joins
    //    its dependencies and its ID-less dependents when the names share a
    //    prefix (`BigWigs_Core` → `BigWigs`). It joins a dependent that has
    //    IDs only when its own name extends that dependent's (`Foo_Libs` used
    //    by `Foo`); otherwise a separate plugin with IDs (`ElvUI_ToxiUI`,
    //    which depends on `ElvUI`) would swallow its host.
    let mut dependents: HashMap<usize, Vec<usize>> = HashMap::new();
    for (i, f) in folders.iter().enumerate() {
        for d in &f.toc.dependencies {
            if let Some(j) = idx(d) {
                dependents.entry(j).or_default().push(i);
            }
        }
    }
    let mut weak = Vec::new();
    for (i, f) in folders.iter().enumerate() {
        if f.toc.has_source_id() {
            continue;
        }
        let deps = f.toc.dependencies.iter().filter_map(|d| idx(d));
        let users = dependents.get(&i).into_iter().flatten().copied();
        let mut linked = false;
        for (j, is_dep) in deps.map(|j| (j, true)).chain(users.map(|j| (j, false))) {
            if j == i {
                continue;
            }
            let other = &folders[j];
            let ok = if !is_dep && other.toc.has_source_id() {
                extends(&f.name, &other.name)
            } else {
                shares_stem(&f.name, &other.name)
            };
            if ok {
                dsu.union(j, i);
                linked = true;
            }
        }
        if !linked && f.toc.dependencies.iter().any(|d| idx(d).is_some()) {
            weak.push(i);
        }
    }

    let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for i in 0..folders.len() {
        let r = dsu.find(i);
        groups.entry(r).or_default().push(i);
    }
    let mut groups: Vec<Vec<usize>> = groups.into_values().collect();

    // 3. Last resort, hint only: lone folders whose name extends another
    //    folder's name (`Foo_Extras` next to `Foo`), one line per parent.
    let mut by_parent: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for g in groups.iter().filter(|g| g.len() == 1) {
        let i = g[0];
        if folders[i].toc.has_source_id() {
            continue;
        }
        let parent = folders[i]
            .name
            .split_once(['_', '-'])
            .and_then(|(base, _)| idx(base))
            .filter(|&j| j != i);
        if let Some(j) = parent {
            by_parent.entry(j).or_default().push(i);
        }
    }
    for (j, kids) in &by_parent {
        let names: Vec<&str> = kids.iter().map(|&i| folders[i].name.as_str()).collect();
        hints.push(format!(
            "{} may be part of {}; left separate until a source confirms it.",
            names.join(", "),
            folders[*j].name
        ));
    }
    let hinted: Vec<usize> = by_parent.values().flatten().copied().collect();
    for i in weak.into_iter().filter(|i| !hinted.contains(i)) {
        let deps: Vec<&str> = folders[i]
            .toc
            .dependencies
            .iter()
            .filter(|d| idx(d).is_some())
            .map(String::as_str)
            .collect();
        hints.push(format!(
            "{} depends on {} but does not look like part of it; left separate.",
            folders[i].name,
            deps.join(", ")
        ));
    }
    groups.sort_by_key(|g| {
        g.iter()
            .map(|&i| folders[i].name.to_ascii_lowercase())
            .min()
    });
    (groups, hints)
}

/// TOC titles often end with the version ("Questie v10.3.14"); drop it.
fn display_name(title: &str, version: &str) -> String {
    let t = title.trim();
    if !version.is_empty() {
        for suffix in [format!(" v{version}"), format!(" {version}")] {
            if let Some(rest) = t.strip_suffix(&suffix) {
                if !rest.trim().is_empty() {
                    return rest.trim().to_string();
                }
            }
        }
    }
    t.to_string()
}

/// The folder that names the group: most in-group dependents, then shortest name.
fn pick_root(folders: &[AddonFolder], group: &[usize]) -> usize {
    let names: Vec<String> = group
        .iter()
        .map(|&i| folders[i].name.to_ascii_lowercase())
        .collect();
    *group
        .iter()
        .max_by_key(|&&i| {
            let me = folders[i].name.to_ascii_lowercase();
            let dependents = group
                .iter()
                .filter(|&&j| {
                    j != i
                        && folders[j]
                            .toc
                            .dependencies
                            .iter()
                            .any(|d| d.eq_ignore_ascii_case(&me))
                })
                .count();
            let part_of_me = folders[i]
                .toc
                .part_of
                .as_deref()
                .map(|p| p.eq_ignore_ascii_case(&me))
                .unwrap_or(false);
            let prefix_of = names.iter().filter(|n| n.starts_with(&me)).count();
            (
                dependents + prefix_of + usize::from(part_of_me),
                std::cmp::Reverse(me.len()),
                std::cmp::Reverse(me),
            )
        })
        .expect("groups are never empty")
}

#[derive(Debug)]
pub struct Report {
    pub file: AddonsFile,
    /// Entries already managed before this run, still present.
    pub kept: Vec<String>,
    /// New entries with a source.
    pub adopted: Vec<String>,
    /// New entries found but not matched to a source.
    pub unmanaged: Vec<String>,
    /// Managed entries with folders no longer in AddOns: `(id, folders)`.
    pub missing: Vec<(String, Vec<String>)>,
    pub ignored: Vec<(String, String)>,
    pub hints: Vec<String>,
}

/// Reconcile `addons_dir` against `existing`. `available` lists the sources
/// that can serve addons right now (enabled, and keyed if they need a key);
/// the first one in [`SourceId::PREFERENCE`] that the TOCs name is chosen.
pub fn reconcile(
    addons_dir: &Path,
    flavour: Flavour,
    existing: &AddonsFile,
    available: &[SourceId],
) -> Result<Report> {
    let scan = scan(addons_dir, flavour)?;
    let present: HashMap<String, usize> = scan
        .folders
        .iter()
        .enumerate()
        .map(|(i, f)| (f.name.to_ascii_lowercase(), i))
        .collect();

    let mut report = Report {
        file: AddonsFile::new(flavour),
        kept: vec![],
        adopted: vec![],
        unmanaged: vec![],
        missing: vec![],
        ignored: scan.ignored.clone(),
        hints: vec![],
    };

    // Managed entries stay as they are; their folders are taken.
    let mut taken = vec![false; scan.folders.len()];
    for e in existing.addons.iter().filter(|e| e.is_managed()) {
        let missing: Vec<String> = e
            .folders
            .iter()
            .filter(|f| !present.contains_key(&f.to_ascii_lowercase()))
            .cloned()
            .collect();
        for f in &e.folders {
            if let Some(&i) = present.get(&f.to_ascii_lowercase()) {
                taken[i] = true;
            }
        }
        if !missing.is_empty() {
            report.missing.push((e.id.clone(), missing));
        }
        report.kept.push(e.id.clone());
        report.file.addons.push(e.clone());
    }

    // Everything else is (re)grouped. Unmanaged entries from an earlier run
    // are rebuilt, keeping their id and pin when the root folder matches.
    let old_unmanaged: HashMap<String, &AddonEntry> = existing
        .addons
        .iter()
        .filter(|e| !e.is_managed())
        .filter_map(|e| Some((e.folders.first()?.to_ascii_lowercase(), e)))
        .collect();

    let free: Vec<AddonFolder> = scan
        .folders
        .iter()
        .enumerate()
        .filter(|(i, _)| !taken[*i])
        .map(|(_, f)| f.clone())
        .collect();
    let (groups, hints) = group(&free);
    report.hints = hints;

    for g in groups {
        let root = pick_root(&free, &g);
        let rf = &free[root];
        let mut folder_names: Vec<String> = g.iter().map(|&i| free[i].name.clone()).collect();
        folder_names.sort_by_key(|n| n.to_ascii_lowercase());
        // Root first, so it is the folder that names the addon.
        folder_names.retain(|n| n != &rf.name);
        folder_names.insert(0, rf.name.clone());

        let mut ids = KnownIds::default();
        for &i in std::iter::once(&root).chain(g.iter()) {
            let t = &free[i].toc;
            if let Some(v) = t.github_repo() {
                ids.set(SourceId::Github, v);
            }
            if let Some(v) = &t.wowi_id {
                ids.set(SourceId::Wowinterface, v.clone());
            }
            if let Some(v) = &t.wago_id {
                ids.set(SourceId::Wago, v.clone());
            }
            if let Some(v) = &t.curse_id {
                ids.set(SourceId::Curseforge, v.clone());
            }
        }

        // TOCs that only name CurseForge or Wago: the bundled GitHub index
        // may know where the addon is released.
        if ids.github.is_none() {
            if let Some(k) = std::iter::once(&root)
                .chain(g.iter())
                .find_map(|&i| crate::known::match_folder(&free[i].name, &free[i].toc))
            {
                ids.set(SourceId::Github, k.source_id());
            }
        }

        let source = SourceId::PREFERENCE
            .into_iter()
            .find(|s| available.contains(s) && ids.get(*s).is_some());
        let version = std::iter::once(&root)
            .chain(g.iter())
            .find_map(|&i| free[i].toc.version.clone())
            .unwrap_or_default();
        let name = display_name(rf.toc.title.as_deref().unwrap_or(&rf.name), &version);

        let old = old_unmanaged.get(&rf.name.to_ascii_lowercase());
        let id = match old {
            Some(o) if source.is_none() && report.file.get(&o.id).is_none() => o.id.clone(),
            _ => report.file.unique_id(&name),
        };
        let entry = AddonEntry {
            id: id.clone(),
            name,
            source,
            source_id: source
                .and_then(|s| ids.get(s))
                .unwrap_or_default()
                .to_string(),
            installed_version: version,
            installed_at: None,
            folders: folder_names,
            pinned: old.map(|o| o.pinned).unwrap_or(false),
            skipped_version: String::new(),
            channel: Channel::Stable,
            author: rf.toc.author.clone(),
            flavour_mismatch: !rf.flavour_match,
            ids,
        };
        if entry.is_managed() {
            report.adopted.push(id);
        } else {
            report.unmanaged.push(id);
        }
        report.file.addons.push(entry);
    }

    report
        .file
        .addons
        .sort_by_key(|a| a.name.to_ascii_lowercase());
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk(dir: &Path, folder: &str, toc_name: &str, body: &str) {
        let d = dir.join(folder);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join(toc_name), body).unwrap();
    }

    fn fixture() -> tempfile::TempDir {
        let t = tempfile::tempdir().unwrap();
        let p = t.path();
        mk(p, "BigWigs", "BigWigs_Vanilla.toc", "## Title: BigWigs\n## Version: v1\n## X-Curse-Project-ID: 2382\n## X-WoWI-ID: 5086\n## X-Website: https://github.com/BigWigsMods\n");
        mk(
            p,
            "BigWigs_Core",
            "BigWigs_Core_Vanilla.toc",
            "## Title: BigWigs [Core]\n## Dependencies: BigWigs\n",
        );
        mk(
            p,
            "BigWigs_Options",
            "BigWigs_Options.toc",
            "## Title: BigWigs [Options]\n## Dependencies: BigWigs_Core\n",
        );
        mk(
            p,
            "BigWigs_Extra",
            "BigWigs_Extra.toc",
            "## Title: Extra v2\n## Version: 2\n",
        );
        mk(p, "BigWigs_Classic", "BigWigs_Classic_Vanilla.toc", "## Title: BigWigs [Classic]\n## Version: v12.0.10\n## Dependencies: BigWigs\n## X-WoWI-ID: 22095\n## X-Website: https://github.com/BigWigsMods/BigWigs_Classic\n");
        mk(p, "ElvUI", "ElvUI_Vanilla.toc", "## Title: ElvUI\n## RequiredDeps: ElvUI_Libraries\n## X-Tukui-ProjectFolders: ElvUI, ElvUI_Libraries, ElvUI_Options\n");
        mk(
            p,
            "ElvUI_Libraries",
            "ElvUI_Libraries_Vanilla.toc",
            "## Title: ElvUI Libraries\n",
        );
        mk(
            p,
            "ElvUI_Options",
            "ElvUI_Options_Vanilla.toc",
            "## Title: ElvUI Options\n## RequiredDeps: ElvUI\n",
        );
        mk(
            p,
            "ElvUI_ToxiUI",
            "ElvUI_ToxiUI_Vanilla.toc",
            "## Title: ToxiUI\n## RequiredDeps: ElvUI\n## X-WoWI-ID: 25770\n",
        );
        mk(
            p,
            "VO",
            "VO.toc",
            "## Title: VoiceOver\n## X-Part-Of: VoiceOver\n",
        );
        mk(
            p,
            "VO_Data",
            "VO_Data.toc",
            "## Title: VoiceOver Data\n## X-Part-Of: VoiceOver\n",
        );
        mk(
            p,
            "Plugin",
            "Plugin.toc",
            "## Title: Plugin\n## Dependencies: BigWigs\n",
        );
        mk(p, "Blizzard_Thing", "Blizzard_Thing.toc", "## Title: x\n");
        mk(p, "Empty", "readme.txt", "");
        std::fs::write(p.join("addons.toml"), "").unwrap();
        t
    }

    #[test]
    fn stems() {
        assert!(shares_stem("BigWigs", "BigWigs_Core"));
        assert!(shares_stem("BigWigs_Options", "BigWigs_Core"));
        assert!(shares_stem("Questie-Loc", "Questie"));
        assert!(!shares_stem("Deathlog", "DeathNotificationLib"));
        assert!(!shares_stem("LittleWigs_Classic", "BigWigs"));
        assert!(shares_stem("WeakAurasOptions", "WeakAuras"));
        assert!(extends("Foo_Libs", "Foo") && !extends("Foobar", "Foo"));
    }

    #[test]
    fn index_keeps_separate_addons_apart() {
        let t = tempfile::tempdir().unwrap();
        let p = t.path();
        mk(
            p,
            "Spoken",
            "Spoken.toc",
            "## Title: Spoken\n## Version: 3.8.0\n## X-Part-Of: Spoken\n",
        );
        mk(
            p,
            "Spoken_Quests",
            "Spoken_Quests.toc",
            "## Title: Spoken Quests\n## Dependencies: Spoken\n## X-Part-Of: Spoken\n",
        );
        mk(
            p,
            "SpokenBossesAudio",
            "SpokenBossesAudio.toc",
            "## Title: Spoken Audio: Bosses\n## Version: 1.0.0\n## X-Part-Of: Spoken\n",
        );
        let r = reconcile(
            p,
            Flavour::Forever,
            &AddonsFile::new(Flavour::Forever),
            &[SourceId::Github],
        )
        .unwrap();
        let spoken = r.file.owner_of("Spoken").unwrap();
        assert_eq!(
            spoken.folders,
            vec!["Spoken", "Spoken_Quests"],
            "modules join the addon"
        );
        assert_eq!(spoken.source_id, "rusty-key/spoken-wow#spoken");
        let pack = r.file.owner_of("SpokenBossesAudio").unwrap();
        assert_eq!(
            pack.folders,
            vec!["SpokenBossesAudio"],
            "the voice pack stays its own addon"
        );
        assert_eq!(pack.source_id, "rusty-key/spoken-wow#bosses-audio");
    }

    #[test]
    fn groups_and_picks_sources() {
        let t = fixture();
        let r = reconcile(
            t.path(),
            Flavour::ClassicEra,
            &AddonsFile::new(Flavour::ClassicEra),
            &[SourceId::Github, SourceId::Wowinterface],
        )
        .unwrap();
        let by_name = |n: &str| r.file.addons.iter().find(|a| a.name == n).unwrap();

        let bw = by_name("BigWigs");
        assert_eq!(
            bw.folders,
            vec!["BigWigs", "BigWigs_Core", "BigWigs_Options"]
        );
        assert_eq!(
            by_name("Extra").folders,
            vec!["BigWigs_Extra"],
            "prefix alone only hints"
        );
        assert!(r.hints.iter().any(|h| h == "BigWigs_Extra may be part of BigWigs; left separate until a source confirms it."), "{:?}", r.hints);
        // The TOC's org-only GitHub URL is not a repo, but the bundled index
        // knows BigWigs by its CurseForge ID.
        assert_eq!(bw.source, Some(SourceId::Github));
        assert_eq!(bw.source_id, "BigWigsMods/BigWigs");
        assert_eq!(bw.ids.curseforge.as_deref(), Some("2382"));

        let bwc = by_name("BigWigs [Classic]");
        assert_eq!(
            bwc.folders,
            vec!["BigWigs_Classic"],
            "own IDs keep it separate"
        );
        assert_eq!(bwc.source, Some(SourceId::Github));
        assert_eq!(bwc.source_id, "BigWigsMods/BigWigs_Classic");
        assert_eq!(bwc.installed_version, "v12.0.10");

        let elv = by_name("ElvUI");
        assert_eq!(
            elv.folders,
            vec!["ElvUI", "ElvUI_Libraries", "ElvUI_Options"]
        );
        assert!(
            elv.source.is_none(),
            "a plugin's IDs must not leak into its host"
        );
        assert_eq!(by_name("ToxiUI").folders, vec!["ElvUI_ToxiUI"]);

        assert_eq!(by_name("VoiceOver").folders, vec!["VO", "VO_Data"]);
        assert_eq!(
            by_name("Plugin").folders,
            vec!["Plugin"],
            "weak dependency link is only a hint"
        );
        assert!(
            r.hints
                .iter()
                .any(|h| h.starts_with("Plugin depends on BigWigs")),
            "{:?}",
            r.hints
        );
        assert_eq!(r.ignored.len(), 2);
        assert!(r.missing.is_empty());
    }

    #[test]
    fn keeps_managed_entries_and_reports_missing() {
        let t = fixture();
        let first = reconcile(
            t.path(),
            Flavour::ClassicEra,
            &AddonsFile::new(Flavour::ClassicEra),
            &[SourceId::Github],
        )
        .unwrap();
        let mut state = first.file.clone();
        state.require_mut("bigwigs-classic").unwrap().pinned = true;
        std::fs::remove_dir_all(t.path().join("BigWigs_Classic")).unwrap();
        let again = reconcile(t.path(), Flavour::ClassicEra, &state, &[SourceId::Github]).unwrap();
        assert_eq!(again.kept, vec!["bigwigs", "bigwigs-classic"]);
        assert!(again.file.get("bigwigs-classic").unwrap().pinned);
        assert_eq!(
            again.missing,
            vec![(
                "bigwigs-classic".to_string(),
                vec!["BigWigs_Classic".to_string()]
            )]
        );
        assert_eq!(again.file.addons.len(), first.file.addons.len());
    }
}
