use std::io::Write;
use std::path::Path;
use std::process::Stdio;

use snafu::{ensure, ResultExt};
use tokio::process::Command;

use crate::error::{
    CommandExecutionSnafu, IoSnafu, Kind, NixParseSnafu, NotFoundSnafu, ProcessSpawnSnafu, Result,
    RockyError,
};
use crate::fs;

#[derive(Debug, Clone)]
pub struct EditorCommand {
    pub program: String,
    pub args: Vec<String>,
    pub display: String,
}

pub fn resolve_editor() -> EditorCommand {
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

    EditorCommand {
        program,
        args,
        display: editor_cmd,
    }
}

pub async fn run_editor(path: &Path, kind: Kind) -> Result<bool> {
    ensure!(
        path.is_file(),
        NotFoundSnafu {
            kind,
            name: path.display().to_string(),
            path: path.to_path_buf(),
        }
    );

    let original_content = fs::read_file(path)?;

    // Stage edits in an open temporary file to prevent live file corruption
    let temp_parent = path.parent().unwrap_or_else(|| Path::new("."));
    let mut temp_file = tempfile::Builder::new()
        .prefix(".rocky-edit-")
        .suffix(
            path.extension()
                .and_then(|s| s.to_str())
                .map(|ext| format!(".{ext}"))
                .as_deref()
                .unwrap_or(".tmp"),
        )
        .tempfile_in(temp_parent)
        .context(IoSnafu {
            path: temp_parent.to_path_buf(),
        })?;

    temp_file
        .write_all(original_content.as_bytes())
        .context(IoSnafu {
            path: temp_file.path().to_path_buf(),
        })?;
    temp_file.flush().context(IoSnafu {
        path: temp_file.path().to_path_buf(),
    })?;

    let temp_path = temp_file.path().to_path_buf();
    let editor = resolve_editor();

    let mut command = Command::new(&editor.program);
    for arg in &editor.args {
        command.arg(arg);
    }
    command.arg(&temp_path);
    command.stdin(Stdio::inherit());
    command.stdout(Stdio::inherit());
    command.stderr(Stdio::inherit());
    command.kill_on_drop(true);

    let child_future = command.status();
    let status = tokio::select! {
        res = child_future => {
            res.context(ProcessSpawnSnafu { command: editor.program.clone() })?
        }
        _ = tokio::signal::ctrl_c() => {
            return Err(RockyError::CommandExecution {
                command: format!("{} {}", editor.display, path.display()),
                status: 130,
                stderr: "Edit cancelled by user (SIGINT)".to_string(),
                log_path: None,
            });
        }
    };

    if !status.success() {
        return CommandExecutionSnafu {
            command: format!("{} {}", editor.display, path.display()),
            status: status.code().unwrap_or(1),
            stderr: "Editor exited with non-zero status; edit aborted".to_string(),
            log_path: None,
        }
        .fail();
    }

    let updated_content = fs::read_file(&temp_path)?;
    if updated_content == original_content {
        return Ok(false);
    }

    // Validate Nix syntax if editing a .nix file
    if path.extension().and_then(|s| s.to_str()) == Some("nix") {
        let parse = rnix::Root::parse(&updated_content);
        let errors = parse.errors();
        if !errors.is_empty() {
            return NixParseSnafu {
                path: path.to_path_buf(),
                reason: format!("syntax errors: {:?}", errors),
            }
            .fail();
        }
    }

    // Atomically write validated content to live path
    fs::write_file(path, &updated_content)?;

    Ok(true)
}
