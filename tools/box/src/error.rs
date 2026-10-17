use std::path::PathBuf;

use snafu::Snafu;

pub type Result<T, E = BoxError> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum BoxError {
    #[snafu(display("Rocky core error: {source}"))]
    Rocky { source: rocky::error::RockyError },

    #[snafu(display("I/O error at '{}': {source}", path.display()))]
    Io {
        source: std::io::Error,
        path: PathBuf,
    },

    #[snafu(display("Failed to spawn process '{command}': {source}"))]
    ProcessSpawn {
        command: String,
        source: std::io::Error,
    },

    #[snafu(display("Box '{name}' is already running with PID {pid}"))]
    AlreadyRunning { name: String, pid: u32 },

    #[snafu(display("Box '{name}' not found"))]
    NotFound { name: String },

    #[snafu(display("Custom ISO not found at '{}'", path.display()))]
    IsoNotFound { path: PathBuf },

    #[snafu(display("Port {port} is already in use by another process"))]
    PortInUse { port: u16 },

    #[snafu(display("Invalid parameter '{param}': {reason}"))]
    InvalidParameter { param: String, reason: String },

    #[snafu(display(
        "Insufficient disk space at '{}': {required_gb} GB required, but only {available_gb} GB available",
        path.display()
    ))]
    InsufficientDiskSpace {
        path: PathBuf,
        required_gb: u64,
        available_gb: u64,
    },

    #[snafu(display("JSON serialization error for '{}': {source}", path.display()))]
    Json {
        source: serde_json::Error,
        path: PathBuf,
    },

    #[snafu(display("Failed to parse PID from '{}': {reason}", path.display()))]
    InvalidPid { path: PathBuf, reason: String },
}

impl From<rocky::error::RockyError> for BoxError {
    fn from(source: rocky::error::RockyError) -> Self {
        BoxError::Rocky { source }
    }
}
