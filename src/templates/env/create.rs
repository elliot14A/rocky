use std::path::Path;

use tap::prelude::*;

use crate::error::{AlreadyExistsSnafu, Kind, Result};
use crate::utils::validate::validate_name;
use crate::{fs, nix};

fn generate_fleet_nix(name: &str) -> String {
    format!(
        r#"{{
  settings = {{
    # Public domain Caddy serves on
    domain = "{name}.rocky.internal";

    # Contact address for Let's Encrypt certificates (update to your real email)
    acmeEmail = "admin@{name}.rocky.internal";
  }};

  # Host declarations (added via 'rocky new host <name> --env {name}')
  hosts = {{ }};
}}
"#
    )
}

fn generate_env_nix() -> &'static str {
    r#"{
  # Environment-specific configuration overrides passed to service modules
  # e.g.:
  # caddy = { };
}
"#
}

fn generate_keys_nix() -> &'static str {
    r#"{
  # Host SSH public keys recorded during bootstrap ('rocky up')
  # Maps host name -> public key, e.g.:
  # "web-01" = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI...";
}
"#
}

fn generate_secrets_nix() -> &'static str {
    r#"let
  operators = import ../../../keys.nix;
  fleet = import ../fleet.nix;
  hostKeys = import ../keys.nix;

  inherit
    (import ../../../lib/secrets.nix {
      inherit operators fleet hostKeys;
    })
    recipientsFor
    allHosts
    ;
in
{
  # Declare age-encrypted secrets here:
  # "tailscale-authkey.age".publicKeys = allHosts;
  # "api-token.env.age".publicKeys = recipientsFor [ "caddy" ];
}
"#
}

pub async fn create(name: &str, fleet_root: &Path) -> Result<()> {
    validate_name(Kind::Environment, name)?;

    let env_dir = fleet_root.join("environments").join(name);
    if env_dir.exists() {
        return AlreadyExistsSnafu {
            kind: Kind::Environment,
            name: name.to_string(),
            path: env_dir,
        }
        .fail();
    }

    let hosts_dir = env_dir.join("hosts");
    let secrets_dir = env_dir.join("secrets");

    fs::ensure_dir(&env_dir)?;

    let mut rollback = fs::Rollback::new();
    rollback.remove_dir_on_failure(&env_dir);

    fs::ensure_dir(&hosts_dir)?;
    fs::ensure_dir(&secrets_dir)?;

    let fleet_path = env_dir.join("fleet.nix");
    let env_path = env_dir.join("env.nix");
    let keys_path = env_dir.join("keys.nix");
    let secrets_path = secrets_dir.join("secrets.nix");

    fs::write_file(&fleet_path, &generate_fleet_nix(name))?;
    fs::write_file(&env_path, generate_env_nix())?;
    fs::write_file(&keys_path, generate_keys_nix())?;
    fs::write_file(&secrets_path, generate_secrets_nix())?;
    nix::fmt([&fleet_path, &env_path, &keys_path, &secrets_path]).await?;

    rollback.commit();

    Ok(()).tap_ok(|_| {
        tracing::info!(env = %name, "scaffolded environment");
    })
}
