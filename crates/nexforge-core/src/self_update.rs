//! Opt-in check for a newer Nexforge (off by default).
//!
//! One request to GitHub for the newest release of Nexforge itself. It only
//! reports; nothing is downloaded or installed.

use crate::error::{Error, Result};
use crate::net::{Fetch, Get};
use crate::version::is_newer;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::time::Duration;

/// Where Nexforge's releases are published.
pub const RELEASES_REPO: &str = "PerkinzPie/Nexforge";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewVersion {
    pub version: String,
    /// Release page (github.com), to open in the player's browser.
    pub url: String,
    pub published_at: Option<DateTime<Utc>>,
}

#[derive(Deserialize)]
struct Latest {
    tag_name: String,
    html_url: String,
    published_at: Option<DateTime<Utc>>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
}

/// The newest stable release if it is newer than `current`.
pub async fn check(fetch: &dyn Fetch, current: &str) -> Result<Option<NewVersion>> {
    let req = Get::new(
        format!("https://api.github.com/repos/{RELEASES_REPO}/releases/latest"),
        "Nexforge update check",
    )
    .header("Accept", "application/vnd.github+json")
    .fresh_for(Duration::from_secs(6 * 3600));
    let body = match fetch.get_text(req).await {
        Ok(b) => b,
        // No release published yet.
        Err(Error::Http { status: 404, .. }) => return Ok(None),
        Err(e) => return Err(e),
    };
    let l: Latest = serde_json::from_str(&body).map_err(|e| Error::BadResponse {
        host: "api.github.com".into(),
        message: e.to_string(),
    })?;
    if l.draft || l.prerelease || !is_newer(&l.tag_name, current) {
        return Ok(None);
    }
    Ok(Some(NewVersion {
        version: l.tag_name.trim_start_matches('v').to_string(),
        url: l.html_url,
        published_at: l.published_at,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::FixtureFetch;

    const URL: &str = "https://api.github.com/repos/PerkinzPie/Nexforge/releases/latest";

    #[tokio::test]
    async fn reports_only_newer_stable_releases() {
        let f = FixtureFetch::new();
        f.route(URL, r#"{"tag_name":"v0.2.0","html_url":"https://github.com/PerkinzPie/Nexforge/releases/tag/v0.2.0","published_at":"2026-11-10T12:00:00Z"}"#);
        let n = check(&f, "0.1.0").await.unwrap().unwrap();
        assert_eq!(n.version, "0.2.0");
        assert!(n.url.starts_with("https://github.com/"));
        assert!(check(&f, "0.2.0").await.unwrap().is_none());
        assert!(check(&f, "0.3.0").await.unwrap().is_none());

        f.route(
            URL,
            r#"{"tag_name":"v0.9.0","html_url":"x","prerelease":true}"#,
        );
        assert!(
            check(&f, "0.1.0").await.unwrap().is_none(),
            "pre-releases are ignored"
        );
    }

    #[tokio::test]
    async fn no_release_yet_is_not_an_error() {
        // FixtureFetch answers 404 for unknown URLs, as GitHub does here.
        assert!(check(&FixtureFetch::new(), "0.1.0")
            .await
            .unwrap()
            .is_none());
    }
}
