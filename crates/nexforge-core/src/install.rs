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
    /// [`archive::fingerprint`] of the folders as they were on disk: when
    /// the folders no longer match, they were edited and get their own backup.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
}

/// Written before any folder moves, so an interrupted swap can be undone
/// (see [`Engine::recover`]).
#[derive(Debug, Default, Serialize, Deserialize)]
struct SwapManifest {
    #[serde(default)]
    old: Vec<String>,
    #[serde(default)]
    new: Vec<String>,
}

const MANIFEST: &str = "manifest.toml";

/// The swap area for one operation, on the AddOns volume.
fn swap_dir(addons: &Path, op: &str) -> PathBuf {
    addons
        .parent()
        .unwrap_or(addons)
        .join(".nexforge-swap")
        .join(op)
}

fn link_error(f: &str) -> Error {
    Error::Other(format!(
        "{f} is a link to a folder elsewhere, so Nexforge won't change it. Remove the link, then try again."
    ))
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
        Engine {
            fetch,
            paths,
            keep: keep.max(1),
        }
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
        // Finish anything an earlier crash left half done (the caller holds
        // the state lock, so no other operation is running).
        self.recover_dir(&addons);
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
        for f in old_folders.iter().chain(&new_folders) {
            if archive::is_link(&addons.join(f)) {
                return Err(link_error(f));
            }
        }

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

        // 5c. Keep this archive before touching AddOns: it is the "previous
        // version" next time, and a full data drive fails here, not mid-swap.
        let kept_new = match self.keep_archive(&bdir, zip, version, &new_folders) {
            Ok(n) => n,
            Err(e) => {
                let _ = std::fs::remove_dir_all(&staging);
                return Err(e);
            }
        };

        // 5b/6. Swap, on the AddOns volume so the final step is a rename.
        let swap = swap_dir(&addons, &op);
        let swap_result = Self::swap(&addons, &staging, &swap, &old_folders, &new_folders);
        let _ = std::fs::remove_dir_all(&staging);
        if let Err(e) = swap_result {
            if kept_new {
                self.forget(&bdir, version);
            }
            return Err(e);
        }
        let _ = std::fs::remove_dir_all(&swap);
        if let Some(parent) = swap.parent() {
            let _ = std::fs::remove_dir(parent); // only if empty
        }
        self.set_fingerprint(&bdir, version, archive::fingerprint(&addons, &new_folders));

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
        // Say what is about to move, so a crash part-way can be undone.
        write_manifest(swap, old_folders, new_folders)?;
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
        // Skip only when a kept copy of this version matches what is on disk
        // now: folders edited by hand (or swapped for a different build with
        // the same version) get a backup of their own.
        let fp = archive::fingerprint(addons, folders);
        if idx.kept.iter().any(|k| {
            k.version == version
                && k.fingerprint.as_deref() == Some(fp.as_str())
                && bdir.join(&k.file).exists()
        }) {
            return Ok(());
        }
        let base = sanitize(if version.is_empty() {
            "unknown"
        } else {
            version
        });
        let plain = format!("{base}.zip");
        let file = if bdir.join(&plain).exists() {
            format!("{base}-local-{}.zip", Utc::now().format("%Y%m%d%H%M%S%3f"))
        } else {
            plain
        };
        archive::zip_folders(addons, folders, &bdir.join(&file))?;
        idx.kept.push(Kept {
            version: version.to_string(),
            file,
            saved_at: Utc::now(),
            folders: folders.to_vec(),
            installed: false,
            fingerprint: Some(fp),
        });
        idx.kept.sort_by_key(|a| std::cmp::Reverse(a.saved_at));
        Self::write_index(bdir, &idx)
    }

    /// Keep `zip` as the archive of `version`. Returns true when this
    /// version was not kept before (so a failed swap can [`Self::forget`] it).
    fn keep_archive(
        &self,
        bdir: &Path,
        zip: &Path,
        version: &str,
        folders: &[String],
    ) -> Result<bool> {
        std::fs::create_dir_all(bdir).ctx("create", bdir)?;
        let mut idx = Self::read_index(bdir);
        let file = format!("{}.zip", sanitize(version));
        let dest = bdir.join(&file);
        let new = !idx.kept.iter().any(|k| k.file == file);
        if !same_file(zip, &dest) {
            std::fs::copy(zip, &dest).ctx("keep", &dest)?;
        }
        idx.kept.retain(|k| k.file != file);
        idx.kept.push(Kept {
            version: version.to_string(),
            file,
            saved_at: Utc::now(),
            folders: folders.to_vec(),
            installed: true,
            fingerprint: None,
        });
        // Prune: the installed version plus `keep` previous ones.
        idx.kept.sort_by_key(|a| std::cmp::Reverse(a.saved_at));
        while idx.kept.len() > self.keep + 1 {
            if let Some(k) = idx.kept.pop() {
                let _ = std::fs::remove_file(bdir.join(&k.file));
            }
        }
        Self::write_index(bdir, &idx)?;
        Ok(new)
    }

    /// Drop a kept archive (a swap that failed after it was kept).
    fn forget(&self, bdir: &Path, version: &str) {
        let mut idx = Self::read_index(bdir);
        let file = format!("{}.zip", sanitize(version));
        idx.kept.retain(|k| k.file != file);
        let _ = std::fs::remove_file(bdir.join(&file));
        let _ = Self::write_index(bdir, &idx);
    }

    /// Record what the installed folders look like, for [`Self::ensure_backup`].
    fn set_fingerprint(&self, bdir: &Path, version: &str, fp: String) {
        let mut idx = Self::read_index(bdir);
        let file = format!("{}.zip", sanitize(version));
        if let Some(k) = idx.kept.iter_mut().find(|k| k.file == file) {
            k.fingerprint = Some(fp);
            let _ = Self::write_index(bdir, &idx);
        }
    }

    /// Undo whatever a crash or power cut interrupted in this installation:
    /// an update half swapped is put back to the version `addons.toml`
    /// records, and folders a removal had moved aside are returned. Only
    /// touches `Interface/.nexforge-swap` and the folders its manifests name.
    /// Call with the state lock held. Returns the folders put back.
    pub fn recover(&self, installation: &Installation) -> Vec<String> {
        self.recover_dir(&installation.addons_dir())
    }

    fn recover_dir(&self, addons: &Path) -> Vec<String> {
        let root = swap_dir(addons, "x");
        let Some(root) = root.parent().map(Path::to_path_buf) else {
            return vec![];
        };
        let Ok(ops) = std::fs::read_dir(&root) else {
            return vec![];
        };
        let mut restored = Vec::new();
        for op in ops.flatten().map(|e| e.path()).filter(|p| p.is_dir()) {
            let aside = op.join("old");
            let manifest: Option<SwapManifest> = std::fs::read_to_string(op.join(MANIFEST))
                .ok()
                .and_then(|t| toml::from_str(&t).ok());
            let has = |list: &[String], f: &str| list.iter().any(|x| x.eq_ignore_ascii_case(f));
            if let Some(m) = &manifest {
                // Folders the interrupted update had already placed go; the
                // old ones come back below. A folder only the new version
                // has was checked absent before the swap, so it is ours.
                for f in m.new.iter().filter(|f| crate::state::valid_folder_name(f)) {
                    let placed = addons.join(f);
                    let ours = !has(&m.old, f) || aside.join(f).is_dir();
                    if ours && placed.is_dir() && !archive::is_link(&placed) {
                        let _ = std::fs::remove_dir_all(&placed);
                    }
                }
            }
            let mut left = false;
            if let Ok(rd) = std::fs::read_dir(&aside) {
                for e in rd.flatten() {
                    let name = e.file_name().to_string_lossy().to_string();
                    let back = addons.join(&name);
                    if !crate::state::valid_folder_name(&name) || back.exists() {
                        left = true;
                        continue;
                    }
                    if std::fs::rename(e.path(), &back).is_ok() {
                        restored.push(name);
                    } else {
                        left = true;
                    }
                }
            }
            if !left {
                let _ = std::fs::remove_dir_all(&op);
            }
        }
        let _ = std::fs::remove_dir(&root); // only if empty
        restored
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
        self.recover_dir(&addons);
        if let Some(f) = present.iter().find(|f| archive::is_link(&addons.join(f))) {
            return Err(link_error(f));
        }
        let bdir = self.backup_dir(t, &entry.id);
        if !present.is_empty() {
            self.ensure_backup(&bdir, &addons, &entry.installed_version, &present)?;
        }
        // Move every folder aside first: if one is locked (the game is
        // running), the ones already moved go back and nothing changes.
        let swap = swap_dir(&addons, &uuid::Uuid::new_v4().to_string());
        let aside = swap.join("old");
        write_manifest(&swap, &present, &[])?;
        let mut moved: Vec<&String> = Vec::new();
        let undo = |moved: &[&String]| {
            for f in moved {
                let _ = std::fs::rename(aside.join(f), addons.join(f));
            }
            let _ = std::fs::remove_dir_all(&swap);
        };
        for f in &present {
            if let Err(e) = std::fs::create_dir_all(&aside)
                .and_then(|_| std::fs::rename(addons.join(f), aside.join(f)))
            {
                undo(&moved);
                return Err(Error::io(
                    "remove",
                    addons.join(f),
                    std::io::Error::new(
                        e.kind(),
                        format!("{e}. If the game is running, close it and try again"),
                    ),
                ));
            }
            moved.push(f);
        }
        // Remember the entry so the removal can be undone (see `restore`),
        // then forget it; only after both are saved do the folders go.
        let remembered = (|| -> Result<()> {
            std::fs::create_dir_all(&bdir).ctx("create", &bdir)?;
            let body = toml::to_string_pretty(&entry).map_err(|e| Error::Other(e.to_string()))?;
            let p = bdir.join(REMOVED_FILE);
            std::fs::write(&p, body).ctx("write", &p)?;
            let mut next = t.state.clone();
            next.addons
                .retain(|a| !a.id.eq_ignore_ascii_case(&entry.id));
            next.save(t.state_path)?;
            *t.state = next;
            Ok(())
        })();
        if let Err(e) = remembered {
            undo(&moved);
            let _ = std::fs::remove_file(bdir.join(REMOVED_FILE));
            return Err(e);
        }
        let _ = std::fs::remove_dir_all(&swap);
        if let Some(parent) = swap.parent() {
            let _ = std::fs::remove_dir(parent); // only if empty
        }
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

fn write_manifest(swap: &Path, old: &[String], new: &[String]) -> Result<()> {
    std::fs::create_dir_all(swap).ctx("create", swap)?;
    let m = SwapManifest {
        old: old.to_vec(),
        new: new.to_vec(),
    };
    let body = toml::to_string(&m).map_err(|e| Error::Other(e.to_string()))?;
    let p = swap.join(MANIFEST);
    std::fs::write(&p, body).ctx("write", &p)
}

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

    fn target<'a>(env: &'a Env, state: &'a mut AddonsFile) -> Target<'a> {
        Target {
            installation: &env.inst,
            state,
            state_path: &env.state_path,
        }
    }

    #[test]
    fn keep_zero_still_keeps_the_previous_version() {
        let mut env = env();
        env.engine = Engine::new(
            Arc::new(FixtureFetch::new()),
            AppPaths::at(env.zips.join("data0")),
            0,
        );
        let mut state = AddonsFile::new(Flavour::Forever);
        let mut t = target(&env, &mut state);
        env.engine
            .apply_archive(&mut t, entry(), &zip(&env, "1.0", false), "1.0")
            .unwrap();
        let e = t.state.get("foo").unwrap().clone();
        env.engine
            .apply_archive(&mut t, e, &zip(&env, "2.0", false), "2.0")
            .unwrap();
        let (_, to) = env.engine.rollback(&mut t, "foo", None).unwrap();
        assert_eq!(to, "1.0");
    }

    #[test]
    fn hand_edited_folders_get_their_own_backup() {
        let env = env();
        let addons = env.inst.addons_dir();
        let mut state = AddonsFile::new(Flavour::Forever);
        let mut t = target(&env, &mut state);
        env.engine
            .apply_archive(&mut t, entry(), &zip(&env, "1.0", false), "1.0")
            .unwrap();
        // The player edits the installed copy, then updates.
        std::fs::write(addons.join("Foo/core.lua"), "edited").unwrap();
        let e = t.state.get("foo").unwrap().clone();
        env.engine
            .apply_archive(&mut t, e, &zip(&env, "2.0", false), "2.0")
            .unwrap();
        let kept = env.engine.kept_versions(&t, "foo");
        assert!(
            kept.iter().any(|k| k.file.starts_with("1.0-local-")),
            "the edited 1.0 is kept apart from the pristine one: {kept:?}"
        );
        env.engine.rollback(&mut t, "foo", None).unwrap();
        assert_eq!(
            std::fs::read_to_string(addons.join("Foo/core.lua")).unwrap(),
            "edited",
            "rolling back restores the player's edits"
        );
    }

    #[test]
    fn an_interrupted_swap_is_put_back() {
        let env = env();
        let addons = env.inst.addons_dir();
        // A crash mid-update: old Foo moved aside, new Foo and Foo_Extra placed.
        let op = swap_dir(&addons, "op1");
        std::fs::create_dir_all(op.join("old/Foo")).unwrap();
        std::fs::write(op.join("old/Foo/core.lua"), "old").unwrap();
        write_manifest(&op, &["Foo".into()], &["Foo".into(), "Foo_Extra".into()]).unwrap();
        for f in ["Foo", "Foo_Extra"] {
            std::fs::create_dir_all(addons.join(f)).unwrap();
            std::fs::write(addons.join(f).join("core.lua"), "new").unwrap();
        }

        assert_eq!(env.engine.recover(&env.inst), vec!["Foo"]);
        assert_eq!(
            std::fs::read_to_string(addons.join("Foo/core.lua")).unwrap(),
            "old"
        );
        assert!(
            !addons.join("Foo_Extra").exists(),
            "half-placed folder gone"
        );
        assert!(addons.join("SomeoneElse").exists(), "others untouched");
        assert!(!op.parent().unwrap().exists(), "swap area cleaned");
        assert!(env.engine.recover(&env.inst).is_empty(), "nothing twice");
    }

    #[test]
    fn remove_leaves_nothing_behind() {
        let env = env();
        let addons = env.inst.addons_dir();
        let mut state = AddonsFile::new(Flavour::Forever);
        let mut t = target(&env, &mut state);
        env.engine
            .apply_archive(&mut t, entry(), &zip(&env, "1.0", true), "1.0")
            .unwrap();
        env.engine.remove(&mut t, "foo").unwrap();
        assert!(!addons.join("Foo").exists() && !addons.join("Foo_Extra").exists());
        assert!(!swap_dir(&addons, "x").parent().unwrap().exists());
        let saved = AddonsFile::load_or_new(&env.state_path, Flavour::Forever).unwrap();
        assert!(saved.get("foo").is_none(), "removal is saved");
    }
}
