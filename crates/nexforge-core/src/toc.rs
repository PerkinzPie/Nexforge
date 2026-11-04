//! `.toc` parsing and flavour-specific TOC selection.

use crate::error::{IoContext, Result};
use crate::flavour::Flavour;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The fields Nexforge reads from one `.toc` file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Toc {
    pub interface: Vec<u32>,
    /// Title with WoW colour and texture codes stripped.
    pub title: Option<String>,
    /// `None` when absent or still a packager placeholder (`@project-version@`).
    pub version: Option<String>,
    pub author: Option<String>,
    pub notes: Option<String>,
    pub curse_id: Option<String>,
    pub wago_id: Option<String>,
    pub wowi_id: Option<String>,
    pub website: Option<String>,
    pub github: Option<String>,
    /// `X-Part-Of`, used by multi-folder addons to name their parent.
    pub part_of: Option<String>,
    /// Required dependencies (`## Dependencies`, `## RequiredDeps`, any `## Dep…`).
    pub dependencies: Vec<String>,
    /// Every `## Key: Value` pair, keys lowercased.
    pub fields: BTreeMap<String, String>,
}

impl Toc {
    pub fn parse(text: &str) -> Toc {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let mut fields = BTreeMap::new();
        for line in text.lines() {
            let Some(rest) = line.trim_start().strip_prefix("##") else {
                continue;
            };
            let Some((key, value)) = rest.split_once(':') else {
                continue;
            };
            let key = key.trim().to_ascii_lowercase();
            if key.is_empty() || key.contains(' ') {
                continue;
            }
            // First occurrence wins, matching the client.
            fields
                .entry(key)
                .or_insert_with(|| value.trim().to_string());
        }

        let get = |k: &str| {
            fields
                .get(k)
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        };

        let interface = get("interface")
            .map(|v| {
                v.split(',')
                    .filter_map(|n| n.trim().parse::<u32>().ok())
                    .collect()
            })
            .unwrap_or_default();

        let version = get("version").filter(|v| !(v.starts_with('@') && v.ends_with('@')));

        let mut dependencies = Vec::new();
        for (k, v) in &fields {
            if k.starts_with("dep") || k == "requireddeps" {
                for d in v.split(',') {
                    let d = d.trim();
                    if !d.is_empty() && !dependencies.iter().any(|x: &String| x == d) {
                        dependencies.push(d.to_string());
                    }
                }
            }
        }

        let numeric = |v: Option<String>| v.filter(|s| s.chars().all(|c| c.is_ascii_digit()));

        Toc {
            interface,
            title: get("title").map(|t| strip_colour_codes(&t)),
            version,
            author: get("author").map(|a| strip_colour_codes(&a)),
            notes: get("notes").map(|n| strip_colour_codes(&n)),
            curse_id: numeric(get("x-curse-project-id")),
            wago_id: get("x-wago-id"),
            wowi_id: numeric(get("x-wowi-id")),
            website: get("x-website"),
            github: get("x-github"),
            part_of: get("x-part-of"),
            dependencies,
            fields,
        }
    }

    pub fn read(path: &Path) -> Result<Toc> {
        let bytes = std::fs::read(path).ctx("read", path)?;
        Ok(Toc::parse(&String::from_utf8_lossy(&bytes)))
    }

    /// Flavours this TOC's `## Interface` numbers say it supports.
    pub fn interface_flavours(&self) -> Vec<Flavour> {
        let mut out: Vec<Flavour> = self
            .interface
            .iter()
            .filter_map(|n| Flavour::from_interface(*n))
            .collect();
        out.sort();
        out.dedup();
        out
    }

    pub fn has_source_id(&self) -> bool {
        self.curse_id.is_some()
            || self.wago_id.is_some()
            || self.wowi_id.is_some()
            || self.github_repo().is_some()
    }

    /// `owner/repo` if `X-GitHub` or `X-Website` points at a GitHub repository
    /// (an organisation page alone is not enough).
    pub fn github_repo(&self) -> Option<String> {
        [self.github.as_deref(), self.website.as_deref()]
            .into_iter()
            .flatten()
            .find_map(parse_github_repo)
    }
}

/// Parse `https://github.com/owner/repo[...]` or a bare `owner/repo` into `owner/repo`.
pub fn parse_github_repo(s: &str) -> Option<String> {
    let s = s.trim();
    let rest = if let Some(i) = s.to_ascii_lowercase().find("github.com/") {
        &s[i + "github.com/".len()..]
    } else if !s.contains("://") && !s.contains('.') && s.matches('/').count() == 1 {
        s
    } else {
        return None;
    };
    let mut parts = rest.split(['/', '?', '#']).filter(|p| !p.is_empty());
    let owner = parts.next()?;
    let repo = parts.next()?.trim_end_matches(".git");
    let ok = |p: &str| {
        !p.is_empty()
            && p.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    (ok(owner) && ok(repo)).then(|| format!("{owner}/{repo}"))
}

/// Remove `|cAARRGGBB`, `|r`, `|T…|t` and `|A…|a` escape sequences and tidy spacing.
pub fn strip_colour_codes(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '|' && i + 1 < chars.len() {
            match chars[i + 1] {
                'c' | 'C' if i + 10 <= chars.len() => {
                    i += 10;
                    continue;
                }
                'r' | 'R' => {
                    i += 2;
                    continue;
                }
                'T' | 'A' => {
                    let end = if chars[i + 1] == 'T' { 't' } else { 'a' };
                    let mut j = i + 2;
                    while j + 1 < chars.len() && !(chars[j] == '|' && chars[j + 1] == end) {
                        j += 1;
                    }
                    i = (j + 2).min(chars.len());
                    continue;
                }
                '|' => {
                    out.push('|');
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// One `.toc` file that belongs to a folder (its stem is the folder name,
/// optionally followed by `_Suffix` or `-Suffix`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TocFile {
    pub path: PathBuf,
    pub file_name: String,
    /// `None` for the plain `<Folder>.toc`.
    pub suffix: Option<String>,
}

/// List the TOC files the client would consider for `folder`.
pub fn toc_files(folder: &Path) -> Result<Vec<TocFile>> {
    let name = folder
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let lname = name.to_ascii_lowercase();
    let mut out = Vec::new();
    for entry in std::fs::read_dir(folder).ctx("read", folder)? {
        let entry = entry.ctx("read", folder)?;
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let file_name = entry.file_name().to_string_lossy().to_string();
        let lower = file_name.to_ascii_lowercase();
        let Some(stem) = lower.strip_suffix(".toc") else {
            continue;
        };
        let suffix = if stem == lname {
            None
        } else if let Some(rest) = stem.strip_prefix(&lname) {
            match rest.strip_prefix(['_', '-']) {
                Some(s) if !s.is_empty() => {
                    Some(file_name[name.len() + 1..file_name.len() - 4].to_string())
                }
                _ => continue,
            }
        } else {
            continue;
        };
        out.push(TocFile {
            path: entry.path(),
            file_name,
            suffix,
        });
    }
    out.sort_by(|a, b| a.file_name.cmp(&b.file_name));
    Ok(out)
}

/// Pick the TOC the client would load for `flavour`, following the suffix
/// order in [`Flavour::toc_suffixes`]. Returns `None` when no TOC applies to
/// this flavour.
pub fn select_toc(files: &[TocFile], flavour: Flavour) -> Option<&TocFile> {
    flavour.toc_suffixes().iter().find_map(|want| {
        files.iter().find(|f| match (&f.suffix, *want) {
            (None, "") => true,
            (Some(s), w) if !w.is_empty() => s.eq_ignore_ascii_case(w),
            _ => false,
        })
    })
}

/// A folder in `AddOns/` with its parsed TOC.
#[derive(Debug, Clone)]
pub struct AddonFolder {
    pub name: String,
    pub toc: Toc,
    /// File name of the TOC that was read.
    pub toc_file: String,
    /// True if the chosen TOC is one the client loads for this flavour. False
    /// means we fell back to some other TOC just to identify the folder.
    pub flavour_match: bool,
    pub toc_count: usize,
}

/// Read one addon folder. `Ok(None)` if it has no TOC that belongs to it.
pub fn read_folder(path: &Path, flavour: Flavour) -> Result<Option<AddonFolder>> {
    let files = toc_files(path)?;
    if files.is_empty() {
        return Ok(None);
    }
    let (chosen, flavour_match) = match select_toc(&files, flavour) {
        Some(f) => (f, true),
        None => (
            files
                .iter()
                .find(|f| f.suffix.is_none())
                .unwrap_or(&files[0]),
            false,
        ),
    };
    let mut toc = Toc::read(&chosen.path)?;
    // IDs and website are the same in every TOC of a folder in practice, but
    // fill any gaps from the siblings so identification does not depend on
    // which file the flavour picked.
    for other in files.iter().filter(|f| f.path != chosen.path) {
        if toc.curse_id.is_some() && toc.wago_id.is_some() && toc.wowi_id.is_some() {
            break;
        }
        if let Ok(o) = Toc::read(&other.path) {
            toc.curse_id = toc.curse_id.or(o.curse_id);
            toc.wago_id = toc.wago_id.or(o.wago_id);
            toc.wowi_id = toc.wowi_id.or(o.wowi_id);
            toc.website = toc.website.or(o.website);
            toc.github = toc.github.or(o.github);
            toc.part_of = toc.part_of.or(o.part_of);
        }
    }
    Ok(Some(AddonFolder {
        name: path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default(),
        toc,
        toc_file: chosen.file_name.clone(),
        flavour_match,
        toc_count: files.len(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_colour_codes() {
        assert_eq!(
            strip_colour_codes("Questie|cFF00FF00 v10.3.14|r"),
            "Questie v10.3.14"
        );
        assert_eq!(
            strip_colour_codes("|cff1784d1ElvUI|r |cfd9b9b9bClassic|r"),
            "ElvUI Classic"
        );
        assert_eq!(
            strip_colour_codes("BigWigs [|cffeda55fCore|r]"),
            "BigWigs [Core]"
        );
        assert_eq!(
            strip_colour_codes("|TInterface\\Icons\\foo:16|t Foo"),
            "Foo"
        );
        assert_eq!(strip_colour_codes("A || B"), "A | B");
    }

    #[test]
    fn parses_fields() {
        let t = Toc::parse(
            "\u{feff}## Interface: 16001, 11508\r\n## Title: |cff00ff00Foo|r\r\n## Version: @project-version@\r\n\
             ## RequiredDeps: Bar, Baz\r\n## Dependencies: Bar\r\n## X-Curse-Project-ID: 123\r\n## X-WoWI-ID: abc\r\n\
             ## X-Website: https://github.com/Owner/Repo/releases\r\nCore.lua\r\n",
        );
        assert_eq!(t.interface, vec![16001, 11508]);
        assert_eq!(t.title.as_deref(), Some("Foo"));
        assert_eq!(t.version, None);
        assert_eq!(t.dependencies, vec!["Bar", "Baz"]);
        assert_eq!(t.curse_id.as_deref(), Some("123"));
        assert_eq!(t.wowi_id, None, "non-numeric WoWI ids are ignored");
        assert_eq!(t.github_repo().as_deref(), Some("Owner/Repo"));
        assert_eq!(
            t.interface_flavours(),
            vec![Flavour::Forever, Flavour::ClassicEra]
        );
    }

    #[test]
    fn github_repo_needs_two_segments() {
        assert_eq!(parse_github_repo("https://github.com/BigWigsMods"), None);
        assert_eq!(
            parse_github_repo("https://github.com/BigWigsMods/BigWigs_Classic").as_deref(),
            Some("BigWigsMods/BigWigs_Classic")
        );
        assert_eq!(
            parse_github_repo("Questie/Questie").as_deref(),
            Some("Questie/Questie")
        );
        assert_eq!(
            parse_github_repo("https://www.curseforge.com/wow/addons/x"),
            None
        );
    }

    fn files(names: &[&str]) -> Vec<TocFile> {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("Addon");
        std::fs::create_dir(&folder).unwrap();
        for n in names {
            std::fs::write(folder.join(n), "## Title: x\n").unwrap();
        }
        let out = toc_files(&folder).unwrap();
        drop(dir);
        out
    }

    #[test]
    fn selects_flavour_toc() {
        let f = files(&[
            "Addon.toc",
            "Addon_Vanilla.toc",
            "Addon_Camelot.toc",
            "Addon-Classic.toc",
            "Other.toc",
            "Addon_.toc",
        ]);
        assert_eq!(f.len(), 4, "foreign and malformed TOCs are ignored: {f:?}");
        assert_eq!(
            select_toc(&f, Flavour::Forever).unwrap().file_name,
            "Addon_Camelot.toc"
        );
        assert_eq!(
            select_toc(&f, Flavour::ClassicEra).unwrap().file_name,
            "Addon_Vanilla.toc"
        );
        assert_eq!(
            select_toc(&f, Flavour::Anniversary).unwrap().file_name,
            "Addon_Vanilla.toc"
        );

        let f = files(&["Addon_Vanilla.toc", "Addon_Mists.toc"]);
        assert!(
            select_toc(&f, Flavour::Forever).is_none(),
            "_Vanilla does not load on Forever"
        );

        let f = files(&["Addon-Classic.toc", "Addon.toc"]);
        assert_eq!(
            select_toc(&f, Flavour::ClassicEra).unwrap().file_name,
            "Addon-Classic.toc"
        );
        assert_eq!(
            select_toc(&f, Flavour::Forever).unwrap().file_name,
            "Addon.toc"
        );
    }
}
