use std::path::Path;

use clap::Args;
use snafu::ensure;

use crate::error::{Kind, NotFoundSnafu, Result};
use crate::process::run_editor;
use crate::{nix, ui};

#[derive(Args, Debug, Clone)]
pub struct EditArgs {
    /// Name of the environment to edit
    pub name: String,
}

pub async fn run(args: EditArgs, target: Option<&Path>) -> Result<()> {
    nix::with_fleet(target, |fleet_root| async move {
        execute(args, &fleet_root).await
    })
    .await
}

async fn execute(args: EditArgs, fleet_root: &Path) -> Result<()> {
    let (env_name, env_dir) = nix::resolve_environment(fleet_root, Some(&args.name))?;
    let fleet_nix = env_dir.join("fleet.nix");

    ensure!(
        fleet_nix.is_file(),
        NotFoundSnafu {
            kind: Kind::Environment,
            name: env_name.clone(),
            path: fleet_nix,
        }
    );

    let updated = run_editor(&fleet_nix, Kind::Environment).await?;
    if updated {
        ui::success(format!(
            "Updated fleet configuration for environment '{env_name}'"
        ));
    } else {
        ui::info(format!("Environment '{env_name}' unchanged"));
    }

    Ok(())
}
