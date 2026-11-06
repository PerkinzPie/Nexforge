//! The local catalogue behind Browse: SQLite with an FTS5 index.
//!
//! Searches hit this file only, so typing never waits on the network. It is a
//! cache: hosts refill it, and a missing or damaged file is simply rebuilt.

use crate::error::{Error, Result};
use crate::flavour::Flavour;
use crate::sources::{CatalogueEntry, SourceId};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::Mutex;

pub struct Catalogue {
    conn: Mutex<Connection>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Stats {
    pub total: u64,
    pub per_source: Vec<(SourceId, u64)>,
    pub refreshed_at: Option<DateTime<Utc>>,
}

const SCHEMA: &str = "
PRAGMA journal_mode = WAL;
CREATE TABLE IF NOT EXISTS entries (
  id INTEGER PRIMARY KEY,
  source TEXT NOT NULL,
  source_id TEXT NOT NULL,
  name TEXT NOT NULL,
  authors TEXT NOT NULL DEFAULT '',
  summary TEXT,
  flavours TEXT NOT NULL DEFAULT '',
  downloads INTEGER,
  downloads_monthly INTEGER,
  updated_at INTEGER,
  icon_url TEXT,
  UNIQUE (source, source_id)
);
CREATE VIRTUAL TABLE IF NOT EXISTS entries_fts USING fts5(
  name, authors, summary, tokenize = 'unicode61 remove_diacritics 2'
);
CREATE TABLE IF NOT EXISTS refreshed (
  source TEXT PRIMARY KEY,
  at INTEGER NOT NULL
);
";

fn db_err(e: rusqlite::Error) -> Error {
    Error::Other(format!("The local addon index could not be read: {e}"))
}

fn flavours_text(f: &[Flavour]) -> String {
    f.iter().map(|x| x.as_str()).collect::<Vec<_>>().join(",")
}

fn parse_flavours(s: &str) -> Vec<Flavour> {
    s.split(',').filter_map(|x| x.parse().ok()).collect()
}

/// `bag nan` -> `"bag"* "nan"*`: every word, as a prefix, all required.
fn fts_query(q: &str) -> Option<String> {
    let words: Vec<String> = q
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| format!("\"{}\"*", w.replace('"', "")))
        .collect();
    (!words.is_empty()).then(|| words.join(" "))
}

/// The words of a search, lower-cased; symbols ("Forever++") are dropped.
fn query_words(q: &str) -> Vec<String> {
    q.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// Does every word of `q` appear in the entry's name, authors or summary?
/// Live host searches (CurseForge) also return loose matches; this keeps
/// them to what the local catalogue would have found.
pub fn matches_words(q: &str, e: &CatalogueEntry) -> bool {
    let hay = format!(
        "{} {} {}",
        e.name,
        e.authors.join(" "),
        e.summary.as_deref().unwrap_or("")
    )
    .to_lowercase();
    query_words(q).iter().all(|w| hay.contains(w.as_str()))
}

/// How well an entry's name matches `q`, best first: 0 the exact name,
/// 1 the name starts with it, 2 the name contains it (symbols and all),
/// 3 the name holds every word, 4 only the authors or summary match.
pub fn relevance(q: &str, e: &CatalogueEntry) -> u8 {
    let (name, ql) = (e.name.to_lowercase(), q.trim().to_lowercase());
    if name == ql {
        0
    } else if name.starts_with(&ql) {
        1
    } else if name.contains(&ql) {
        2
    } else if query_words(q).iter().all(|w| name.contains(w.as_str())) {
        3
    } else {
        4
    }
}

impl Catalogue {
    /// Open (or create) the catalogue. A damaged file is deleted and rebuilt.
    pub fn open(path: &Path) -> Result<Catalogue> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| Error::io("create", dir, e))?;
        }
        let open = |p: &Path| -> rusqlite::Result<Connection> {
            let c = Connection::open(p)?;
            c.busy_timeout(std::time::Duration::from_secs(3))?;
            c.execute_batch(SCHEMA)?;
            Ok(c)
        };
        let conn = match open(path) {
            Ok(c) => c,
            Err(_) => {
                for ext in ["", "-wal", "-shm"] {
                    let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
                }
                open(path).map_err(db_err)?
            }
        };
        Ok(Catalogue {
            conn: Mutex::new(conn),
        })
    }

    /// An in-memory catalogue (tests).
    pub fn in_memory() -> Result<Catalogue> {
        let c = Connection::open_in_memory().map_err(db_err)?;
        c.execute_batch(SCHEMA).map_err(db_err)?;
        Ok(Catalogue {
            conn: Mutex::new(c),
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Replace everything from `source` with `entries` (a full refresh).
    pub fn replace_source(&self, source: SourceId, entries: &[CatalogueEntry]) -> Result<()> {
        let mut conn = self.lock();
        let tx = conn.transaction().map_err(db_err)?;
        tx.execute(
            "DELETE FROM entries_fts WHERE rowid IN (SELECT id FROM entries WHERE source = ?1)",
            [source.as_str()],
        )
        .map_err(db_err)?;
        tx.execute("DELETE FROM entries WHERE source = ?1", [source.as_str()])
            .map_err(db_err)?;
        insert_all(&tx, entries)?;
        tx.execute(
            "INSERT INTO refreshed (source, at) VALUES (?1, ?2)
             ON CONFLICT(source) DO UPDATE SET at = excluded.at",
            params![source.as_str(), Utc::now().timestamp()],
        )
        .map_err(db_err)?;
        tx.commit().map_err(db_err)
    }

    /// Add or update entries without touching the rest (live search results).
    pub fn upsert(&self, entries: &[CatalogueEntry]) -> Result<()> {
        let mut conn = self.lock();
        let tx = conn.transaction().map_err(db_err)?;
        for e in entries {
            tx.execute(
                "DELETE FROM entries_fts WHERE rowid IN (SELECT id FROM entries WHERE source = ?1 AND source_id = ?2)",
                params![e.source.as_str(), e.source_id],
            )
            .map_err(db_err)?;
            tx.execute(
                "DELETE FROM entries WHERE source = ?1 AND source_id = ?2",
                params![e.source.as_str(), e.source_id],
            )
            .map_err(db_err)?;
        }
        insert_all(&tx, entries)?;
        tx.commit().map_err(db_err)
    }

    /// When `source` was last fully refreshed.
    pub fn refreshed_at(&self, source: SourceId) -> Option<DateTime<Utc>> {
        self.lock()
            .query_row(
                "SELECT at FROM refreshed WHERE source = ?1",
                [source.as_str()],
                |r| r.get::<_, i64>(0),
            )
            .optional()
            .ok()
            .flatten()
            .and_then(|t| DateTime::from_timestamp(t, 0))
    }

    /// Full-text search over name, authors and summary. Names that start
    /// with the query rank first, then relevance, then downloads.
    pub fn search(&self, q: &str, limit: usize) -> Result<Vec<CatalogueEntry>> {
        let conn = self.lock();
        let Some(fts) = fts_query(q) else {
            return Ok(vec![]);
        };
        let mut stmt = conn
            .prepare(
                "SELECT e.source, e.source_id, e.name, e.authors, e.summary, e.flavours,
                        e.downloads, e.downloads_monthly, e.updated_at, e.icon_url
                 FROM entries_fts f JOIN entries e ON e.id = f.rowid
                 WHERE entries_fts MATCH ?1
                 ORDER BY (lower(e.name) LIKE lower(?2) || '%') DESC,
                          bm25(entries_fts, 10.0, 3.0, 1.0),
                          coalesce(e.downloads, 0) DESC
                 LIMIT ?3",
            )
            .map_err(db_err)?;
        let rows = stmt
            .query_map(params![fts, q.trim(), limit as i64], row)
            .map_err(db_err)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(db_err)
    }

    /// Most downloaded this month among addons listing `flavour`; `None`
    /// for every flavour.
    pub fn popular(&self, flavour: Flavour, limit: usize) -> Result<Vec<CatalogueEntry>> {
        self.popular_in(Some(flavour), limit)
    }

    pub fn popular_in(
        &self,
        flavour: Option<Flavour>,
        limit: usize,
    ) -> Result<Vec<CatalogueEntry>> {
        let conn = self.lock();
        let mut stmt = conn
            .prepare(
                "SELECT source, source_id, name, authors, summary, flavours,
                        downloads, downloads_monthly, updated_at, icon_url
                 FROM entries
                 WHERE (',' || flavours || ',') LIKE ?1 AND coalesce(downloads_monthly, 0) > 0
                 ORDER BY downloads_monthly DESC
                 LIMIT ?2",
            )
            .map_err(db_err)?;
        let rows = stmt
            .query_map(
                params![
                    flavour
                        .map(|f| format!("%,{},%", f.as_str()))
                        .unwrap_or_else(|| "%".into()),
                    limit as i64
                ],
                row,
            )
            .map_err(db_err)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(db_err)
    }

    /// Most downloaded of all time from one host (Browse with nothing typed
    /// and a host chip picked).
    pub fn top_of_source(&self, source: SourceId, limit: usize) -> Result<Vec<CatalogueEntry>> {
        let conn = self.lock();
        let mut stmt = conn
            .prepare(
                "SELECT source, source_id, name, authors, summary, flavours,
                        downloads, downloads_monthly, updated_at, icon_url
                 FROM entries WHERE source = ?1
                 ORDER BY coalesce(downloads, 0) DESC, name
                 LIMIT ?2",
            )
            .map_err(db_err)?;
        let rows = stmt
            .query_map(params![source.as_str(), limit as i64], row)
            .map_err(db_err)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(db_err)
    }

    /// The icon known for one addon, if any.
    pub fn icon(&self, source: SourceId, source_id: &str) -> Option<String> {
        self.lock()
            .query_row(
                "SELECT icon_url FROM entries WHERE source = ?1 AND source_id = ?2",
                params![source.as_str(), source_id],
                |r| r.get::<_, Option<String>>(0),
            )
            .optional()
            .ok()
            .flatten()
            .flatten()
    }

    pub fn stats(&self) -> Stats {
        let conn = self.lock();
        let per_source: Vec<(SourceId, u64)> = conn
            .prepare("SELECT source, count(*) FROM entries GROUP BY source ORDER BY source")
            .and_then(|mut s| {
                s.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
                    .map(|rows| {
                        rows.flatten()
                            .filter_map(|(s, n)| Some((s.parse().ok()?, n as u64)))
                            .collect()
                    })
            })
            .unwrap_or_default();
        let refreshed_at = conn
            .query_row("SELECT max(at) FROM refreshed", [], |r| {
                r.get::<_, Option<i64>>(0)
            })
            .ok()
            .flatten()
            .and_then(|t| DateTime::from_timestamp(t, 0));
        Stats {
            total: per_source.iter().map(|(_, n)| n).sum(),
            per_source,
            refreshed_at,
        }
    }
}

fn insert_all(tx: &rusqlite::Transaction<'_>, entries: &[CatalogueEntry]) -> Result<()> {
    let mut ins = tx
        .prepare(
            "INSERT OR REPLACE INTO entries (source, source_id, name, authors, summary, flavours,
                                             downloads, downloads_monthly, updated_at, icon_url)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        )
        .map_err(db_err)?;
    let mut fts = tx
        .prepare("INSERT INTO entries_fts (rowid, name, authors, summary) VALUES (?1, ?2, ?3, ?4)")
        .map_err(db_err)?;
    for e in entries {
        let authors = e.authors.join(", ");
        ins.execute(params![
            e.source.as_str(),
            e.source_id,
            e.name,
            authors,
            e.summary,
            flavours_text(&e.flavours),
            e.downloads.map(|d| d as i64),
            e.downloads_monthly.map(|d| d as i64),
            e.updated_at.map(|t| t.timestamp()),
            e.icon_url,
        ])
        .map_err(db_err)?;
        let id = tx.last_insert_rowid();
        fts.execute(params![
            id,
            e.name,
            authors,
            e.summary.as_deref().unwrap_or("")
        ])
        .map_err(db_err)?;
    }
    Ok(())
}

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<CatalogueEntry> {
    let source: String = r.get(0)?;
    let authors: String = r.get(3)?;
    Ok(CatalogueEntry {
        source: source.parse().unwrap_or(SourceId::Github),
        source_id: r.get(1)?,
        name: r.get(2)?,
        authors: authors
            .split(", ")
            .filter(|a| !a.is_empty())
            .map(str::to_string)
            .collect(),
        summary: r.get(4)?,
        flavours: parse_flavours(&r.get::<_, String>(5)?),
        downloads: r.get::<_, Option<i64>>(6)?.map(|d| d as u64),
        downloads_monthly: r.get::<_, Option<i64>>(7)?.map(|d| d as u64),
        updated_at: r
            .get::<_, Option<i64>>(8)?
            .and_then(|t| DateTime::from_timestamp(t, 0)),
        icon_url: r.get(9)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(
        source: SourceId,
        id: &str,
        name: &str,
        author: &str,
        monthly: u64,
        fl: &[Flavour],
    ) -> CatalogueEntry {
        CatalogueEntry {
            source,
            source_id: id.into(),
            name: name.into(),
            authors: vec![author.into()],
            summary: Some(format!("{name} does things")),
            flavours: fl.to_vec(),
            downloads: Some(monthly * 10),
            downloads_monthly: Some(monthly),
            updated_at: None,
            icon_url: Some(format!("https://cdn-wow.mmoui.com/{id}.jpg")),
        }
    }

    #[test]
    fn relevance_and_word_matching() {
        let cf = SourceId::Curseforge;
        let exact = e(cf, "1", "Forever++", "x", 1, &[]);
        let named = e(cf, "2", "AtlasLoot Classic Forever", "x", 1, &[]);
        let mut summary_only = e(cf, "3", "Baganator", "plusmouse", 9, &[]);
        summary_only.summary = Some("Bags for Forever and Classic".into());
        let mut unrelated = e(cf, "4", "Chattynator", "plusmouse", 9, &[]);
        unrelated.summary = Some("Chat frames".into());

        assert_eq!(relevance("forever++", &exact), 0, "exact, symbols and all");
        assert_eq!(relevance("Forever++", &named), 3, "every word in the name");
        assert_eq!(relevance("Forever++", &summary_only), 4);
        assert_eq!(relevance("atlas", &named), 1);
        assert_eq!(relevance("loot classic", &named), 2);

        // A loose live match with none of the words anywhere is dropped.
        assert!(matches_words("Forever++", &summary_only));
        assert!(!matches_words("Forever++", &unrelated));
        assert!(matches_words("plusmouse chat", &unrelated), "authors count");
    }

    #[test]
    fn search_ranks_prefix_names_first() {
        let c = Catalogue::in_memory().unwrap();
        c.replace_source(
            SourceId::Wowinterface,
            &[
                e(
                    SourceId::Wowinterface,
                    "1",
                    "AdiBags",
                    "Adirelle",
                    10,
                    &[Flavour::Retail],
                ),
                e(
                    SourceId::Wowinterface,
                    "2",
                    "Bagnon",
                    "Tuller",
                    50,
                    &[Flavour::Forever, Flavour::ClassicEra],
                ),
                e(
                    SourceId::Wowinterface,
                    "3",
                    "BagSync",
                    "Xruptor",
                    5,
                    &[Flavour::Forever],
                ),
                e(
                    SourceId::Wowinterface,
                    "4",
                    "Questie",
                    "Aero",
                    900,
                    &[Flavour::Forever],
                ),
            ],
        )
        .unwrap();
        let names: Vec<String> = c
            .search("bag", 10)
            .unwrap()
            .into_iter()
            .map(|x| x.name)
            .collect();
        assert_eq!(
            names[..2],
            ["Bagnon", "BagSync"],
            "names starting with the query first: {names:?}"
        );
        assert!(!names.contains(&"Questie".to_string()));
        // Author search, and diacritics/case do not matter.
        assert_eq!(c.search("TULLER", 5).unwrap()[0].name, "Bagnon");
        // Every word must match.
        assert!(c.search("bag questie", 5).unwrap().is_empty());
        let hit = &c.search("questie", 1).unwrap()[0];
        assert_eq!(hit.flavours, vec![Flavour::Forever]);
        assert_eq!(
            c.icon(SourceId::Wowinterface, "4").as_deref(),
            Some("https://cdn-wow.mmoui.com/4.jpg")
        );
    }

    #[test]
    fn refresh_replaces_one_source_and_upsert_adds() {
        let c = Catalogue::in_memory().unwrap();
        c.replace_source(
            SourceId::Github,
            &[e(SourceId::Github, "a/b", "Questie", "Questie", 0, &[])],
        )
        .unwrap();
        c.replace_source(
            SourceId::Wowinterface,
            &[e(SourceId::Wowinterface, "1", "Old", "x", 1, &[])],
        )
        .unwrap();
        c.replace_source(
            SourceId::Wowinterface,
            &[e(SourceId::Wowinterface, "2", "New", "x", 1, &[])],
        )
        .unwrap();
        assert!(c.search("old", 5).unwrap().is_empty(), "replaced");
        assert_eq!(
            c.search("questie", 5).unwrap().len(),
            1,
            "other source kept"
        );
        c.upsert(&[e(
            SourceId::Curseforge,
            "77",
            "Dungeon Journal",
            "ExehnTV",
            3,
            &[Flavour::Forever],
        )])
        .unwrap();
        c.upsert(&[e(
            SourceId::Curseforge,
            "77",
            "DungeonJournal",
            "ExehnTV",
            4,
            &[Flavour::Forever],
        )])
        .unwrap();
        assert_eq!(
            c.search("dungeon", 5).unwrap().len(),
            1,
            "upsert replaces the same id"
        );
        let s = c.stats();
        assert_eq!(s.total, 3);
        assert!(s.refreshed_at.is_some());
        assert!(c.refreshed_at(SourceId::Wowinterface).is_some());
        assert!(
            c.refreshed_at(SourceId::Curseforge).is_none(),
            "upsert is not a full refresh"
        );
    }

    #[test]
    fn popular_for_a_flavour() {
        let c = Catalogue::in_memory().unwrap();
        c.replace_source(
            SourceId::Wowinterface,
            &[
                e(
                    SourceId::Wowinterface,
                    "1",
                    "A",
                    "x",
                    10,
                    &[Flavour::Forever],
                ),
                e(
                    SourceId::Wowinterface,
                    "2",
                    "B",
                    "x",
                    99,
                    &[Flavour::ClassicEra],
                ),
                e(
                    SourceId::Wowinterface,
                    "3",
                    "C",
                    "x",
                    50,
                    &[Flavour::Forever, Flavour::ClassicEra],
                ),
            ],
        )
        .unwrap();
        let names: Vec<String> = c
            .popular(Flavour::Forever, 5)
            .unwrap()
            .into_iter()
            .map(|x| x.name)
            .collect();
        assert_eq!(names, ["C", "A"]);
        let top: Vec<String> = c
            .top_of_source(SourceId::Wowinterface, 2)
            .unwrap()
            .into_iter()
            .map(|x| x.name)
            .collect();
        assert_eq!(top, ["B", "C"], "by all-time downloads");
        let any: Vec<String> = c
            .popular_in(None, 5)
            .unwrap()
            .into_iter()
            .map(|x| x.name)
            .collect();
        assert_eq!(any, ["B", "C", "A"]);
    }

    #[test]
    fn damaged_file_is_rebuilt() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("catalogue.db");
        std::fs::write(&p, b"this is not a database").unwrap();
        let c = Catalogue::open(&p).unwrap();
        c.upsert(&[e(SourceId::Github, "a/b", "X", "y", 0, &[])])
            .unwrap();
        assert_eq!(c.stats().total, 1);
    }
}
