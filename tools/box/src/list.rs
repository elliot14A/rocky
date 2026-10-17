use rocky::ui;
use snafu::ResultExt;
use tap::prelude::*;

use crate::error::{IoSnafu, Result};
use crate::meta;
use crate::start::{boxes_dir, read_active_pid};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoxInfo {
    pub name: String,
    pub running: bool,
    pub pid: Option<u32>,
    pub port: Option<u16>,
    pub disk_size_gb: Option<u64>,
}

pub fn list_boxes() -> Result<Vec<BoxInfo>> {
    let b_dir = boxes_dir()?;
    if !b_dir.exists() {
        return Ok(Vec::new());
    }

    std::fs::read_dir(&b_dir)
        .context(IoSnafu { path: &b_dir })?
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .map(|entry| {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let pid = read_active_pid(&path.join("pid")).unwrap_or(None);
            let running = pid.is_some();

            let (port, disk_size_gb) = meta::load(&path.join("meta.json"))
                .map(|m| (Some(m.port), Some(m.disk_size_gb)))
                .unwrap_or_else(|_| {
                    let size = std::fs::metadata(path.join("disk.raw"))
                        .ok()
                        .map(|m| m.len() / (1024 * 1024 * 1024));
                    (None, size)
                });

            BoxInfo {
                name,
                running,
                pid,
                port,
                disk_size_gb,
            }
        })
        .collect::<Vec<_>>()
        .tap_mut(|items| items.sort_by(|a, b| a.name.cmp(&b.name)))
        .tap(|items| tracing::debug!(count = items.len(), "retrieved local boxes"))
        .pipe(Ok)
}

pub fn print_boxes(boxes: &[BoxInfo]) {
    if boxes.is_empty() {
        ui::info("No local boxes found. Start one with 'rocky-box start <name>'");
        return;
    }

    ui::info(format!(
        "{:<16} {:<10} {:<8} {:<12} {:<8}",
        "NAME", "STATUS", "PORT", "DISK SIZE", "PID"
    ));

    boxes.iter().for_each(|b| {
        let status = if b.running { "Running" } else { "Stopped" };
        let port = b
            .port
            .map(|p| p.to_string())
            .unwrap_or_else(|| "-".to_string());
        let disk = b
            .disk_size_gb
            .map(|d| format!("{d} GB"))
            .unwrap_or_else(|| "-".to_string());
        let pid = b
            .pid
            .map(|p| p.to_string())
            .unwrap_or_else(|| "-".to_string());

        ui::info(format!(
            "{:<16} {:<10} {:<8} {:<12} {:<8}",
            b.name, status, port, disk, pid
        ));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_box_info_fields() {
        let info = BoxInfo {
            name: "test-01".to_string(),
            running: false,
            pid: None,
            port: Some(2222),
            disk_size_gb: Some(10),
        };
        assert_eq!(info.name, "test-01");
        assert!(!info.running);
        assert_eq!(info.port, Some(2222));
    }
}
