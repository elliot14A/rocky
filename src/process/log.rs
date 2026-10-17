use snafu::ResultExt;
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter, Layer};

use crate::error::{IoSnafu, Kind, Result};
use crate::fs;
use crate::utils::validate::validate_name;

pub fn init(verbose: bool) {
    let stderr_filter = if verbose {
        EnvFilter::new("debug")
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("off"))
    };

    let stderr_layer = fmt::layer()
        .with_writer(std::io::stderr)
        .with_filter(stderr_filter);

    let log_file = log_path().ok().and_then(|path| {
        let mut options = OpenOptions::new();
        options.create(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        options.open(path).ok()
    });

    if let Some(file) = log_file {
        let file_layer = fmt::layer()
            .with_ansi(false)
            .with_writer(file)
            .with_filter(EnvFilter::new("debug"));

        tracing_subscriber::registry()
            .with(stderr_layer)
            .with(file_layer)
            .init();
    } else {
        tracing_subscriber::registry().with(stderr_layer).init();
    }
}

fn log_dir() -> Result<PathBuf> {
    let base_dir = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            #[cfg(target_os = "macos")]
            {
                std::env::var_os("HOME").map(|home| Path::new(&home).join("Library").join("Logs"))
            }

            #[cfg(not(target_os = "macos"))]
            {
                std::env::var_os("HOME").map(|home| Path::new(&home).join(".local").join("state"))
            }
        })
        .unwrap_or_else(std::env::temp_dir);

    let log_dir = base_dir.join("rocky");
    fs::ensure_dir(&log_dir)?;

    Ok(log_dir)
}

pub fn log_path() -> Result<PathBuf> {
    log_dir().map(|dir| dir.join("rocky.log"))
}

pub fn host_log_path(action: &str, host: &str) -> Result<PathBuf> {
    validate_name(Kind::Host, host)?;
    log_dir().map(|dir| dir.join(format!("{action}-{host}.log")))
}

pub fn open_log_pair(log_path: &Path) -> Result<(std::fs::File, std::fs::File)> {
    if let Some(parent) = log_path.parent() {
        fs::ensure_dir(parent)?;
    }

    let mut open_options = OpenOptions::new();
    open_options.create(true).write(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        open_options.mode(0o600);
    }

    let out_file = open_options.open(log_path).context(IoSnafu {
        path: log_path.to_path_buf(),
    })?;
    let err_file = out_file.try_clone().context(IoSnafu {
        path: log_path.to_path_buf(),
    })?;

    Ok((out_file, err_file))
}

pub fn tail_log(log_path: &Path, lines_count: usize) -> String {
    std::fs::read_to_string(log_path)
        .unwrap_or_default()
        .lines()
        .rev()
        .take(lines_count)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_path_resolves_and_creates_dir() {
        let path = log_path();
        assert!(path.is_ok());
        let p = path.unwrap();
        assert!(p.ends_with("rocky/rocky.log"));
        assert!(p.parent().unwrap().exists());
    }

    #[test]
    fn test_host_log_path_resolves_and_creates_dir() {
        let path = host_log_path("install", "web-01");
        assert!(path.is_ok());
        let p = path.unwrap();
        assert!(p.ends_with("rocky/install-web-01.log"));
        assert!(p.parent().unwrap().exists());
    }

    #[test]
    fn test_host_log_path_rejects_path_traversal() {
        assert!(host_log_path("install", "../evil").is_err());
        assert!(host_log_path("install", "web/01").is_err());
        assert!(host_log_path("install", "").is_err());
    }

    #[test]
    fn test_open_log_pair_and_tail_log() {
        let temp = tempfile::tempdir().unwrap();
        let log_file = temp.path().join("test.log");

        let (mut out, _) = open_log_pair(&log_file).unwrap();
        use std::io::Write;
        for i in 1..=20 {
            writeln!(out, "line {i}").unwrap();
        }
        drop(out);

        let tail = tail_log(&log_file, 3);
        assert_eq!(tail, "line 18\nline 19\nline 20");
    }
}
