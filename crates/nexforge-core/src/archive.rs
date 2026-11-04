//! Zip and folder helpers for the install engine.

use crate::error::{Error, IoContext, Result};
use md5::Md5;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn digest<D: Digest>(path: &Path) -> Result<String> {
    let mut f = File::open(path).ctx("open", path)?;
    let mut h = D::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf).ctx("read", path)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex(&h.finalize()))
}

pub fn sha256_file(path: &Path) -> Result<String> {
    digest::<Sha256>(path)
}

pub fn md5_file(path: &Path) -> Result<String> {
    digest::<Md5>(path)
}

/// Top-level folders in an addon zip. Requires at least one `Folder/*.toc`.
pub fn inspect(zip_path: &Path) -> Result<Vec<String>> {
    let file = File::open(zip_path).ctx("open", zip_path)?;
    let mut zip = zip::ZipArchive::new(file)
        .map_err(|e| Error::BadArchive(format!("it is not a zip file ({e})")))?;
    let mut folders: Vec<String> = Vec::new();
    let mut has_toc = false;
    for i in 0..zip.len() {
        let entry = zip
            .by_index(i)
            .map_err(|e| Error::BadArchive(e.to_string()))?;
        let Some(path) = entry.enclosed_name() else {
            return Err(Error::BadArchive(format!(
                "it contains an unsafe path ({})",
                entry.name()
            )));
        };
        let parts: Vec<String> = path
            .components()
            .filter_map(|c| match c {
                Component::Normal(s) => Some(s.to_string_lossy().to_string()),
                _ => None,
            })
            .collect();
        let Some(top) = parts.first() else { continue };
        if top == "__MACOSX" || (parts.len() == 1 && !entry.is_dir()) {
            // Mac resource forks and loose top-level files are not addons.
            continue;
        }
        if !folders.iter().any(|f| f == top) {
            folders.push(top.clone());
        }
        if parts.len() == 2 && parts[1].to_ascii_lowercase().ends_with(".toc") {
            has_toc = true;
        }
    }
    if !has_toc {
        return Err(Error::BadArchive(
            "no addon folder with a .toc file at the top of the archive".into(),
        ));
    }
    folders.sort();
    Ok(folders)
}

/// Extract the given top-level folders of `zip_path` into `dest`.
pub fn extract(zip_path: &Path, dest: &Path, folders: &[String]) -> Result<()> {
    let file = File::open(zip_path).ctx("open", zip_path)?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| Error::BadArchive(e.to_string()))?;
    std::fs::create_dir_all(dest).ctx("create", dest)?;
    for i in 0..zip.len() {
        let mut entry = zip
            .by_index(i)
            .map_err(|e| Error::BadArchive(e.to_string()))?;
        let Some(rel) = entry.enclosed_name() else {
            return Err(Error::BadArchive(format!("unsafe path {}", entry.name())));
        };
        let top = rel
            .components()
            .next()
            .map(|c| c.as_os_str().to_string_lossy().to_string());
        if !top.map(|t| folders.contains(&t)).unwrap_or(false) {
            continue;
        }
        let out = dest.join(&rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&out).ctx("create", &out)?;
            continue;
        }
        if let Some(p) = out.parent() {
            std::fs::create_dir_all(p).ctx("create", p)?;
        }
        let mut f = File::create(&out).ctx("create", &out)?;
        std::io::copy(&mut entry, &mut f).ctx("write", &out)?;
    }
    Ok(())
}

/// Zip `folders` (each a directory under `base`) into `dest`.
pub fn zip_folders(base: &Path, folders: &[String], dest: &Path) -> Result<()> {
    if let Some(p) = dest.parent() {
        std::fs::create_dir_all(p).ctx("create", p)?;
    }
    let tmp = dest.with_extension("zip.tmp");
    let file = File::create(&tmp).ctx("create", &tmp)?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .large_file(true);
    let zerr = |e: zip::result::ZipError| {
        Error::Other(format!("Could not write backup {}: {e}", dest.display()))
    };
    for folder in folders {
        let root = base.join(folder);
        if !root.is_dir() {
            continue;
        }
        let mut stack = vec![(root, PathBuf::from(folder))];
        while let Some((dir, rel)) = stack.pop() {
            zip.add_directory(rel_name(&rel), opts).map_err(zerr)?;
            let mut entries: Vec<_> = std::fs::read_dir(&dir)
                .ctx("read", &dir)?
                .flatten()
                .collect();
            entries.sort_by_key(|e| e.file_name());
            for e in entries {
                let p = e.path();
                let r = rel.join(e.file_name());
                if e.file_type().ctx("read", &p)?.is_dir() {
                    stack.push((p, r));
                } else {
                    zip.start_file(rel_name(&r), opts).map_err(zerr)?;
                    let mut f = File::open(&p).ctx("open", &p)?;
                    std::io::copy(&mut f, &mut zip).ctx("read", &p)?;
                }
            }
        }
    }
    let mut file = zip.finish().map_err(zerr)?;
    file.flush().ctx("write", &tmp)?;
    drop(file);
    std::fs::rename(&tmp, dest).ctx("save", dest)?;
    Ok(())
}

fn rel_name(p: &Path) -> String {
    p.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst).ctx("create", dst)?;
    for e in std::fs::read_dir(src).ctx("read", src)? {
        let e = e.ctx("read", src)?;
        let (s, d) = (e.path(), dst.join(e.file_name()));
        if e.file_type().ctx("read", &s)?.is_dir() {
            copy_dir(&s, &d)?;
        } else {
            std::fs::copy(&s, &d).ctx("copy", &s)?;
        }
    }
    Ok(())
}

fn crosses_devices(e: &std::io::Error) -> bool {
    e.kind() == std::io::ErrorKind::CrossesDevices
        || (cfg!(windows) && e.raw_os_error() == Some(17))
}

/// Move a directory: a rename when possible, else copy then delete.
pub fn move_dir(src: &Path, dst: &Path) -> Result<()> {
    if let Some(p) = dst.parent() {
        std::fs::create_dir_all(p).ctx("create", p)?;
    }
    match std::fs::rename(src, dst) {
        Ok(()) => Ok(()),
        Err(e) if crosses_devices(&e) => {
            if let Err(err) = copy_dir(src, dst) {
                let _ = std::fs::remove_dir_all(dst);
                return Err(err);
            }
            std::fs::remove_dir_all(src).ctx("clean up", src)
        }
        Err(e) => Err(Error::io(
            "move",
            src,
            std::io::Error::new(
                e.kind(),
                format!("{e}. If the game is running, close it and try again"),
            ),
        )),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Build a zip from `(path, contents)` pairs.
    pub fn make_zip(dest: &Path, files: &[(&str, &str)]) {
        let mut zip = zip::ZipWriter::new(File::create(dest).unwrap());
        let opts = zip::write::SimpleFileOptions::default();
        for (p, c) in files {
            zip.start_file(*p, opts).unwrap();
            zip.write_all(c.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
    }

    #[test]
    fn inspect_and_round_trip() {
        let t = tempfile::tempdir().unwrap();
        let z = t.path().join("a.zip");
        make_zip(
            &z,
            &[
                ("Foo/Foo.toc", "## Title: Foo"),
                ("Foo/sub/x.lua", "x"),
                ("Foo_Data/Foo_Data.toc", "## Title: D"),
                ("README.md", "hi"),
                ("__MACOSX/Foo/._x", ""),
            ],
        );
        assert_eq!(inspect(&z).unwrap(), vec!["Foo", "Foo_Data"]);
        let out = t.path().join("out");
        extract(&z, &out, &["Foo".into()]).unwrap();
        assert!(out.join("Foo/sub/x.lua").exists());
        assert!(!out.join("Foo_Data").exists());
        let back = t.path().join("back.zip");
        zip_folders(&out, &["Foo".into()], &back).unwrap();
        assert_eq!(inspect(&back).unwrap(), vec!["Foo"]);
    }

    #[test]
    fn rejects_bad_archives() {
        let t = tempfile::tempdir().unwrap();
        let z = t.path().join("a.zip");
        make_zip(&z, &[("Foo/sub/Foo.toc", "")]);
        assert!(matches!(inspect(&z), Err(Error::BadArchive(_))));
        make_zip(&z, &[("../evil/Foo.toc", "")]);
        assert!(matches!(inspect(&z), Err(Error::BadArchive(_))));
        std::fs::write(&z, b"not a zip").unwrap();
        assert!(matches!(inspect(&z), Err(Error::BadArchive(_))));
    }
}
