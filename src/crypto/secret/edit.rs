use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::process::Stdio;

use snafu::{ensure, ResultExt};
use tap::prelude::*;
use tokio::process::Command;

use super::eval_secret_recipients;
use crate::crypto::age;
use crate::crypto::secret::format::{detect_secret_format, validate_key_value, SecretFormat};
use crate::error::{
    CommandExecutionSnafu, IoSnafu, Kind, NotFoundSnafu, ProcessSpawnSnafu, Result, RockyError,
};
use crate::utils::validate::validate_name;
use crate::{fs, nix};

pub async fn edit_secret(env_dir: &Path, name: &str, identity: Option<&Path>) -> Result<bool> {
    ensure!(
        env_dir.is_dir(),
        NotFoundSnafu {
            kind: Kind::Environment,
            name: env_dir.display().to_string(),
            path: env_dir.to_path_buf(),
        }
    );

    let clean_name = name.strip_suffix(".age").unwrap_or(name);
    validate_name(Kind::Secret, clean_name)?;
    let file_name = format!("{clean_name}.age");

    let secrets_dir = nix::resolve_secrets_dir(env_dir);

    let secret_path = secrets_dir.join(&file_name);
    ensure!(
        secret_path.is_file(),
        NotFoundSnafu {
            kind: Kind::Secret,
            name: file_name.clone(),
            path: secret_path.clone(),
        }
    );

    let ciphertext = fs::read_file_bytes(&secret_path)?;
    let decrypted = age::decrypt(&ciphertext, identity).await?;
    let original_content = String::from_utf8(decrypted).map_err(|e| RockyError::Io {
        path: secret_path.clone(),
        source: std::io::Error::new(std::io::ErrorKind::InvalidData, e),
    })?;
    let was_key_value = detect_secret_format(&original_content) == SecretFormat::KeyValue;

    let temp_file = tempfile::Builder::new()
        .prefix("rocky-secret-")
        .tempfile()
        .context(IoSnafu {
            path: env_dir.to_path_buf(),
        })?;

    let temp_path = temp_file.path().to_path_buf();
    fs::write_file(&temp_path, &original_content)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&temp_path, std::fs::Permissions::from_mode(0o600));
    }

    let editor_cmd = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "nano".to_string());

    let parsed_parts = shlex::split(&editor_cmd).unwrap_or_else(|| {
        editor_cmd
            .split_whitespace()
            .map(|s| s.to_string())
            .collect()
    });
    let program = parsed_parts
        .first()
        .cloned()
        .unwrap_or_else(|| "nano".to_string());
    let args = if parsed_parts.len() > 1 {
        parsed_parts[1..].to_vec()
    } else {
        Vec::new()
    };

    let updated_content = loop {
        let mut command = Command::new(&program);
        for arg in &args {
            command.arg(arg);
        }
        command.arg(&temp_path);
        command.stdin(Stdio::inherit());
        command.stdout(Stdio::inherit());
        command.stderr(Stdio::inherit());

        let child_future = command.status();
        let status = tokio::select! {
            res = child_future => {
                res.context(ProcessSpawnSnafu { command: program.clone() })?
            }
            _ = tokio::signal::ctrl_c() => {
                if temp_path.exists() {
                    let _ = std::fs::remove_file(&temp_path);
                }
                return Err(RockyError::CommandExecution {
                    command: format!("{editor_cmd} {}", temp_path.display()),
                    status: 130,
                    stderr: "Secret edit cancelled by user (SIGINT)".to_string(),
                    log_path: None,
                });
            }
        };

        if !status.success() {
            return CommandExecutionSnafu {
                command: format!("{editor_cmd} {}", temp_path.display()),
                status: status.code().unwrap_or(1),
                stderr: "Editor exited with non-zero status; secret edit aborted".to_string(),
                log_path: None,
            }
            .fail();
        }

        let content = fs::read_file(&temp_path)?;
        if content == original_content {
            return Ok(false);
        }

        if was_key_value {
            if let Err(err) = validate_key_value(&content) {
                if io::stdin().is_terminal() {
                    eprintln!("{err}");
                    eprint!("Edit again to fix syntax? [Y/n]: ");
                    io::stderr().flush().ok();

                    let mut input = String::new();
                    let read_bytes = io::stdin().read_line(&mut input).unwrap_or(0);
                    let trimmed = input.trim();
                    if read_bytes > 0
                        && (trimmed.is_empty()
                            || trimmed.eq_ignore_ascii_case("y")
                            || trimmed.eq_ignore_ascii_case("yes"))
                    {
                        continue;
                    }
                }
                return Err(err);
            }
        }

        break content;
    };

    let recipients = eval_secret_recipients(&secrets_dir, &file_name).await?;
    let new_ciphertext = age::encrypt(updated_content.as_bytes(), &recipients).await?;
    fs::write_file_bytes(&secret_path, &new_ciphertext)?;

    Ok(true).tap_ok(|_| {
        tracing::debug!(path = %secret_path.display(), "edited secret");
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_edit_secret_missing_file_fails() {
        let dir = tempfile::tempdir().unwrap();
        let res = edit_secret(dir.path(), "missing", None).await;
        assert!(matches!(res, Err(RockyError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_edit_secret_path_traversal_fails() {
        let dir = tempfile::tempdir().unwrap();
        let res = edit_secret(dir.path(), "../../evil", None).await;
        assert!(matches!(
            res,
            Err(RockyError::InvalidName {
                kind: Kind::Secret,
                ..
            })
        ));
    }
}
