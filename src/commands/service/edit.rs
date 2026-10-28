use std::path::Path;

use clap::Args;
use snafu::ensure;

use crate::error::{Kind, NotFoundSnafu, Result};
use crate::process::run_editor;
use crate::utils::validate::validate_name;
use crate::{nix, ui};

#[derive(Args, Debug, Clone)]
pub struct EditArgs {
    /// Name of the service to edit (e.g. 'caddy' or 'caddy.nix')
    pub name: String,
}

pub async fn run(args: EditArgs, target: Option<&Path>) -> Result<()> {
    nix::with_fleet(target, |fleet_root| async move {
        execute(args, &fleet_root).await
    })
    .await
}

async fn execute(args: EditArgs, fleet_root: &Path) -> Result<()> {
    let clean_name = args.name.strip_suffix(".nix").unwrap_or(&args.name);
    validate_name(Kind::Service, clean_name)?;

    let service_nix = fleet_root
        .join("services")
        .join(format!("{clean_name}.nix"));

    ensure!(
        service_nix.is_file(),
        NotFoundSnafu {
            kind: Kind::Service,
            name: clean_name.to_string(),
            path: service_nix,
        }
    );

    let updated = run_editor(&service_nix, Kind::Service).await?;
    if updated {
        ui::success(format!("Updated service '{clean_name}'"));
    } else {
        ui::info(format!("Service '{clean_name}' unchanged"));
    }

    Ok(())
}
