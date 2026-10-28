use std::path::Path;

use clap::Args;

use crate::error::Result;
use crate::{nix, ui};

#[derive(Args, Debug, Clone, Default)]
pub struct ListArgs {
    /// Target environment to inspect
    #[arg(value_name = "ENV", help = "Target environment to inspect")]
    pub env: Option<String>,

    #[arg(
        short,
        long = "env",
        value_name = "ENV",
        conflicts_with = "env",
        help = "Target environment"
    )]
    pub env_flag: Option<String>,
}

impl ListArgs {
    pub fn resolved_env(&self) -> Option<&str> {
        self.env.as_deref().or(self.env_flag.as_deref())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretInfo {
    pub name: String,
    pub environment: String,
    pub recipients: String,
}

pub async fn run(args: ListArgs, target: Option<&Path>) -> Result<()> {
    nix::with_fleet(target, |fleet_root| async move {
        execute(args, &fleet_root).await
    })
    .await
}

pub async fn collect_secrets(
    fleet_root: &Path,
    env_filter: Option<&str>,
) -> Result<Vec<SecretInfo>> {
    let mut entries = Vec::new();

    if let Some(env_name) = env_filter {
        let (resolved_name, env_dir) = nix::resolve_environment(fleet_root, Some(env_name))?;
        collect_secrets_from_env(&resolved_name, &env_dir, &mut entries).await;
    } else {
        let env_dirs = nix::list_environments(fleet_root)?;
        for (name, env_dir) in env_dirs {
            collect_secrets_from_env(&name, &env_dir, &mut entries).await;
        }
    }

    entries.sort_by(|a, b| {
        a.environment
            .cmp(&b.environment)
            .then_with(|| a.name.cmp(&b.name))
    });
    Ok(entries)
}

async fn execute(args: ListArgs, fleet_root: &Path) -> Result<()> {
    let entries = collect_secrets(fleet_root, args.resolved_env()).await?;

    if entries.is_empty() {
        if let Some(env_name) = args.resolved_env() {
            ui::warn(format!("No secrets found in environment '{env_name}'"));
        } else {
            ui::warn("No secrets found in fleet");
        }
        return Ok(());
    }

    println!("{:<24} {:<16} {:<24}", "SECRET", "ENVIRONMENT", "RECIPIENTS");
    println!("{:-<24} {:-<16} {:-<24}", "", "", "");

    for entry in entries {
        println!(
            "{:<24} {:<16} {:<24}",
            entry.name, entry.environment, entry.recipients
        );
    }

    Ok(())
}

async fn collect_secrets_from_env(
    env_name: &str,
    env_dir: &Path,
    entries: &mut Vec<SecretInfo>,
) {
    let secrets_dir = nix::resolve_secrets_dir(env_dir);
    if let Ok(dir_entries) = std::fs::read_dir(&secrets_dir) {
        let mut age_files: Vec<String> = dir_entries
            .flatten()
            .filter(|e| {
                e.path().is_file()
                    && e.path().extension().and_then(|s| s.to_str()) == Some("age")
            })
            .filter_map(|e| e.file_name().into_string().ok())
            .collect();
        age_files.sort();

        let counts = crate::crypto::secret::eval_all_secret_recipients(&secrets_dir)
            .await
            .unwrap_or_default();

        for file in age_files {
            let recipients = match counts.get(&file) {
                Some(0) => "0 keys".to_string(),
                Some(n) => format!("{n} keys"),
                None => "unregistered".to_string(),
            };

            entries.push(SecretInfo {
                name: file,
                environment: env_name.to_string(),
                recipients,
            });
        }
    }
}
