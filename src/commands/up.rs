use std::collections::HashMap;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};

use clap::Args;
use snafu::{ensure, ResultExt};

use crate::crypto;
use crate::error::{
    CommandExecutionSnafu, IoSnafu, Kind, MissingTargetIpSnafu, NotFoundSnafu, Result,
};
use crate::fs::Rollback;
use crate::nix::{
    build_plan, diff_env, extract_host_services, format_plan, register_host_key, with_fleet,
    FleetPlan,
};
use crate::process::{deploy_fleet, install_fleet, HostDeploy, HostInstall};
use crate::ssh::{generate_host_key, parse_target, parse_targets, Target};
use crate::ui;

#[derive(Args, Debug, Clone)]
pub struct UpArgs {
    #[arg(help = "Target environment to deploy (e.g. prod, staging)")]
    pub environment: String,

    #[arg(
        short,
        long,
        help = "Map host to rescue IP/port (e.g. --target node-01=192.168.1.10:2222)"
    )]
    pub target: Vec<String>,

    #[arg(short, long, help = "SSH private key path for authentication")]
    pub identity: Option<PathBuf>,

    #[arg(short, long, help = "Skip interactive confirmation")]
    pub yes: bool,

    #[arg(
        short,
        long,
        help = "Maximum concurrent host installations/deployments"
    )]
    pub concurrency: Option<usize>,
}

pub fn resolve_targets(
    raw_targets: &[String],
    pending_hosts: &[String],
    interactive: bool,
) -> Result<HashMap<String, Target>> {
    let mut targets = parse_targets(raw_targets)?;

    for host in pending_hosts {
        if targets.contains_key(host) {
            continue;
        }

        if !interactive {
            return MissingTargetIpSnafu { host: host.clone() }.fail();
        }

        let target = prompt_target(host)?;
        targets.insert(host.clone(), target);
    }

    Ok(targets)
}

fn prompt_target(host: &str) -> Result<Target> {
    loop {
        print!("Enter rescue IP or address for host '{host}': ");
        io::stdout().flush().ok();

        let mut input = String::new();
        let bytes_read = io::stdin().read_line(&mut input).context(IoSnafu {
            path: PathBuf::from("stdin"),
        })?;

        if bytes_read == 0 {
            return MissingTargetIpSnafu {
                host: host.to_string(),
            }
            .fail();
        }

        let trimmed = input.trim();
        if trimmed.is_empty() {
            ui::error("Address cannot be empty. Please try again.");
            continue;
        }

        let raw = if let Some((entered_host, addr)) = trimmed.split_once('=') {
            let entered_host = entered_host.trim();
            if entered_host != host {
                ui::error(format!(
                    "Host name mismatch in '{trimmed}'. Expected '{host}' but got '{entered_host}'."
                ));
                continue;
            }
            format!("{entered_host}={}", addr.trim())
        } else {
            format!("{host}={trimmed}")
        };

        match parse_target(&raw) {
            Ok(target) => return Ok(target),
            Err(e) => {
                ui::error(format!("Invalid target: {e}. Please try again."));
            }
        }
    }
}

pub fn confirm_plan(assume_yes: bool, interactive: bool) -> Result<bool> {
    if assume_yes {
        return Ok(true);
    }

    if !interactive {
        return Ok(false);
    }

    print!("Proceed with deployment? [y/N]: ");
    io::stdout().flush().ok();

    let mut input = String::new();
    let bytes_read = io::stdin().read_line(&mut input).unwrap_or(0);
    if bytes_read == 0 {
        return Ok(false);
    }

    let trimmed = input.trim();
    Ok(trimmed.eq_ignore_ascii_case("y") || trimmed.eq_ignore_ascii_case("yes"))
}

async fn install_hosts(
    fleet_root: &Path,
    env_dir: &Path,
    plan: &FleetPlan,
    identity: Option<&Path>,
    concurrency: Option<usize>,
) -> Result<()> {
    let keys_nix_path = env_dir.join("keys.nix");
    let mut rollback = Rollback::new();

    if keys_nix_path.exists() {
        let original_keys = crate::fs::read_file(&keys_nix_path)?;
        rollback.restore_file_on_failure(&keys_nix_path, original_keys);
    } else {
        rollback.remove_file_on_failure(&keys_nix_path);
    }

    let mut installs = Vec::new();
    let mut key_guards = Vec::new();

    for item in &plan.install {
        ui::info(format!("Generating host key for '{}'...", item.host));
        let key = generate_host_key(&item.host).await?;

        ui::info(format!(
            "Registering public key in keys.nix for '{}'...",
            item.host
        ));
        register_host_key(&keys_nix_path, &item.host, &key.public_key).await?;

        installs.push(HostInstall {
            name: item.host.clone(),
            target_ip: item.address.clone(),
            ssh_port: item.port,
            extra_files_dir: key.extra_files_dir.path().to_path_buf(),
        });

        key_guards.push(key);
    }

    rekey_environment(env_dir, &plan.environment, identity).await?;

    ui::info(format!("Installing {} host(s)...", installs.len()));
    let results = install_fleet(&installs, fleet_root, identity, concurrency.unwrap_or(4)).await;

    let mut failed_hosts = Vec::new();
    for res in results {
        match res.status {
            Ok(()) => ui::success(format!("Installed host '{}'", res.name)),
            Err(err) => {
                ui::error(format!(
                    "Failed to install host '{}': {err} (log: {})",
                    res.name,
                    res.log_path.display()
                ));
                failed_hosts.push(res.name);
            }
        }
    }

    if !failed_hosts.is_empty() {
        return CommandExecutionSnafu {
            command: "install hosts".to_string(),
            status: 1,
            stderr: format!(
                "Installation failed for host(s): {}",
                failed_hosts.join(", ")
            ),
            log_path: None,
        }
        .fail();
    }

    rollback.commit();
    ui::success("All pending hosts installed successfully.");
    Ok(())
}

async fn rekey_environment(
    env_dir: &Path,
    environment: &str,
    identity: Option<&Path>,
) -> Result<()> {
    let secrets_dir = env_dir.join("secrets");
    if secrets_dir.join("secrets.nix").is_file() {
        ui::info(format!(
            "Rekeying secrets for environment '{environment}'..."
        ));
        crypto::rekey(&secrets_dir, identity).await?;
        ui::success("Secrets rekeyed successfully.");
    }
    Ok(())
}

async fn deploy_hosts(
    fleet_root: &Path,
    plan: &FleetPlan,
    identity: Option<&Path>,
    concurrency: Option<usize>,
) -> Result<()> {
    let deploys = plan
        .deploy
        .iter()
        .map(|item| HostDeploy {
            name: item.host.clone(),
            hostname: if item.address == item.host {
                None
            } else {
                Some(item.address.clone())
            },
            port: if item.port == 22 {
                None
            } else {
                Some(item.port)
            },
        })
        .collect::<Vec<_>>();

    ui::info(format!("Deploying {} host(s)...", deploys.len()));
    let results = deploy_fleet(&deploys, fleet_root, identity, concurrency.unwrap_or(4)).await;

    let mut failed_hosts = Vec::new();
    for res in results {
        match res.status {
            Ok(()) => ui::success(format!("Deployed host '{}'", res.name)),
            Err(err) => {
                ui::error(format!(
                    "Failed to deploy host '{}': {err} (log: {})",
                    res.name,
                    res.log_path.display()
                ));
                failed_hosts.push(res.name);
            }
        }
    }

    if !failed_hosts.is_empty() {
        return CommandExecutionSnafu {
            command: "deploy hosts".to_string(),
            status: 1,
            stderr: format!("Deployment failed for host(s): {}", failed_hosts.join(", ")),
            log_path: None,
        }
        .fail();
    }

    ui::success("All hosts deployed successfully.");
    Ok(())
}

pub async fn run(args: UpArgs, target: Option<&Path>) -> Result<()> {
    with_fleet(target, |fleet_root| async move {
        execute(args, &fleet_root).await
    })
    .await
}

async fn execute(args: UpArgs, fleet_root: &Path) -> Result<()> {
    let env_dir = fleet_root.join("environments").join(&args.environment);
    ensure!(
        env_dir.is_dir(),
        NotFoundSnafu {
            kind: Kind::Environment,
            name: args.environment.clone(),
            path: env_dir,
        }
    );

    let fleet_path = env_dir.join("fleet.nix");
    ensure!(
        fleet_path.is_file(),
        NotFoundSnafu {
            kind: Kind::Fleet,
            name: "fleet.nix".to_string(),
            path: fleet_path,
        }
    );

    let interactive = io::stdin().is_terminal() && io::stdout().is_terminal();

    let diff = diff_env(&env_dir)?;
    let pending_hosts = diff.pending_install.clone();

    let targets = resolve_targets(&args.target, &pending_hosts, interactive)?;

    let fleet_src = crate::fs::read_file(&fleet_path)?;
    let services = extract_host_services(&fleet_src, &fleet_path)?;

    let plan = build_plan(&args.environment, &diff, &targets, &services)?;
    let formatted = format_plan(&plan);
    ui::info(&formatted);

    if plan.is_empty() {
        return Ok(());
    }

    if !confirm_plan(args.yes, interactive)? {
        ui::info("Deployment aborted.");
        return Ok(());
    }

    let identity = args.identity.as_deref();

    if !plan.install.is_empty() {
        install_hosts(fleet_root, &env_dir, &plan, identity, args.concurrency).await?;
    }

    if !plan.deploy.is_empty() {
        deploy_hosts(fleet_root, &plan, identity, args.concurrency).await?;
    }

    ui::success(format!(
        "Environment '{}' is now up to date.",
        args.environment
    ));

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_targets_non_interactive_missing_fails() {
        let raw = vec!["node-01=192.168.1.10".to_string()];
        let pending = vec!["node-01".to_string(), "node-02".to_string()];
        let err = resolve_targets(&raw, &pending, false).unwrap_err();
        assert!(err.to_string().contains("node-02"));
    }

    #[test]
    fn resolve_targets_all_provided_succeeds() {
        let raw = vec![
            "node-01=192.168.1.10:22".to_string(),
            "node-02=10.0.0.1:2222".to_string(),
        ];
        let pending = vec!["node-01".to_string(), "node-02".to_string()];
        let targets = resolve_targets(&raw, &pending, false).unwrap();
        assert_eq!(targets.len(), 2);
        assert_eq!(targets.get("node-01").unwrap().port, 22);
        assert_eq!(targets.get("node-02").unwrap().port, 2222);
    }

    #[test]
    fn confirm_plan_assume_yes() {
        assert!(confirm_plan(true, false).unwrap());
    }

    #[test]
    fn confirm_plan_non_interactive_no_assume_yes() {
        assert!(!confirm_plan(false, false).unwrap());
    }
}
