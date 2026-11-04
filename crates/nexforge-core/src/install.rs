//! Install / update / roll back / remove.
//!
//! download → verify → stage → back up → swap → keep. Nothing is ever
//! extracted straight into AddOns, and a failure part-way restores the
//! previous folders.

use crate::archive;
use crate::error::{Error, IoContext, Result};
use crate::installations::Installation;
use crate::net::{Fetch, Progress};
use crate::paths::{sanitize, AppPaths};
use crate::sources::Release;
use crate::state::{AddonEntry, AddonsFile};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// One kept archive in `backups/<installation>/<addon>/index.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Kept {
    pub version: String,
    pub file: String,
    pub saved_at: DateTime<Utc>,
    pub folders: Vec<String>,
    /// True for an archive Nexforge installed (downloaded, or restored by
    /// rollback); false for a zip it made of folders it adopted.
    #[serde(default)]
    pub installed: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct KeptIndex {
    #[serde(default)]
    kept: Vec<Kept>,
}

pub struct Engine {
    fetch: Arc<dyn Fetch>,
    paths: AppPaths,
    /// Previous versions to keep, besides the installed one.
    keep: usize,
}

/// Where an operation applies.
pub struct Target<'a> {
    pub installation: &'a Installation,
    pub state: &'a mut AddonsFile,
    pub state_path: &'a Path,
}

impl Target<'_> {
    fn addons_dir(&self) -> PathBuf {
        self.installation.addons_dir()
    }
}

impl Engine {
    pub fn new(fetch: Arc<dyn Fetch>, paths: AppPaths, keep: usize) -> Self {
        Engine { fetch, paths, keep }
    }

    fn backup_dir(&self, t: &Target<'_>, addon: &str) -> PathBuf {
        self.paths.backups(&t.installation.id, addon)
    }

    fn read_index(dir: &Path) -> KeptIndex {
        std::fs::read_to_string(dir.join("index.toml"))
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn write_index(dir: &Path, idx: &KeptIndex) -> Result<()> {
        std::fs::create_dir_all(dir).ctx("create", dir)?;
        let body = toml::to_string_pretty(idx).map_err(|e| Error::Other(e.to_string()))?;
        let p = dir.join("index.toml");
        let tmp = dir.join("index.toml.tmp");
        std::fs::write(&tmp, body).ctx("write", &tmp)?;
        std::fs::rename(&tmp, &p).ctx("save", &p)
    }

    /// Kept versions of an addon, newest first.
    pub fn kept_versions(&self, t: &Target<'_>, addon: &str) -> Vec<Kept> {
        self.kept_for(&t.installation.id, addon)
    }

    /// Kept versions of an addon in an installation, newest first.
    pub fn kept_for(&self, installation_id: &str, addon: &str) -> Vec<Kept> {
        let mut k = Self::read_index(&self.paths.backups(installation_id, addon)).kept;
        k.sort_by_key(|a| std::cmp::Reverse(a.saved_at));
        k
    }

    /// Download `release` and install it as `entry` (new or existing).
    pub async fn install(
        &self,
        t: &mut Target<'_>,
        entry: AddonEntry,
        release: &Release,
        progress: Progress<'_>,
    ) -> Result<AddonEntry> {
        if release.download_url.is_empty() {
            return Err(Error::Other(format!(
                "{} {} has no download link.",
                entry.name, release.version
            )));
        }
        let tmp = self
            .paths
            .tmp()
            .join(format!("{}.zip", uuid::Uuid::new_v4()));
        let reason = format!("download {} {}", entry.name, release.version);
        let result = async {
            self.fetch
                .download(&release.download_url, &reason, &tmp, progress)
                .await?;
            if let Some(want) = &release.sha256 {
                if !archive::sha256_file(&tmp)?.eq_ignore_ascii_case(want) {
                    return Err(Error::ChecksumMismatch);
                }
            }
            if let Some(want) = &release.md5 {
                if !archive::md5_file(&tmp)?.eq_ignore_ascii_case(want) {
                    return Err(Error::ChecksumMismatch);
                }
            }
            self.apply_archive(t, entry, &tmp, &release.version)
        }
        .await;
        let _ = std::fs::remove_file(&tmp);
        result
    }

    /// Install from an archive already on disk: inspect, stage, back up,
    /// swap, keep the archive, save state.
    pub fn apply_archive(
        &self,
        t: &mut Target<'_>,
        mut entry: AddonEntry,
        zip: &Path,
        version: &str,
    ) -> Result<AddonEntry> {
        let addons = t.addons_dir();
        if !addons.is_dir() {
            return Err(Error::NoInstallation(format!(
                "{} does not exist.",
                addons.display()
            )));
        }
        let new_folders = archive::inspect(zip)?;

        // Never overwrite a folder that belongs to another managed addon.
        // Folders reconcile found but could not match (entries without a
        // source) are taken over: they are backed up and swapped like ours.
        let mut absorbed: Vec<String> = Vec::new();
        for f in &new_folders {
            match t.state.owner_of(f) {
                Some(o) if o.id.eq_ignore_ascii_case(&entry.id) => {}
                Some(o) if !o.is_managed() => absorbed.push(f.clone()),
                Some(o) => {
                    return Err(Error::FolderOwned {
                        folder: f.clone(),
                        owner: o.name.clone(),
                    })
                }
                None if addons.join(f).exists() => return Err(Error::FolderUnmanaged(f.clone())),
                None => {}
            }
        }

        let old = t.state.get(&entry.id).cloned();
        let mut old_folders: Vec<String> =
            old.as_ref().map(|o| o.folders.clone()).unwrap_or_default();
        for f in &absorbed {
            if !old_folders.iter().any(|o| o.eq_ignore_ascii_case(f)) {
                old_folders.push(f.clone());
            }
        }
        old_folders.retain(|f| addons.join(f).is_dir());
        let bdir = self.backup_dir(t, &entry.id);

        // Folders the old version had that the new archive does not ship are
        // removed only if a Nexforge-installed archive put them there. An
        // adopted folder may be a separate addon that reconcile grouped in
        // (Spoken's voice packs declare `X-Part-Of: Spoken`), so it stays on
        // disk and gets its own entry.
        let ours: Vec<String> = old
            .as_ref()
            .and_then(|o| {
                Self::read_index(&bdir)
                    .kept
                    .into_iter()
                    .find(|k| k.installed && k.version == o.installed_version)
            })
            .map(|k| k.folders)
            .unwrap_or_default();
        let has = |list: &[String], f: &str| list.iter().any(|x| x.eq_ignore_ascii_case(f));
        let left_in_place: Vec<String> = old_folders
            .iter()
            .filter(|f| !has(&new_folders, f) && !has(&ours, f))
            .cloned()
            .collect();
        old_folders.retain(|f| !has(&left_in_place, f));

        let backup_version = old
            .as_ref()
            .map(|o| o.installed_version.clone())
            .or_else(|| {
                absorbed
                    .first()
                    .and_then(|f| t.state.owner_of(f))
                    .map(|o| o.installed_version.clone())
            })
            .unwrap_or_default();

        // 3. Stage.
        let op = uuid::Uuid::new_v4().to_string();
        let staging = self.paths.staging().join(&op);
        archive::extract(zip, &staging, &new_folders)?;

        // 5a. Make sure the version being replaced can be restored.
        if !old_folders.is_empty() {
            self.ensure_backup(&bdir, &addons, &backup_version, &old_folders)?;
        }

        // 5b/6. Swap, on the AddOns volume so the final step is a rename.
        let swap = addons
            .parent()
            .unwrap_or(&addons)
            .join(".nexforge-swap")
            .join(&op);
        let swap_result = Self::swap(&addons, &staging, &swap, &old_folders, &new_folders);
        let _ = std::fs::remove_dir_all(&staging);
        swap_result?;
        let _ = std::fs::remove_dir_all(&swap);
        if let Some(parent) = swap.parent() {
            let _ = std::fs::remove_dir(parent); // only if empty
        }

        // Keep this archive: it is the "previous version" next time.
        self.keep_archive(&bdir, zip, version, &new_folders)?;

        // 7. State.
        entry.installed_version = version.to_string();
        entry.installed_at = Some(Utc::now());
        entry.folders = order_folders(new_folders, &entry.name);
        if let Some(o) = &old {
            entry.pinned = o.pinned;
            if entry.ids.is_empty() {
                entry.ids = o.ids.clone();
            }
        }
        match t.state.get_mut(&entry.id) {
            Some(slot) => *slot = entry.clone(),
            None => t.state.addons.push(entry.clone()),
        }
        for other in t.state.addons.iter_mut().filter(|a| !a.is_managed()) {
            other
                .folders
                .retain(|f| !absorbed.iter().any(|x| x.eq_ignore_ascii_case(f)));
        }
        t.state
            .addons
            .retain(|a| a.is_managed() || !a.folders.is_empty());
        for f in &left_in_place {
            let toc = crate::toc::read_folder(&addons.join(f), t.installation.flavour)
                .ok()
                .flatten();
            let name = toc
                .as_ref()
                .and_then(|x| x.toc.title.clone())
                .unwrap_or_else(|| f.clone());
            let lone = AddonEntry {
                id: t.state.unique_id(&name),
                name,
                source: None,
                source_id: String::new(),
                installed_version: toc
                    .as_ref()
                    .and_then(|x| x.toc.version.clone())
                    .unwrap_or_default(),
                installed_at: None,
                folders: vec![f.clone()],
                pinned: false,
                skipped_version: String::new(),
                channel: crate::sources::Channel::Stable,
                author: toc.as_ref().and_then(|x| x.toc.author.clone()),
                flavour_mismatch: toc.as_ref().is_some_and(|x| !x.flavour_match),
                ids: Default::default(),
            };
            t.state.addons.push(lone);
        }
        t.state.addons.sort_by_key(|a| a.name.to_ascii_lowercase());
        t.state.save(t.state_path)?;
        Ok(entry)
    }

    fn swap(
        addons: &Path,
        staging: &Path,
        swap: &Path,
        old_folders: &[String],
        new_folders: &[String],
    ) -> Result<()> {
        let old_aside = swap.join("old");
        let new_ready = swap.join("new");
        // Bring the new folders onto the AddOns volume first (may be a copy).
        for f in new_folders {
            archive::move_dir(&staging.join(f), &new_ready.join(f))?;
        }
        let mut moved_old: Vec<&String> = Vec::new();
        let mut placed: Vec<&String> = Vec::new();
        let restore = |moved_old: &[&String], placed: &[&String]| {
            for f in placed {
                let _ = std::fs::remove_dir_all(addons.join(f));
            }
            for f in moved_old {
                let _ = std::fs::rename(old_aside.join(f), addons.join(f));
            }
        };
        for f in old_folders {
            if let Err(e) = std::fs::create_dir_all(&old_aside)
                .and_then(|_| std::fs::rename(addons.join(f), old_aside.join(f)))
            {
                restore(&moved_old, &placed);
                return Err(Error::io(
                    "move aside",
                    addons.join(f),
                    std::io::Error::new(
                        e.kind(),
                        format!("{e}. If the game is running, close it and try again"),
                    ),
                ));
            }
            moved_old.push(f);
        }
        for f in new_folders {
            if let Err(e) = std::fs::rename(new_ready.join(f), addons.join(f)) {
                restore(&moved_old, &placed);
                return Err(Error::io("place", addons.join(f), e));
            }
            placed.push(f);
        }
        Ok(())
    }

    fn ensure_backup(
        &self,
        bdir: &Path,
        addons: &Path,
        version: &str,
        folders: &[String],
    ) -> Result<()> {
        let mut idx = Self::read_index(bdir);
        if idx
            .kept
            .iter()
            .any(|k| k.version == version && bdir.join(&k.file).exists())
        {
            return Ok(());
        }
        let file = format!(
            "{}.zip",
            sanitize(if version.is_empty() {
                "unknown"
            } else {
                version
            })
        );
        archive::zip_folders(addons, folders, &bdir.join(&file))?;
        idx.kept.retain(|k| k.version != version);
        idx.kept.push(Kept {
            version: version.to_string(),
            file,
            saved_at: Utc::now(),
            folders: folders.to_vec(),
            installed: false,
        });
        Self::write_index(bdir, &idx)
    }

    fn keep_archive(
        &self,
        bdir: &Path,
        zip: &Path,
        version: &str,
        folders: &[String],
    ) -> Result<()> {
        std::fs::create_dir_all(bdir).ctx("create", bdir)?;
        let mut idx = Self::read_index(bdir);
        let file = format!("{}.zip", sanitize(version));
        let dest = bdir.join(&file);
        if !same_file(zip, &dest) {
            std::fs::copy(zip, &dest).ctx("keep", &dest)?;
        }
        idx.kept.retain(|k| k.version != version);
        idx.kept.push(Kept {
            version: version.to_string(),
            file,
            saved_at: Utc::now(),
            folders: folders.to_vec(),
            installed: true,
        });
        // Prune: the installed version plus `keep` previous ones.
        idx.kept.sort_by_key(|a| std::cmp::Reverse(a.saved_at));
        while idx.kept.len() > self.keep + 1 {
            if let Some(k) = idx.kept.pop() {
                let _ = std::fs::remove_file(bdir.join(&k.file));
            }
        }
        Self::write_index(bdir, &idx)
    }

    /// Restore a kept version (default: the newest one that is not installed).
    /// Returns `(from, to)`. The version rolled back from is marked skipped.
    pub fn rollback(
        &self,
        t: &mut Target<'_>,
        id: &str,
        to: Option<&str>,
    ) -> Result<(String, String)> {
        let entry = t
            .state
            .get(id)
            .cloned()
            .ok_or_else(|| Error::UnknownAddon(id.into()))?;
        let bdir = self.backup_dir(t, &entry.id);
        let kept = self.kept_versions(t, &entry.id);
        let pick = match to {
            Some(v) => kept.iter().find(|k| k.version == v).ok_or_else(|| {
                Error::Other(format!(
                    "Version {v} of {} is not kept. Kept: {}.",
                    entry.name,
                    kept.iter()
                        .map(|k| k.version.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            })?,
            None => kept
                .iter()
                .find(|k| k.version != entry.installed_version)
                .ok_or_else(|| Error::NoBackup(entry.name.clone()))?,
        }
        .clone();
        let from = entry.installed_version.clone();
        let mut e = self.apply_archive(t, entry, &bdir.join(&pick.file), &pick.version)?;
        if from != pick.version {
            e.skipped_version = from.clone();
            if let Some(slot) = t.state.get_mut(&e.id) {
                slot.skipped_version = from.clone();
            }
            t.state.save(t.state_path)?;
        }
        Ok((from, pick.version))
    }

    /// Remove an addon's folders (kept as a backup first) and its entry.
    pub fn remove(&self, t: &mut Target<'_>, id: &str) -> Result<AddonEntry> {
        let entry = t
            .state
            .get(id)
            .cloned()
            .ok_or_else(|| Error::UnknownAddon(id.into()))?;
        let addons = t.addons_dir();
        let present: Vec<String> = entry
            .folders
            .iter()
            .filter(|f| addons.join(f).is_dir())
            .cloned()
            .collect();
        if !present.is_empty() {
            let bdir = self.backup_dir(t, &entry.id);
            self.ensure_backup(&bdir, &addons, &entry.installed_version, &present)?;
        }
        for f in &present {
            let p = addons.join(f);
            std::fs::remove_dir_all(&p).ctx("remove", &p)?;
        }
        t.state
            .addons
            .retain(|a| !a.id.eq_ignore_ascii_case(&entry.id));
        t.state.save(t.state_path)?;
        // Remember the entry so the removal can be undone (see `restore`).
        let bdir = self.backup_dir(t, &entry.id);
        std::fs::create_dir_all(&bdir).ctx("create", &bdir)?;
        let body = toml::to_string_pretty(&entry).map_err(|e| Error::Other(e.to_string()))?;
        let p = bdir.join(REMOVED_FILE);
        std::fs::write(&p, body).ctx("write", &p)?;
        Ok(entry)
    }

    /// Undo [`Engine::remove`]: put the addon back from its kept archive,
    /// with its entry as it was (source, pin, skipped version).
    pub fn restore(&self, t: &mut Target<'_>, id: &str) -> Result<AddonEntry> {
        let bdir = self.backup_dir(t, id);
        let p = bdir.join(REMOVED_FILE);
        let text = std::fs::read_to_string(&p).map_err(|_| Error::NoBackup(id.to_string()))?;
        let entry: AddonEntry = toml::from_str(&text).map_err(|e| Error::BadStateFile {
            path: p.clone(),
            message: e.message().to_string(),
        })?;
        if t.state.get(&entry.id).is_some() {
            return Err(Error::Other(format!(
                "{} is already installed.",
                entry.name
            )));
        }
        let kept = self
            .kept_versions(t, &entry.id)
            .into_iter()
            .find(|k| k.version == entry.installed_version)
            .ok_or_else(|| Error::NoBackup(entry.name.clone()))?;
        let (pinned, skipped) = (entry.pinned, entry.skipped_version.clone());
        let mut back = self.apply_archive(t, entry, &bdir.join(&kept.file), &kept.version)?;
        back.pinned = pinned;
        back.skipped_version = skipped;
        if let Some(slot) = t.state.get_mut(&back.id) {
            slot.pinned = back.pinned;
            slot.skipped_version = back.skipped_version.clone();
        }
        t.state.save(t.state_path)?;
        let _ = std::fs::remove_file(&p);
        Ok(back)
    }
}

const REMOVED_FILE: &str = "removed.toml";

fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

/// Main folder first (the one whose name is a prefix of the others), then the rest.
fn order_folders(mut folders: Vec<String>, name: &str) -> Vec<String> {
    folders.sort_by_key(|f| f.to_ascii_lowercase());
    let main = folders
        .iter()
        .position(|f| f.eq_ignore_ascii_case(name))
        .or_else(|| {
            folders.iter().position(|f| {
                let l = f.to_ascii_lowercase();
                folders
                    .iter()
                    .all(|o| o.to_ascii_lowercase().starts_with(&l))
            })
        });
    if let Some(i) = main {
        let m = folders.remove(i);
        folders.insert(0, m);
    }
    folders
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::tests::make_zip;
    use crate::flavour::Flavour;
    use crate::net::FixtureFetch;
    use crate::sources::{Channel, SourceId};
    use crate::state::KnownIds;

    struct Env {
        _dir: tempfile::TempDir,
        inst: Installation,
        state_path: PathBuf,
        engine: Engine,
        zips: PathBuf,
    }

    fn env() -> Env {
        let dir = tempfile::tempdir().unwrap();
        let client = dir.path().join("_forever_");
        std::fs::create_dir_all(client.join("Interface/AddOns/SomeoneElse")).unwrap();
        let inst = Installation {
            id: "forever".into(),
            flavour: Flavour::Forever,
            path: client,
        };
        let zips = dir.path().join("zips");
        std::fs::create_dir_all(&zips).unwrap();
        let engine = Engine::new(
            Arc::new(FixtureFetch::new()),
            AppPaths::at(dir.path().join("data")),
            2,
        );
        Env {
            state_path: inst.state_file(),
            inst,
            engine,
            zips,
            _dir: dir,
        }
    }

    fn entry() -> AddonEntry {
        AddonEntry {
            id: "foo".into(),
            name: "Foo".into(),
            source: Some(SourceId::Github),
            source_id: "o/foo".into(),
            installed_version: String::new(),
            installed_at: None,
            folders: vec![],
            pinned: false,
            skipped_version: String::new(),
            channel: Channel::Stable,
            author: None,
            flavour_mismatch: false,
            ids: KnownIds::default(),
        }
    }

    fn zip(env: &Env, v: &str, extra_folder: bool) -> PathBuf {
        let p = env.zips.join(format!("foo-{v}.zip"));
        let toc = format!("## Title: Foo\n## Version: {v}\n");
        let mut files = vec![("Foo/Foo.toc", toc.as_str()), ("Foo/core.lua", v)];
        if extra_folder {
            files.push(("Foo_Extra/Foo_Extra.toc", "## Title: x"));
        }
        make_zip(&p, &files);
        p
    }

    #[test]
    fn install_update_rollback_remove() {
        let env = env();
        let mut state = AddonsFile::new(Flavour::Forever);
        let addons = env.inst.addons_dir();
        let mut t = Target {
            installation: &env.inst,
            state: &mut state,
            state_path: &env.state_path,
        };

        let e = env
            .engine
            .apply_archive(&mut t, entry(), &zip(&env, "1.0", true), "1.0")
            .unwrap();
        assert_eq!(e.folders, vec!["Foo", "Foo_Extra"]);
        assert!(addons.join("Foo_Extra/Foo_Extra.toc").exists());

        env.engine
            .apply_archive(&mut t, e, &zip(&env, "2.0", false), "2.0")
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(addons.join("Foo/core.lua")).unwrap(),
            "2.0"
        );
        assert!(
            !addons.join("Foo_Extra").exists(),
            "folders the new version dropped are gone"
        );
        assert!(
            addons.join("SomeoneElse").exists(),
            "other folders untouched"
        );
        assert!(
            !addons.parent().unwrap().join(".nexforge-swap").exists(),
            "swap dir cleaned"
        );

        let (from, to) = env.engine.rollback(&mut t, "foo", None).unwrap();
        assert_eq!((from.as_str(), to.as_str()), ("2.0", "1.0"));
        assert_eq!(
            std::fs::read_to_string(addons.join("Foo/core.lua")).unwrap(),
            "1.0"
        );
        assert!(addons.join("Foo_Extra").exists());
        assert_eq!(t.state.get("foo").unwrap().skipped_version, "2.0");

        // On disk state matches.
        let saved = AddonsFile::load_or_new(&env.state_path, Flavour::Forever).unwrap();
        assert_eq!(saved.get("foo").unwrap().installed_version, "1.0");

        // Pruning keeps installed + 2.
        for v in ["3.0", "4.0", "5.0"] {
            let e = t.state.get("foo").unwrap().clone();
            env.engine
                .apply_archive(&mut t, e, &zip(&env, v, false), v)
                .unwrap();
        }
        let kept: Vec<String> = env
            .engine
            .kept_versions(&t, "foo")
            .into_iter()
            .map(|k| k.version)
            .collect();
        assert_eq!(kept, vec!["5.0", "4.0", "3.0"]);

        t.state.require_mut("foo").unwrap().pinned = true;
        env.engine.remove(&mut t, "foo").unwrap();
        assert!(!addons.join("Foo").exists());
        assert!(t.state.get("foo").is_none());

        // Undo the removal: folders and entry come back as they were.
        let back = env.engine.restore(&mut t, "foo").unwrap();
        assert_eq!(back.installed_version, "5.0");
        assert!(back.pinned, "pin survives remove + restore");
        assert_eq!(
            std::fs::read_to_string(addons.join("Foo/core.lua")).unwrap(),
            "5.0"
        );
        assert!(matches!(
            env.engine.restore(&mut t, "foo"),
            Err(Error::NoBackup(_)) | Err(Error::Other(_))
        ));
    }

    #[test]
    fn refuses_foreign_folders() {
        let env = env();
        let mut state = AddonsFile::new(Flavour::Forever);
        let mut t = Target {
            installation: &env.inst,
            state: &mut state,
            state_path: &env.state_path,
        };
        let p = env.zips.join("x.zip");
        make_zip(&p, &[("SomeoneElse/SomeoneElse.toc", "## Title: x")]);
        let err = env
            .engine
            .apply_archive(&mut t, entry(), &p, "1")
            .unwrap_err();
        assert!(matches!(err, Error::FolderUnmanaged(_)), "{err}");

        let mut other = entry();
        other.id = "other".into();
        other.name = "Other".into();
        other.folders = vec!["Foo".into()];
        t.state.addons.push(other);
        let err = env
            .engine
            .apply_archive(&mut t, entry(), &zip(&env, "1", false), "1")
            .unwrap_err();
        assert!(matches!(err, Error::FolderOwned { .. }), "{err}");
        assert!(!env.inst.addons_dir().join("Foo").exists());
    }

    #[test]
    fn absorbs_unmatched_folders() {
        let env = env();
        let addons = env.inst.addons_dir();
        for f in ["Foo", "Foo_Extra"] {
            std::fs::create_dir_all(addons.join(f)).unwrap();
            std::fs::write(addons.join(f).join(format!("{f}.toc")), "## Title: old").unwrap();
        }
        let mut state = AddonsFile::new(Flavour::Forever);
        let mut foo = entry();
        foo.installed_version = "0.9".into();
        foo.folders = vec!["Foo".into()];
        state.addons.push(foo.clone());
        let mut extra = entry();
        extra.id = "foo-extra".into();
        extra.name = "Foo Extra".into();
        extra.source = None;
        extra.source_id = String::new();
        extra.folders = vec!["Foo_Extra".into()];
        state.addons.push(extra);
        let mut t = Target {
            installation: &env.inst,
            state: &mut state,
            state_path: &env.state_path,
        };
        let e = env
            .engine
            .apply_archive(&mut t, foo, &zip(&env, "1.0", true), "1.0")
            .unwrap();
        assert_eq!(e.folders, vec!["Foo", "Foo_Extra"]);
        assert!(
            t.state.get("foo-extra").is_none(),
            "emptied unmatched entry is dropped"
        );
        // The adopted 0.9 folders were kept and can be restored.
        let (_, to) = env.engine.rollback(&mut t, "foo", None).unwrap();
        assert_eq!(to, "0.9");
        assert_eq!(
            std::fs::read_to_string(addons.join("Foo_Extra/Foo_Extra.toc")).unwrap(),
            "## Title: old"
        );
    }

    #[test]
    fn keeps_adopted_folders_the_new_version_does_not_ship() {
        let env = env();
        let addons = env.inst.addons_dir();
        // Reconcile grouped a voice pack into the addon (it declares X-Part-Of).
        for (f, title) in [("Foo", "old"), ("FooVoicePack", "Foo Voice Pack")] {
            std::fs::create_dir_all(addons.join(f)).unwrap();
            std::fs::write(
                addons.join(f).join(format!("{f}.toc")),
                format!("## Title: {title}\n## Version: 0.9\n"),
            )
            .unwrap();
        }
        let mut state = AddonsFile::new(Flavour::Forever);
        let mut foo = entry();
        foo.installed_version = "0.9".into();
        foo.folders = vec!["Foo".into(), "FooVoicePack".into()];
        state.addons.push(foo.clone());
        let mut t = Target {
            installation: &env.inst,
            state: &mut state,
            state_path: &env.state_path,
        };
        let e = env
            .engine
            .apply_archive(&mut t, foo, &zip(&env, "1.0", false), "1.0")
            .unwrap();
        assert_eq!(e.folders, vec!["Foo"]);
        assert!(
            addons.join("FooVoicePack/FooVoicePack.toc").exists(),
            "an adopted folder the update does not ship must not be deleted"
        );
        let lone = t.state.owner_of("FooVoicePack").unwrap();
        assert_eq!(lone.name, "Foo Voice Pack");
        assert!(!lone.is_managed());
    }

    #[tokio::test]
    async fn checksum_mismatch_is_rejected() {
        let env = env();
        let fx = FixtureFetch::new();
        let z = zip(&env, "1.0", false);
        fx.route_file("https://github.com/o/foo/releases/download/1.0/foo.zip", &z);
        let engine = Engine::new(Arc::new(fx), AppPaths::at(env.zips.join("d")), 3);
        let mut state = AddonsFile::new(Flavour::Forever);
        let mut t = Target {
            installation: &env.inst,
            state: &mut state,
            state_path: &env.state_path,
        };
        let mut rel = Release {
            version: "1.0".into(),
            download_url: "https://github.com/o/foo/releases/download/1.0/foo.zip".into(),
            file_name: None,
            published_at: None,
            size: None,
            channel: Channel::Stable,
            flavours: vec![Flavour::Forever],
            changelog: None,
            sha256: Some("00".repeat(32)),
            md5: None,
            file_id: None,
        };
        let err = engine
            .install(&mut t, entry(), &rel, &|_, _| {})
            .await
            .unwrap_err();
        assert!(matches!(err, Error::ChecksumMismatch));
        rel.sha256 = Some(archive::sha256_file(&z).unwrap());
        engine
            .install(&mut t, entry(), &rel, &|_, _| {})
            .await
            .unwrap();
        assert!(env.inst.addons_dir().join("Foo/Foo.toc").exists());
    }
}
