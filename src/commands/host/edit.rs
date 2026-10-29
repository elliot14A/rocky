use std::path::{Path, PathBuf};

use clap::Args;
use snafu::ensure;

use crate::error::{AmbiguousEnvironmentSnafu, Kind, NotFoundSnafu, Result};
use crate::process::run_editor;
use crate::validate::validate_name;
use crate::{nix, ui};

#[derive(Args, Debug, Clone)]
pub struct EditArgs {
    /// Name of the host to edit
    pub name: String,

    /// Target environment (auto-detected if omitted and unambiguous)
    #[arg(short, long)]
    pub env: Option<String>,
}

pub async fn run(args: EditArgs, target: Option<&Path>) -> Result<()> {
    nix::with_fleet(target, |fleet_root| async move {
        execute(args, &fleet_root).await
    })
    .await
}

async fn execute(args: EditArgs, fleet_root: &Path) -> Result<()> {
    validate_name(Kind::Host, &args.name)?;

    let (env_name, env_dir) = if let Some(ref env) = args.env {
        nix::resolve_environment(fleet_root, Some(env))?
    } else {
        resolve_host_environment(fleet_root, &args.name)?
    };

    let host_dir = env_dir.join("hosts").join(&args.name);
    let host_nix = host_dir.join("default.nix");

    ensure!(
        host_nix.is_file(),
        NotFoundSnafu {
            kind: Kind::Host,
            name: args.name.clone(),
            path: host_nix,
        }
    );

    let updated = run_editor(&host_nix, Kind::Host).await?;
    if updated {
        ui::success(format!(
            "Updated host '{}' in environment '{}'",
            args.name, env_name
        ));
    } else {
        ui::info(format!(
            "Host '{}' unchanged in environment '{}'",
            args.name, env_name
        ));
    }

    Ok(())
}

fn resolve_host_environment(fleet_root: &Path, host_name: &str) -> Result<(String, PathBuf)> {
    let env_dirs = nix::list_environments(fleet_root)?;
    let mut matching_envs = Vec::new();

    for (env_name, env_path) in env_dirs {
        let host_file = env_path.join("hosts").join(host_name).join("default.nix");
        if host_file.is_file() {
            matching_envs.push((env_name, env_path));
        }
    }

    match matching_envs.len() {
        1 => Ok(matching_envs.remove(0)),
        0 => {
            // If the host does not exist in any environment, fail with NotFound Host
            NotFoundSnafu {
                kind: Kind::Host,
                name: host_name.to_string(),
                path: fleet_root.to_path_buf(),
            }
            .fail()
        }
        _ => {
            let env_names: Vec<String> = matching_envs.into_iter().map(|(e, _)| e).collect();
            AmbiguousEnvironmentSnafu {
                environments: env_names,
            }
            .fail()
        }
    }
}
