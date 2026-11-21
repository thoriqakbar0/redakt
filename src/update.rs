use semver::Version;
use serde::Deserialize;
use std::fs::{self, OpenOptions};
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

#[cfg(unix)]
use std::io::Read;

const RELEASES: &str = "https://github.com/thoriqakbar0/redakt/releases";
const LATEST: &str = "https://api.github.com/repos/thoriqakbar0/redakt/releases/latest";
const CURRENT: &str = env!("CARGO_PKG_VERSION");
const METADATA_LIMIT: u64 = 2 * 1024 * 1024;

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    #[cfg(unix)]
    digest: Option<String>,
}

struct Update {
    version: Version,
    #[cfg(unix)]
    folder: String,
    asset: Asset,
}

impl Update {
    fn download_url(&self) -> String {
        let asset = &self.asset.name;
        format!("{RELEASES}/download/v{}/{asset}", self.version)
    }
}

fn platform() -> Result<(&'static str, &'static str), String> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") if cfg!(target_env = "gnu") => {
            Ok(("x86_64-unknown-linux-gnu", "tar.gz"))
        }
        ("linux", "aarch64") if cfg!(target_env = "gnu") => {
            Ok(("aarch64-unknown-linux-gnu", "tar.gz"))
        }
        ("macos", "x86_64") => Ok(("x86_64-apple-darwin", "tar.gz")),
        ("macos", "aarch64") => Ok(("aarch64-apple-darwin", "tar.gz")),
        ("windows", "x86_64") if cfg!(target_env = "msvc") => Ok(("x86_64-pc-windows-msvc", "zip")),
        _ => Err("No prebuilt redakt updates for this platform".into()),
    }
}

fn agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .https_only(true)
        .timeout_global(Some(timeout))
        .build()
        .into()
}

fn latest(agent: &ureq::Agent) -> Result<Option<Update>, String> {
    let (target, extension) = platform()?;
    let mut response = match agent.get(LATEST).call() {
        Ok(response) => response,
        Err(ureq::Error::StatusCode(404)) => return Ok(None),
        Err(error) => return Err(format!("Could not check GitHub Releases: {error}")),
    };
    let bytes = response
        .body_mut()
        .with_config()
        .limit(METADATA_LIMIT)
        .read_to_vec()
        .map_err(|error| format!("Could not read release metadata: {error}"))?;
    let release: Release = serde_json::from_slice(&bytes)
        .map_err(|error| format!("Invalid release metadata: {error}"))?;
    select_update(
        release,
        &Version::parse(CURRENT).unwrap(),
        target,
        extension,
    )
}

fn select_update(
    release: Release,
    current: &Version,
    target: &str,
    extension: &str,
) -> Result<Option<Update>, String> {
    if release.draft || release.prerelease {
        return Ok(None);
    }
    let version = release
        .tag_name
        .strip_prefix('v')
        .and_then(|tag| Version::parse(tag).ok())
        .filter(|version| version.pre.is_empty() && version.build.is_empty())
        .ok_or("GitHub release tag is not a stable redakt version")?;
    if version.cmp_precedence(current).is_le() {
        return Ok(None);
    }
    let folder = format!("redakt-v{version}-{target}");
    let expected = format!("{folder}.{extension}");
    let asset = release
        .assets
        .into_iter()
        .find(|asset| asset.name == expected)
        .ok_or_else(|| format!("Redakt {version} has no archive for this platform"))?;
    Ok(Some(Update {
        version,
        #[cfg(unix)]
        folder,
        asset,
    }))
}

pub fn run(check_only: bool) -> Result<String, String> {
    let agent = agent(Duration::from_secs(30));
    let Some(update) = latest(&agent)? else {
        return Ok(format!(
            "No newer prebuilt redakt release (installed {CURRENT})."
        ));
    };
    if check_only {
        return Ok(format!(
            "Redakt {} is available (installed {CURRENT}).\n{}",
            update.version,
            update.download_url()
        ));
    }
    install_update(&agent, &update)
}

pub fn notify() {
    let mut stderr = io::stderr().lock();
    if std::env::var_os("REDAKT_UPDATE_NOTIFY").as_deref() == Some("0".as_ref())
        || !stderr.is_terminal()
    {
        return;
    }
    let Some(cache) = cache_directory() else {
        return;
    };
    if let Some(message) = notification(&cache, || latest(&agent(Duration::from_secs(2)))) {
        let _ = writeln!(stderr, "{message}");
    }
}

fn notification(
    cache: &Path,
    check: impl FnOnce() -> Result<Option<Update>, String>,
) -> Option<String> {
    if !matches!(reserve_check(cache), Ok(true)) {
        return None;
    }
    let update = check().ok()??;
    Some(format!(
        "Redakt {} is available. Run redakt update --check for details.",
        update.version
    ))
}

fn cache_directory() -> Option<PathBuf> {
    let cache = if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from)
    };
    cache
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::home_dir().map(|home| home.join(".cache")))
        .map(|path| path.join("redakt"))
}

fn reserve_check(cache: &Path) -> io::Result<bool> {
    const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

    fs::create_dir_all(cache)?;
    let mut stamp = OpenOptions::new()
        .read(true)
        .append(true)
        .create(true)
        .open(cache.join("update-check"))?;
    stamp.try_lock()?;
    let now = SystemTime::now();
    let metadata = stamp.metadata()?;
    if metadata.len() != 0
        && now
            .duration_since(metadata.modified()?)
            .is_ok_and(|age| age < CHECK_INTERVAL)
    {
        return Ok(false);
    }

    if metadata.len() == 0 {
        stamp.write_all(b"1")?;
    }
    stamp.set_modified(now)?;
    Ok(true)
}

#[cfg(not(unix))]
fn install_update(_agent: &ureq::Agent, update: &Update) -> Result<String, String> {
    Err(format!(
        "Redakt {} is available. Download and install it from {}",
        update.version,
        update.download_url()
    ))
}

#[cfg(unix)]
fn install_update(agent: &ureq::Agent, update: &Update) -> Result<String, String> {
    const ARCHIVE_LIMIT: u64 = 32 * 1024 * 1024;

    let executable = std::env::current_exe()
        .and_then(|path| path.canonicalize())
        .map_err(|error| format!("Could not locate redakt: {error}"))?;
    let installed = std::fs::File::open(&executable)
        .map_err(|error| format!("Could not open the installed redakt executable: {error}"))?;
    installed
        .try_lock()
        .map_err(|error| format!("Could not lock the installed redakt executable: {error}"))?;

    let digest = update
        .asset
        .digest
        .as_deref()
        .ok_or("GitHub did not provide a SHA-256 digest for this archive")?;
    let bytes = agent
        .get(update.download_url())
        .call()
        .map_err(|error| format!("Could not download the update: {error}"))?
        .body_mut()
        .with_config()
        .limit(ARCHIVE_LIMIT)
        .read_to_vec()
        .map_err(|error| format!("Could not read the update: {error}"))?;
    replace_executable(&executable, &installed, &update.folder, &bytes, digest)
        .map_err(|error| format!("Could not install redakt {}: {error}", update.version))?;
    Ok(format!(
        "Updated redakt from {CURRENT} to {}.",
        update.version
    ))
}

#[cfg(unix)]
fn replace_executable(
    executable: &std::path::Path,
    installed: &std::fs::File,
    folder: &str,
    bytes: &[u8],
    digest: &str,
) -> std::io::Result<()> {
    use sha2::{Digest, Sha256};
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    const UNPACKED_LIMIT: u64 = 64 * 1024 * 1024;

    if digest != format!("sha256:{:x}", Sha256::digest(bytes)) {
        return Err(io::Error::other("Update SHA-256 digest did not match"));
    }

    let expected = std::path::Path::new(folder).join("redakt");
    let decoder = flate2::read::GzDecoder::new(bytes);
    let mut archive = tar::Archive::new(decoder.take(UNPACKED_LIMIT));
    let mut staged = tempfile::NamedTempFile::new_in(
        executable
            .parent()
            .ok_or_else(|| io::Error::other("Executable has no parent directory"))?,
    )?;
    let metadata = installed.metadata()?;
    for entry in archive.entries()? {
        let mut entry = entry?;
        if entry.path()? != expected || !entry.header().entry_type().is_file() {
            continue;
        }
        let size = entry.size();
        if size == 0 || size > UNPACKED_LIMIT || io::copy(&mut entry, &mut staged)? != size {
            return Err(io::Error::other("Invalid executable size"));
        }
        staged.flush()?;
        staged
            .as_file()
            .set_permissions(std::fs::Permissions::from_mode(metadata.mode() & 0o777))?;
        staged.as_file().sync_all()?;

        let current = executable.symlink_metadata()?;
        if (metadata.dev(), metadata.ino()) != (current.dev(), current.ino()) {
            return Err(io::Error::other(
                "Installed executable changed during the update",
            ));
        }
        staged.persist(executable)?;
        return Ok(());
    }
    Err(io::Error::other(
        "Archive does not contain the redakt executable",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_newer_stable_release() {
        let target = "x86_64-unknown-linux-gnu";
        let current = Version::new(1, 9, 0);
        let expected = "redakt-v1.10.0-x86_64-unknown-linux-gnu.tar.gz";
        let release = |tag: &str, draft, prerelease| Release {
            tag_name: tag.into(),
            draft,
            prerelease,
            assets: vec![Asset {
                name: expected.into(),
                #[cfg(unix)]
                digest: None,
            }],
        };
        for ignored in [
            release("v1.8.0", false, false),
            release("v1.9.0", false, false),
            release("v1.10.0", true, false),
            release("v1.10.0-rc.1", false, true),
        ] {
            assert!(select_update(ignored, &current, target, "tar.gz")
                .unwrap()
                .is_none());
        }

        let newer = release("v1.10.0", false, false);
        let update = select_update(newer, &current, target, "tar.gz")
            .unwrap()
            .unwrap();
        assert_eq!(
            update.download_url(),
            format!("{RELEASES}/download/v1.10.0/{expected}")
        );
        let wrong_target = release("v1.10.0", false, false);
        assert!(select_update(wrong_target, &current, "aarch64-apple-darwin", "tar.gz").is_err());
    }

    #[test]
    fn notifies_at_most_once_per_day() {
        let cache = tempfile::tempdir().unwrap();
        let available = || {
            Ok(Some(Update {
                version: Version::new(1, 10, 0),
                #[cfg(unix)]
                folder: "redakt-v1.10.0-x86_64-unknown-linux-gnu".into(),
                asset: Asset {
                    name: "redakt-v1.10.0-x86_64-unknown-linux-gnu.tar.gz".into(),
                    #[cfg(unix)]
                    digest: None,
                },
            }))
        };
        assert!(notification(cache.path(), available)
            .unwrap()
            .starts_with("Redakt 1.10.0 is available."));
        assert!(notification(cache.path(), || panic!("checked twice")).is_none());

        let stamp = OpenOptions::new()
            .write(true)
            .open(cache.path().join("update-check"))
            .unwrap();
        stamp
            .set_modified(SystemTime::now() - Duration::from_secs(25 * 60 * 60))
            .unwrap();
        assert!(notification(cache.path(), available).is_some());
    }

    #[cfg(unix)]
    #[test]
    fn installs_verified_executable() {
        use flate2::{write::GzEncoder, Compression};
        use sha2::{Digest, Sha256};
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        let executable = directory.path().join("redakt");
        fs::write(&executable, b"installed binary").unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o751)).unwrap();
        let installed = fs::File::open(&executable).unwrap();

        let folder = "redakt-v1.10.0-x86_64-unknown-linux-gnu";
        let replacement = b"new binary";
        let mut archive = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::default()));
        let mut header = tar::Header::new_ustar();
        header.set_size(replacement.len() as u64);
        header.set_mode(0o755);
        archive
            .append_data(
                &mut header,
                Path::new(folder).join("redakt"),
                &replacement[..],
            )
            .unwrap();
        let bytes = archive.into_inner().unwrap().finish().unwrap();
        let digest = format!("sha256:{:x}", Sha256::digest(&bytes));

        assert!(replace_executable(&executable, &installed, folder, &bytes, "invalid").is_err());
        assert!(
            replace_executable(&executable, &installed, "wrong-folder", &bytes, &digest).is_err()
        );
        assert_eq!(fs::read(&executable).unwrap(), b"installed binary");
        replace_executable(&executable, &installed, folder, &bytes, &digest).unwrap();
        assert_eq!(fs::read(&executable).unwrap(), replacement);
        assert_eq!(
            executable.metadata().unwrap().permissions().mode() & 0o777,
            0o751
        );
    }
}
