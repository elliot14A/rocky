use std::path::Path;

use clap::Args;
use tap::prelude::*;

use crate::error::Result;
use crate::{nix, templates, ui};

#[derive(Args, Debug, Clone)]
pub struct HostArgs {
    /// Name of the host
    pub name: String,

    /// Target environment (defaults to the first available environment if omitted)
    #[arg(short, long)]
    pub env: Option<String>,

    /// Cloud VPS provider (defaults to flake.nix defaultProvider)
    #[arg(short, long)]
    pub provider: Option<String>,
}

pub async fn run(args: HostArgs, target: Option<&Path>) -> Result<()> {
    nix::with_fleet(target, |fleet_root| async move {
        execute(args, &fleet_root).await
    })
    .await
}

async fn execute(args: HostArgs, fleet_root: &Path) -> Result<()> {
    let provider = match args.provider {
        Some(p) => p,
        None => nix::eval_default_provider(fleet_root).await?,
    };

    templates::host::create(&args.name, args.env.as_deref(), &provider, fleet_root)
        .await
        .tap_ok(|env| {
            ui::success(format!(
                "Created host '{}' in environment '{}' ({})",
                args.name, env, provider
            ));

            ui::next_steps(&[
                format!(
                    "Add '{}' to environments/{}/fleet.nix hosts list",
                    args.name, env
                ),
                format!("Deploy with 'rocky up {}'", env),
            ]);
        })
        .map(|_| ())
}
