use std::collections::HashMap;

use snafu::OptionExt;

use crate::error::{MissingTargetIpSnafu, Result};
use crate::nix::HostDiff;
use crate::ssh::Target;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallItem {
    pub host: String,
    pub address: String,
    pub port: u16,
    pub services: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeployItem {
    pub host: String,
    pub address: String,
    pub port: u16,
    pub services: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetPlan {
    pub environment: String,
    pub install: Vec<InstallItem>,
    pub deploy: Vec<DeployItem>,
    pub removed: Vec<String>,
}

impl FleetPlan {
    pub fn is_empty(&self) -> bool {
        self.install.is_empty() && self.deploy.is_empty() && self.removed.is_empty()
    }
}

pub fn format_address(address: &str, port: u16) -> String {
    if port == 22 {
        address.to_string()
    } else if address.contains(':') && !address.starts_with('[') {
        format!("[{}]:{}", address, port)
    } else {
        format!("{}:{}", address, port)
    }
}

pub fn build_plan(
    environment: impl Into<String>,
    diff: &HostDiff,
    targets: &HashMap<String, Target>,
    services_by_host: &HashMap<String, Vec<String>>,
) -> Result<FleetPlan> {
    let install = diff
        .pending_install
        .iter()
        .map(|host| {
            let target = targets
                .get(host)
                .context(MissingTargetIpSnafu { host: host.clone() })?;
            let services = services_by_host.get(host).cloned().unwrap_or_default();
            Ok(InstallItem {
                host: host.clone(),
                address: target.address.clone(),
                port: target.port,
                services,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let deploy = diff
        .installed
        .iter()
        .map(|host| {
            let (address, port) = if let Some(target) = targets.get(host) {
                (target.address.clone(), target.port)
            } else {
                (host.clone(), 22)
            };
            let services = services_by_host.get(host).cloned().unwrap_or_default();
            DeployItem {
                host: host.clone(),
                address,
                port,
                services,
            }
        })
        .collect::<Vec<_>>();

    Ok(FleetPlan {
        environment: environment.into(),
        install,
        deploy,
        removed: diff.removed.clone(),
    })
}

pub fn format_plan(plan: &FleetPlan) -> String {
    let mut lines = Vec::new();
    lines.push(format!("Plan for environment '{}':", plan.environment));

    if plan.is_empty() {
        lines.push("  No changes to apply. Fleet is up to date.".to_string());
        return lines.join("\n");
    }

    if !plan.install.is_empty() {
        lines.push(format!("  Hosts to install ({}):", plan.install.len()));
        for item in &plan.install {
            let addr_str = format_address(&item.address, item.port);
            if item.services.is_empty() {
                lines.push(format!("    + {} -> {}", item.host, addr_str));
            } else {
                lines.push(format!(
                    "    + {} -> {} (services: {})",
                    item.host,
                    addr_str,
                    item.services.join(", ")
                ));
            }
        }
    }

    if !plan.deploy.is_empty() {
        lines.push(format!("  Hosts to deploy ({}):", plan.deploy.len()));
        for item in &plan.deploy {
            let addr_str = format_address(&item.address, item.port);
            if item.services.is_empty() {
                lines.push(format!("    ~ {} ({})", item.host, addr_str));
            } else {
                lines.push(format!(
                    "    ~ {} ({}) (services: {})",
                    item.host,
                    addr_str,
                    item.services.join(", ")
                ));
            }
        }
    }

    if !plan.removed.is_empty() {
        lines.push(format!(
            "  Hosts removed from fleet ({}):",
            plan.removed.len()
        ));
        for host in &plan.removed {
            lines.push(format!(
                "    - {} (recorded in keys.nix but removed from fleet.nix)",
                host
            ));
        }
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_plan_missing_install_target_fails() {
        let diff = HostDiff {
            declared: vec!["node-01".to_string()],
            pending_install: vec!["node-01".to_string()],
            installed: Vec::new(),
            removed: Vec::new(),
        };
        let targets = HashMap::new();
        let services = HashMap::new();

        let err = build_plan("prod", &diff, &targets, &services).unwrap_err();
        assert!(err.to_string().contains("node-01"));
    }

    #[test]
    fn build_plan_and_format_success() {
        let diff = HostDiff {
            declared: vec!["node-01".to_string(), "node-02".to_string()],
            pending_install: vec!["node-01".to_string()],
            installed: vec!["node-02".to_string()],
            removed: vec!["node-03".to_string()],
        };

        let mut targets = HashMap::new();
        targets.insert(
            "node-01".to_string(),
            Target {
                host: "node-01".to_string(),
                address: "192.168.1.10".to_string(),
                port: 2222,
            },
        );

        let mut services = HashMap::new();
        services.insert(
            "node-01".to_string(),
            vec!["web".to_string(), "monitoring".to_string()],
        );
        services.insert("node-02".to_string(), vec!["database".to_string()]);

        let plan = build_plan("prod", &diff, &targets, &services).unwrap();
        assert_eq!(plan.install.len(), 1);
        assert_eq!(plan.deploy.len(), 1);
        assert_eq!(plan.removed.len(), 1);
        assert_eq!(plan.install[0].port, 2222);
        assert_eq!(plan.deploy[0].port, 22);
        assert_eq!(plan.deploy[0].address, "node-02");

        let formatted = format_plan(&plan);
        assert!(formatted.contains("Plan for environment 'prod':"));
        assert!(formatted.contains("Hosts to install (1):"));
        assert!(formatted.contains("+ node-01 -> 192.168.1.10:2222 (services: web, monitoring)"));
        assert!(formatted.contains("Hosts to deploy (1):"));
        assert!(formatted.contains("~ node-02 (node-02) (services: database)"));
        assert!(formatted.contains("Hosts removed from fleet (1):"));
        assert!(formatted.contains("- node-03"));
    }

    #[test]
    fn format_plan_empty_message() {
        let plan = FleetPlan {
            environment: "staging".to_string(),
            install: Vec::new(),
            deploy: Vec::new(),
            removed: Vec::new(),
        };
        let formatted = format_plan(&plan);
        assert!(formatted.contains("No changes to apply"));
    }

    #[test]
    fn format_address_bracketed_ipv6() {
        assert_eq!(format_address("1.2.3.4", 22), "1.2.3.4");
        assert_eq!(format_address("1.2.3.4", 2222), "1.2.3.4:2222");
        assert_eq!(format_address("2001:db8::1", 22), "2001:db8::1");
        assert_eq!(format_address("2001:db8::1", 2222), "[2001:db8::1]:2222");
        assert_eq!(format_address("[2001:db8::1]", 2222), "[2001:db8::1]:2222");
    }
}
