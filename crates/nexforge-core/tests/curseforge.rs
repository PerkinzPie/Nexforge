//! CurseForge source, offline.
//!
//! The fixtures in tests/fixtures/curseforge are synthetic, not recordings:
//! no API key was available, so they were built from the documented `Mod`,
//! `File`, `FileIndex`, `HashAlgo` and `FileReleaseType` shapes. DungeonJournal
//! (ExehnTV, project 1707971, game version 1.60.1) is real; file ids,
//! versions, hashes, image URLs and project 200000 are made up.
//! `PLACEHOLDER_MD5` is replaced at test time with the synthetic zip's MD5.
//! Replace them with recorded responses once a key exists.

use md5::{Digest, Md5};
use nexforge_core::net::FixtureFetch;
use nexforge_core::sources::{curseforge::CurseForge, pick_release, Source};
use nexforge_core::state::{AddonsFile, Config};
use nexforge_core::{
    AppPaths, Channel, Error, Flavour, Installation, Manager, SourceId, SourceKeys, Status,
};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

const API: &str = "https://api.curseforge.com/v1";
const ZIP: &str = "https://edge.forgecdn.net/files/7012/345/DungeonJournal-v1.4.2.zip";

fn fx(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/curseforge")
        .join(rel)
}

fn zip_bytes() -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut buf);
        let o = zip::write::SimpleFileOptions::default();
        z.start_file("DungeonJournal/DungeonJournal.toc", o)
            .unwrap();
        write!(
            z,
            "## Interface: 16001\n## Title: DungeonJournal\n## Version: v1.4.2\n## X-Curse-Project-ID: 1707971\n"
        )
        .unwrap();
        z.finish().unwrap();
    }
    buf.into_inner()
}

/// Serve every fixture, with the placeholder MD5 replaced by the zip's.
fn fixtures() -> FixtureFetch {
    let zip = zip_bytes();
    let md5: String = Md5::digest(&zip)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let read = |n: &str| {
        std::fs::read_to_string(fx(n))
            .unwrap()
            .replace("PLACEHOLDER_MD5", &md5)
    };
    let f = FixtureFetch::new();
    f.route(&format!("{API}/mods/1707971"), read("mod_1707971.json"));
    f.route(
        &format!("{API}/mods/1707971/files?pageSize=50"),
        read("files_1707971.json"),
    );
    f.route(
        &format!("{API}/mods/200000/files?pageSize=50"),
        read("files_200000.json"),
    );
    f.route(&format!("{API}/mods"), read("mods_batch.json"));
    f.route(
        &format!("{API}/mods/search?gameId=1&slug=dungeonjournal"),
        read("search_slug_dungeonjournal.json"),
    );
    f.route(
        &format!("{API}/mods/search?gameId=1&searchFilter=dungeon&gameVersionTypeId=88568&sortField=2&sortOrder=desc&pageSize=50"),
        read("search_dungeon.json"),
    );
    f.route(ZIP, zip);
    f
}

#[tokio::test]
async fn releases_flavours_and_search() {
    let f = fixtures();
    let cf = CurseForge::new(Arc::new(f.clone()), "test-key".into());

    let rels = cf.releases("1707971", Flavour::Forever).await.unwrap();
    assert!(
        rels.iter().all(|r| r.supports(Flavour::Forever)),
        "the TBC file is filtered out"
    );
    let r = pick_release(&rels, Flavour::Forever, Channel::Stable).unwrap();
    assert_eq!(
        r.version, "DungeonJournal v1.4.2",
        "newest stable, not the beta"
    );
    assert_eq!(r.download_url, ZIP);
    assert_eq!(r.size, Some(1048576));
    assert!(r.md5.as_deref().is_some_and(|m| m.len() == 32));
    let beta = pick_release(&rels, Flavour::Forever, Channel::Beta).unwrap();
    assert_eq!(beta.version, "DungeonJournal v1.5.0-beta");
    let tbc = cf.releases("1707971", Flavour::Anniversary).await.unwrap();
    assert_eq!(tbc.len(), 1);

    let info = cf.resolve("1707971").await.unwrap();
    assert_eq!(
        (info.name.as_str(), info.authors[0].as_str()),
        ("DungeonJournal", "ExehnTV")
    );
    assert!(info
        .icon_url
        .unwrap()
        .starts_with("https://media.forgecdn.net/"));
    assert_eq!(info.screenshots.len(), 1);

    let hits = cf.search("dungeon", Flavour::Forever).await.unwrap();
    assert_eq!(hits[0].source_id, "1707971");
    assert!(hits[0].flavours.contains(&Flavour::Forever));
    assert_eq!(hits[0].downloads, Some(1_700_000));

    assert_eq!(
        cf.resolve_url("https://www.curseforge.com/wow/addons/dungeonjournal")
            .await
            .unwrap(),
        Some("1707971".into())
    );
    assert_eq!(
        cf.resolve_url("https://www.curseforge.com/projects/1707971")
            .await
            .unwrap(),
        Some("1707971".into())
    );

    // Every request carries the key header.
    for (url, headers) in f.sent_headers() {
        assert!(
            headers
                .iter()
                .any(|(k, v)| k == "x-api-key" && v == "test-key"),
            "{url} sent without the key"
        );
    }
}

fn manager(f: &FixtureFetch, dir: &std::path::Path, key: Option<&str>) -> Manager {
    let mut config = Config::default();
    config.sources.curseforge = true;
    Manager::with_fetch(
        AppPaths::at(dir.join("data")),
        config,
        Arc::new(f.clone()),
        SourceKeys {
            github: None,
            curseforge: key.map(str::to_string),
        },
    )
}

#[tokio::test]
async fn install_check_and_opt_out() {
    let tmp = tempfile::tempdir().unwrap();
    let client = tmp.path().join("_forever_");
    std::fs::create_dir_all(client.join("Interface/AddOns")).unwrap();
    std::fs::write(
        client.join(".flavor.info"),
        "Product Flavor!STRING:0\nwowf\n",
    )
    .unwrap();
    let inst = Installation::from_client_dir(&client).unwrap();
    let state_path = inst.state_file();
    let f = fixtures();
    let mgr = manager(&f, tmp.path(), Some("test-key"));
    assert!(mgr.available_sources().contains(&SourceId::Curseforge));

    // Install from the CurseForge page address.
    let mut state = AddonsFile::new(inst.flavour);
    let e = mgr
        .install(
            &inst,
            &mut state,
            &state_path,
            "https://www.curseforge.com/wow/addons/dungeonjournal",
            None,
            Channel::Stable,
            false,
            &|_, _| {},
        )
        .await
        .unwrap();
    assert_eq!(e.source, Some(SourceId::Curseforge));
    assert_eq!(e.source_id, "1707971");
    assert_eq!(e.installed_version, "DungeonJournal v1.4.2");
    assert_eq!(e.author.as_deref(), Some("ExehnTV"));
    assert!(inst
        .addons_dir()
        .join("DungeonJournal/DungeonJournal.toc")
        .exists());

    // Update check: one POST for every CurseForge addon, and it is current.
    let report = mgr.check(inst.flavour, &state).await;
    assert_eq!(report.status_of(&e.id), Some(&Status::UpToDate));
    let posts = f.posts();
    assert_eq!(posts.len(), 1);
    assert!(posts[0].1.contains("1707971"), "{}", posts[0].1);

    // An author who opted out: listed, never downloaded, plain explanation.
    let mut closed = e.clone();
    closed.id = "closed".into();
    closed.name = "Closed Addon".into();
    closed.source_id = "200000".into();
    let err = mgr.pick(&closed, inst.flavour, false).await.unwrap_err();
    assert!(
        matches!(&err, Error::Unsupported(m) if m.contains("does not allow downloads outside the CurseForge app")),
        "{err}"
    );
    assert!(
        !f.hits().iter().any(|u| u.contains("Closed-v2.0.0.zip")),
        "no download is attempted or built by hand"
    );
}

#[tokio::test]
async fn without_a_key_curseforge_is_off() {
    let tmp = tempfile::tempdir().unwrap();
    let f = fixtures();
    let mgr = manager(&f, tmp.path(), None);
    assert!(!mgr.available_sources().contains(&SourceId::Curseforge));
    let err = mgr
        .resolve_spec("https://www.curseforge.com/wow/addons/dungeonjournal")
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("nexforge key set curseforge"),
        "{err}"
    );
    assert!(
        f.hits().is_empty(),
        "nothing is sent to CurseForge without a key"
    );
}
