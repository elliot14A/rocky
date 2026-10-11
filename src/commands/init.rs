use std::path::{Path, PathBuf};

use clap::Args;

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
        || templates::fetch(&args.template, &args.path, args.force),
    )
    .await?;

    let mut steps = Vec::new();
    if args.path != Path::new(".") && args.path != Path::new("") {
        steps.push(format!("cd {}", args.path.display()));
    }
    steps.push("rocky new env <name>".to_string());

    ui::next_steps(&steps);

    Ok(())
}
