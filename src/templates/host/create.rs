use std::path::Path;

use tap::prelude::*;

use crate::error::{AlreadyExistsSnafu, Kind, NotFoundSnafu, Result};
use crate::templates::{fetch_partitions, fetch_provider};
use crate::validate::validate_name;
use crate::{fs, nix};

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

    let (env_name, env_dir) = nix::resolve_environment(fleet_root, env)?;
    let fleet_nix_path = env_dir.join("fleet.nix");

    if !fleet_nix_path.is_file() {
        return NotFoundSnafu {
            kind: Kind::Fleet,
            name: "fleet.nix".to_string(),
            path: fleet_nix_path,
        }
        .fail();
    }

    let fleet_content = fs::read_file(&fleet_nix_path)?;
    let updated_fleet = nix::insert_host_entry(&fleet_content, name, &fleet_nix_path)?;

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

    let mut rollback = fs::Rollback::new();
    rollback.remove_dir_on_failure(&host_dir);
    rollback.restore_file_on_failure(&fleet_nix_path, &fleet_content);

    fetch_partitions(&host_dir).await?;
    fetch_provider(provider, &host_dir).await?;

    let default_path = host_dir.join("default.nix");
    let default_content = generate_default_nix(&env_name, name);
    fs::write_file(&default_path, &default_content)?;

    fs::write_file(&fleet_nix_path, &updated_fleet)?;

    nix::fmt([
        &default_path,
        &host_dir.join("disko.nix"),
        &host_dir.join("hardware.nix"),
        &fleet_nix_path,
    ])
    .await?;

    rollback.commit();

    Ok(env_name).tap_ok(|env| {
        tracing::info!(host = %name, env = %env, provider = %provider, "scaffolded host");
    })
}
