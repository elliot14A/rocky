#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use super::workspace::TestWorkspace;

pub fn setup_fleet(ws: &TestWorkspace) -> PathBuf {
    let fleet_path = ws.path.join("rocky");
    fs::create_dir_all(&fleet_path).expect("failed to create fleet directory");
    fs::write(
        fleet_path.join("flake.nix"),
        r#"{
  description = "rocky test fleet";
  outputs = inputs: {
    rockyFleet = true;
    config = {
      defaultProvider = "hetzner";
    };
  };
}
"#,
    )
    .expect("failed to write flake.nix");
    fleet_path
}

pub fn setup_fleet_bare(ws: &TestWorkspace) -> PathBuf {
    let fleet_path = ws.path.join("rocky");
    fs::create_dir_all(&fleet_path).expect("failed to create fleet directory");
    fs::write(
        fleet_path.join("flake.nix"),
        r#"{
  description = "rocky test fleet";
  outputs = inputs: {
    rockyFleet = true;
  };
}
"#,
    )
    .expect("failed to write bare flake.nix");
    fleet_path
}

pub fn setup_env(fleet_path: &Path, env_name: &str) -> PathBuf {
    let env_dir = fleet_path.join("environments").join(env_name);
    fs::create_dir_all(&env_dir).expect("failed to create environment directory");
    fs::write(env_dir.join("fleet.nix"), "{\n  hosts = { };\n}\n")
        .expect("failed to write fleet.nix");
    env_dir
}

pub fn setup_env_with_hosts(
    fleet_path: &Path,
    env_name: &str,
    hosts_content: &str,
    keys_content: Option<&str>,
) -> PathBuf {
    let env_dir = fleet_path.join("environments").join(env_name);
    fs::create_dir_all(&env_dir).expect("failed to create environment directory");

    let fleet_nix = format!(
        r#"{{
  domain = "{env_name}.rocky.internal";
  hosts = {{
{hosts_content}
  }};
}}
"#
    );
    fs::write(env_dir.join("fleet.nix"), fleet_nix).expect("failed to write fleet.nix");

    if let Some(keys) = keys_content {
        fs::write(env_dir.join("keys.nix"), keys).expect("failed to write keys.nix");
    }

    env_dir
}
