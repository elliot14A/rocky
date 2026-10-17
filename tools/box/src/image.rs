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

fn append_tar_dir<W: std::io::Write>(
    tar: &mut Builder<W>,
    path: &str,
    mode: u32,
    target_path: &Path,
) -> Result<()> {
    Header::new_gnu()
        .tap_mut(|h| {
            h.set_size(0);
            h.set_mode(mode);
            h.set_entry_type(tar::EntryType::Directory);
            h.set_cksum();
        })
        .pipe(|mut header| tar.append_data(&mut header, path, std::io::empty()))
        .context(IoSnafu { path: target_path })
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

fn is_valid_public_key_format(line: &str) -> bool {
    let clean = line.trim();
    clean.starts_with("ssh-") || clean.starts_with("ecdsa-") || clean.starts_with("sk-")
}

pub async fn resolve_authorized_keys(
    box_dir: &Path,
    custom_ssh_key: Option<&Path>,
) -> Result<(Vec<String>, Option<PathBuf>)> {
    if let Some(custom) = custom_ssh_key {
        let content = rocky::fs::read_file(custom)?;
        let mut keys = Vec::new();
        let mut identity_path = None;

        if content.contains("BEGIN") && content.contains("PRIVATE KEY") {
            let pub_candidate = custom.with_extension("pub");
            let pub_key = if pub_candidate.is_file() {
                rocky::fs::read_file(&pub_candidate)?
            } else {
                rocky::process::exec("ssh-keygen", ["-y", "-f", &custom.to_string_lossy()], None)
                    .await?
            };
            let clean = pub_key.trim();
            if is_valid_public_key_format(clean) {
                keys.push(clean.to_string());
                identity_path = Some(custom.to_path_buf());
            }
        } else {
            for line in content.lines() {
                let clean = line.trim();
                if is_valid_public_key_format(clean) {
                    keys.push(clean.to_string());
                }
            }
            if keys.is_empty() {
                if let Ok(nix_keys) = rocky::nix::extract_public_keys(&content, custom) {
                    keys.extend(nix_keys);
                }
            } else if custom.extension().and_then(|s| s.to_str()) == Some("pub") {
                let priv_candidate = custom.with_extension("");
                if priv_candidate.is_file() {
                    identity_path = Some(priv_candidate);
                }
            }
        }

        snafu::ensure!(
            !keys.is_empty(),
            crate::error::InvalidParameterSnafu {
                param: "ssh-key",
                reason: format!("no valid OpenSSH keys found in '{}'", custom.display()),
            }
        );

        return Ok((keys, identity_path));
    }

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let candidates = [cwd.join("keys.nix"), cwd.join("fleet/keys.nix")];
    for candidate in &candidates {
        if candidate.is_file() {
            if let Ok(content) = rocky::fs::read_file(candidate) {
                if let Ok(keys) = rocky::nix::extract_public_keys(&content, candidate) {
                    if !keys.is_empty() {
                        tracing::debug!(
                            path = %candidate.display(),
                            count = keys.len(),
                            "using operator keys from keys.nix"
                        );
                        return Ok((keys, None));
                    }
                }
            }
        }
    }

    let priv_path = box_dir.join("id_ed25519");
    let pub_path = box_dir.join("id_ed25519.pub");
    if priv_path.exists() && pub_path.exists() {
        let pub_key = rocky::fs::read_file(&pub_path)?.trim().to_string();
        return Ok((vec![pub_key], Some(priv_path)));
    }

    let priv_str = priv_path.to_string_lossy().to_string();
    rocky::process::exec(
        "ssh-keygen",
        [
            "-t",
            "ed25519",
            "-N",
            "",
            "-C",
            "rocky-box-operator",
            "-f",
            &priv_str,
        ],
        None,
    )
    .await?;

    let pub_key = rocky::fs::read_file(&pub_path)?.trim().to_string();
    Ok((vec![pub_key], Some(priv_path)))
}

pub async fn create_seed_apkovl(
    seed_dir: &Path,
    box_dir: &Path,
    box_name: &str,
    port: u16,
    custom_ssh_key: Option<&Path>,
) -> Result<Option<PathBuf>> {
    rocky::fs::ensure_dir(seed_dir)?;
    let apkovl_path = seed_dir.join("localhost.apkovl.tar.gz");

    rocky::ui::info("Generating seed overlay for root SSH access (localhost.apkovl.tar.gz)...");
    let file = File::create(&apkovl_path).context(IoSnafu { path: &apkovl_path })?;
    let enc = GzEncoder::new(file, Compression::default());
    let mut tar = Builder::new(enc);

    append_tar_dir(&mut tar, "etc", 0o755, &apkovl_path)?;
    append_tar_dir(&mut tar, "etc/apk", 0o755, &apkovl_path)?;
    append_tar_dir(&mut tar, "etc/network", 0o755, &apkovl_path)?;
    append_tar_dir(&mut tar, "etc/ssh", 0o755, &apkovl_path)?;
    append_tar_dir(&mut tar, "etc/local.d", 0o755, &apkovl_path)?;
    append_tar_dir(&mut tar, "etc/runlevels", 0o755, &apkovl_path)?;
    append_tar_dir(&mut tar, "etc/runlevels/boot", 0o755, &apkovl_path)?;
    append_tar_dir(&mut tar, "etc/runlevels/default", 0o755, &apkovl_path)?;
    append_tar_dir(&mut tar, "root", 0o700, &apkovl_path)?;
    append_tar_dir(&mut tar, "root/.ssh", 0o700, &apkovl_path)?;

    let modules = b"virtio_net\nvirtio_pci\n";
    append_tar_file(&mut tar, "etc/modules", modules, 0o644, &apkovl_path)?;

    let apk_world =
        b"alpine-base\nopenssh\nopenssh-server\nopenssh-server-common-openrc\nopenssh-sftp-server\n";
    append_tar_file(&mut tar, "etc/apk/world", apk_world, 0o644, &apkovl_path)?;

    let network_interfaces = b"auto lo\niface lo inet loopback\n\nauto eth0\niface eth0 inet static\n    address 10.0.2.15\n    netmask 255.255.255.0\n    gateway 10.0.2.2\n";
    append_tar_file(
        &mut tar,
        "etc/network/interfaces",
        network_interfaces,
        0o644,
        &apkovl_path,
    )?;

    let inittab = b"::sysinit:/sbin/openrc sysinit\n::sysinit:/sbin/openrc boot\n::wait:/sbin/openrc default\n\ntty1::respawn:/sbin/getty 38400 tty1\nttyS0::respawn:/sbin/getty -L 115200 ttyS0 vt100\n\n::ctrlaltdel:/sbin/reboot\n::shutdown:/sbin/openrc shutdown\n";
    append_tar_file(&mut tar, "etc/inittab", inittab, 0o644, &apkovl_path)?;

    let host_priv_path = box_dir.join("ssh_host_ed25519_key");
    let host_pub_path = box_dir.join("ssh_host_ed25519_key.pub");

    if !host_priv_path.exists() || !host_pub_path.exists() {
        let host_key = rocky::ssh::generate_host_key(&format!("box-{}", box_name)).await?;
        let src_priv = host_key
            .extra_files_dir
            .path()
            .join("etc/ssh/ssh_host_ed25519_key");
        let src_pub = host_key
            .extra_files_dir
            .path()
            .join("etc/ssh/ssh_host_ed25519_key.pub");
        let priv_bytes = std::fs::read(&src_priv).context(IoSnafu { path: &src_priv })?;
        let pub_bytes = std::fs::read(&src_pub).context(IoSnafu { path: &src_pub })?;
        rocky::fs::write_file(
            &host_priv_path,
            String::from_utf8_lossy(&priv_bytes).as_ref(),
        )?;
        rocky::fs::write_file(&host_pub_path, String::from_utf8_lossy(&pub_bytes).as_ref())?;
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&host_priv_path, std::fs::Permissions::from_mode(0o600));
        let _ = std::fs::set_permissions(&host_pub_path, std::fs::Permissions::from_mode(0o644));
    }

    let priv_key_data = std::fs::read(&host_priv_path).context(IoSnafu {
        path: &host_priv_path,
    })?;
    let pub_key_data = std::fs::read(&host_pub_path).context(IoSnafu {
        path: &host_pub_path,
    })?;

    append_tar_file(
        &mut tar,
        "etc/ssh/ssh_host_ed25519_key",
        &priv_key_data,
        0o600,
        &apkovl_path,
    )?;
    append_tar_file(
        &mut tar,
        "etc/ssh/ssh_host_ed25519_key.pub",
        &pub_key_data,
        0o644,
        &apkovl_path,
    )?;

    let (auth_keys, identity_path) = resolve_authorized_keys(box_dir, custom_ssh_key).await?;
    let mut auth_keys_str = auth_keys.join("\n");
    auth_keys_str.push('\n');
    append_tar_file(
        &mut tar,
        "root/.ssh/authorized_keys",
        auth_keys_str.as_bytes(),
        0o600,
        &apkovl_path,
    )?;

    let sshd_config = b"Port 22\nPermitRootLogin prohibit-password\nPasswordAuthentication no\nGatewayPorts yes\nSubsystem sftp /usr/lib/ssh/sftp-server\n";
    append_tar_file(
        &mut tar,
        "etc/ssh/sshd_config",
        sshd_config,
        0o644,
        &apkovl_path,
    )?;

    let shadow = b"root:::0:99999:7:::\n";
    append_tar_file(&mut tar, "etc/shadow", shadow, 0o600, &apkovl_path)?;

    let local_script = format!(
        "#!/bin/sh\n\
        ip link set eth0 up 2>/dev/null || true\n\
        ip addr add 10.0.2.15/24 dev eth0 2>/dev/null || true\n\
        ip route add default via 10.0.2.2 2>/dev/null || true\n\
        echo \"nameserver 10.0.2.3\" > /etc/resolv.conf 2>/dev/null || true\n\
        echo \"\" > /dev/ttyS0 2>/dev/null || true\n\
        echo \"=========================================\" > /dev/ttyS0 2>/dev/null || true\n\
        echo \" Rocky Local Test Box Ready              \" > /dev/ttyS0 2>/dev/null || true\n\
        echo \" SSH: ssh root@127.0.0.1 -p {port}       \" > /dev/ttyS0 2>/dev/null || true\n\
        echo \"=========================================\" > /dev/ttyS0 2>/dev/null || true\n"
    );
    append_tar_file(
        &mut tar,
        "etc/local.d/ready.start",
        local_script.as_bytes(),
        0o755,
        &apkovl_path,
    )?;

    let issue = format!(
        "Rocky Local Box (Alpine Rescue Environment)\nSSH: ssh root@127.0.0.1 -p {port}\n\n"
    );
    append_tar_file(&mut tar, "etc/issue", issue.as_bytes(), 0o644, &apkovl_path)?;

    let motd = format!(
        "Welcome to Rocky Test Box (Rescue Mode)\nConnect via: ssh root@127.0.0.1 -p {port}\n"
    );
    append_tar_file(&mut tar, "etc/motd", motd.as_bytes(), 0o644, &apkovl_path)?;

    append_tar_symlink(
        &mut tar,
        "etc/runlevels/boot/networking",
        "/etc/init.d/networking",
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

    Ok(identity_path)
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

    #[tokio::test]
    async fn test_create_seed_apkovl_generates_valid_tar_gz() {
        let temp = tempdir().unwrap();
        let seed_dir = temp.path().join("seed");
        let box_dir = temp.path().join("box");
        std::fs::create_dir_all(&box_dir).unwrap();

        let identity = create_seed_apkovl(&seed_dir, &box_dir, "test-box", 2222, None)
            .await
            .unwrap();
        assert!(identity.is_some());

        let apkovl = seed_dir.join("localhost.apkovl.tar.gz");
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

        assert!(entries.contains(&"etc/modules".to_string()));
        assert!(entries.contains(&"etc/apk/world".to_string()));
        assert!(entries.contains(&"etc/network/interfaces".to_string()));
        assert!(entries.contains(&"etc/inittab".to_string()));
        assert!(entries.contains(&"etc/ssh/sshd_config".to_string()));
        assert!(entries.contains(&"etc/ssh/ssh_host_ed25519_key".to_string()));
        assert!(entries.contains(&"etc/ssh/ssh_host_ed25519_key.pub".to_string()));
        assert!(entries.contains(&"root/.ssh/authorized_keys".to_string()));
        assert!(entries.contains(&"etc/shadow".to_string()));
        assert!(entries.contains(&"etc/local.d/ready.start".to_string()));
        assert!(entries.contains(&"etc/runlevels/boot/networking".to_string()));
        assert!(entries.contains(&"etc/runlevels/default/sshd".to_string()));
        assert!(entries.contains(&"etc/runlevels/default/local".to_string()));
    }

    #[tokio::test]
    async fn test_ensure_alpine_iso_rejects_missing_custom_iso() {
        let missing = Path::new("/nonexistent/custom.iso");
        let res = ensure_alpine_iso(Some(missing)).await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn test_resolve_authorized_keys_custom_pub() {
        let temp = tempdir().unwrap();
        let box_dir = temp.path().join("box");
        std::fs::create_dir_all(&box_dir).unwrap();

        let priv_key = temp.path().join("id_custom");
        let pub_key = temp.path().join("id_custom.pub");
        std::fs::write(&priv_key, "mock-private-content").unwrap();
        std::fs::write(
            &pub_key,
            "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI custom@host\n",
        )
        .unwrap();

        let (keys, id) = resolve_authorized_keys(&box_dir, Some(&pub_key))
            .await
            .unwrap();
        assert_eq!(keys.len(), 1);
        assert!(keys[0].starts_with("ssh-ed25519"));
        assert_eq!(id, Some(priv_key));
    }

    #[tokio::test]
    async fn test_resolve_authorized_keys_rejects_empty_keys() {
        let temp = tempdir().unwrap();
        let box_dir = temp.path().join("box");
        std::fs::create_dir_all(&box_dir).unwrap();

        let empty_key = temp.path().join("empty.txt");
        std::fs::write(&empty_key, "invalid line without ssh key\n").unwrap();

        let res = resolve_authorized_keys(&box_dir, Some(&empty_key)).await;
        assert!(res.is_err());
    }
}
