use clap::Parser;
use tempfile::{tempdir, TempDir};

use rocky::commands::init::{self, InitArgs};
use rocky::Result;

#[allow(dead_code)]
pub struct TestWorkspace {
    _dir: TempDir,
    pub path: std::path::PathBuf,
}

#[allow(dead_code)]
impl TestWorkspace {
    pub fn new() -> Self {
        let dir = tempdir().expect("failed to create temp dir");
        let path = dir.path().to_path_buf();
        Self { _dir: dir, path }
    }

    pub async fn init(&self, template: &str, force: bool) -> Result<()> {
        self.init_path("rocky", template, force).await
    }

    pub async fn init_default(&self) -> Result<()> {
        #[derive(Parser)]
        struct DummyCli {
            #[command(subcommand)]
            command: rocky::Command,
        }

        let cli = DummyCli::parse_from(["rocky", "init"]);
        match cli.command {
            rocky::Command::Init(mut args) => {
                args.path = self.path.join(args.path);
                init::run(args).await
            }
            _ => unreachable!(),
        }
    }

    pub async fn init_path(
        &self,
        path: impl Into<std::path::PathBuf>,
        template: &str,
        force: bool,
    ) -> Result<()> {
        init::run(InitArgs {
            path: self.path.join(path.into()),
            template: template.to_string(),
            force,
        })
        .await
    }

    pub fn file_exists(&self, relative: &str) -> bool {
        self.path.join(relative).exists()
    }
}
