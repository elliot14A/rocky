use std::path::Path;

use snafu::ResultExt;
use tap::prelude::*;

use crate::error::{AlreadyExistsSnafu, IoSnafu, Kind, NotFoundSnafu, Result};
use crate::templates::fetch_provider;
use crate::utils::validate::validate_name;
use crate::{fs, nix};

fn resolve_environment(envs_dir: &Path, env: Option<&str>) -> Result<String> {
    match env {
        Some(e) => {
            validate_name(Kind::Environment, e)?;
            let target_env_dir = envs_dir.join(e);
            target_env_dir
                .is_dir()
                .then(|| e.to_string())
                .ok_or_else(|| {
                    NotFoundSnafu {
                        kind: Kind::Environment,
                        name: e.to_string(),
                        path: target_env_dir,
                    }
                    .build()
                })
        }
        None => {
            if !envs_dir.is_dir() {
                return NotFoundSnafu {
                    kind: Kind::Environment,
                    name: "any".to_string(),
                    path: envs_dir,
                }
                .fail();
            }

            let available: Vec<String> = std::fs::read_dir(envs_dir)
                .context(IoSnafu { path: envs_dir })?
                .filter_map(|entry| entry.ok())
                .filter(|entry| entry.path().is_dir())
                .filter_map(|entry| entry.file_name().into_string().ok())
                .collect::<Vec<_>>()
                .tap_mut(|v| v.sort());

            match available.as_slice() {
                [first, ..] => Ok(first.clone()),
                [] => NotFoundSnafu {
                    kind: Kind::Environment,
                    name: "any".to_string(),
                    path: envs_dir,
                }
                .fail(),
            }
        }
    }
}

fn generate_default_nix(env: &str, name: &str) -> String {
    format!(
        r#"{{ inputs, ... }}:
{{
  imports = [
    inputs.disko.nixosModules.disko
    ./disko.nix
    ./hardware.nix
  ];
  networking.hostName = "{env}-{name}";
  system.stateVersion = "26.05";
}}
"#
    )
}

pub async fn create(
    name: &str,
    env: Option<&str>,
    provider: &str,
    fleet_root: &Path,
) -> Result<String> {
    validate_name(Kind::Host, name)?;
    validate_name(Kind::Provider, provider)?;

    let env_name = resolve_environment(&fleet_root.join("environments"), env)?;
    let host_dir = fleet_root
        .join("environments")
        .join(&env_name)
        .join("hosts")
        .join(name);

    if host_dir.exists() {
        return AlreadyExistsSnafu {
            kind: Kind::Host,
            name: name.to_string(),
            path: host_dir,
        }
        .fail();
    }

    fs::ensure_dir(&host_dir)?;

    fetch_provider(provider, &host_dir).await.tap_err(|_| {
        let _ = std::fs::remove_dir_all(&host_dir);
    })?;

    let default_path = host_dir.join("default.nix");
    generate_default_nix(&env_name, name)
        .pipe(|content| fs::write_file(&default_path, &content))?;

    nix::fmt([&default_path]).await?;

    tracing::info!(host = %name, env = %env_name, provider = %provider, "scaffolded host");

    Ok(env_name)
}
