use std::ffi::OsStr;
use std::path::Path;
use std::process::Stdio;

use snafu::ResultExt;
use tap::prelude::*;
use tokio::process::Command;

use crate::error::{CommandExecutionSnafu, CommandSignaledSnafu, ProcessSpawnSnafu, Result};

pub async fn exec<I, S>(cmd: &str, args: I, cwd: Option<&Path>) -> Result<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let args_vec: Vec<_> = args
        .into_iter()
        .map(|s| s.as_ref().to_os_string())
        .collect();

    let mut command = Command::new(cmd);
    command.args(&args_vec);
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    if let Some(dir) = cwd {
        command.current_dir(dir);
    }

    let output = command
        .output()
        .await
        .context(ProcessSpawnSnafu { command: cmd })
        .tap_err(|err| tracing::error!(%cmd, %err, "failed to spawn process"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let display_cmd = if args_vec.is_empty() {
            cmd.to_string()
        } else {
            format!(
                "{} {}",
                cmd,
                args_vec
                    .iter()
                    .map(|a| a.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        };

        return match output.status.code() {
            Some(status) => CommandExecutionSnafu {
                command: display_cmd,
                status,
                stderr,
            }
            .fail(),
            None => CommandSignaledSnafu {
                command: display_cmd,
            }
            .fail(),
        };
    }

    String::from_utf8_lossy(&output.stdout).to_string().pipe(Ok)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_exec_echo_success() {
        let res = exec("echo", ["rocky-test"], None).await;
        assert!(res.is_ok());
        assert_eq!(res.unwrap().trim(), "rocky-test");
    }

    #[tokio::test]
    async fn test_exec_failure_status() {
        let res = exec("false", [] as [&str; 0], None).await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn test_exec_spawn_error_on_nonexistent_binary() {
        let res = exec("nonexistent_binary_xyz_123", [] as [&str; 0], None).await;
        assert!(res.is_err());
    }
}
