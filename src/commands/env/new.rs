use std::path::Path;

use clap::Args;
use tap::prelude::*;

use crate::error::Result;
use crate::{nix, templates, ui};

#[derive(Args, Debug, Clone)]
pub struct NewArgs {
    /// Name of the environment
    pub name: String,
}

pub type EnvArgs = NewArgs;

pub async fn run(args: NewArgs, target: Option<&Path>) -> Result<()> {
    nix::with_fleet(target, |fleet_root| async move {
        execute(args, &fleet_root).await
    })
    .await
}

async fn execute(args: NewArgs, fleet_root: &Path) -> Result<()> {
    templates::env::create(&args.name, fleet_root)
        .await
        .tap_ok(|_| {
            ui::success(format!("Created environment '{}'", args.name));

            ui::next_steps(&[
                format!(
                    "Add a host with 'rocky host new <name> --env {}'",
                    args.name
                ),
                format!("Deploy with 'rocky up {}'", args.name),
            ]);
        })
}
