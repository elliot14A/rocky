use std::fs::File;
use std::path::{Path, PathBuf};

use flate2::write::GzEncoder;
use flate2::Compression;
use snafu::ResultExt;
use tap::prelude::*;
use tar::{Builder, Header};

use crate::error::{IoSnafu, IsoNotFoundSnafu, Result};

pub const DEFAULT_ALPINE_URL: &str =
    "https://dl-cdn.alpinelinux.org/alpine/v3.21/releases/x86_64/alpine-virt-3.21.3-x86_64.iso";
pub const DEFAULT_ALPINE_FILENAME: &str = "alpine-virt-3.21.3-x86_64.iso";

pub fn cache_dir() -> Result<PathBuf> {
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
        .unwrap_or_else(std::env::temp_dir)
        .join("rocky")
        .join("iso")
        .pipe(|dir| {
            rocky::fs::ensure_dir(&dir)?;
            tracing::debug!(dir = %dir.display(), "resolved iso cache directory");
            Ok(dir)
        })
}

pub async fn ensure_alpine_iso(custom_iso: Option<&Path>) -> Result<PathBuf> {
    if let Some(custom) = custom_iso {
        if !custom.exists() {
            return IsoNotFoundSnafu { path: custom }.fail();
        }
        return custom
            .to_path_buf()
            .tap(|p| tracing::info!(path = %p.display(), "using custom iso"))
            .pipe(Ok);
    }

    let cache = cache_dir()?;
    let iso_path = cache.join(DEFAULT_ALPINE_FILENAME);
    if iso_path.exists() {
        return iso_path
            .tap(|p| tracing::debug!(path = %p.display(), "cached alpine virtual iso exists"))
            .pipe(Ok);
    }

    let temp_download = cache.join(format!("{DEFAULT_ALPINE_FILENAME}.download"));
    rocky::ui::info("Downloading minimal Alpine virtual ISO (~60 MB)...");
    tracing::info!(
        url = DEFAULT_ALPINE_URL,
        target = %iso_path.display(),
        "downloading minimal alpine virtual iso"
    );

    let download_target = temp_download.to_string_lossy();

    rocky::process::exec(
        "curl",
        [
            "-fSL",
            "--retry",
            "3",
            "-o",
            &download_target,
            DEFAULT_ALPINE_URL,
        ],
        None,
    )
    .await
    .tap_err(|err| tracing::error!(%err, "curl download failed"))?;

    std::fs::rename(&temp_download, &iso_path)
        .context(IoSnafu { path: &iso_path })
        .tap_ok(|_| tracing::info!(path = %iso_path.display(), "cached alpine virtual iso"))
        .tap_err(|err| tracing::error!(%err, path = %iso_path.display(), "failed to rename downloaded iso"))?;

    Ok(iso_path)
}

fn append_tar_file<W: std::io::Write>(
    tar: &mut Builder<W>,
    path: &str,
    data: &[u8],
    mode: u32,
    target_path: &Path,
) -> Result<()> {
    Header::new_gnu()
        .tap_mut(|h| {
            h.set_size(data.len() as u64);
            h.set_mode(mode);
            h.set_cksum();
        })
        .pipe(|mut header| tar.append_data(&mut header, path, data))
        .context(IoSnafu { path: target_path })
}

fn append_tar_symlink<W: std::io::Write>(
    tar: &mut Builder<W>,
    path: &str,
    target: &str,
    target_path: &Path,
) -> Result<()> {
    Header::new_gnu()
        .tap_mut(|h| {
            h.set_size(0);
            h.set_mode(0o777);
            h.set_entry_type(tar::EntryType::Symlink);
            h.set_cksum();
        })
        .pipe(|mut header| {
            header
                .set_link_name(target)
                .context(IoSnafu { path: target_path })?;
            tar.append_data(&mut header, path, std::io::empty())
                .context(IoSnafu { path: target_path })
        })
}

pub fn create_seed_apkovl(seed_dir: &Path) -> Result<PathBuf> {
    rocky::fs::ensure_dir(seed_dir)?;
    let apkovl_path = seed_dir.join("localhost.apkovl.tar.gz");

    rocky::ui::info("Generating seed overlay for root SSH access (localhost.apkovl.tar.gz)...");
    let file = File::create(&apkovl_path).context(IoSnafu { path: &apkovl_path })?;
    let enc = GzEncoder::new(file, Compression::default());
    let mut tar = Builder::new(enc);

    let sshd_config = b"PermitRootLogin yes\nPermitEmptyPasswords yes\nPasswordAuthentication yes\nGatewayPorts yes\n";
    append_tar_file(
        &mut tar,
        "etc/ssh/sshd_config",
        sshd_config,
        0o644,
        &apkovl_path,
    )?;

    let shadow = b"root:::0:99999:7:::\n";
    append_tar_file(&mut tar, "etc/shadow", shadow, 0o600, &apkovl_path)?;

    let local_script = b"#!/bin/sh\nrc-service sshd restart\n";
    append_tar_file(
        &mut tar,
        "etc/local.d/sshd.start",
        local_script,
        0o755,
        &apkovl_path,
    )?;

    append_tar_symlink(
        &mut tar,
        "etc/runlevels/default/local",
        "/etc/init.d/local",
        &apkovl_path,
    )?;
    append_tar_symlink(
        &mut tar,
        "etc/runlevels/default/sshd",
        "/etc/init.d/sshd",
        &apkovl_path,
    )?;

    tar.finish().context(IoSnafu { path: &apkovl_path })?;
    let enc = tar.into_inner().context(IoSnafu { path: &apkovl_path })?;
    enc.finish().context(IoSnafu { path: &apkovl_path })?;

    tracing::debug!(
        path = %apkovl_path.display(),
        "generated alpine seed apkovl overlay"
    );

    Ok(apkovl_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_cache_dir_resolves() {
        let dir = cache_dir();
        assert!(dir.is_ok());
        assert!(dir.unwrap().ends_with("rocky/iso"));
    }

    #[test]
    fn test_create_seed_apkovl_generates_valid_tar_gz() {
        let temp = tempdir().unwrap();
        let apkovl = create_seed_apkovl(temp.path()).unwrap();
        assert!(apkovl.exists());
        assert_eq!(apkovl.file_name().unwrap(), "localhost.apkovl.tar.gz");

        let file = File::open(&apkovl).unwrap();
        let dec = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(dec);

        let entries: Vec<String> = archive
            .entries()
            .unwrap()
            .map(|e| e.unwrap().path().unwrap().display().to_string())
            .collect();

        assert!(entries.contains(&"etc/ssh/sshd_config".to_string()));
        assert!(entries.contains(&"etc/shadow".to_string()));
        assert!(entries.contains(&"etc/local.d/sshd.start".to_string()));
        assert!(entries.contains(&"etc/runlevels/default/sshd".to_string()));
    }

    #[tokio::test]
    async fn test_ensure_alpine_iso_rejects_missing_custom_iso() {
        let missing = Path::new("/nonexistent/custom.iso");
        let res = ensure_alpine_iso(Some(missing)).await;
        assert!(res.is_err());
    }
}
