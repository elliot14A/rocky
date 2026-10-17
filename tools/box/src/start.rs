use std::fs::File;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use rocky::error::Kind;
use rocky::ui;
use rocky::utils::validate_name;
use snafu::{ensure, IntoError, ResultExt};
use tap::prelude::*;

use crate::error::{
    AlreadyRunningSnafu, InsufficientDiskSpaceSnafu, InvalidParameterSnafu, InvalidPidSnafu,
    IoSnafu, PortInUseSnafu, ProcessSpawnSnafu, Result,
};
use crate::image;
use crate::meta::{self, BoxMeta};

#[derive(Debug, Clone)]
pub struct StartArgs {
    pub name: String,
    pub port: u16,
    pub disk_size_gb: u64,
    pub installed: bool,
    pub detach: bool,
    pub iso: Option<PathBuf>,
}

pub fn boxes_dir() -> Result<PathBuf> {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("state")))
        .unwrap_or_else(std::env::temp_dir)
        .join("rocky")
        .join("boxes")
        .pipe(|dir| {
            rocky::fs::ensure_dir(&dir)?;
            tracing::debug!(dir = %dir.display(), "resolved boxes directory");
            Ok(dir)
        })
}

pub fn box_dir(name: &str) -> Result<PathBuf> {
    validate_name(Kind::Host, name)?;
    boxes_dir().map(|dir| dir.join(name))
}

pub fn is_pid_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        unsafe { libc::kill(pid as i32, 0) == 0 }
    }
    #[cfg(not(unix))]
    {
        false
    }
}

pub fn read_active_pid(pid_file: &Path) -> Result<Option<u32>> {
    if !pid_file.exists() {
        return Ok(None);
    }

    rocky::fs::read_file(pid_file)?
        .trim()
        .parse::<u32>()
        .map_err(|e| {
            InvalidPidSnafu {
                path: pid_file,
                reason: e.to_string(),
            }
            .build()
        })?
        .pipe(|pid| {
            if is_pid_alive(pid) {
                tracing::debug!(pid = pid, file = %pid_file.display(), "active pid detected");
                Ok(Some(pid))
            } else {
                tracing::debug!(pid = pid, file = %pid_file.display(), "stale pid file, pruning");
                let _ = std::fs::remove_file(pid_file);
                Ok(None)
            }
        })
}

pub fn check_port_available(port: u16) -> Result<()> {
    ensure!(
        port > 0,
        InvalidParameterSnafu {
            param: "port",
            reason: "port must be greater than 0",
        }
    );

    TcpListener::bind(("127.0.0.1", port))
        .map(drop)
        .map_err(|_| PortInUseSnafu { port }.build())
        .tap_ok(|_| tracing::debug!(port = port, "port is available"))
        .tap_err(|_| tracing::warn!(port = port, "port is already bound"))
}

#[cfg(unix)]
pub fn check_available_space(dir: &Path, required_bytes: u64) -> Result<()> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let c_path = CString::new(dir.as_os_str().as_bytes()).map_err(|e| {
        IoSnafu {
            path: dir.to_path_buf(),
        }
        .into_error(std::io::Error::new(std::io::ErrorKind::InvalidInput, e))
    })?;

    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    let res = unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) };
    if res != 0 {
        return Err(IoSnafu {
            path: dir.to_path_buf(),
        }
        .into_error(std::io::Error::last_os_error()));
    }

    let available_bytes = (stat.f_bavail as u64) * (stat.f_frsize as u64);
    if available_bytes < required_bytes {
        let available_gb = available_bytes / (1024 * 1024 * 1024);
        let required_gb = required_bytes.div_ceil(1024 * 1024 * 1024);
        return InsufficientDiskSpaceSnafu {
            path: dir.to_path_buf(),
            required_gb,
            available_gb,
        }
        .fail();
    }

    tracing::debug!(
        path = %dir.display(),
        available_gb = available_bytes / (1024 * 1024 * 1024),
        required_gb = required_bytes / (1024 * 1024 * 1024),
        "disk space check passed"
    );

    Ok(())
}

#[cfg(not(unix))]
pub fn check_available_space(_dir: &Path, _required_bytes: u64) -> Result<()> {
    Ok(())
}

pub fn ensure_disk(disk_path: &Path, size_gb: u64) -> Result<()> {
    ensure!(
        size_gb > 0,
        InvalidParameterSnafu {
            param: "disk-size",
            reason: "disk size must be greater than 0 GB",
        }
    );

    if disk_path.exists() {
        tracing::debug!(path = %disk_path.display(), "reusing existing raw disk");
        return Ok(());
    }

    let required_bytes = size_gb * 1024 * 1024 * 1024;
    let target_dir = disk_path.parent().unwrap_or(Path::new("."));
    check_available_space(target_dir, required_bytes)?;

    ui::info(format!("Allocating sparse {size_gb} GB raw disk..."));
    let file = File::create(disk_path).context(IoSnafu { path: disk_path })?;
    file.set_len(required_bytes)
        .context(IoSnafu { path: disk_path })?;

    tracing::info!(
        path = %disk_path.display(),
        size_gb = size_gb,
        "allocated sparse raw disk"
    );
    Ok(())
}

pub fn build_qemu_args(
    disk_path: &Path,
    port: u16,
    iso_path: Option<&Path>,
    seed_dir: Option<&Path>,
) -> Vec<String> {
    let mut args = vec![
        "-m".to_string(),
        "2048".to_string(),
        "-smp".to_string(),
        "2".to_string(),
    ];

    let kvm_path = Path::new("/dev/kvm");
    if kvm_path.exists() {
        let accessible = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(kvm_path)
            .is_ok();

        if accessible {
            args.push("-enable-kvm".to_string());
        } else {
            tracing::warn!(
                "/dev/kvm exists but is not readable/writable by current user; running without hardware virtualization"
            );
        }
    }

    // VirtIO primary hard drive
    args.push("-drive".to_string());
    args.push(format!("file={},format=raw,if=virtio", disk_path.display()));

    // User-mode networking strictly forwarding 127.0.0.1 host port to guest SSH
    args.push("-nic".to_string());
    args.push(format!(
        "user,model=virtio-net-pci,hostfwd=tcp:127.0.0.1:{port}-:22"
    ));

    args.push("-nographic".to_string());

    if let Some(iso) = iso_path {
        args.push("-cdrom".to_string());
        args.push(iso.display().to_string());
        args.push("-boot".to_string());
        args.push("d".to_string());

        if let Some(seed) = seed_dir {
            args.push("-drive".to_string());
            args.push(format!(
                "file=fat:ro:{},format=raw,if=virtio,read-only=on",
                seed.display()
            ));
        }
    }

    args
}

struct PidGuard<'a> {
    pid_file: &'a Path,
}

impl<'a> Drop for PidGuard<'a> {
    fn drop(&mut self) {
        if self.pid_file.exists() {
            let _ = std::fs::remove_file(self.pid_file);
            tracing::debug!(path = %self.pid_file.display(), "cleaned up pid file on exit");
        }
    }
}

pub async fn start_box(args: &StartArgs) -> Result<()> {
    validate_name(Kind::Host, &args.name)?;

    let b_dir = box_dir(&args.name)?;
    rocky::fs::ensure_dir(&b_dir)?;
    let pid_file = b_dir.join("pid");

    if let Some(pid) = read_active_pid(&pid_file)? {
        return AlreadyRunningSnafu {
            name: args.name.clone(),
            pid,
        }
        .fail()
        .tap_err(|err| tracing::error!(%err, "box is already running"));
    }

    check_port_available(args.port)?;

    let disk_path = b_dir.join("disk.raw");
    ensure_disk(&disk_path, args.disk_size_gb)?;

    let (iso_path, seed_dir) = if args.installed {
        (None, None)
    } else {
        let iso = image::ensure_alpine_iso(args.iso.as_deref()).await?;
        let seed = b_dir.join("seed");
        image::create_seed_apkovl(&seed)?;
        (Some(iso), Some(seed))
    };

    let qemu_args = build_qemu_args(
        &disk_path,
        args.port,
        iso_path.as_deref(),
        seed_dir.as_deref(),
    );

    let mut cmd = Command::new("qemu-system-x86_64");
    cmd.args(&qemu_args);

    if args.detach {
        let log_path = b_dir.join("qemu.log");
        let log_file = File::create(&log_path).context(IoSnafu { path: &log_path })?;
        let err_file = log_file.try_clone().context(IoSnafu { path: &log_path })?;

        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::from(log_file));
        cmd.stderr(Stdio::from(err_file));

        let child = cmd.spawn().context(ProcessSpawnSnafu {
            command: "qemu-system-x86_64",
        })?;

        let pid = child.id();
        rocky::fs::write_file(&pid_file, &pid.to_string())?;

        let meta = BoxMeta {
            name: args.name.clone(),
            port: args.port,
            disk_size_gb: args.disk_size_gb,
            pid,
        };
        meta::save(&b_dir.join("meta.json"), &meta)?;

        ui::success(format!(
            "Started box '{}' in background (PID: {})",
            args.name, pid
        ));
        ui::next_steps(&[
            format!("Connect: ssh root@127.0.0.1 -p {}", args.port),
            format!(
                "Deploy:  rocky up <env> --target {}=127.0.0.1:{}",
                args.name, args.port
            ),
            format!("Stop:    rocky-box stop {}", args.name),
        ]);

        tracing::info!(
            name = %args.name,
            pid = pid,
            port = args.port,
            "started box in background"
        );
    } else {
        ui::info(format!(
            "Starting box '{}' in foreground on port {} (Ctrl+A then X to exit QEMU)...",
            args.name, args.port
        ));

        tracing::info!(
            name = %args.name,
            port = args.port,
            "starting box in foreground"
        );

        let mut child = cmd.spawn().context(ProcessSpawnSnafu {
            command: "qemu-system-x86_64",
        })?;

        let pid = child.id();
        rocky::fs::write_file(&pid_file, &pid.to_string())?;

        let meta = BoxMeta {
            name: args.name.clone(),
            port: args.port,
            disk_size_gb: args.disk_size_gb,
            pid,
        };
        meta::save(&b_dir.join("meta.json"), &meta)?;

        let _guard = PidGuard {
            pid_file: &pid_file,
        };

        let _ = child.wait();
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_ensure_disk_creates_sparse_file() {
        let temp = tempdir().unwrap();
        let disk = temp.path().join("test.raw");
        ensure_disk(&disk, 1).unwrap();
        assert!(disk.exists());
        assert_eq!(disk.metadata().unwrap().len(), 1024 * 1024 * 1024);
    }

    #[test]
    fn test_ensure_disk_rejects_zero_size() {
        let temp = tempdir().unwrap();
        let disk = temp.path().join("test.raw");
        assert!(ensure_disk(&disk, 0).is_err());
    }

    #[test]
    fn test_build_qemu_args_rescue_mode() {
        let disk = Path::new("/tmp/test.raw");
        let iso = Path::new("/tmp/alpine.iso");
        let seed = Path::new("/tmp/seed");
        let args = build_qemu_args(disk, 2222, Some(iso), Some(seed));

        assert!(args.contains(&"-m".to_string()));
        assert!(args.contains(&"2048".to_string()));
        assert!(args
            .iter()
            .any(|a| a.contains("hostfwd=tcp:127.0.0.1:2222-:22")));
        assert!(args
            .iter()
            .any(|a| a.contains("file=fat:ro:/tmp/seed,format=raw,if=virtio,read-only=on")));
        assert!(args.contains(&"-cdrom".to_string()));
        assert!(args.contains(&"/tmp/alpine.iso".to_string()));
    }

    #[test]
    fn test_build_qemu_args_installed_mode() {
        let disk = Path::new("/tmp/test.raw");
        let args = build_qemu_args(disk, 2222, None, None);

        assert!(!args.contains(&"-cdrom".to_string()));
        assert!(!args.iter().any(|a| a.contains("file=fat:ro")));
        assert!(args.iter().any(|a| a.contains("file=/tmp/test.raw")));
        assert!(args
            .iter()
            .any(|a| a.contains("hostfwd=tcp:127.0.0.1:2222-:22")));
    }

    #[test]
    fn test_box_dir_does_not_create_phantom_directory() {
        let _temp = tempdir().unwrap();
        let name = "phantom-box";
        let b_dir = box_dir(name).unwrap();
        // Must NOT create the directory as a side effect
        if let Some(parent) = b_dir.parent() {
            let target = parent.join(name);
            assert!(!target.exists());
        }
    }

    #[test]
    fn test_check_available_space_success() {
        let temp = tempdir().unwrap();
        // Checking for 1 KB should succeed on any valid filesystem
        assert!(check_available_space(temp.path(), 1024).is_ok());
    }

    #[test]
    fn test_check_available_space_insufficient() {
        let temp = tempdir().unwrap();
        // Checking for u64::MAX bytes will always exceed available space
        assert!(check_available_space(temp.path(), u64::MAX).is_err());
    }
}
