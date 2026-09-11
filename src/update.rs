use crate::{
    client,
    error::{Error, Result},
    output,
};
use flate2::read::GzDecoder;
use semver::Version;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{io::Write, path::Path};

const RELEASE_URL: &str = "https://api.github.com/repos/oneortwo/mlc-cli/releases/latest";

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    #[serde(default)]
    body: Option<String>,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

impl Release {
    fn asset(&self, name: &str) -> Result<&str> {
        self.assets
            .iter()
            .find(|asset| asset.name == name)
            .map(|asset| asset.browser_download_url.as_str())
            .filter(|url| url.starts_with("https://github.com/oneortwo/mlc-cli/releases/download/"))
            .ok_or_else(|| Error::new(1, &format!("Release is missing a valid {name} asset")))
    }
}

fn target(os: &str, arch: &str) -> Result<String> {
    let os = match os {
        "macos" => "apple-darwin",
        "linux" => "unknown-linux-gnu",
        _ => return Err(Error::new(1, "Unsupported platform for self-update")),
    };
    match arch {
        "aarch64" | "x86_64" => Ok(format!("{arch}-{os}")),
        _ => Err(Error::new(1, "Unsupported architecture for self-update")),
    }
}

fn newer(tag: &str, current: &str) -> Result<bool> {
    let latest = Version::parse(tag.strip_prefix('v').unwrap_or(tag))
        .map_err(|_| Error::new(1, "Release has an invalid version"))?;
    let current =
        Version::parse(current).map_err(|_| Error::new(1, "Current version is invalid"))?;
    Ok(latest > current)
}

fn verify(archive: &[u8], manifest: &[u8], name: &str) -> Result<()> {
    let manifest =
        std::str::from_utf8(manifest).map_err(|_| Error::new(1, "Invalid release checksums"))?;
    let expected = manifest
        .lines()
        .find_map(|line| {
            let mut parts = line.split_whitespace();
            let hash = parts.next()?;
            let file = parts.next()?.trim_start_matches('*');
            (file == name).then_some(hash)
        })
        .ok_or_else(|| Error::new(1, "Release checksum is missing for this platform"))?;
    let actual = format!("{:x}", Sha256::digest(archive));
    if !actual.eq_ignore_ascii_case(expected) {
        return Err(Error::new(
            1,
            "Update checksum mismatch; executable was not changed",
        ));
    }
    Ok(())
}

fn install(archive: &[u8], executable: &Path) -> Result<()> {
    // Stage beside the executable so the final rename is atomic, even when /tmp
    // lives on another filesystem. Never unpack archive paths onto the filesystem.
    let parent = executable
        .parent()
        .ok_or_else(|| Error::new(1, "Invalid executable path"))?;
    let mut staged = tempfile::NamedTempFile::new_in(parent)
        .map_err(|_| Error::new(1, "Cannot write to executable directory"))?;
    let mut tar = tar::Archive::new(GzDecoder::new(archive));
    let mut found = false;
    for entry in tar
        .entries()
        .map_err(|_| Error::new(1, "Invalid update archive"))?
    {
        let mut entry = entry.map_err(|_| Error::new(1, "Invalid update archive entry"))?;
        let path = entry
            .path()
            .map_err(|_| Error::new(1, "Invalid archive path"))?;
        if path != Path::new("mlc") {
            continue;
        }
        if found || !entry.header().entry_type().is_file() {
            return Err(Error::new(
                1,
                "Archive must contain exactly one regular mlc executable",
            ));
        }
        let written = std::io::copy(&mut entry, &mut staged)
            .map_err(|_| Error::new(1, "Cannot stage update executable"))?;
        if written == 0 {
            return Err(Error::new(1, "Update executable is empty"));
        }
        found = true;
    }
    if !found {
        return Err(Error::new(1, "Archive does not contain mlc"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        staged
            .as_file()
            .set_permissions(std::fs::Permissions::from_mode(0o755))
            .map_err(|_| Error::new(1, "Cannot make update executable"))?;
    }
    staged
        .flush()
        .and_then(|_| staged.as_file().sync_all())
        .map_err(|_| Error::new(1, "Cannot flush update executable"))?;
    staged
        .persist(executable)
        .map_err(|_| Error::new(1, "Cannot replace executable; original was not changed"))?;
    Ok(())
}

pub fn run(json_output: bool) -> Result<()> {
    let target = target(std::env::consts::OS, std::env::consts::ARCH)?;
    let current = env!("CARGO_PKG_VERSION");
    eprintln!("Current version: {current}. Checking latest release...");
    let release: Release = serde_json::from_slice(&client::release_download(RELEASE_URL)?)
        .map_err(|_| Error::new(1, "Invalid release response from GitHub"))?;
    if !newer(&release.tag_name, current)? {
        eprintln!("Already up to date ({current}).");
        return output::print(&json!({"updated":false,"version":current}), json_output);
    }
    let name = format!("mlc-{target}.tar.gz");
    let archive_url = release.asset(&name)?;
    let checksum_url = release.asset("SHA256SUMS")?;
    eprintln!("Downloading {} for {target}...", release.tag_name);
    let archive = client::release_download(archive_url)?;
    let checksums = client::release_download(checksum_url)?;
    verify(&archive, &checksums, &name)?;
    let executable =
        std::env::current_exe().map_err(|_| Error::new(1, "Cannot locate current executable"))?;
    install(&archive, &executable)?;
    eprintln!("Updated to {}!", release.tag_name);
    if let Some(notes) = release.body.filter(|notes| !notes.trim().is_empty()) {
        eprintln!("\nWhat's new:\n{}", notes.trim());
    }
    refresh_completions(&executable);
    output::print(
        &json!({"updated":true,"previous_version":current,"version":release.tag_name.trim_start_matches('v')}),
        json_output,
    )
}

fn refresh_completions(executable: &Path) {
    let Some(home) = dirs::home_dir() else {
        return;
    };
    let fish = dirs::config_dir().unwrap_or_else(|| home.join(".config"));
    for (shell, path) in [
        ("fish", fish.join("fish/completions/mlc.fish")),
        (
            "bash",
            home.join(".local/share/bash-completion/completions/mlc"),
        ),
        ("zsh", home.join(".zfunc/_mlc")),
    ] {
        if !path.is_file() {
            continue;
        }
        let result = std::process::Command::new(executable)
            .args(["completions", shell])
            .output();
        match result {
            Ok(result) if result.status.success() => {
                if std::fs::write(&path, result.stdout).is_err() {
                    eprintln!("warning: Could not refresh {shell} completions");
                }
            }
            _ => eprintln!("warning: Could not generate {shell} completions"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn archive(name: &str, data: &[u8], kind: tar::EntryType) -> Vec<u8> {
        let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        let mut tar = tar::Builder::new(encoder);
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(0o755);
        header.set_entry_type(kind);
        header.set_cksum();
        tar.append_data(&mut header, name, data).unwrap();
        tar.into_inner().unwrap().finish().unwrap()
    }

    #[test]
    fn supports_every_release_platform() {
        for os in ["macos", "linux"] {
            for arch in ["aarch64", "x86_64"] {
                assert!(target(os, arch).is_ok());
            }
        }
        assert!(target("windows", "x86_64").is_err());
        assert!(target("linux", "arm").is_err());
    }

    #[test]
    fn never_downgrades_or_accepts_invalid_versions() {
        assert!(newer("v0.3.0", "0.2.0").unwrap());
        assert!(!newer("v0.2.0", "0.2.0").unwrap());
        assert!(!newer("v0.1.0", "0.2.0").unwrap());
        assert!(newer("", "0.2.0").is_err());
    }

    #[test]
    fn requires_matching_platform_checksum() {
        let manifest = format!("{:x}  mlc-test.tar.gz\n", Sha256::digest(b"archive"));
        assert!(verify(b"archive", manifest.as_bytes(), "mlc-test.tar.gz").is_ok());
        assert!(verify(b"tampered", manifest.as_bytes(), "mlc-test.tar.gz").is_err());
        assert!(verify(b"archive", manifest.as_bytes(), "other.tar.gz").is_err());
    }

    #[test]
    fn replaces_binary_and_preserves_original_on_bad_archives() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("mlc");
        std::fs::write(&exe, b"old").unwrap();
        for bad in [
            b"invalid".to_vec(),
            archive("other", b"new", tar::EntryType::Regular),
            archive("mlc", b"", tar::EntryType::Regular),
            archive("mlc", b"", tar::EntryType::Symlink),
        ] {
            assert!(install(&bad, &exe).is_err());
            assert_eq!(std::fs::read(&exe).unwrap(), b"old");
        }
        install(&archive("mlc", b"new", tar::EntryType::Regular), &exe).unwrap();
        assert_eq!(std::fs::read(&exe).unwrap(), b"new");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(exe).unwrap().permissions().mode() & 0o777,
                0o755
            );
        }
    }

    #[test]
    fn rejects_missing_and_untrusted_assets() {
        let release = Release {
            tag_name: "v0.3.0".into(),
            body: None,
            assets: vec![Asset {
                name: "mlc.tar.gz".into(),
                browser_download_url: "https://example.com/mlc".into(),
            }],
        };
        assert!(release.asset("SHA256SUMS").is_err());
        assert!(release.asset("mlc.tar.gz").is_err());
    }
}
