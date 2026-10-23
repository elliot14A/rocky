use std::path::{Path, PathBuf};

use clap::Args;
use tap::prelude::*;

use crate::error::Result;
use crate::process;
use crate::{crypto, nix, ui};

#[derive(Args, Debug, Clone)]
pub struct CheckArgs {
    /// Generate mock .age files for any missing secrets declared in secrets.nix
    #[arg(long)]
    pub mock_secrets: bool,
}

pub async fn run(args: CheckArgs, target: Option<&Path>) -> Result<()> {
    nix::with_fleet(target, |fleet_root| async move {
        execute(args, &fleet_root).await
    })
    .await
}

struct MockSecretGuard<'a> {
    fleet_root: &'a Path,
    paths: Vec<PathBuf>,
}

impl<'a> Drop for MockSecretGuard<'a> {
    fn drop(&mut self) {
        let has_git = self.fleet_root.join(".git").exists();
        for path in &self.paths {
            if has_git {
                let _ = std::process::Command::new("git")
                    .current_dir(self.fleet_root)
                    .args(["rm", "--cached", "-f", "--quiet"])
                    .arg(path)
                    .output();
            }
            if path.exists() {
                let _ = std::fs::remove_file(path);
                tracing::debug!(path = %path.display(), "cleaned up mock secret");
            }
        }
    }
}

async fn execute(args: CheckArgs, fleet_root: &Path) -> Result<()> {
    let _guard = if args.mock_secrets {
        let mocked = crypto::generate_mock_secrets(fleet_root).await?;
        if mocked.is_empty() {
            ui::info("All declared secrets exist on disk; no mock secrets needed.");
        } else {
            let has_git = fleet_root.join(".git").exists();
            for path in &mocked {
                if has_git {
                    let _ = std::process::Command::new("git")
                        .current_dir(fleet_root)
                        .args(["add", "-N", "--quiet"])
                        .arg(path)
                        .output();
                }
                ui::success(format!("Generated mock secret at {}", path.display()));
            }
        }
        Some(MockSecretGuard {
            fleet_root,
            paths: mocked,
        })
    } else {
        None
    };

    ui::info("Running Nix flake checks...");
    process::exec("nix", ["flake", "check", "--no-build"], Some(fleet_root))
        .await
        .tap_ok(|_| {
            ui::success("Nix flake check passed.");
        })?;

    Ok(())
}
