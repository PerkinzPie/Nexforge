//! Loose version comparison. Addon versions are free text (`v12.0.10`,
//! `10.3.14`, `bundle/v12.5.0+v1.0.10`, `r1234`), so compare the first dotted
//! number run and fall back to string inequality.

use std::cmp::Ordering;

fn numeric_run(s: &str) -> Option<Vec<u64>> {
    let start = s.find(|c: char| c.is_ascii_digit())?;
    let run: String = s[start..]
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    let parts: Vec<u64> = run
        .split('.')
        .filter(|p| !p.is_empty())
        .filter_map(|p| p.parse().ok())
        .collect();
    (!parts.is_empty()).then_some(parts)
}

/// Compare two version strings. `None` when they cannot be ordered.
pub fn compare(a: &str, b: &str) -> Option<Ordering> {
    if a.trim() == b.trim() {
        return Some(Ordering::Equal);
    }
    let (x, y) = (numeric_run(a)?, numeric_run(b)?);
    let n = x.len().max(y.len());
    for i in 0..n {
        let (p, q) = (
            x.get(i).copied().unwrap_or(0),
            y.get(i).copied().unwrap_or(0),
        );
        match p.cmp(&q) {
            Ordering::Equal => continue,
            o => return Some(o),
        }
    }
    // Same numbers: equal if both are plain ("v12.4" vs "12.4.0", or a file
    // name around the number, "ForeverDungeonJournal1.6.6.zip" vs "1.6.6"),
    // otherwise the text differs ("1.2.3" vs "1.2.3-beta") and they are
    // unordered.
    let plain = |s: &str| {
        let s = s.trim();
        let s = s
            .strip_suffix(".zip")
            .or_else(|| s.strip_suffix(".ZIP"))
            .unwrap_or(s);
        let Some(start) = s.find(|c: char| c.is_ascii_digit()) else {
            return false;
        };
        s[start..].chars().all(|c| c.is_ascii_digit() || c == '.')
    };
    (plain(a) && plain(b)).then_some(Ordering::Equal)
}

/// Is `candidate` an update over `installed`?
///
/// Ordered versions compare numerically; unorderable ones count as an update
/// when they differ, so a host that changes its scheme still surfaces.
pub fn is_newer(candidate: &str, installed: &str) -> bool {
    if installed.trim().is_empty() {
        return true;
    }
    match compare(candidate, installed) {
        Some(o) => o == Ordering::Greater,
        None => candidate.trim() != installed.trim(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares() {
        assert!(is_newer("v12.1.2", "v12.0.10"));
        assert!(!is_newer("v12.0.10", "v12.0.10"));
        assert!(!is_newer("v12.0.9", "v12.0.10"));
        assert!(is_newer("bundle/v12.5.0+v1.0.10", "10.3.14"));
        assert!(is_newer("12.5.0", "v12.4"));
        assert!(!is_newer("12.4.0", "v12.4"));
        assert!(is_newer("1.2.3-beta", "1.2.3"));
        assert!(is_newer("abc", ""));
        assert!(is_newer("release-b", "release-a"));
        // A version once recorded as the file name is the same version.
        assert!(!is_newer("1.6.6", "ForeverDungeonJournal1.6.6.zip"));
        assert!(is_newer("1.6.7", "ForeverDungeonJournal1.6.6.zip"));
        assert!(is_newer("1.6.6-beta", "ForeverDungeonJournal1.6.6.zip"));
    }
}
