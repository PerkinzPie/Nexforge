//! The only way Nexforge talks to the network.
//!
//! One `reqwest` client (rustls) for API calls and downloads, a separate one
//! for images, a host allowlist enforced on every request and every redirect
//! hop, a line in `network.log` per request, and a small
//! ETag/If-Modified-Since cache on disk.

use crate::error::{Error, IoContext, Result};
use async_trait::async_trait;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use url::Url;

/// Every host Nexforge may contact. Anything else is a bug.
///
/// `release-assets.githubusercontent.com`: GitHub moved release downloads
/// there from `objects.githubusercontent.com` in 2025; `github.com` asset
/// URLs redirect to it (observed 2026-11-04).
pub const ALLOWED_HOSTS: &[&str] = &[
    "api.github.com",
    "github.com",
    "objects.githubusercontent.com",
    "release-assets.githubusercontent.com",
    "api.mmoui.com",
    "cdn.wowinterface.com",
    "www.wowinterface.com",
    "addons.wago.io",
    "api.curseforge.com",
    "edge.forgecdn.net",
    "mediafilez.forgecdn.net",
];

/// Hosts that serve addon icons and screenshots. They are reachable only
/// through [`Fetch::get_image`], and only for responses that are images, so
/// they can never be used for API calls or addon downloads.
///
/// - `cdn-wow.mmoui.com`: WoWInterface `UIIMG_Thumbs` / `UIIMGs` (recorded 2026-11-04).
/// - `avatars.githubusercontent.com`: GitHub `owner.avatar_url` (recorded 2026-11-04).
/// - `cdn.wago.io`: Wago `thumbnail_image` (seen on addons.wago.io, 2026-11-04).
/// - `media.forgecdn.net`: CurseForge `logo` / `screenshots`. Unconfirmed until a
///   CurseForge response is recorded.
pub const IMAGE_HOSTS: &[&str] = &[
    "cdn-wow.mmoui.com",
    "avatars.githubusercontent.com",
    "cdn.wago.io",
    "media.forgecdn.net",
];

/// Largest image Nexforge will fetch.
pub const MAX_IMAGE_BYTES: u64 = 10 * 1024 * 1024;
/// Largest addon download accepted (the biggest real addons are tens of MB).
pub const MAX_DOWNLOAD_BYTES: u64 = 512 * 1024 * 1024;
/// The one host that receives an API key in a header (CurseForge).
const KEYED_API_HOST: &str = "api.curseforge.com";

fn norm(host: &str) -> String {
    host.trim_end_matches('.').to_ascii_lowercase()
}

pub fn host_allowed(host: &str) -> bool {
    ALLOWED_HOSTS.contains(&norm(host).as_str())
}

pub fn image_host_allowed(host: &str) -> bool {
    IMAGE_HOSTS.contains(&norm(host).as_str())
}

/// An icon or screenshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub bytes: Vec<u8>,
    /// `image/png`, `image/jpeg` …
    pub content_type: String,
}

pub fn user_agent() -> String {
    format!(
        "Nexforge/{} (+https://nexabyte.uk)",
        env!("CARGO_PKG_VERSION")
    )
}

/// A GET for a small text/JSON body.
#[derive(Debug, Clone)]
pub struct Get {
    pub url: String,
    /// Why we are asking; written to `network.log`.
    pub reason: String,
    pub headers: Vec<(String, String)>,
    /// Serve from cache without asking the host if the cached copy is younger.
    pub fresh_for: Option<Duration>,
}

impl Get {
    pub fn new(url: impl Into<String>, reason: impl Into<String>) -> Self {
        Get {
            url: url.into(),
            reason: reason.into(),
            headers: Vec::new(),
            fresh_for: None,
        }
    }
    pub fn header(mut self, k: &str, v: impl Into<String>) -> Self {
        self.headers.push((k.into(), v.into()));
        self
    }
    pub fn fresh_for(mut self, d: Duration) -> Self {
        self.fresh_for = Some(d);
        self
    }
}

/// A POST with a JSON body (CurseForge batch lookups). Never cached.
#[derive(Debug, Clone)]
pub struct Post {
    pub url: String,
    pub reason: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl Post {
    pub fn json(
        url: impl Into<String>,
        reason: impl Into<String>,
        body: impl Into<String>,
    ) -> Self {
        Post {
            url: url.into(),
            reason: reason.into(),
            headers: Vec::new(),
            body: body.into(),
        }
    }
    pub fn header(mut self, k: &str, v: impl Into<String>) -> Self {
        self.headers.push((k.into(), v.into()));
        self
    }
}

pub type Progress<'a> = &'a (dyn Fn(u64, Option<u64>) + Send + Sync);

#[async_trait]
pub trait Fetch: Send + Sync {
    async fn get_text(&self, req: Get) -> Result<String>;
    async fn post_json(&self, req: Post) -> Result<String>;
    /// Stream `url` to `dest`. Returns bytes written.
    async fn download(
        &self,
        url: &str,
        reason: &str,
        dest: &Path,
        progress: Progress<'_>,
    ) -> Result<u64>;
    /// Fetch an icon or screenshot from one of [`IMAGE_HOSTS`].
    async fn get_image(&self, url: &str, reason: &str) -> Result<Image>;
}

fn host_path(url: &Url) -> (String, String) {
    (
        url.host_str().unwrap_or("").to_string(),
        url.path().to_string(),
    )
}

fn parse_allowed(url: &str) -> Result<Url> {
    parse_allowed_in(url, host_allowed)
}

fn parse_allowed_in(url: &str, allowed: fn(&str) -> bool) -> Result<Url> {
    let u =
        Url::parse(url).map_err(|_| Error::Other(format!("\"{url}\" is not a web address.")))?;
    if u.scheme() != "https" {
        return Err(Error::HostNotAllowed {
            host: format!("{} (not https)", u.host_str().unwrap_or(url)),
        });
    }
    let host = u.host_str().unwrap_or("");
    if !allowed(host) {
        return Err(Error::HostNotAllowed { host: host.into() });
    }
    Ok(u)
}

#[derive(Serialize, Deserialize)]
struct CacheEntry {
    url: String,
    etag: Option<String>,
    last_modified: Option<String>,
    fetched_at: i64,
    body: String,
}

/// The real network.
pub struct Net {
    client: reqwest::Client,
    image_client: reqwest::Client,
    log_path: Option<PathBuf>,
    cache_dir: Option<PathBuf>,
    log_lock: Mutex<()>,
}

impl Net {
    pub fn new(log_path: Option<PathBuf>, cache_dir: Option<PathBuf>) -> Result<Net> {
        Ok(Net {
            client: build_client(host_allowed)?,
            image_client: build_client(image_host_allowed)?,
            log_path,
            cache_dir,
            log_lock: Mutex::new(()),
        })
    }

    fn log(&self, host: &str, path: &str, status: &str, bytes: u64, reason: &str) {
        let Some(p) = &self.log_path else { return };
        let _guard = self.log_lock.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(p)
        {
            let _ = writeln!(
                f,
                "{} {} {} {} {} {}",
                chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ"),
                host,
                path,
                status,
                bytes,
                reason.replace(['\n', '\r'], " ")
            );
        }
    }

    fn cache_file(&self, url: &str) -> Option<PathBuf> {
        let dir = self.cache_dir.as_ref()?;
        let hash = Sha256::digest(url.as_bytes());
        Some(dir.join(format!("{}.json", hex(&hash[..16]))))
    }

    fn read_cache(&self, url: &str) -> Option<CacheEntry> {
        let p = self.cache_file(url)?;
        let e: CacheEntry = serde_json::from_slice(&std::fs::read(p).ok()?).ok()?;
        (e.url == url).then_some(e)
    }

    fn write_cache(&self, e: &CacheEntry) {
        let Some(p) = self.cache_file(&e.url) else {
            return;
        };
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(bytes) = serde_json::to_vec(e) {
            let tmp = p.with_extension("tmp");
            if std::fs::write(&tmp, bytes).is_ok() {
                let _ = std::fs::rename(tmp, p);
            }
        }
    }

    fn map_send_error(&self, url: &Url, e: reqwest::Error, reason: &str) -> Error {
        let (host, path) = host_path(url);
        // A blocked redirect surfaces as a reqwest error wrapping ours.
        let mut src: Option<&(dyn std::error::Error + 'static)> = Some(&e);
        while let Some(s) = src {
            if let Some(Error::HostNotAllowed { host: blocked }) = s.downcast_ref::<Error>() {
                self.log(
                    blocked,
                    "-",
                    "blocked",
                    0,
                    &format!("{reason} (redirect from {host})"),
                );
                return Error::HostNotAllowed {
                    host: blocked.clone(),
                };
            }
            src = s.source();
        }
        self.log(&host, &path, "error", 0, reason);
        let message = if e.is_timeout() {
            "it took too long to answer".to_string()
        } else if e.is_connect() {
            "the connection failed. Check your internet connection".to_string()
        } else {
            e.to_string()
        };
        Error::Network { host, message }
    }
}

/// A client whose redirects may only land on hosts `allowed` accepts.
fn build_client(allowed: fn(&str) -> bool) -> Result<reqwest::Client> {
    let policy = reqwest::redirect::Policy::custom(move |attempt| {
        if attempt.previous().len() >= 10 {
            return attempt.error("too many redirects");
        }
        let host = attempt.url().host_str().unwrap_or("").to_string();
        // Requests to CurseForge's API carry the player's key in a header
        // that a redirect would forward: never let it leave that host.
        let from_keyed_api = attempt
            .previous()
            .iter()
            .any(|u| u.host_str() == Some(KEYED_API_HOST));
        if from_keyed_api && host != KEYED_API_HOST {
            return attempt.error(Error::HostNotAllowed { host });
        }
        if attempt.url().scheme() == "https" && allowed(&host) {
            attempt.follow()
        } else {
            attempt.error(Error::HostNotAllowed { host })
        }
    });
    reqwest::Client::builder()
        .user_agent(user_agent())
        .connect_timeout(Duration::from_secs(20))
        .read_timeout(Duration::from_secs(20))
        .redirect(policy)
        .build()
        .map_err(|e| Error::Other(format!("Could not start the network client: {e}")))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn rate_limit_error(resp: &reqwest::Response, host: &str) -> Option<Error> {
    let status = resp.status().as_u16();
    if status != 403 && status != 429 {
        return None;
    }
    let h = resp.headers();
    let remaining = h
        .get("x-ratelimit-remaining")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());
    let reset = h
        .get("x-ratelimit-reset")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<i64>().ok());
    if status == 429 || remaining == Some(0) {
        let reset_at = reset
            .and_then(|t| chrono::DateTime::from_timestamp(t, 0))
            .map(|t| t.with_timezone(&chrono::Local).format("%H:%M").to_string())
            .unwrap_or_else(|| "a little later".into());
        let source_name = match host {
            "api.github.com" => "GitHub",
            "api.curseforge.com" => "CurseForge",
            "addons.wago.io" => "Wago",
            h => h,
        }
        .to_string();
        return Some(Error::RateLimited {
            source_name,
            reset_at,
        });
    }
    None
}

#[async_trait]
impl Fetch for Net {
    async fn get_text(&self, req: Get) -> Result<String> {
        let url = match parse_allowed(&req.url) {
            Ok(u) => u,
            Err(e) => {
                let host = Url::parse(&req.url)
                    .ok()
                    .and_then(|u| u.host_str().map(str::to_string))
                    .unwrap_or_else(|| req.url.clone());
                self.log(&host, "-", "blocked", 0, &req.reason);
                return Err(e);
            }
        };
        let cached = self.read_cache(&req.url);
        let now = chrono::Utc::now().timestamp();
        if let (Some(c), Some(fresh)) = (&cached, req.fresh_for) {
            if now - c.fetched_at < fresh.as_secs() as i64 {
                return Ok(c.body.clone());
            }
        }
        let mut rb = self.client.get(url.clone());
        for (k, v) in &req.headers {
            rb = rb.header(k, v);
        }
        if let Some(c) = &cached {
            if let Some(etag) = &c.etag {
                rb = rb.header("If-None-Match", etag);
            }
            if let Some(lm) = &c.last_modified {
                rb = rb.header("If-Modified-Since", lm);
            }
        }
        let resp = rb
            .send()
            .await
            .map_err(|e| self.map_send_error(&url, e, &req.reason))?;
        let (host, path) = host_path(resp.url());
        let status = resp.status();
        if status == reqwest::StatusCode::NOT_MODIFIED {
            if let Some(mut c) = cached {
                self.log(&host, &path, "304", 0, &req.reason);
                c.fetched_at = now;
                self.write_cache(&c);
                return Ok(c.body);
            }
        }
        if let Some(e) = rate_limit_error(&resp, &host) {
            self.log(&host, &path, &status.as_u16().to_string(), 0, &req.reason);
            return Err(e);
        }
        if !status.is_success() {
            self.log(&host, &path, &status.as_u16().to_string(), 0, &req.reason);
            return Err(Error::Http {
                host,
                path,
                status: status.as_u16(),
            });
        }
        let header = |n: &str| {
            resp.headers()
                .get(n)
                .and_then(|v| v.to_str().ok())
                .map(str::to_string)
        };
        let (etag, last_modified) = (header("etag"), header("last-modified"));
        let body = resp
            .text()
            .await
            .map_err(|e| self.map_send_error(&url, e, &req.reason))?;
        self.log(
            &host,
            &path,
            &status.as_u16().to_string(),
            body.len() as u64,
            &req.reason,
        );
        self.write_cache(&CacheEntry {
            url: req.url.clone(),
            etag,
            last_modified,
            fetched_at: now,
            body: body.clone(),
        });
        Ok(body)
    }

    async fn post_json(&self, req: Post) -> Result<String> {
        let url = match parse_allowed(&req.url) {
            Ok(u) => u,
            Err(e) => {
                let host = Url::parse(&req.url)
                    .ok()
                    .and_then(|u| u.host_str().map(str::to_string))
                    .unwrap_or_else(|| req.url.clone());
                self.log(&host, "-", "blocked", 0, &req.reason);
                return Err(e);
            }
        };
        let mut rb = self
            .client
            .post(url.clone())
            .header("Content-Type", "application/json")
            .body(req.body.clone());
        for (k, v) in &req.headers {
            rb = rb.header(k, v);
        }
        let resp = rb
            .send()
            .await
            .map_err(|e| self.map_send_error(&url, e, &req.reason))?;
        let (host, path) = host_path(resp.url());
        let status = resp.status();
        let code = status.as_u16().to_string();
        if let Some(e) = rate_limit_error(&resp, &host) {
            self.log(&host, &path, &code, 0, &req.reason);
            return Err(e);
        }
        if !status.is_success() {
            self.log(&host, &path, &code, 0, &req.reason);
            return Err(Error::Http {
                host,
                path,
                status: status.as_u16(),
            });
        }
        let body = resp
            .text()
            .await
            .map_err(|e| self.map_send_error(&url, e, &req.reason))?;
        self.log(&host, &path, &code, body.len() as u64, &req.reason);
        Ok(body)
    }

    async fn download(
        &self,
        url_str: &str,
        reason: &str,
        dest: &Path,
        progress: Progress<'_>,
    ) -> Result<u64> {
        let url = match parse_allowed(url_str) {
            Ok(u) => u,
            Err(e) => {
                self.log(url_str, "-", "blocked", 0, reason);
                return Err(e);
            }
        };
        let resp = self
            .client
            .get(url.clone())
            .send()
            .await
            .map_err(|e| self.map_send_error(&url, e, reason))?;
        let (host, path) = host_path(resp.url());
        let status = resp.status();
        if !status.is_success() {
            self.log(&host, &path, &status.as_u16().to_string(), 0, reason);
            return Err(Error::Http {
                host,
                path,
                status: status.as_u16(),
            });
        }
        let total = resp.content_length();
        let too_big = |host: &str| Error::BadResponse {
            host: host.to_string(),
            message: "the download is larger than 512 MB, far more than any addon".into(),
        };
        if total.is_some_and(|n| n > MAX_DOWNLOAD_BYTES) {
            self.log(&host, &path, "refused", 0, &format!("{reason} (too large)"));
            return Err(too_big(&host));
        }
        if let Some(dir) = dest.parent() {
            tokio::fs::create_dir_all(dir).await.ctx("create", dir)?;
        }
        let mut file = tokio::fs::File::create(dest).await.ctx("create", dest)?;
        let mut stream = resp.bytes_stream();
        let mut written = 0u64;
        progress(0, total);
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| self.map_send_error(&url, e, reason))?;
            written += chunk.len() as u64;
            // A server can lie about the size; count what actually arrives.
            if written > MAX_DOWNLOAD_BYTES {
                drop(file);
                let _ = tokio::fs::remove_file(dest).await;
                self.log(&host, &path, "refused", 0, &format!("{reason} (too large)"));
                return Err(too_big(&host));
            }
            file.write_all(&chunk).await.ctx("write", dest)?;
            progress(written, total);
        }
        file.flush().await.ctx("write", dest)?;
        let via = if url.host_str() != Some(host.as_str()) {
            format!("{reason} (via {})", url.host_str().unwrap_or(""))
        } else {
            reason.to_string()
        };
        self.log(&host, &path, &status.as_u16().to_string(), written, &via);
        Ok(written)
    }

    async fn get_image(&self, url_str: &str, reason: &str) -> Result<Image> {
        let url = match parse_allowed_in(url_str, image_host_allowed) {
            Ok(u) => u,
            Err(e) => {
                let host = Url::parse(url_str)
                    .ok()
                    .and_then(|u| u.host_str().map(str::to_string))
                    .unwrap_or_else(|| url_str.to_string());
                self.log(&host, "-", "blocked", 0, reason);
                return Err(e);
            }
        };
        let resp = self
            .image_client
            .get(url.clone())
            .header("Accept", "image/*")
            .send()
            .await
            .map_err(|e| self.map_send_error(&url, e, reason))?;
        let (host, path) = host_path(resp.url());
        let status = resp.status();
        let code = status.as_u16().to_string();
        if !status.is_success() {
            self.log(&host, &path, &code, 0, reason);
            return Err(Error::Http {
                host,
                path,
                status: status.as_u16(),
            });
        }
        let content_type = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        let too_big = |host: &str| Error::BadResponse {
            host: host.to_string(),
            message: "the image is larger than 10 MB".into(),
        };
        if resp.content_length().is_some_and(|n| n > MAX_IMAGE_BYTES) {
            self.log(&host, &path, "refused", 0, &format!("{reason} (too large)"));
            return Err(too_big(&host));
        }
        let mut bytes = Vec::new();
        let mut stream = resp.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| self.map_send_error(&url, e, reason))?;
            bytes.extend_from_slice(&chunk);
            if bytes.len() as u64 > MAX_IMAGE_BYTES {
                self.log(&host, &path, "refused", 0, &format!("{reason} (too large)"));
                return Err(too_big(&host));
            }
        }
        // Trust the bytes, never the label: only JPEG, PNG, GIF and WebP
        // pass, whatever the server says. CurseForge's CDN labels its
        // thumbnails `application/x-amz-json-1.0` (seen 2026-11-05), and an
        // SVG labelled `image/svg+xml` can carry script.
        let content_type = if let Some(t) = sniff_image(&bytes) {
            t.to_string()
        } else {
            self.log(
                &host,
                &path,
                "refused",
                0,
                &format!("{reason} (not an image)"),
            );
            return Err(Error::BadResponse {
                host,
                message: format!("expected an image, got \"{content_type}\""),
            });
        };
        self.log(&host, &path, &code, bytes.len() as u64, reason);
        Ok(Image {
            bytes,
            content_type,
        })
    }
}

/// The image type a file's first bytes say it is (JPEG, PNG, GIF or WebP).
fn sniff_image(b: &[u8]) -> Option<&'static str> {
    if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if b.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if b.len() >= 12 && &b[..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

/// `(url, headers)` of one request a [`FixtureFetch`] served.
pub type SentHeaders = (String, Vec<(String, String)>);

/// Replays recorded responses so tests never touch the network. Requests for
/// unknown URLs fail, and the allowlist is still enforced.
#[derive(Default, Clone)]
pub struct FixtureFetch {
    routes: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    statuses: Arc<Mutex<HashMap<String, u16>>>,
    hits: Arc<Mutex<Vec<String>>>,
    posts: Arc<Mutex<Vec<(String, String)>>>,
    headers: Arc<Mutex<Vec<SentHeaders>>>,
}

impl FixtureFetch {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn route(&self, url: &str, body: impl Into<Vec<u8>>) -> &Self {
        self.routes
            .lock()
            .unwrap()
            .insert(url.to_string(), body.into());
        self
    }
    /// Answer `url` with an HTTP error status instead of a body.
    pub fn route_status(&self, url: &str, status: u16) -> &Self {
        self.statuses
            .lock()
            .unwrap()
            .insert(url.to_string(), status);
        self
    }
    pub fn route_file(&self, url: &str, path: impl AsRef<Path>) -> &Self {
        let bytes = std::fs::read(path.as_ref())
            .unwrap_or_else(|e| panic!("fixture {}: {e}", path.as_ref().display()));
        self.route(url, bytes)
    }
    /// URLs requested so far, in order.
    pub fn hits(&self) -> Vec<String> {
        self.hits.lock().unwrap().clone()
    }
    /// `(url, body)` of every POST so far.
    pub fn posts(&self) -> Vec<(String, String)> {
        self.posts.lock().unwrap().clone()
    }
    /// `(url, headers)` of every GET and POST so far.
    pub fn sent_headers(&self) -> Vec<SentHeaders> {
        self.headers.lock().unwrap().clone()
    }

    fn lookup(&self, url: &str) -> Result<Vec<u8>> {
        self.lookup_in(url, host_allowed)
    }

    fn lookup_in(&self, url: &str, allowed: fn(&str) -> bool) -> Result<Vec<u8>> {
        let u = parse_allowed_in(url, allowed)?;
        self.hits.lock().unwrap().push(url.to_string());
        if let Some(status) = self.statuses.lock().unwrap().get(url).copied() {
            let (host, path) = host_path(&u);
            return Err(Error::Http { host, path, status });
        }
        self.routes
            .lock()
            .unwrap()
            .get(url)
            .cloned()
            .ok_or_else(|| {
                let (host, path) = host_path(&u);
                Error::Http {
                    host,
                    path,
                    status: 404,
                }
            })
    }
}

#[async_trait]
impl Fetch for FixtureFetch {
    async fn get_text(&self, req: Get) -> Result<String> {
        self.headers
            .lock()
            .unwrap()
            .push((req.url.clone(), req.headers.clone()));
        Ok(String::from_utf8_lossy(&self.lookup(&req.url)?).into_owned())
    }

    async fn post_json(&self, req: Post) -> Result<String> {
        self.headers
            .lock()
            .unwrap()
            .push((req.url.clone(), req.headers.clone()));
        self.posts
            .lock()
            .unwrap()
            .push((req.url.clone(), req.body.clone()));
        Ok(String::from_utf8_lossy(&self.lookup(&req.url)?).into_owned())
    }

    async fn download(
        &self,
        url: &str,
        _reason: &str,
        dest: &Path,
        progress: Progress<'_>,
    ) -> Result<u64> {
        let bytes = self.lookup(url)?;
        if let Some(dir) = dest.parent() {
            std::fs::create_dir_all(dir).ctx("create", dir)?;
        }
        std::fs::write(dest, &bytes).ctx("write", dest)?;
        progress(bytes.len() as u64, Some(bytes.len() as u64));
        Ok(bytes.len() as u64)
    }

    async fn get_image(&self, url: &str, _reason: &str) -> Result<Image> {
        let bytes = self.lookup_in(url, image_host_allowed)?;
        let path = url
            .split(['?', '#'])
            .next()
            .unwrap_or(url)
            .to_ascii_lowercase();
        let content_type = match path.rsplit('.').next() {
            Some("png") => "image/png",
            Some("jpg" | "jpeg") => "image/jpeg",
            Some("gif") => "image/gif",
            Some("webp") => "image/webp",
            _ => "application/octet-stream",
        };
        Ok(Image {
            bytes,
            content_type: content_type.into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniffs_images_by_their_bytes() {
        // First bytes of real CurseForge thumbnails served as JSON.
        assert_eq!(
            sniff_image(&[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'J']),
            Some("image/jpeg")
        );
        assert_eq!(sniff_image(b"GIF89a\x16\x01"), Some("image/gif"));
        assert_eq!(sniff_image(b"\x89PNG\r\n\x1a\n\0\0"), Some("image/png"));
        assert_eq!(sniff_image(b"RIFF\0\0\0\0WEBPVP8 "), Some("image/webp"));
        assert_eq!(sniff_image(b"{\"__type\":\"x\"}"), None);
        assert_eq!(
            sniff_image(b"<svg xmlns="),
            None,
            "SVG can carry script; never as an image"
        );
        assert_eq!(sniff_image(b""), None);
    }

    #[test]
    fn allowlist() {
        assert!(host_allowed("api.github.com"));
        assert!(host_allowed("API.GITHUB.COM."));
        assert!(!host_allowed("evil.com"));
        assert!(!host_allowed("api.github.com.evil.com"));
        assert!(
            parse_allowed("http://api.github.com/x").is_err(),
            "https only"
        );
        assert!(parse_allowed("https://example.com/").is_err());
    }

    #[test]
    fn image_hosts_are_separate() {
        for h in IMAGE_HOSTS {
            assert!(
                !host_allowed(h),
                "{h} must not serve API calls or downloads"
            );
            assert!(image_host_allowed(h));
        }
        for h in ALLOWED_HOSTS {
            assert!(!image_host_allowed(h), "{h} is not an image host");
        }
    }

    #[tokio::test]
    async fn images_only_through_get_image() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("network.log");
        let net = Net::new(Some(log.clone()), None).unwrap();
        let img = "https://cdn-wow.mmoui.com/preview/tiny/pvw75842.jpg";
        let err = net
            .get_text(Get::new(img, "json from an image host"))
            .await
            .unwrap_err();
        assert!(matches!(err, Error::HostNotAllowed { .. }));
        let err = net
            .get_image("https://api.github.com/repos/a/b", "image from an API host")
            .await
            .unwrap_err();
        assert!(matches!(err, Error::HostNotAllowed { .. }));
        let text = std::fs::read_to_string(log).unwrap();
        assert!(text.contains("cdn-wow.mmoui.com - blocked"), "{text}");
        assert!(text.contains("api.github.com - blocked"), "{text}");

        let fx = FixtureFetch::new();
        fx.route(img, vec![1, 2, 3]);
        let got = fx.get_image(img, "test").await.unwrap();
        assert_eq!(
            (got.bytes.len(), got.content_type.as_str()),
            (3, "image/jpeg")
        );
        assert!(fx.get_text(Get::new(img, "test")).await.is_err());
    }

    #[tokio::test]
    async fn real_client_refuses_unknown_hosts_and_logs() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("network.log");
        let net = Net::new(Some(log.clone()), None).unwrap();
        let err = net
            .get_text(Get::new("https://example.com/x", "test"))
            .await
            .unwrap_err();
        assert!(matches!(err, Error::HostNotAllowed { .. }));
        let text = std::fs::read_to_string(log).unwrap();
        assert!(text.contains("example.com - blocked 0 test"), "{text}");
    }
}
