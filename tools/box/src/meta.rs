use std::path::Path;

use serde::{Deserialize, Serialize};
use snafu::ResultExt;
use tap::prelude::*;

use crate::error::{JsonSnafu, Result};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoxMeta {
    pub name: String,
    pub port: u16,
    pub disk_size_gb: u64,
    pub pid: u32,
}

pub fn save(path: &Path, meta: &BoxMeta) -> Result<()> {
    serde_json::to_string_pretty(meta)
        .context(JsonSnafu { path })?
        .pipe(|content| rocky::fs::write_file(path, &content))
        .map_err(Into::into)
        .tap_ok(|_| tracing::debug!(path = %path.display(), "saved box metadata"))
        .tap_err(|err| tracing::error!(%err, path = %path.display(), "failed to save box metadata"))
}

pub fn load(path: &Path) -> Result<BoxMeta> {
    rocky::fs::read_file(path)?
        .pipe(|content| serde_json::from_str(&content))
        .context(JsonSnafu { path })
        .tap_ok(|meta: &BoxMeta| {
            tracing::debug!(
                name = %meta.name,
                port = meta.port,
                pid = meta.pid,
                "loaded box metadata"
            )
        })
        .tap_err(|err| tracing::debug!(%err, path = %path.display(), "failed to load box metadata"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_save_and_load_roundtrip() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("meta.json");

        let meta = BoxMeta {
            name: "node-01".to_string(),
            port: 2221,
            disk_size_gb: 10,
            pid: 1234,
        };

        save(&path, &meta).unwrap();
        let loaded = load(&path).unwrap();
        assert_eq!(meta, loaded);
    }
}
