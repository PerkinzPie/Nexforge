//! Phase 1 acceptance, offline: reconcile an AddOns folder, get a sensible
//! addons.toml, update one GitHub addon, roll it back. Host responses are the
//! recorded fixtures; the download is a synthetic zip served in their place.

use nexforge_core::net::FixtureFetch;
use nexforge_core::reconcile::reconcile;
use nexforge_core::state::{AddonsFile, Config};
use nexforge_core::{AppPaths, Installation, Manager, SourceId, Status};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn fx(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(rel)
}

const RELEASES: &str =
    "https://api.github.com/repos/BigWigsMods/BigWigs_Classic/releases?per_page=10";
const ZIP_URL: &str = "https://github.com/BigWigsMods/BigWigs_Classic/releases/download/v12.1.2/BigWigs_Classic-v12.1.2.zip";

fn make_zip(version: &str) -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut buf);
        let o = zip::write::SimpleFileOptions::default();
        z.start_file("BigWigs_Classic/BigWigs_Classic_Vanilla.toc", o)
            .unwrap();
        write!(z, "## Interface: 11508\n## Title: BigWigs [|cffeda55fClassic|r]\n## Version: {version}\n## Dependencies: BigWigs\n## X-Website: https://github.com/BigWigsMods/BigWigs_Classic\n## X-WoWI-ID: 22095\n").unwrap();
        z.start_file("BigWigs_Classic/Naxxramas/Kel.lua", o)
            .unwrap();
        write!(z, "-- {version}").unwrap();
        z.finish().unwrap();
    }
    buf.into_inner()
}

fn setup_addons(root: &Path) -> Installation {
    let client = root.join("World of Warcraft").join("_classic_era_");
    let addons = client.join("Interface").join("AddOns");
    std::fs::create_dir_all(&addons).unwrap();
    std::fs::write(
        client.join(".flavor.info"),
        "Product Flavor!STRING:0\nwow_classic_era\n",
    )
    .unwrap();
    for folder in [
        "BigWigs_Classic",
        "Questie",
        "ElvUI",
        "ElvUI_Libraries",
        "Details_TinyThreat",
    ] {
        let src = fx(&format!("toc/{folder}"));
        let dst = addons.join(folder);
        std::fs::create_dir_all(&dst).unwrap();
        for e in std::fs::read_dir(src).unwrap().flatten() {
            std::fs::copy(e.path(), dst.join(e.file_name())).unwrap();
        }
    }
    std::fs::create_dir_all(addons.join("Blizzard_Foo")).unwrap();
    std::fs::create_dir_all(addons.join("BigWigs_Classic/Naxxramas")).unwrap();
    std::fs::write(
        addons.join("BigWigs_Classic/Naxxramas/Kel.lua"),
        "-- v12.0.10",
    )
    .unwrap();
    std::fs::create_dir_all(root.join("WTF")).unwrap();
    Installation::from_addons_dir(&addons, None).unwrap()
}

#[tokio::test]
async fn reconcile_update_rollback() {
    let tmp = tempfile::tempdir().unwrap();
    let inst = setup_addons(tmp.path());
    assert_eq!(
        inst.flavour,
        nexforge_core::Flavour::ClassicEra,
        "flavour from .flavor.info"
    );
    let addons = inst.addons_dir();
    let state_path = inst.state_file();

    // Serve the recorded GitHub responses, with the asset digest pointed at our zip.
    let zip = make_zip("v12.1.2");
    let digest: String = Sha256::digest(&zip)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let releases = std::fs::read_to_string(fx("github/bigwigs_classic_releases.json"))
        .unwrap()
        .replace(
            "sha256:fd6576bae7538ce69562bf19f2e9f7cae7a8146653a51310a410bcd5191ae9c1",
            &format!("sha256:{digest}"),
        );
    let v: serde_json::Value = serde_json::from_str(&releases).unwrap();
    let rj_url = v[0]["assets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["name"] == "release.json")
        .unwrap()["browser_download_url"]
        .as_str()
        .unwrap()
        .to_string();
    let f = FixtureFetch::new();
    f.route(RELEASES, releases);
    f.route_file(&rj_url, fx("github/bigwigs_classic_release.json"));
    f.route(ZIP_URL, zip);
    let q_releases = std::fs::read_to_string(fx("github/questie_releases.json")).unwrap();
    let qv: serde_json::Value = serde_json::from_str(&q_releases).unwrap();
    let q_rj = qv[0]["assets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["name"] == "release.json")
        .unwrap()["browser_download_url"]
        .as_str()
        .unwrap()
        .to_string();
    f.route(
        "https://api.github.com/repos/Questie/Questie/releases?per_page=10",
        q_releases,
    );
    f.route_file(&q_rj, fx("github/questie_release.json"));
    f.route_file(
        "https://api.mmoui.com/v3/game/WOW/filelist.json",
        fx("wowinterface/filelist.json"),
    );

    let mgr = Manager::with_fetch(
        AppPaths::at(tmp.path().join("data")),
        Config::default(),
        Arc::new(f),
        nexforge_core::SourceKeys::default(),
    );

    // 1. Reconcile.
    let r = reconcile(
        &addons,
        inst.flavour,
        &AddonsFile::new(inst.flavour),
        &mgr.available_sources(),
    )
    .unwrap();
    r.file.save(&state_path).unwrap();
    let state = AddonsFile::load_or_new(&state_path, inst.flavour).unwrap();
    let bwc = state
        .get("bigwigs-classic")
        .expect("BigWigs [Classic] adopted");
    assert_eq!(
        bwc.source,
        Some(SourceId::Github),
        "GitHub preferred over WoWInterface"
    );
    assert_eq!(bwc.source_id, "BigWigsMods/BigWigs_Classic");
    assert_eq!(bwc.installed_version, "v12.0.10");
    assert_eq!(bwc.ids.wowinterface.as_deref(), Some("22095"));
    let questie = state
        .get("questie")
        .expect("Questie found, title version stripped");
    // Questie's TOC names only CurseForge and Wago; the GitHub index maps it.
    assert_eq!(questie.source, Some(SourceId::Github));
    assert_eq!(questie.source_id, "Questie/Questie");
    assert_eq!(questie.ids.wago.as_deref(), Some("qv634BKb"));
    let elv = state.get("elvui-classic").unwrap();
    assert_eq!(elv.folders, vec!["ElvUI", "ElvUI_Libraries"]);
    assert!(state.owner_of("Blizzard_Foo").is_none());
    let text = std::fs::read_to_string(&state_path).unwrap();
    assert!(text.contains("flavour = \"classic-era\""), "{text}");

    // 2. Check: one update, nothing applied.
    let report = mgr.check(inst.flavour, &state).await;
    match report.status_of("bigwigs-classic") {
        Some(Status::Available(rel)) => assert_eq!(rel.version, "v12.1.2"),
        other => panic!("expected an update, got {other:?}"),
    }
    match report.status_of("questie") {
        Some(Status::Available(rel)) => assert_eq!(rel.version, "bundle/v12.5.0+v1.0.10"),
        other => panic!("expected a Questie update, got {other:?}"),
    }
    assert!(
        report.source_errors.is_empty(),
        "{:?}",
        report.source_errors
    );
    assert_eq!(
        std::fs::read_to_string(addons.join("BigWigs_Classic/Naxxramas/Kel.lua")).unwrap(),
        "-- v12.0.10"
    );

    // 3. Update.
    let mut state = state;
    let (from, e) = mgr
        .update(
            &inst,
            &mut state,
            &state_path,
            "bigwigs-classic",
            &|_, _| {},
        )
        .await
        .unwrap()
        .expect("an update was applied");
    assert_eq!(
        (from.as_str(), e.installed_version.as_str()),
        ("v12.0.10", "v12.1.2")
    );
    assert_eq!(
        std::fs::read_to_string(addons.join("BigWigs_Classic/Naxxramas/Kel.lua")).unwrap(),
        "-- v12.1.2"
    );
    assert!(
        addons.join("Questie").exists() && tmp.path().join("WTF").exists(),
        "nothing else touched"
    );
    let on_disk = AddonsFile::load_or_new(&state_path, inst.flavour).unwrap();
    assert_eq!(
        on_disk.get("bigwigs-classic").unwrap().installed_version,
        "v12.1.2"
    );

    // Up to date now.
    assert!(mgr
        .update(
            &inst,
            &mut state,
            &state_path,
            "bigwigs-classic",
            &|_, _| {}
        )
        .await
        .unwrap()
        .is_none());

    // 4. Roll back to the adopted version.
    let engine = mgr.engine();
    let mut t = nexforge_core::install::Target {
        installation: &inst,
        state: &mut state,
        state_path: &state_path,
    };
    let kept: Vec<String> = engine
        .kept_versions(&t, "bigwigs-classic")
        .into_iter()
        .map(|k| k.version)
        .collect();
    assert_eq!(kept, vec!["v12.1.2", "v12.0.10"]);
    let (from, to) = engine.rollback(&mut t, "bigwigs-classic", None).unwrap();
    assert_eq!((from.as_str(), to.as_str()), ("v12.1.2", "v12.0.10"));
    assert_eq!(
        std::fs::read_to_string(addons.join("BigWigs_Classic/Naxxramas/Kel.lua")).unwrap(),
        "-- v12.0.10"
    );
    let toc = std::fs::read_to_string(addons.join("BigWigs_Classic/BigWigs_Classic_Vanilla.toc"))
        .unwrap();
    assert!(toc.contains("## Version: v12.0.10"));

    // The rolled-back-from version is skipped, so check does not nag.
    let report = mgr.check(inst.flavour, t.state).await;
    assert!(matches!(
        report.status_of("bigwigs-classic"),
        Some(Status::Skipped(_))
    ));

    // Pinned addons are never updated.
    t.state.require_mut("bigwigs-classic").unwrap().pinned = true;
    t.state
        .require_mut("bigwigs-classic")
        .unwrap()
        .skipped_version
        .clear();
    let err = mgr
        .update(&inst, t.state, &state_path, "bigwigs-classic", &|_, _| {})
        .await
        .unwrap_err();
    assert!(matches!(err, nexforge_core::Error::Pinned(_)));
}
