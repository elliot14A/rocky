use std::path::Path;

use clap::Args;
use tap::prelude::*;

use crate::error::Result;
use crate::{nix, templates, ui};

#[derive(Args, Debug, Clone)]
pub struct ServiceArgs {
    /// Name of the service
    pub name: String,
}

pub async fn run(args: ServiceArgs, target: Option<&Path>) -> Result<()> {
    nix::with_fleet(target, |fleet_root| async move {
        execute(args, &fleet_root).await
    })
    .await
}

async fn execute(args: ServiceArgs, fleet_root: &Path) -> Result<()> {
    templates::service::create(&args.name, fleet_root)
        .await
        .tap_ok(|_| {
            ui::success(format!("Created service '{}'", args.name));

            ui::next_steps(&[
                format!(
                    "Enable the service by adding \"{}\" to a host's services list in environments/<env>/fleet.nix",
                    args.name
                ),
                format!("Configure the service in services/{}.nix", args.name),
            ]);
        })
}
