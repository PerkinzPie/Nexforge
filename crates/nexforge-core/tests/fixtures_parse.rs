//! Parser and source tests against real TOCs and recorded host responses
//! (recorded 2026-11-04). No network.

use nexforge_core::net::FixtureFetch;
use nexforge_core::sources::{github::GitHub, pick_release, wowinterface::WowInterface, Source};
use nexforge_core::toc::{read_folder, Toc};
use nexforge_core::{Channel, Flavour};
use std::path::PathBuf;
use std::sync::Arc;

fn fx(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(rel)
}

#[test]
fn real_tocs() {
    let q = Toc::read(&fx("toc/Questie/Questie-Classic.toc")).unwrap();
    assert_eq!(q.title.as_deref(), Some("Questie v10.3.14"));
    assert_eq!(q.version.as_deref(), Some("10.3.14"));
    assert_eq!(q.interface, vec![11503]);
    assert_eq!(q.curse_id.as_deref(), Some("334372"));
    assert_eq!(q.wago_id.as_deref(), Some("qv634BKb"));
    assert!(
        q.dependencies.is_empty(),
        "empty RequiredDeps is not a dependency"
    );

    let b = Toc::read(&fx("toc/BigWigs_Classic/BigWigs_Classic_Vanilla.toc")).unwrap();
    assert_eq!(b.title.as_deref(), Some("BigWigs [Classic]"));
    assert_eq!(b.interface, vec![11508, 11507]);
    assert_eq!(b.dependencies, vec!["BigWigs"]);
    assert_eq!(
        b.github_repo().as_deref(),
        Some("BigWigsMods/BigWigs_Classic")
    );
    assert_eq!(b.wowi_id.as_deref(), Some("22095"));

    let e = Toc::read(&fx("toc/ElvUI/ElvUI_Vanilla.toc")).unwrap();
    assert_eq!(e.title.as_deref(), Some("ElvUI Classic"));
    assert_eq!(e.dependencies, vec!["ElvUI_Libraries"]);
    assert_eq!(
        e.fields.get("x-tukui-projectfolders").map(String::as_str),
        Some("ElvUI, ElvUI_Libraries, ElvUI_Options")
    );

    let v = Toc::read(&fx("toc/AI_VoiceOver/AI_VoiceOver_Vanilla.toc")).unwrap();
    assert_eq!(v.part_of.as_deref(), Some("VoiceOver"));
}

#[test]
fn real_folder_selection() {
    // Questie ships Questie.toc and Questie-Classic.toc (dash separator).
    let f = read_folder(&fx("toc/Questie"), Flavour::ClassicEra)
        .unwrap()
        .unwrap();
    assert_eq!(f.toc_file, "Questie-Classic.toc");
    let f = read_folder(&fx("toc/Questie"), Flavour::Forever)
        .unwrap()
        .unwrap();
    assert_eq!(f.toc_file, "Questie.toc");
    // ElvUI has only suffixed TOCs, none for Forever.
    let f = read_folder(&fx("toc/ElvUI"), Flavour::Forever)
        .unwrap()
        .unwrap();
    assert!(!f.flavour_match);
}

const BWC_RELEASES: &str =
    "https://api.github.com/repos/BigWigsMods/BigWigs_Classic/releases?per_page=10";
const Q_RELEASES: &str = "https://api.github.com/repos/Questie/Questie/releases?per_page=10";

fn release_json_url(list: &str) -> String {
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(fx(list)).unwrap()).unwrap();
    v[0]["assets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["name"] == "release.json")
        .unwrap()["browser_download_url"]
        .as_str()
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn github_release_json_flavours() {
    let f = FixtureFetch::new();
    f.route_file(BWC_RELEASES, fx("github/bigwigs_classic_releases.json"));
    f.route_file(
        &release_json_url("github/bigwigs_classic_releases.json"),
        fx("github/bigwigs_classic_release.json"),
    );
    f.route_file(Q_RELEASES, fx("github/questie_releases.json"));
    f.route_file(
        &release_json_url("github/questie_releases.json"),
        fx("github/questie_release.json"),
    );
    let gh = GitHub::new(Arc::new(f.clone()), None);

    let era = gh
        .releases("BigWigsMods/BigWigs_Classic", Flavour::ClassicEra)
        .await
        .unwrap();
    let latest = pick_release(&era, Flavour::ClassicEra, Channel::Stable).unwrap();
    assert_eq!(latest.version, "v12.1.2");
    assert!(latest.supports(Flavour::ClassicEra) && latest.supports(Flavour::Anniversary));
    assert_eq!(
        latest.sha256.as_deref(),
        Some("fd6576bae7538ce69562bf19f2e9f7cae7a8146653a51310a410bcd5191ae9c1")
    );
    assert_eq!(latest.size, Some(498191));

    // BigWigs_Classic v12.1.2's release.json lists no Forever build; older
    // releases (no release.json fetched) are flavour-unknown, so they stay
    // listed for the user to confirm.
    let fv = gh
        .releases("BigWigsMods/BigWigs_Classic", Flavour::Forever)
        .await
        .unwrap();
    assert!(fv.iter().all(|r| r.version != "v12.1.2"));
    assert!(fv.iter().all(|r| r.flavour_unknown()));

    // Questie's newest bundle does list Forever.
    let q = gh
        .releases("Questie/Questie", Flavour::Forever)
        .await
        .unwrap();
    let r = pick_release(&q, Flavour::Forever, Channel::Stable).unwrap();
    assert_eq!(r.version, "bundle/v12.5.0+v1.0.10");
    assert!(r.supports(Flavour::Forever));
    assert_eq!(r.file_name.as_deref(), Some("Questie-v12.5.0+v1.0.10.zip"));

    // Only api.github.com and github.com were asked, and release.json reads are capped.
    assert!(f
        .hits()
        .iter()
        .all(|u| u.starts_with("https://api.github.com/") || u.starts_with("https://github.com/")));
}

#[tokio::test]
async fn catalogue_answers_search_without_the_network() {
    use nexforge_core::catalogue::Catalogue;
    use nexforge_core::{AppPaths, Config, Manager, SourceId, SourceKeys};
    let f = FixtureFetch::new();
    f.route_file(
        "https://api.mmoui.com/v3/game/WOW/filelist.json",
        fx("wowinterface/filelist.json"),
    );
    let dir = tempfile::tempdir().unwrap();
    let mgr = Manager::with_fetch(
        AppPaths::at(dir.path()),
        Config::default(),
        Arc::new(f.clone()),
        SourceKeys::default(),
    )
    .with_catalogue(Arc::new(Catalogue::in_memory().unwrap()));

    assert!(mgr.refresh_catalogue(false).await.is_empty());
    let stats = mgr.catalogue_stats();
    assert!(stats
        .per_source
        .iter()
        .any(|(s, n)| *s == SourceId::Github && *n >= 30));
    assert!(stats
        .per_source
        .iter()
        .any(|(s, n)| *s == SourceId::Wowinterface && *n >= 3));
    // Fresh: a second refresh does nothing.
    let hits_before = f.hits().len();
    mgr.refresh_catalogue(false).await;
    assert_eq!(f.hits().len(), hits_before);

    let (hits, errors) = mgr.search("bigwigs", Flavour::ClassicEra).await;
    assert!(errors.is_empty());
    assert_eq!(
        f.hits().len(),
        hits_before,
        "search used the catalogue only"
    );
    assert!(hits
        .iter()
        .any(|h| h.source == SourceId::Github && h.source_id == "BigWigsMods/BigWigs"));
    assert!(hits
        .iter()
        .any(|h| h.source == SourceId::Wowinterface && h.source_id == "22095"));
    assert_eq!(
        mgr.icon_for(SourceId::Wowinterface, "22095").as_deref(),
        Some("https://cdn-wow.mmoui.com/preview/tiny/pvw75842.jpg")
    );
}

#[tokio::test]
async fn icons_come_from_image_hosts_and_are_cached() {
    use nexforge_core::sources::github::avatar_url;
    use nexforge_core::{AppPaths, Config, Manager, SourceKeys};
    let url = avatar_url("Questie/Questie").unwrap();
    assert_eq!(url, "https://avatars.githubusercontent.com/Questie?s=96");
    assert_eq!(
        avatar_url("rusty-key/spoken-wow#spoken").as_deref(),
        Some("https://avatars.githubusercontent.com/rusty-key?s=96")
    );

    let f = FixtureFetch::new();
    f.route(&url, vec![0x89, b'P', b'N', b'G']);
    let dir = tempfile::tempdir().unwrap();
    let mgr = Manager::with_fetch(
        AppPaths::at(dir.path()),
        Config::default(),
        Arc::new(f.clone()),
        SourceKeys::default(),
    );
    let img = mgr.image(&url, "addon icon").await.unwrap();
    assert_eq!(img.bytes.len(), 4);
    // Second call is served from the disk cache.
    mgr.image(&url, "addon icon").await.unwrap();
    assert_eq!(f.hits().len(), 1);

    // GitHub search rows carry the owner avatar.
    let gh = GitHub::new(Arc::new(FixtureFetch::new()), None);
    let hits = gh.search("questie", Flavour::Forever).await.unwrap();
    assert_eq!(hits[0].icon_url.as_deref(), Some(url.as_str()));
}

#[tokio::test]
async fn github_shared_repo_is_split_by_tag() {
    // rusty-key/spoken-wow releases its desktop app (manager/…), the addon
    // (spoken/…) and voice packs (quests-audio/…, bosses-audio/…) from one repo.
    let f = FixtureFetch::new();
    f.route_file(
        "https://api.github.com/repos/rusty-key/spoken-wow/releases?per_page=100",
        fx("github/spoken_releases.json"),
    );
    let gh = GitHub::new(Arc::new(f.clone()), None);

    let addon = gh
        .releases("rusty-key/spoken-wow#spoken", Flavour::Forever)
        .await
        .unwrap();
    let r = pick_release(&addon, Flavour::Forever, Channel::Stable).unwrap();
    assert_eq!(r.version, "v3.8.0", "tag prefix stripped");
    assert_eq!(r.file_name.as_deref(), Some("Spoken-3.8.0.zip"));
    assert_eq!(
        r.flavours,
        vec![Flavour::Forever],
        "flavour from the index hint"
    );
    assert!(
        addon.iter().all(|r| !r
            .file_name
            .as_deref()
            .unwrap_or_default()
            .contains("Manager")),
        "the desktop app is never an addon release"
    );
    // The Spoken addon TOC is Forever-only.
    assert!(gh
        .releases("rusty-key/spoken-wow#spoken", Flavour::ClassicEra)
        .await
        .unwrap()
        .is_empty());

    let pack = gh
        .releases("rusty-key/spoken-wow#bosses-audio", Flavour::Forever)
        .await
        .unwrap();
    assert_eq!(pack.len(), 1);
    assert_eq!(
        pack[0].file_name.as_deref(),
        Some("SpokenBossesAudio-1.0.0.zip")
    );

    // A bare repo URL never installs the whole repo.
    assert_eq!(
        gh.resolve_url("https://github.com/rusty-key/spoken-wow")
            .await
            .unwrap(),
        Some("rusty-key/spoken-wow#spoken".into())
    );
    assert_eq!(
        gh.resolve_url("https://github.com/rusty-key/spoken-wow/releases/tag/bosses-audio/v1.0.0")
            .await
            .unwrap(),
        Some("rusty-key/spoken-wow#bosses-audio".into())
    );
    // All tagged entries share one cached request.
    assert!(f.hits().iter().all(|u| u.ends_with("per_page=100")));
}

#[tokio::test]
async fn wowinterface_details_and_batch() {
    let f = FixtureFetch::new();
    f.route_file(
        "https://api.mmoui.com/v3/game/WOW/filelist.json",
        fx("wowinterface/filelist.json"),
    );
    f.route_file(
        "https://api.mmoui.com/v3/game/WOW/filedetails/22095.json",
        fx("wowinterface/filedetails_22095.json"),
    );
    let wowi = WowInterface::new(Arc::new(f));

    let rels = wowi.releases("22095", Flavour::ClassicEra).await.unwrap();
    assert_eq!(rels.len(), 1);
    let r = &rels[0];
    assert_eq!(r.version, "v12.1.2");
    assert_eq!(r.md5.as_deref(), Some("9e3a40f355a683c377f19a403f103530"));
    assert!(r
        .download_url
        .starts_with("https://cdn.wowinterface.com/downloads/getfile.php?id=22095"));
    assert_eq!(
        r.flavours,
        vec![Flavour::ClassicEra, Flavour::Anniversary, Flavour::Retail]
    );

    let info = wowi.resolve("22095").await.unwrap();
    assert_eq!(
        info.icon_url.as_deref(),
        Some("https://cdn-wow.mmoui.com/preview/tiny/pvw75842.jpg")
    );
    assert_eq!(
        info.screenshots,
        vec!["https://cdn-wow.mmoui.com/preview/pvw75842.jpg"]
    );
    assert_eq!(
        info.screenshot_thumbs,
        vec!["https://cdn-wow.mmoui.com/preview/tiny/pvw75842.jpg"]
    );
    assert!(nexforge_core::net::image_host_allowed("cdn-wow.mmoui.com"));

    // No Forever compatibility listed yet (pre-launch), so nothing for Forever.
    assert!(wowi
        .releases("22095", Flavour::Forever)
        .await
        .unwrap()
        .is_empty());

    let batch = wowi
        .batch_latest(
            &["22095".into(), "20509".into(), "999999".into()],
            Flavour::ClassicEra,
            Channel::Stable,
        )
        .await
        .unwrap();
    assert_eq!(batch.found["22095"].version, "v12.1.2");
    assert_eq!(batch.found["20509"].version, "v2.31.13");
    // An id the host no longer lists is a failure, not "no build".
    assert!(!batch.found.contains_key("999999"));
    assert!(batch.failed["999999"].contains("no longer lists"));

    assert_eq!(
        wowi.resolve_url("https://www.wowinterface.com/downloads/info22095-BigWigs_Classic.html")
            .await
            .unwrap(),
        Some("22095".into())
    );
}
