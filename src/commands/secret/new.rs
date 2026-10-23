use std::io::{self, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};

use clap::Args;
use tap::prelude::*;

use crate::error::Result;
use crate::{crypto, fs, nix, ui};

#[derive(Args, Debug, Clone)]
pub struct NewArgs {
    /// Name of the secret (e.g. 'jwt' or 'jwt.age')
    pub name: String,

    /// Target environment (auto-detected if only one exists)
    #[arg(short, long)]
    pub env: Option<String>,

    /// Service(s) to scope recipient keys for (can be comma-separated, e.g. -s postgrest,storage)
    #[arg(short, long, value_delimiter = ',')]
    pub service: Vec<String>,

    /// Scope secret to all hosts in the environment
    #[arg(long)]
    pub all_hosts: bool,

    /// Read secret plaintext content from a file
    #[arg(long, conflicts_with = "value")]
    pub from_file: Option<PathBuf>,

    /// Literal secret plaintext value (discouraged: visible in process cmdline)
    #[arg(long, conflicts_with = "from_file")]
    pub value: Option<String>,
}

pub async fn run(args: NewArgs, target: Option<&Path>) -> Result<()> {
    nix::with_fleet(target, |fleet_root| async move {
        execute(args, &fleet_root).await
    })
    .await
}

async fn execute(args: NewArgs, fleet_root: &Path) -> Result<()> {
    let (env_name, env_dir) = nix::resolve_environment(fleet_root, args.env.as_deref())?;

    let content = if let Some(path) = &args.from_file {
        fs::read_file(path)?
    } else if let Some(val) = &args.value {
        ui::warn("Passing secrets via --value exposes them in process cmdline; prefer stdin or --from-file");
        val.clone()
    } else if !io::stdin().is_terminal() {
        let mut buffer = String::new();
        io::stdin()
            .read_to_string(&mut buffer)
            .map_err(|e| crate::error::RockyError::Io {
                path: PathBuf::from("stdin"),
                source: e,
            })?;
        buffer
    } else {
        eprint!("Enter secret value: ");
        io::stderr()
            .flush()
            .map_err(|e| crate::error::RockyError::Io {
                path: PathBuf::from("stderr"),
                source: e,
            })?;
        rpassword::read_password().map_err(|e| crate::error::RockyError::Io {
            path: PathBuf::from("stdin"),
            source: e,
        })?
    };

    let clean_name = args.name.strip_suffix(".age").unwrap_or(&args.name);
    crypto::secret::create_secret(
        &env_dir,
        clean_name,
        &content,
        &args.service,
        args.all_hosts,
    )
    .await
    .tap_ok(|path| {
        ui::success(format!(
            "Created secret '{clean_name}.age' in environment '{env_name}' at {}",
            path.display()
        ));
    })?;

    Ok(())
}
