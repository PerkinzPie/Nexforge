//! Nexforge desktop app: Tauri commands over `nexforge-core`.
//!
//! The webview never talks to the network. Every request goes through the
//! core (allowlist, network log); icons come back as `data:` URLs.

mod dto;

use dto::*;
use nexforge_core::install::Target;
use nexforge_core::installations::{default_roots, detect_in};
use nexforge_core::reconcile::reconcile;
use nexforge_core::state::AddonsFile;
use nexforge_core::{keys, AppPaths, Channel, Error, Installation, Manager, SourceId};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

/// Serialises everything that writes an `addons.toml` or touches AddOns.
#[derive(Default)]
struct Writes(tokio::sync::Mutex<()>);

type Res<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn manager() -> Res<Manager> {
    Manager::open(AppPaths::discover().map_err(err)?).map_err(err)
}

/// Make sure config has the installations on this machine (first run).
fn ensure_installations(mgr: &mut Manager) -> Res<()> {
    if !mgr.config.installations.is_empty() {
        return Ok(());
    }
    for root in default_roots() {
        for i in detect_in(&root).0 {
            if !mgr.config.installations.iter().any(|c| c.id == i.id) {
                mgr.config.installations.push(i.to_config());
            }
        }
    }
    if !mgr.config.installations.is_empty() {
        mgr.save_config().map_err(err)?;
    }
    Ok(())
}

fn installation(mgr: &Manager, id: &str) -> Res<Installation> {
    mgr.config
        .installations
        .iter()
        .find(|c| c.id == id)
        .map(Installation::from_config)
        .ok_or_else(|| format!("No installation called \"{id}\". Add it in Settings."))
}

fn load_state(inst: &Installation) -> Res<AddonsFile> {
    AddonsFile::load_or_new(&inst.state_file(), inst.flavour).map_err(err)
}

/// Everything the UI needs to paint, from local files only.
#[tauri::command]
async fn snapshot(installation_id: Option<String>, writes: State<'_, Writes>) -> Res<Snapshot> {
    let mut mgr = manager()?;
    // First run: no config yet. The UI shows the welcome screen, which
    // adopts every client (see `adopt_all`).
    let first_run = !mgr.paths.config_file().exists();
    ensure_installations(&mut mgr)?;
    let installs: Vec<InstallationDto> = mgr
        .config
        .installations
        .iter()
        .map(|c| InstallationDto::from(&Installation::from_config(c)))
        .collect();
    let current = installation_id
        .filter(|id| installs.iter().any(|i| &i.id == id))
        .or_else(|| {
            installs
                .iter()
                .find(|i| i.flavour == "forever")
                .or(installs.first())
                .map(|i| i.id.clone())
        });
    let Some(current) = current else {
        let mut s = Snapshot::empty(installs, settings_dto(&mgr));
        s.first_run = first_run;
        return Ok(s);
    };
    let inst = installation(&mgr, &current)?;
    let mut adopted_now = false;
    // First open of an installation: adopt its AddOns folder. Reconcile only
    // writes addons.toml; it never moves or deletes anything. On first run
    // the welcome screen does this for every client instead.
    if !first_run && !inst.state_file().exists() && inst.addons_dir().is_dir() {
        let _w = writes.0.lock().await;
        if !inst.state_file().exists() {
            let r = reconcile(
                &inst.addons_dir(),
                inst.flavour,
                &AddonsFile::new(inst.flavour),
                &mgr.available_sources(),
            )
            .map_err(err)?;
            r.file.save(&inst.state_file()).map_err(err)?;
            adopted_now = true;
        }
    }
    let state = load_state(&inst)?;
    let engine = mgr.engine();
    let mut last_run: Option<LastRun> = None;
    let addons: Vec<AddonDto> = state
        .addons
        .iter()
        .map(|a| {
            let kept = engine.kept_for(&inst.id, &a.id);
            if let Some(newest) = kept
                .first()
                .filter(|k| k.version == a.installed_version && kept.len() > 1)
            {
                let at = newest.saved_at;
                match &mut last_run {
                    Some(l) if l.at.date_naive() == at.date_naive() => {
                        l.count += 1;
                        l.at = l.at.max(at);
                    }
                    Some(l) if l.at >= at => {}
                    _ => last_run = Some(LastRun { at, count: 1 }),
                }
            }
            let mut dto = AddonDto::from(a);
            for k in engine.kept_for(&inst.id, &a.id) {
                if k.version != a.installed_version && !dto.kept.contains(&k.version) {
                    dto.kept.push(k.version);
                }
            }
            if dto.icon_url.is_none() {
                dto.icon_url = a
                    .source
                    .and_then(|s| mgr.icon_for(s, &a.source_id))
                    .or_else(|| {
                        [SourceId::Wowinterface, SourceId::Curseforge]
                            .into_iter()
                            .find_map(|s| mgr.icon_for(s, a.ids.get(s)?))
                    });
            }
            dto
        })
        .collect();
    Ok(Snapshot {
        installations: installs,
        current: Some(current),
        flavour: inst.flavour.as_str().into(),
        flavour_label: inst.flavour.label().into(),
        flavour_short: inst.flavour.short_label().into(),
        addons,
        state_exists: inst.state_file().exists(),
        addons_dir_exists: inst.addons_dir().is_dir(),
        adopted_now,
        first_run,
        last_run,
        settings: settings_dto(&mgr),
        sources_on: mgr
            .available_sources()
            .iter()
            .map(|s| s.as_str().to_string())
            .collect(),
    })
}

/// First run: adopt every client's AddOns folder that has no addons.toml yet.
/// Reconcile only writes addons.toml; nothing in AddOns is moved or deleted.
#[tauri::command]
async fn adopt_all(writes: State<'_, Writes>) -> Res<Vec<AdoptedDto>> {
    let mut mgr = manager()?;
    ensure_installations(&mut mgr)?;
    let _w = writes.0.lock().await;
    let mut out = Vec::new();
    for c in &mgr.config.installations {
        let inst = Installation::from_config(c);
        if !inst.addons_dir().is_dir() {
            out.push(AdoptedDto {
                installation: InstallationDto::from(&inst),
                addons: 0,
                managed: 0,
            });
            continue;
        }
        let existing = load_state(&inst)?;
        let file = if inst.state_file().exists() {
            existing
        } else {
            let r = reconcile(
                &inst.addons_dir(),
                inst.flavour,
                &existing,
                &mgr.available_sources(),
            )
            .map_err(err)?;
            r.file.save(&inst.state_file()).map_err(err)?;
            r.file
        };
        out.push(AdoptedDto {
            installation: InstallationDto::from(&inst),
            addons: file.addons.len(),
            managed: file.addons.iter().filter(|a| a.is_managed()).count(),
        });
    }
    Ok(out)
}

/// Re-adopt the AddOns folder (keeps managed entries as they are).
#[tauri::command]
/// Look at the AddOns folder again (addons copied in by hand, or deleted).
/// Writes addons.toml only when something changed; returns how many addons
/// are new. Managed entries are never altered, and AddOns is never touched.
async fn reconcile_now(installation_id: String, writes: State<'_, Writes>) -> Res<usize> {
    let mgr = manager()?;
    let inst = installation(&mgr, &installation_id)?;
    if !inst.addons_dir().is_dir() {
        return Ok(0);
    }
    let _w = writes.0.lock().await;
    let existing = load_state(&inst)?;
    let r = reconcile(
        &inst.addons_dir(),
        inst.flavour,
        &existing,
        &mgr.available_sources(),
    )
    .map_err(err)?;
    let shape = |f: &AddonsFile| {
        let mut v: Vec<(String, Vec<String>)> = f
            .addons
            .iter()
            .map(|a| (a.id.clone(), a.folders.clone()))
            .collect();
        v.sort();
        v
    };
    if shape(&r.file) == shape(&existing) {
        return Ok(0);
    }
    let new = r
        .file
        .addons
        .iter()
        .filter(|a| existing.get(&a.id).is_none())
        .count();
    r.file.save(&inst.state_file()).map_err(err)?;
    Ok(new)
}

#[tauri::command]
async fn check(installation_id: String) -> Res<CheckDto> {
    let mgr = manager()?;
    let inst = installation(&mgr, &installation_id)?;
    let state = load_state(&inst)?;
    let report = mgr.check(inst.flavour, &state).await;
    Ok(CheckDto {
        checked_at: chrono::Utc::now(),
        results: report
            .results
            .iter()
            .map(|r| StatusDto::from_status(&r.id, &r.status))
            .collect(),
        source_errors: report
            .source_errors
            .iter()
            .map(|(s, e)| SourceErrorDto {
                source: s.as_str().into(),
                source_label: s.label().into(),
                message: e.clone(),
            })
            .collect(),
    })
}

/// A progress callback that emits `progress` events when the percent changes.
fn progress(app: &AppHandle, id: &str) -> impl Fn(u64, Option<u64>) + Send + Sync {
    let app = app.clone();
    let id = id.to_string();
    let last = AtomicU64::new(u64::MAX);
    move |done, total| {
        let pct = total.filter(|t| *t > 0).map(|t| done * 100 / t);
        let key = pct.unwrap_or(done / (256 * 1024));
        if last.swap(key, Ordering::Relaxed) != key {
            let _ = app.emit(
                "progress",
                ProgressDto {
                    id: id.clone(),
                    done,
                    total,
                    percent: pct.map(|p| p as u8),
                },
            );
        }
    }
}

#[tauri::command]
async fn update(
    app: AppHandle,
    installation_id: String,
    id: String,
    writes: State<'_, Writes>,
) -> Res<Option<AddonDto>> {
    let mgr = manager()?;
    let inst = installation(&mgr, &installation_id)?;
    let _w = writes.0.lock().await;
    let mut state = load_state(&inst)?;
    let cb = progress(&app, &id);
    let r = mgr
        .update(&inst, &mut state, &inst.state_file(), &id, &cb)
        .await
        .map_err(err)?;
    Ok(r.map(|(_, e)| AddonDto::from(&e)))
}

#[tauri::command]
async fn install(
    app: AppHandle,
    installation_id: String,
    spec: String,
    adopt: Option<String>,
    anyway: bool,
    writes: State<'_, Writes>,
) -> Res<AddonDto> {
    let mgr = manager()?;
    let inst = installation(&mgr, &installation_id)?;
    let _w = writes.0.lock().await;
    let mut state = load_state(&inst)?;
    let cb = progress(&app, adopt.as_deref().unwrap_or(&spec));
    let e = mgr
        .install(
            &inst,
            &mut state,
            &inst.state_file(),
            &spec,
            adopt.as_deref(),
            Channel::Stable,
            anyway,
            &cb,
        )
        .await
        .map_err(err)?;
    Ok(AddonDto::from(&e))
}

#[tauri::command]
async fn remove(installation_id: String, id: String, writes: State<'_, Writes>) -> Res<()> {
    let mgr = manager()?;
    let inst = installation(&mgr, &installation_id)?;
    let _w = writes.0.lock().await;
    let mut state = load_state(&inst)?;
    let path = inst.state_file();
    let mut t = Target {
        installation: &inst,
        state: &mut state,
        state_path: &path,
    };
    mgr.engine().remove(&mut t, &id).map_err(err)?;
    Ok(())
}

/// Undo a removal: reinstall from the kept archive with the old entry.
#[tauri::command]
async fn restore(installation_id: String, id: String, writes: State<'_, Writes>) -> Res<AddonDto> {
    let mgr = manager()?;
    let inst = installation(&mgr, &installation_id)?;
    let _w = writes.0.lock().await;
    let mut state = load_state(&inst)?;
    let path = inst.state_file();
    let mut t = Target {
        installation: &inst,
        state: &mut state,
        state_path: &path,
    };
    let e = mgr.engine().restore(&mut t, &id).map_err(err)?;
    Ok(AddonDto::from(&e))
}

#[tauri::command]
async fn set_pinned(
    installation_id: String,
    id: String,
    pinned: bool,
    writes: State<'_, Writes>,
) -> Res<()> {
    let mgr = manager()?;
    let inst = installation(&mgr, &installation_id)?;
    let _w = writes.0.lock().await;
    let mut state = load_state(&inst)?;
    state.require_mut(&id).map_err(err)?.pinned = pinned;
    state.save(&inst.state_file()).map_err(err)
}

#[tauri::command]
async fn skip(
    installation_id: String,
    id: String,
    version: String,
    writes: State<'_, Writes>,
) -> Res<()> {
    let mgr = manager()?;
    let inst = installation(&mgr, &installation_id)?;
    let _w = writes.0.lock().await;
    let mut state = load_state(&inst)?;
    state.require_mut(&id).map_err(err)?.skipped_version = version;
    state.save(&inst.state_file()).map_err(err)
}

#[tauri::command]
async fn rollback(
    installation_id: String,
    id: String,
    to: Option<String>,
    writes: State<'_, Writes>,
) -> Res<RollbackDto> {
    let mgr = manager()?;
    let inst = installation(&mgr, &installation_id)?;
    let _w = writes.0.lock().await;
    let mut state = load_state(&inst)?;
    let path = inst.state_file();
    let mut t = Target {
        installation: &inst,
        state: &mut state,
        state_path: &path,
    };
    let (from, to) = mgr
        .engine()
        .rollback(&mut t, &id, to.as_deref())
        .map_err(err)?;
    Ok(RollbackDto { from, to })
}

/// Addons updated by Nexforge that can be rolled back, newest first.
#[tauri::command]
async fn recent(installation_id: String) -> Res<Vec<RecentDto>> {
    let mgr = manager()?;
    let inst = installation(&mgr, &installation_id)?;
    let state = load_state(&inst)?;
    let engine = mgr.engine();
    let mut out: Vec<RecentDto> = state
        .addons
        .iter()
        .filter_map(|a| {
            let kept = engine.kept_for(&inst.id, &a.id);
            let current = kept.first().filter(|k| k.version == a.installed_version)?;
            let previous = kept.iter().find(|k| k.version != a.installed_version)?;
            Some(RecentDto {
                id: a.id.clone(),
                name: a.name.clone(),
                source: a.source.map(|s| s.as_str().to_string()),
                from: previous.version.clone(),
                to: a.installed_version.clone(),
                at: current.saved_at,
            })
        })
        .collect();
    out.sort_by_key(|r| std::cmp::Reverse(r.at));
    out.truncate(10);
    Ok(out)
}

#[tauri::command]
async fn search(installation_id: Option<String>, query: String) -> Res<SearchDto> {
    let mgr = manager()?;
    let (flavour, state) = match installation_id
        .as_deref()
        .map(|id| installation(&mgr, id))
        .transpose()?
    {
        Some(inst) => (inst.flavour, load_state(&inst).ok()),
        None => (nexforge_core::Flavour::Forever, None),
    };
    let (hits, errors) = mgr.search(&query, flavour).await;
    let installed = |source: SourceId, id: &str, name: &str| {
        state.as_ref().is_some_and(|s| {
            s.addons.iter().any(|a| {
                (a.source == Some(source) && a.source_id == id)
                    || a.ids.get(source) == Some(id)
                    || a.name.eq_ignore_ascii_case(name)
            })
        })
    };
    // One row per addon: the same name on several hosts shows "also on …".
    let mut rows: Vec<HitDto> = Vec::new();
    let mut by_name: HashMap<String, usize> = HashMap::new();
    for h in hits {
        let key: String = h
            .name
            .chars()
            .filter(|c| c.is_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect();
        if let Some(&i) = by_name.get(&key) {
            let label = h.source.label().to_string();
            let row = &mut rows[i];
            if row.source != h.source.as_str() && !row.also_on.contains(&label) {
                row.also_on.push(label);
            }
            // Prefer the addon's own image (WoWInterface, CurseForge) over a
            // GitHub owner avatar, and fill gaps from the other hosts.
            if h.icon_url.is_some() && (row.icon_url.is_none() || row.source == "github") {
                row.icon_url = h.icon_url.clone();
            }
            if row.summary.is_none() {
                row.summary = h.summary.clone();
            }
            if row.downloads.is_none() {
                row.downloads = h.downloads;
            }
            if row.updated_at.is_none() {
                row.updated_at = h.updated_at;
            }
            continue;
        }
        by_name.insert(key, rows.len());
        let is_installed = installed(h.source, &h.source_id, &h.name);
        rows.push(HitDto::new(h, flavour, is_installed));
    }
    rows.truncate(60);
    Ok(SearchDto {
        hits: rows,
        errors: errors
            .into_iter()
            .map(|(s, e)| SourceErrorDto {
                source: s.as_str().into(),
                source_label: s.label().into(),
                message: e,
            })
            .collect(),
    })
}

/// Release notes for an addon's pending update (fetched on demand: bulk
/// update checks do not carry them for WoWInterface or CurseForge).
#[tauri::command]
async fn release_notes(installation_id: String, id: String) -> Res<NotesDto> {
    let mgr = manager()?;
    let inst = installation(&mgr, &installation_id)?;
    let state = load_state(&inst)?;
    let entry = state
        .get(&id)
        .cloned()
        .ok_or_else(|| err(Error::UnknownAddon(id.clone())))?;
    let text = mgr.notes(&entry, inst.flavour).await.map_err(err)?;
    let lines = dto::note_lines(text.as_deref(), 3);
    let has_more = dto::note_lines(text.as_deref(), 4).len() > lines.len();
    Ok(NotesDto { lines, has_more })
}

/// Opt-in check for a newer Nexforge. `None` when off, current, or offline.
#[tauri::command]
async fn self_update() -> Res<Option<NewVersionDto>> {
    let mgr = manager()?;
    if !mgr.config.self_update_check {
        return Ok(None);
    }
    Ok(mgr
        .newer_nexforge(env!("CARGO_PKG_VERSION"))
        .await
        .ok()
        .flatten()
        .map(|n| NewVersionDto {
            version: n.version,
            url: n.url,
        }))
}

/// Refill the Browse catalogue (at most daily unless `force`).
#[tauri::command]
async fn refresh_catalogue(force: bool) -> Res<CatalogueDto> {
    let mgr = manager()?;
    let errors = mgr.refresh_catalogue(force).await;
    let s = mgr.catalogue_stats();
    Ok(CatalogueDto {
        total: s.total,
        refreshed_at: s.refreshed_at,
        errors: errors
            .into_iter()
            .map(|(src, e)| SourceErrorDto {
                source: src.as_str().into(),
                source_label: src.label().into(),
                message: e,
            })
            .collect(),
    })
}

/// Most downloaded this month for the installation's flavour, or with
/// `source`, the most downloaded from that host.
#[tauri::command]
async fn popular(installation_id: Option<String>, source: Option<String>) -> Res<Vec<HitDto>> {
    let mgr = manager()?;
    let (flavour, state) = match installation_id
        .as_deref()
        .map(|id| installation(&mgr, id))
        .transpose()?
    {
        Some(inst) => (inst.flavour, load_state(&inst).ok()),
        None => (nexforge_core::Flavour::Forever, None),
    };
    let rows = match source.as_deref().map(str::parse::<SourceId>) {
        Some(Ok(s)) => mgr.top_of(s, 50),
        Some(Err(e)) => return Err(e),
        None => mgr.popular(flavour, 6),
    };
    Ok(rows
        .into_iter()
        .map(|h| {
            let installed = state.as_ref().is_some_and(|s| {
                s.addons.iter().any(|a| {
                    (a.source == Some(h.source) && a.source_id == h.source_id)
                        || a.ids.get(h.source) == Some(h.source_id.as_str())
                })
            });
            HitDto::new(h, flavour, installed)
        })
        .collect())
}

/// An icon as a `data:` URL, via the core's image-only allowlist and cache.
#[tauri::command]
async fn image(url: String) -> Res<String> {
    use base64::Engine as _;
    // Images need no keys or config: skip the credential store reads.
    let paths = AppPaths::discover().map_err(err)?;
    let net = nexforge_core::net::Net::new(Some(paths.network_log()), Some(paths.http_cache()))
        .map_err(err)?;
    let mgr = Manager::with_fetch(
        paths,
        nexforge_core::state::Config::default(),
        std::sync::Arc::new(net),
        nexforge_core::SourceKeys::default(),
    );
    let img = mgr.image(&url).await.map_err(err)?;
    Ok(format!(
        "data:{};base64,{}",
        img.content_type,
        base64::engine::general_purpose::STANDARD.encode(&img.bytes)
    ))
}

/// The game client's own icon, read from its `Wow.exe` / `WowClassic.exe`
/// on this machine and returned as an `.ico` data URL. Nothing is bundled or
/// downloaded; the result is cached per client build. `None` if the client
/// has no readable icon (the UI then shows a letter tile).
#[tauri::command]
async fn client_icon(installation_id: String) -> Res<Option<String>> {
    use base64::Engine as _;
    let paths = AppPaths::discover().map_err(err)?;
    let config =
        nexforge_core::state::Config::load_or_default(&paths.config_file()).map_err(err)?;
    let Some(c) = config
        .installations
        .iter()
        .find(|c| c.id == installation_id)
    else {
        return Ok(None);
    };
    let Some(exe) = ["Wow.exe", "WowClassic.exe"]
        .iter()
        .map(|n| c.path.join(n))
        .find(|p| p.is_file())
    else {
        return Ok(None);
    };
    let stamp = std::fs::metadata(&exe)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let cache = paths.image_cache().join(format!(
        "client-{}-{stamp}.ico",
        nexforge_core::paths::sanitize(&c.id)
    ));
    let bytes = match std::fs::read(&cache) {
        Ok(b) => Some(b),
        Err(_) => {
            let exe2 = exe.clone();
            let ico = tokio::task::spawn_blocking(move || exe_icon(&exe2))
                .await
                .map_err(err)?;
            if let Some(b) = &ico {
                if let Some(dir) = cache.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                let _ = std::fs::write(&cache, b);
            }
            ico
        }
    };
    Ok(bytes.map(|b| {
        format!(
            "data:image/x-icon;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(b)
        )
    }))
}

/// First icon group in a Windows executable, written out as an `.ico` file.
fn exe_icon(path: &std::path::Path) -> Option<Vec<u8>> {
    let map = pelite::FileMap::open(path).ok()?;
    let mut out = Vec::new();
    if let Ok(file) = pelite::pe64::PeFile::from_bytes(&map) {
        use pelite::pe64::Pe;
        let res = file.resources().ok()?;
        let (_, group) = res.icons().filter_map(Result::ok).next()?;
        group.write(&mut out).ok()?;
    } else {
        use pelite::pe32::Pe;
        let file = pelite::pe32::PeFile::from_bytes(&map).ok()?;
        let res = file.resources().ok()?;
        let (_, group) = res.icons().filter_map(Result::ok).next()?;
        group.write(&mut out).ok()?;
    }
    (!out.is_empty()).then_some(out)
}

/// Website for an addon's page on its host (opened in the player's browser).
#[tauri::command]
async fn addon_info(source: String, source_id: String) -> Res<InfoDto> {
    let mgr = manager()?;
    let s: SourceId = source.parse().map_err(err)?;
    let src = mgr
        .source(s)
        .ok_or_else(|| format!("{} is not available.", s.label()))?;
    let info = src.resolve(&source_id).await.map_err(err)?;
    Ok(InfoDto {
        name: info.name,
        authors: info.authors,
        icon_url: info.icon_url,
        summary: info.summary,
        url: info.url,
    })
}

/// The details view: the host's description, screenshots and the last ten
/// releases. Releases failing to load still shows the rest.
#[tauri::command]
async fn details(source: String, source_id: String) -> Res<DetailsDto> {
    let mgr = manager()?;
    let s: SourceId = source.parse().map_err(err)?;
    let src = mgr
        .source(s)
        .ok_or_else(|| format!("{} is not available.", s.label()))?;
    let (info, releases) = tokio::join!(src.resolve(&source_id), src.all_releases(&source_id));
    let info = info.map_err(err)?;
    let mut releases = releases.unwrap_or_default();
    releases.sort_by_key(|r| std::cmp::Reverse(r.published_at));
    Ok(DetailsDto {
        about: paragraphs(info.description.as_deref().or(info.summary.as_deref()), 12),
        name: info.name,
        authors: info.authors,
        icon_url: info.icon_url,
        screenshot_thumbs: info.screenshot_thumbs.into_iter().take(12).collect(),
        screenshots: info.screenshots.into_iter().take(12).collect(),
        summary: info.summary,
        url: info.url,
        releases: releases.iter().take(10).map(HistoryDto::from).collect(),
    })
}

#[tauri::command]
fn settings() -> Res<SettingsDto> {
    Ok(settings_dto(&manager()?))
}

fn settings_dto(mgr: &Manager) -> SettingsDto {
    let has = |n: &str| keys::get(n).ok().flatten().is_some();
    SettingsDto {
        check_on_launch: mgr.config.check_on_launch,
        auto_apply: mgr.config.auto_apply,
        keep_previous_versions: mgr.config.keep_previous_versions,
        self_update_check: mgr.config.self_update_check,
        github: mgr.config.sources.github,
        wowinterface: mgr.config.sources.wowinterface,
        curseforge: mgr.config.sources.curseforge,
        key_github: has("github"),
        key_curseforge: has("curseforge"),
        key_wago: has("wago"),
        data_dir: mgr.paths.root().display().to_string(),
        network_log: mgr.paths.network_log().display().to_string(),
        version: env!("CARGO_PKG_VERSION").into(),
    }
}

#[tauri::command]
fn save_settings(s: SettingsInput) -> Res<SettingsDto> {
    let mut mgr = manager()?;
    let c = &mut mgr.config;
    c.check_on_launch = s.check_on_launch;
    c.auto_apply = s.auto_apply;
    c.keep_previous_versions = s.keep_previous_versions.min(20);
    c.self_update_check = s.self_update_check;
    c.sources.github = s.github;
    c.sources.wowinterface = s.wowinterface;
    // CurseForge can only be on with a key saved.
    c.sources.curseforge = s.curseforge && keys::get("curseforge").ok().flatten().is_some();
    mgr.save_config().map_err(err)?;
    Ok(settings_dto(&mgr))
}

/// Store a key in the OS credential store. Never written to a file or log.
#[tauri::command]
fn set_key(name: String, value: String) -> Res<SettingsDto> {
    if !keys::KNOWN.contains(&name.as_str()) {
        return Err(format!("Unknown key \"{name}\"."));
    }
    if value.trim().is_empty() {
        return Err("Paste a key first.".into());
    }
    keys::set(&name, &value).map_err(err)?;
    let mut mgr = manager()?;
    if name == "curseforge" {
        mgr.config.sources.curseforge = true;
        mgr.save_config().map_err(err)?;
    }
    Ok(settings_dto(&mgr))
}

#[tauri::command]
fn clear_key(name: String) -> Res<SettingsDto> {
    keys::delete(&name).map_err(err)?;
    let mut mgr = manager()?;
    if name == "curseforge" {
        mgr.config.sources.curseforge = false;
        mgr.save_config().map_err(err)?;
    }
    Ok(settings_dto(&mgr))
}

/// Open the network log, the data folder, or an addon's folder.
#[tauri::command]
fn open(
    app: AppHandle,
    what: String,
    installation_id: Option<String>,
    id: Option<String>,
) -> Res<()> {
    let mgr = manager()?;
    let path: PathBuf = match what.as_str() {
        "log" => {
            let p = mgr.paths.network_log();
            if !p.exists() {
                if let Some(d) = p.parent() {
                    std::fs::create_dir_all(d).map_err(err)?;
                }
                std::fs::write(&p, "").map_err(err)?;
            }
            p
        }
        "data" => {
            std::fs::create_dir_all(mgr.paths.root()).map_err(err)?;
            mgr.paths.root().to_path_buf()
        }
        "addons" => {
            installation(&mgr, installation_id.as_deref().unwrap_or_default())?.addons_dir()
        }
        "addon" => {
            let inst = installation(&mgr, installation_id.as_deref().unwrap_or_default())?;
            let state = load_state(&inst)?;
            let a = state
                .get(id.as_deref().unwrap_or_default())
                .ok_or_else(|| err(Error::UnknownAddon(id.unwrap_or_default())))?;
            inst.addons_dir()
                .join(a.folders.first().cloned().unwrap_or_default())
        }
        other => return Err(format!("Cannot open \"{other}\".")),
    };
    app.opener()
        .open_path(path.display().to_string(), None::<&str>)
        .map_err(err)
}

/// Open an addon's page on its host in the player's browser.
#[tauri::command]
fn open_url(app: AppHandle, url: String) -> Res<()> {
    let ok = url.starts_with("https://github.com/")
        || url.starts_with("https://www.wowinterface.com/")
        || url.starts_with("https://www.curseforge.com/")
        || url.starts_with("https://addons.wago.io/");
    if !ok {
        return Err("Nexforge only opens addon pages on its known hosts.".into());
    }
    app.opener().open_url(url, None::<&str>).map_err(err)
}

/// Pick a WoW client folder (`_forever_`, `_classic_era_` …) and add it.
#[tauri::command]
async fn add_installation(app: AppHandle, replace: Option<String>) -> Res<Option<InstallationDto>> {
    let Some(picked) = app
        .dialog()
        .file()
        .set_title("Choose a WoW client folder such as _forever_")
        .blocking_pick_folder()
    else {
        return Ok(None);
    };
    let path = picked.into_path().map_err(err)?;
    let inst = Installation::from_client_dir(&path)
        .or_else(|_| Installation::from_addons_dir(&path, None))
        .map_err(err)?;
    let mut mgr = manager()?;
    let cfg = &mut mgr.config.installations;
    if let Some(old) = replace {
        cfg.retain(|c| c.id != old);
    }
    cfg.retain(|c| c.id != inst.id);
    cfg.push(inst.to_config());
    mgr.save_config().map_err(err)?;
    Ok(Some(InstallationDto::from(&inst)))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(Writes::default())
        .invoke_handler(tauri::generate_handler![
            snapshot,
            adopt_all,
            reconcile_now,
            check,
            update,
            install,
            remove,
            restore,
            set_pinned,
            skip,
            rollback,
            recent,
            search,
            image,
            client_icon,
            refresh_catalogue,
            popular,
            release_notes,
            self_update,
            addon_info,
            details,
            settings,
            save_settings,
            set_key,
            clear_key,
            open,
            open_url,
            add_installation,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Nexforge");
}

#[cfg(test)]
mod tests {
    /// Runs only where a WoW client is installed (the author's machine).
    #[test]
    fn reads_the_icon_from_a_local_client() {
        let root = std::path::Path::new(r"E:\Program Files (x86)\World of Warcraft");
        for (dir, exe) in [
            ("_forever_", "Wow.exe"),
            ("_classic_era_", "WowClassic.exe"),
        ] {
            let p = root.join(dir).join(exe);
            if !p.is_file() {
                continue;
            }
            let ico = super::exe_icon(&p).expect("icon");
            assert_eq!(&ico[..4], &[0, 0, 1, 0], "{dir}: ICO header");
            let images = u16::from_le_bytes([ico[4], ico[5]]);
            assert!(images >= 1, "{dir}: no images");
            eprintln!(
                "{dir}: {} bytes, {images} sizes, largest {}px",
                ico.len(),
                (0..images as usize)
                    .map(|i| {
                        let w = ico[6 + i * 16];
                        if w == 0 {
                            256
                        } else {
                            w as u32
                        }
                    })
                    .max()
                    .unwrap()
            );
        }
    }
}
