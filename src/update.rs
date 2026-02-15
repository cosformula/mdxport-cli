use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use semver::Version;
use serde::{Deserialize, Serialize};

const NPM_LATEST_URL: &str = "https://registry.npmjs.org/@mdxport/cli/latest";
const CACHE_TTL_SECONDS: u64 = 86_400;
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const UPDATE_OPT_OUT_ENV: &str = "MDXPORT_NO_UPDATE_CHECK";

#[derive(Debug, Deserialize)]
struct NpmLatestResponse {
    version: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct UpdateCache {
    latest: String,
    checked_at: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InstallChannel {
    Npm,
    Cargo,
    Brew,
    Unknown,
}

pub fn check_for_updates() {
    if std::env::var(UPDATE_OPT_OUT_ENV).ok().as_deref() == Some("1") {
        return;
    }

    let Some(cache_path) = cache_file_path() else {
        return;
    };

    let now = unix_timestamp();
    let latest = read_cached_latest(&cache_path, now).or_else(|| {
        let latest = fetch_latest_version()?;
        let _ = write_cache(&cache_path, &latest, now);
        Some(latest)
    });

    let Some(latest) = latest else {
        return;
    };

    if !is_newer_than(CURRENT_VERSION, &latest) {
        return;
    }

    print_update_notice(CURRENT_VERSION, &latest, detect_install_channel());
}

fn fetch_latest_version() -> Option<String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .ok()?;

    let response = client.get(NPM_LATEST_URL).send().ok()?;
    if !response.status().is_success() {
        return None;
    }

    let body = response.text().ok()?;
    let parsed = serde_json::from_str::<NpmLatestResponse>(&body).ok()?;
    Some(parsed.version)
}

fn read_cached_latest(cache_path: &Path, now: u64) -> Option<String> {
    let cache = fs::read_to_string(cache_path).ok()?;
    let parsed = serde_json::from_str::<UpdateCache>(&cache).ok()?;
    if now.saturating_sub(parsed.checked_at) <= CACHE_TTL_SECONDS {
        Some(parsed.latest)
    } else {
        None
    }
}

fn write_cache(cache_path: &Path, latest: &str, checked_at: u64) -> io::Result<()> {
    if let Some(parent) = cache_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let cache = UpdateCache {
        latest: latest.to_string(),
        checked_at,
    };
    let payload = serde_json::to_vec(&cache)
        .map_err(|error| io::Error::other(format!("serialize update cache: {error}")))?;
    fs::write(cache_path, payload)
}

fn cache_file_path() -> Option<PathBuf> {
    home_dir().map(|home| home.join(".mdxport").join("update-check.json"))
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn is_newer_than(current: &str, latest: &str) -> bool {
    let Ok(current_version) = Version::parse(current) else {
        return false;
    };
    let Ok(latest_version) = Version::parse(latest) else {
        return false;
    };
    latest_version > current_version
}

fn detect_install_channel() -> InstallChannel {
    let Ok(exe_path) = std::env::current_exe() else {
        return InstallChannel::Unknown;
    };
    detect_install_channel_from_path(&exe_path, home_dir().as_deref())
}

fn detect_install_channel_from_path(exe_path: &Path, home: Option<&Path>) -> InstallChannel {
    let normalized = normalize_path(exe_path);

    if normalized.contains("node_modules")
        || normalized.contains("/npm/")
        || normalized.contains("/npx/")
    {
        return InstallChannel::Npm;
    }

    if normalized.contains(".cargo/bin") {
        return InstallChannel::Cargo;
    }

    if let Some(home) = home {
        let cargo_bin_prefix = format!("{}/.cargo/bin/", normalize_path(home));
        if normalized.starts_with(&cargo_bin_prefix) {
            return InstallChannel::Cargo;
        }
    }

    if normalized.contains("homebrew")
        || normalized.contains("cellar")
        || normalized.contains("linuxbrew")
    {
        return InstallChannel::Brew;
    }

    InstallChannel::Unknown
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase()
}

fn print_update_notice(current: &str, latest: &str, channel: InstallChannel) {
    eprintln!("A new version of mdxport is available: {current} → {latest}");
    match channel {
        InstallChannel::Npm => {
            eprintln!("  Update: npm update -g @mdxport/cli");
        }
        InstallChannel::Cargo => {
            eprintln!("  Update: cargo install mdxport");
        }
        InstallChannel::Brew => {
            eprintln!("  Update: brew upgrade mdxport");
        }
        InstallChannel::Unknown => {
            eprintln!("  Update (npm): npm update -g @mdxport/cli");
            eprintln!("  Update (cargo): cargo install mdxport");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_semver_versions() {
        assert!(is_newer_than("0.2.1", "0.3.0"));
        assert!(!is_newer_than("0.3.0", "0.2.1"));
        assert!(!is_newer_than("0.2.1", "not-a-version"));
    }

    #[test]
    fn reads_fresh_cache_only() {
        let path = temp_cache_path();
        write_cache(&path, "0.9.0", 100).expect("write cache");

        assert_eq!(
            read_cached_latest(&path, 100 + CACHE_TTL_SECONDS),
            Some("0.9.0".to_string())
        );
        assert_eq!(read_cached_latest(&path, 100 + CACHE_TTL_SECONDS + 1), None);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn detects_npm_install_channel() {
        let channel = detect_install_channel_from_path(
            Path::new("/tmp/project/node_modules/.bin/mdxport"),
            Some(Path::new("/Users/alice")),
        );
        assert_eq!(channel, InstallChannel::Npm);
    }

    #[test]
    fn detects_cargo_install_channel() {
        let channel = detect_install_channel_from_path(
            Path::new("/Users/alice/.cargo/bin/mdxport"),
            Some(Path::new("/Users/alice")),
        );
        assert_eq!(channel, InstallChannel::Cargo);
    }

    #[test]
    fn detects_brew_install_channel() {
        let channel = detect_install_channel_from_path(
            Path::new("/opt/homebrew/Cellar/mdxport/0.2.1/bin/mdxport"),
            Some(Path::new("/Users/alice")),
        );
        assert_eq!(channel, InstallChannel::Brew);
    }

    #[test]
    fn falls_back_to_unknown_channel() {
        let channel = detect_install_channel_from_path(
            Path::new("/usr/local/bin/mdxport"),
            Some(Path::new("/Users/alice")),
        );
        assert_eq!(channel, InstallChannel::Unknown);
    }

    fn temp_cache_path() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "mdxport-update-check-test-{}-{nonce}.json",
            std::process::id()
        ))
    }
}
