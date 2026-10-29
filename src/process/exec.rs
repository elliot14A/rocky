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
    command.stdin(Stdio::null());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    command.kill_on_drop(true);

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
        let sanitized_stderr = sanitize_sensitive_string(&stderr);
        let display_cmd = if args_vec.is_empty() {
            cmd.to_string()
        } else {
            format!(
                "{} {}",
                cmd,
                sanitize_display_args(&args_vec).join(" ")
            )
        };
        let sanitized_display_cmd = sanitize_sensitive_string(&display_cmd);

        return match output.status.code() {
            Some(status) => CommandExecutionSnafu {
                command: sanitized_display_cmd,
                status,
                stderr: sanitized_stderr,
                log_path: None,
            }
            .fail(),
            None => CommandSignaledSnafu {
                command: sanitized_display_cmd,
            }
            .fail(),
        };
    }

    String::from_utf8_lossy(&output.stdout).to_string().pipe(Ok)
}

fn sanitize_sensitive_string(raw: &str) -> String {
    let mut sanitized = raw.to_string();
    for prefix in &["tskey-auth-", "hskey-auth-", "tskey-", "hskey-"] {
        let mut search_from = 0;
        while let Some(rel_start) = sanitized[search_from..].find(prefix) {
            let start = search_from + rel_start;
            let rel_end = sanitized[start..]
                .find(|c: char| c.is_whitespace() || c == '\'' || c == '"' || c == ',' || c == '=')
                .unwrap_or(sanitized[start..].len());
            let end = start + rel_end;
            let replacement = "[AUTHKEY-REDACTED]";
            sanitized.replace_range(start..end, replacement);
            search_from = start + replacement.len();
        }
    }
    sanitized
}

fn sanitize_display_args(args: &[std::ffi::OsString]) -> Vec<String> {
    let mut out = Vec::with_capacity(args.len());
    let mut redact_next = false;
    for arg in args {
        let s = arg.to_string_lossy();
        if redact_next {
            out.push("[REDACTED]".to_string());
            redact_next = false;
            continue;
        }

        if s == "--authkey" || s == "-authkey" || s == "--auth-key" || s == "-auth-key" {
            out.push(s.to_string());
            redact_next = true;
            continue;
        }

        if let Some((k, _)) = s.split_once('=') {
            let k_lower = k.to_ascii_lowercase();
            if k_lower.contains("auth")
                || k_lower.contains("password")
                || k_lower.contains("token")
                || k_lower.contains("secret")
            {
                out.push(format!("{k}=[REDACTED]"));
                continue;
            }
        }

        out.push(s.to_string());
    }
    out
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

    #[test]
    fn test_sanitize_sensitive_string_redacts_authkeys() {
        let raw = "error: tailscale up --authkey tskey-auth-k123456789 --login-server https://headscale.foo";
        let sanitized = sanitize_sensitive_string(raw);
        assert_eq!(
            sanitized,
            "error: tailscale up --authkey [AUTHKEY-REDACTED] --login-server https://headscale.foo"
        );

        let multiple = "first: tskey-auth-1234, second: hskey-auth-5678, done";
        let sanitized_mult = sanitize_sensitive_string(multiple);
        assert_eq!(
            sanitized_mult,
            "first: [AUTHKEY-REDACTED], second: [AUTHKEY-REDACTED], done"
        );
    }

    #[test]
    fn test_sanitize_display_args_redacts_auth_key_variations() {
        let args = vec![
            std::ffi::OsString::from("tailscale"),
            std::ffi::OsString::from("up"),
            std::ffi::OsString::from("--auth-key=tskey-auth-secret"),
            std::ffi::OsString::from("--login-server=https://example.com"),
        ];
        let sanitized = sanitize_display_args(&args);
        assert_eq!(
            sanitized,
            vec![
                "tailscale",
                "up",
                "--auth-key=[REDACTED]",
                "--login-server=https://example.com"
            ]
        );
    }
}
