use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter, Layer};

use crate::error::Result;
use crate::fs;

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

pub fn log_path() -> Result<PathBuf> {
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

    Ok(log_dir.join("rocky.log"))
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
}
