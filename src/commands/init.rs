use std::path::{Path, PathBuf};

use clap::Args;
use tap::prelude::*;

use crate::error::Result;
use crate::templates;
use crate::ui;

#[derive(Args, Debug, Clone)]
pub struct InitArgs {
    #[arg(default_value = "rocky")]
    pub path: PathBuf,

    #[arg(short, long, default_value = "base")]
    pub template: String,

    #[arg(short, long)]
    pub force: bool,
}

pub async fn run(args: InitArgs) -> Result<()> {
    ui::with_spinner(
        format!("Initializing fleet with '{}' template...", args.template),
        format!(
            "Initialized fleet with '{}' template in {}",
            args.template,
            args.path.display()
        ),
        || templates::fetch_template(&args.template, &args.path, args.force),
    )
    .await?;

    (args.path != Path::new(".") && args.path != Path::new(""))
        .then(|| format!("cd {}", args.path.display()))
        .into_iter()
        .chain(std::iter::once("rocky new env <name>".to_string()))
        .collect::<Vec<_>>()
        .pipe(|steps| ui::next_steps(&steps));

    Ok(())
}
