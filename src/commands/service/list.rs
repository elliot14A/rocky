use std::path::Path;

use clap::Args;

use crate::error::Result;
use crate::process::journal::detect_service_kind;
use crate::{fs, nix, ui};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceInfo {
    pub name: String,
    pub kind: String,
    pub target: String,
}

#[derive(Args, Debug, Clone, Default)]
pub struct ListArgs {}

pub async fn run(args: ListArgs, target: Option<&Path>) -> Result<()> {
    nix::with_fleet(target, |fleet_root| async move {
        execute(args, &fleet_root).await
    })
    .await
}

pub fn collect_services(fleet_root: &Path) -> Result<Vec<ServiceInfo>> {
    let services_dir = fleet_root.join("services");
    let mut services = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&services_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            if path.extension().and_then(|s| s.to_str()) != Some("nix") {
                continue;
            }

            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };

            let (kind_str, target_str) = match fs::read_file(&path) {
                Ok(content) => match detect_service_kind(stem, &content, &path) {
                    Ok((kind, unit)) => (format!("{kind:?}").to_lowercase(), unit),
                    Err(_) => ("custom".to_string(), "-".to_string()),
                },
                Err(_) => ("unknown".to_string(), "-".to_string()),
            };

            services.push(ServiceInfo {
                name: stem.to_string(),
                kind: kind_str,
                target: target_str,
            });
        }
    }

    services.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(services)
}

async fn execute(_args: ListArgs, fleet_root: &Path) -> Result<()> {
    let services = collect_services(fleet_root)?;

    if services.is_empty() {
        ui::warn("No services found in fleet");
        return Ok(());
    }

    println!("{:<16} {:<12} {:<24}", "SERVICE", "KIND", "TARGET");
    println!("{:-<16} {:-<12} {:-<24}", "", "", "");

    for service in services {
        println!("{:<16} {:<12} {:<24}", service.name, service.kind, service.target);
    }

    Ok(())
}
