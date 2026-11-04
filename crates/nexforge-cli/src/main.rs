//! `nexforge` — command-line front end over nexforge-core, for testing and scripting.

use anyhow::{anyhow, bail, Context, Result};
use clap::{Parser, Subcommand};
use nexforge_core::installations::{default_roots, detect_in};
use nexforge_core::manager::Status;
use nexforge_core::reconcile::{reconcile, scan};
use nexforge_core::state::AddonsFile;
use nexforge_core::toc::read_folder;
use nexforge_core::{keys, AppPaths, Channel, Flavour, Installation, Manager};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(
    name = "nexforge",
    version,
    about = "A fast, trustworthy WoW addon manager. No account, no telemetry."
)]
struct Cli {
    /// Where Nexforge keeps config, backups and logs (default: %LOCALAPPDATA%\Nexabyte\Nexforge, or NEXFORGE_HOME).
    #[arg(long, global = true, value_name = "DIR")]
    data_dir: Option<PathBuf>,
    /// Installation id from config.toml (e.g. forever).
    #[arg(long = "install", global = true, value_name = "ID")]
    installation: Option<String>,
    /// Work on this Interface\AddOns folder instead of a configured installation.
    #[arg(long, global = true, value_name = "DIR")]
    addons_dir: Option<PathBuf>,
    /// Game flavour for --addons-dir (forever, classic-era, anniversary). Detected if omitted.
    #[arg(long, global = true)]
    flavour: Option<Flavour>,
    /// Read and write this addons.toml instead of the one next to the addons.
    #[arg(long, global = true, value_name = "FILE")]
    state: Option<PathBuf>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Find game installations.
    Detect {
        /// A "World of Warcraft" folder to scan (default: common locations on every drive).
        #[arg(long)]
        root: Vec<PathBuf>,
        /// Save what was found to config.toml.
        #[arg(long)]
        save: bool,
    },
    /// Parse TOC files in one addon folder, or every folder in an AddOns folder.
    Toc {
        path: PathBuf,
    },
    /// Adopt the AddOns folder: group folders into addons and write addons.toml.
    Reconcile {
        /// Show the result without writing addons.toml.
        #[arg(long)]
        dry_run: bool,
    },
    /// Search GitHub (bundled index) and WoWInterface by name.
    Search {
        query: Vec<String>,
    },
    /// List addons in addons.toml.
    List,
    /// Check for updates. Nothing is applied.
    Check,
    /// Update addons (pinned addons are never updated).
    Update {
        ids: Vec<String>,
        #[arg(long)]
        all: bool,
    },
    /// Install from source:id (github:owner/repo, wowi:12345) or a GitHub/WoWInterface address.
    Install {
        spec: String,
        /// Take over an existing entry (e.g. one reconcile could not match).
        #[arg(long = "as", value_name = "ID")]
        adopt: Option<String>,
        #[arg(long, default_value = "stable")]
        channel: Channel,
        /// Install even if no build is listed for this flavour.
        #[arg(long)]
        anyway: bool,
    },
    /// Remove an addon's folders (a backup is kept) and its entry.
    Remove {
        id: String,
    },
    /// Never update this addon.
    Pin {
        id: String,
    },
    Unpin {
        id: String,
    },
    /// Skip a version (default: the newest available).
    Skip {
        id: String,
        version: Option<String>,
    },
    /// Restore a kept previous version.
    Rollback {
        id: String,
        #[arg(long)]
        to: Option<String>,
    },
    /// List kept versions of an addon.
    Kept {
        id: String,
    },
    /// Store, clear or list keys in the OS credential store (github token, curseforge, wago).
    Key {
        #[arg(value_parser = ["set", "clear", "status"])]
        action: String,
        #[arg(value_parser = ["github", "wago", "curseforge"])]
        name: Option<String>,
    },
}

struct Ctx {
    mgr: Manager,
    inst: Installation,
    state_path: PathBuf,
}

impl Ctx {
    fn load_state(&self) -> Result<AddonsFile> {
        Ok(AddonsFile::load_or_new(
            &self.state_path,
            self.inst.flavour,
        )?)
    }
}

fn paths(cli: &Cli) -> Result<AppPaths> {
    Ok(match &cli.data_dir {
        Some(d) => AppPaths::at(d),
        None => AppPaths::discover()?,
    })
}

fn context(cli: &Cli) -> Result<Ctx> {
    let mgr = Manager::open(paths(cli)?)?;
    let inst = if let Some(dir) = &cli.addons_dir {
        Installation::from_addons_dir(dir, cli.flavour)?
    } else {
        let cfg = &mgr.config.installations;
        let pick = match &cli.installation {
            Some(id) => cfg
                .iter()
                .find(|i| i.id.eq_ignore_ascii_case(id))
                .ok_or_else(|| anyhow!("No installation called \"{id}\" in config.toml. Run `nexforge detect --save`."))?,
            None => cfg
                .iter()
                .find(|i| i.flavour == Flavour::Forever)
                .or_else(|| cfg.first())
                .ok_or_else(|| anyhow!("No game installation is set up yet. Run `nexforge detect --save`, or pass --addons-dir."))?,
        };
        Installation::from_config(pick)
    };
    let state_path = cli.state.clone().unwrap_or_else(|| inst.state_file());
    Ok(Ctx {
        mgr,
        inst,
        state_path,
    })
}

fn progress_bar(label: String) -> impl Fn(u64, Option<u64>) + Send + Sync {
    let last = std::sync::atomic::AtomicU64::new(u64::MAX);
    move |done, total| {
        // Redraw only when the shown number changes (percent, or 256 KB steps).
        let (shown, text) = match total {
            Some(t) if t > 0 => (done * 100 / t, format!("{:>3}%", done * 100 / t)),
            _ => (done / (256 * 1024), format!("{} KB", done / 1024)),
        };
        if last.swap(shown, std::sync::atomic::Ordering::Relaxed) == shown {
            return;
        }
        let mut err = std::io::stderr();
        let _ = write!(err, "\r  {label}  {text}  ");
        let _ = err.flush();
    }
}

fn end_progress() {
    eprint!("\r{:60}\r", "");
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli).await {
        eprintln!("error: {e:#}");
        // The core's wording points at the app's Settings; say the CLI way too.
        let key_problem = e.chain().any(|c| {
            matches!(
                c.downcast_ref::<nexforge_core::Error>(),
                Some(
                    nexforge_core::Error::CurseForgeKeyNeeded
                        | nexforge_core::Error::CurseForgeKeyRejected
                )
            )
        });
        if key_problem {
            eprintln!("hint: run `nexforge key set curseforge` to save a key.");
        }
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<()> {
    match &cli.cmd {
        Cmd::Detect { root, save } => detect(&cli, root, *save),
        Cmd::Toc { path } => toc(path, cli.flavour),
        Cmd::Key { action, name } => key(&cli, action, name.as_deref()),
        Cmd::Search { query } => {
            let mgr = Manager::open(paths(&cli)?)?;
            let flavour = match &cli.flavour {
                Some(f) => *f,
                None => context(&cli)
                    .map(|c| c.inst.flavour)
                    .unwrap_or(Flavour::Forever),
            };
            search(&mgr, &query.join(" "), flavour).await
        }
        _ => {
            let ctx = context(&cli)?;
            dispatch(&cli.cmd, ctx).await
        }
    }
}

fn detect(cli: &Cli, roots: &[PathBuf], save: bool) -> Result<()> {
    let roots = if roots.is_empty() {
        default_roots()
    } else {
        roots.to_vec()
    };
    if roots.is_empty() {
        bail!("No World of Warcraft folder found in the usual places. Pass --root <folder>.");
    }
    let mut found = Vec::new();
    for r in &roots {
        let (f, skipped) = detect_in(r);
        println!("{}", r.display());
        for i in &f {
            println!(
                "  {:<14} {:<12} {}",
                i.id,
                i.flavour.label(),
                i.path.display()
            );
        }
        for s in skipped {
            println!(
                "  {:<14} skipped: {}",
                s.path.file_name().unwrap_or_default().to_string_lossy(),
                s.reason
            );
        }
        found.extend(f);
    }
    if save {
        let mut mgr = Manager::open(paths(cli)?)?;
        for i in found {
            if !mgr.config.installations.iter().any(|c| c.id == i.id) {
                mgr.config.installations.push(i.to_config());
            }
        }
        mgr.save_config()?;
        println!("Saved to {}", mgr.paths.config_file().display());
    }
    Ok(())
}

fn ids_line(t: &nexforge_core::toc::Toc) -> String {
    let mut v = Vec::new();
    if let Some(x) = t.github_repo() {
        v.push(format!("gh:{x}"));
    }
    if let Some(x) = &t.wago_id {
        v.push(format!("wago:{x}"));
    }
    if let Some(x) = &t.wowi_id {
        v.push(format!("wowi:{x}"));
    }
    if let Some(x) = &t.curse_id {
        v.push(format!("cf:{x}"));
    }
    v.join(" ")
}

fn toc(path: &Path, flavour: Option<Flavour>) -> Result<()> {
    let guess = || {
        // …\_forever_\Interface\AddOns[\Folder]
        path.ancestors()
            .find_map(nexforge_core::installations::detect_flavour)
    };
    let flavour = flavour.or_else(guess).unwrap_or(Flavour::Forever);
    if let Some(f) = read_folder(path, flavour)? {
        let t = &f.toc;
        println!(
            "{} ({} for {}{})",
            f.name,
            f.toc_file,
            flavour.label(),
            if f.flavour_match {
                ""
            } else {
                ", NOT loaded on this flavour"
            }
        );
        println!("  Title:     {}", t.title.as_deref().unwrap_or("-"));
        println!("  Version:   {}", t.version.as_deref().unwrap_or("-"));
        println!("  Author:    {}", t.author.as_deref().unwrap_or("-"));
        println!(
            "  Interface: {:?} {:?}",
            t.interface,
            t.interface_flavours()
        );
        println!("  Deps:      {}", t.dependencies.join(", "));
        println!("  IDs:       {}", ids_line(t));
        if let Some(p) = &t.part_of {
            println!("  Part of:   {p}");
        }
        return Ok(());
    }
    let s = scan(path, flavour).with_context(|| format!("reading {}", path.display()))?;
    println!(
        "{} folders in {} (flavour {}):\n",
        s.folders.len(),
        path.display(),
        flavour.label()
    );
    println!(
        "{:<34} {:<38} {:<3} {:<14} IDs",
        "Folder", "TOC", "OK", "Version"
    );
    for f in &s.folders {
        println!(
            "{:<34} {:<38} {:<3} {:<14} {}",
            f.name,
            f.toc_file,
            if f.flavour_match { "yes" } else { "no" },
            f.toc.version.as_deref().unwrap_or("-"),
            ids_line(&f.toc)
        );
    }
    for (n, why) in &s.ignored {
        println!("{n:<34} ignored: {why}");
    }
    Ok(())
}

async fn search(mgr: &Manager, q: &str, flavour: Flavour) -> Result<()> {
    if q.trim().is_empty() {
        bail!("Say what to search for, e.g. `nexforge search questie`.");
    }
    let (hits, errors) = mgr.search(q, flavour).await;
    for (s, e) in &errors {
        println!("{}: {e}", s.label());
    }
    if hits.is_empty() {
        println!("Nothing found for \"{q}\".");
        return Ok(());
    }
    println!("{:<40} {:<44} Flavours", "Name", "Install with");
    for h in hits.iter().take(40) {
        let fl = if h.flavours.is_empty() {
            "see releases".to_string()
        } else if h.flavours.contains(&flavour) {
            h.flavours
                .iter()
                .map(|f| f.short_label())
                .collect::<Vec<_>>()
                .join(", ")
        } else {
            format!("no {} build listed", flavour.short_label())
        };
        println!(
            "{:<40} {:<44} {}",
            h.name,
            format!("{}:{}", h.source, h.source_id),
            fl
        );
    }
    Ok(())
}

fn key(cli: &Cli, action: &str, name: Option<&str>) -> Result<()> {
    if action == "status" {
        // Says whether each key is stored; never prints a value.
        for n in keys::KNOWN {
            let state = match keys::get(n) {
                Ok(Some(_)) => "saved",
                Ok(None) => "not set",
                Err(_) => "could not read the credential store",
            };
            println!("{n:<12} {state}");
        }
        return Ok(());
    }
    let Some(name) = name else {
        bail!("Name the key: github, curseforge or wago.");
    };
    // Saving or clearing the CurseForge key turns the source on or off.
    let set_curseforge = |on: bool| -> Result<()> {
        if name == "curseforge" {
            let mut mgr = Manager::open(paths(cli)?)?;
            mgr.config.sources.curseforge = on;
            mgr.save_config()?;
        }
        Ok(())
    };
    if action == "clear" {
        keys::delete(name)?;
        set_curseforge(false)?;
        println!("Cleared nexforge/{name}.");
        return Ok(());
    }
    eprint!("Paste the {name} key and press Enter: ");
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line)?;
    if line.trim().is_empty() {
        bail!("Nothing entered; no key saved.");
    }
    keys::set(name, &line)?;
    set_curseforge(true)?;
    println!("Saved to the system credential store as nexforge/{name}.");
    if name == "curseforge" {
        println!("CurseForge is now on. `nexforge reconcile` will use it for addons only CurseForge has.");
    }
    Ok(())
}

fn status_text(s: &Status, flavour: Flavour) -> (String, String) {
    match s {
        Status::UpToDate => ("".into(), "Up to date".into()),
        Status::Available(r) => (
            r.version.clone(),
            if r.flavour_unknown() {
                "Update (flavour not stated)".into()
            } else {
                "Update".into()
            },
        ),
        Status::Pinned(Some(r)) => (r.version.clone(), "Pinned".into()),
        Status::Pinned(None) => ("".into(), "Pinned".into()),
        Status::Skipped(r) => (r.version.clone(), "Skipped".into()),
        Status::NoFlavourBuild => ("".into(), format!("No {} build", flavour.short_label())),
        Status::NotManaged => ("".into(), "Identify source".into()),
        Status::SourceUnavailable(s) => ("".into(), format!("{} not available yet", s.label())),
        Status::Failed(e) => ("".into(), format!("Failed: {e}")),
    }
}

async fn dispatch(cmd: &Cmd, ctx: Ctx) -> Result<()> {
    let flavour = ctx.inst.flavour;
    match cmd {
        Cmd::Reconcile { dry_run } => {
            let addons = ctx.inst.addons_dir();
            let existing = ctx.load_state()?;
            let r = reconcile(&addons, flavour, &existing, &ctx.mgr.available_sources())?;
            let managed = r.file.addons.iter().filter(|a| a.is_managed()).count();
            println!(
                "{} addons in {} ({} folders): {} managed ({} already, {} new), {} not matched to a source yet.",
                r.file.addons.len(),
                addons.display(),
                r.file.addons.iter().map(|a| a.folders.len()).sum::<usize>(),
                managed,
                r.kept.len(),
                r.adopted.len(),
                r.unmanaged.len()
            );
            for a in &r.file.addons {
                let src = match a.source {
                    Some(s) => format!("{}:{}", s, a.source_id),
                    None => "— identify source".into(),
                };
                println!(
                    "  {:<34} {:<14} {:>2} folder{}  {}{}",
                    a.name,
                    a.installed_version,
                    a.folders.len(),
                    if a.folders.len() == 1 { " " } else { "s" },
                    src,
                    if a.flavour_mismatch {
                        format!("  (no {} TOC)", flavour.short_label())
                    } else {
                        String::new()
                    }
                );
            }
            for (id, folders) in &r.missing {
                println!(
                    "  missing: {id} has no folder {} any more",
                    folders.join(", ")
                );
            }
            for h in &r.hints {
                println!("  hint: {h}");
            }
            if *dry_run {
                println!("Dry run; {} not written.", ctx.state_path.display());
            } else {
                r.file.save(&ctx.state_path)?;
                println!("Wrote {}", ctx.state_path.display());
            }
        }
        Cmd::List => {
            let s = ctx.load_state()?;
            if s.addons.is_empty() {
                println!(
                    "No addons in {}. Run `nexforge reconcile` first.",
                    ctx.state_path.display()
                );
            }
            for a in &s.addons {
                println!(
                    "{:<30} {:<34} {:<14} {:<36} {}{}",
                    a.id,
                    a.name,
                    a.installed_version,
                    a.source
                        .map(|s| format!("{}:{}", s, a.source_id))
                        .unwrap_or_else(|| "-".into()),
                    if a.pinned { "pinned " } else { "" },
                    if a.skipped_version.is_empty() {
                        String::new()
                    } else {
                        format!("skip {}", a.skipped_version)
                    },
                );
            }
        }
        Cmd::Check => {
            let s = ctx.load_state()?;
            let report = ctx.mgr.check(flavour, &s).await;
            let mut n = 0;
            for r in &report.results {
                let a = s.get(&r.id).expect("from state");
                let (latest, status) = status_text(&r.status, flavour);
                if matches!(r.status, Status::Available(_)) {
                    n += 1;
                }
                let arrow = if latest.is_empty() {
                    String::new()
                } else {
                    format!("→ {latest}")
                };
                println!(
                    "{:<34} {:<14} {:<26} {}",
                    a.name, a.installed_version, arrow, status
                );
            }
            for (src, e) in &report.source_errors {
                println!("{}: {e}", src.label());
            }
            println!(
                "\n{n} update{} available. Nothing is applied until you run `nexforge update`.",
                if n == 1 { "" } else { "s" }
            );
        }
        Cmd::Update { ids, all } => {
            let mut s = ctx.load_state()?;
            let targets: Vec<String> = if *all {
                let report = ctx.mgr.check(flavour, &s).await;
                for (src, e) in &report.source_errors {
                    println!("{}: {e}", src.label());
                }
                report.available().map(|(id, _)| id.to_string()).collect()
            } else if ids.is_empty() {
                bail!("Name the addons to update, or pass --all.");
            } else {
                ids.clone()
            };
            if targets.is_empty() {
                println!("Everything is up to date.");
            }
            let mut failed = 0;
            for id in targets {
                let label = s.get(&id).map(|a| a.name.clone()).unwrap_or(id.clone());
                let bar = progress_bar(label.clone());
                match ctx
                    .mgr
                    .update(&ctx.inst, &mut s, &ctx.state_path, &id, &bar)
                    .await
                {
                    Ok(Some((from, e))) => {
                        end_progress();
                        println!("Updated {} {} → {}", e.name, from, e.installed_version);
                    }
                    Ok(None) => {
                        end_progress();
                        println!("{label} is already up to date.");
                    }
                    Err(e) => {
                        end_progress();
                        failed += 1;
                        println!("{label}: {e}");
                    }
                }
            }
            if failed > 0 {
                bail!(
                    "{failed} update{} failed.",
                    if failed == 1 { "" } else { "s" }
                );
            }
        }
        Cmd::Install {
            spec,
            adopt,
            channel,
            anyway,
        } => {
            let mut s = ctx.load_state()?;
            let bar = progress_bar(spec.clone());
            let e = ctx
                .mgr
                .install(
                    &ctx.inst,
                    &mut s,
                    &ctx.state_path,
                    spec,
                    adopt.as_deref(),
                    *channel,
                    *anyway,
                    &bar,
                )
                .await;
            end_progress();
            let e = e?;
            println!(
                "Installed {} {} ({})",
                e.name,
                e.installed_version,
                e.folders.join(", ")
            );
            if e.flavour_mismatch {
                println!(
                    "Note: this release does not say it supports {}.",
                    flavour.label()
                );
            }
        }
        Cmd::Remove { id } => {
            let mut s = ctx.load_state()?;
            let mut t = nexforge_core::install::Target {
                installation: &ctx.inst,
                state: &mut s,
                state_path: &ctx.state_path,
            };
            let e = ctx.mgr.engine().remove(&mut t, id)?;
            println!(
                "Removed {} ({}). A copy is kept; `nexforge kept {}` lists it.",
                e.name,
                e.folders.join(", "),
                e.id
            );
        }
        Cmd::Pin { id } | Cmd::Unpin { id } => {
            let pin = matches!(cmd, Cmd::Pin { .. });
            let mut s = ctx.load_state()?;
            let e = s.require_mut(id)?;
            e.pinned = pin;
            let name = e.name.clone();
            s.save(&ctx.state_path)?;
            println!(
                "{name} {}",
                if pin {
                    "is pinned and will not be updated."
                } else {
                    "is no longer pinned."
                }
            );
        }
        Cmd::Skip { id, version } => {
            let mut s = ctx.load_state()?;
            let entry = s
                .get(id)
                .cloned()
                .ok_or_else(|| anyhow!("No addon called \"{id}\" in addons.toml."))?;
            let v = match version {
                Some(v) => v.clone(),
                None => ctx.mgr.pick(&entry, flavour, false).await?.0.version,
            };
            s.require_mut(id)?.skipped_version = v.clone();
            s.save(&ctx.state_path)?;
            println!("Skipping {} {v}.", entry.name);
        }
        Cmd::Rollback { id, to } => {
            let mut s = ctx.load_state()?;
            let mut t = nexforge_core::install::Target {
                installation: &ctx.inst,
                state: &mut s,
                state_path: &ctx.state_path,
            };
            let (from, to) = ctx.mgr.engine().rollback(&mut t, id, to.as_deref())?;
            println!("Rolled back {id} {from} → {to}. {from} is marked skipped.");
        }
        Cmd::Kept { id } => {
            let mut s = ctx.load_state()?;
            let installed = s
                .get(id)
                .map(|a| a.installed_version.clone())
                .unwrap_or_default();
            let t = nexforge_core::install::Target {
                installation: &ctx.inst,
                state: &mut s,
                state_path: &ctx.state_path,
            };
            let kept = ctx.mgr.engine().kept_versions(&t, id);
            if kept.is_empty() {
                println!("No versions of {id} are kept yet.");
            }
            for k in kept {
                println!(
                    "{:<20} {}  {}{}",
                    k.version,
                    k.saved_at.format("%Y-%m-%d %H:%M"),
                    k.folders.join(", "),
                    if k.version == installed {
                        "  (installed)"
                    } else {
                        ""
                    }
                );
            }
        }
        Cmd::Detect { .. } | Cmd::Toc { .. } | Cmd::Key { .. } | Cmd::Search { .. } => {
            unreachable!()
        }
    }
    Ok(())
}
