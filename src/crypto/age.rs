use std::path::{Path, PathBuf};
use std::process::Stdio;

use snafu::{ensure, ResultExt};
use tap::prelude::*;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::error::{
    CommandExecutionSnafu, CommandSignaledSnafu, InvalidNameSnafu, Kind, MissingIdentitySnafu,
    NotFoundSnafu, ProcessSpawnSnafu, Result,
};

fn is_valid_recipient(key: &str) -> bool {
    let trimmed = key.trim();
    if trimmed.starts_with("age1") {
        return trimmed.len() >= 58 && trimmed.chars().all(|c| c.is_ascii_alphanumeric());
    }
    if let Some((key_type, rest)) = trimmed.split_once(char::is_whitespace) {
        if matches!(
            key_type,
            "ssh-ed25519"
                | "ssh-rsa"
                | "ecdsa-sha2-nistp256"
                | "ecdsa-sha2-nistp384"
                | "ecdsa-sha2-nistp521"
                | "sk-ssh-ed25519@openssh.com"
                | "sk-ecdsa-sha2-nistp256@openssh.com"
        ) {
            let b64 = rest.split_whitespace().next().unwrap_or("");
            return !b64.is_empty()
                && b64
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=');
        }
    }
    false
}

pub async fn encrypt(plaintext: &[u8], recipients: &[String]) -> Result<Vec<u8>> {
    ensure!(
        !recipients.is_empty(),
        NotFoundSnafu {
            kind: Kind::Secret,
            name: "recipient public keys".to_string(),
            path: PathBuf::from("secrets.nix"),
        }
    );

    let mut args = Vec::new();
    for recipient in recipients {
        let trimmed = recipient.trim();
        if trimmed.is_empty() {
            continue;
        }
        ensure!(
            is_valid_recipient(trimmed),
            InvalidNameSnafu {
                kind: Kind::Secret,
                name: format!("recipient key '{trimmed}'"),
            }
        );
        args.push("-r".to_string());
        args.push(trimmed.to_string());
    }

    ensure!(
        !args.is_empty(),
        NotFoundSnafu {
            kind: Kind::Secret,
            name: "valid recipient keys".to_string(),
            path: PathBuf::from("secrets.nix"),
        }
    );

    let mut child = Command::new("age")
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .context(ProcessSpawnSnafu { command: "age" })
        .tap_err(|err| tracing::error!(%err, "failed to spawn age encryption"))?;

    if let Some(mut stdin) = child.stdin.take() {
        let data = plaintext.to_vec();
        tokio::spawn(async move {
            let _ = stdin.write_all(&data).await;
        });
    }

    let output = child
        .wait_with_output()
        .await
        .context(ProcessSpawnSnafu { command: "age" })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let display_cmd = format!("age {}", args.join(" "));
        return match output.status.code() {
            Some(status) => CommandExecutionSnafu {
                command: display_cmd,
                status,
                stderr,
                log_path: None,
            }
            .fail(),
            None => CommandSignaledSnafu {
                command: display_cmd,
            }
            .fail(),
        };
    }

    output.stdout.pipe(Ok)
}

pub async fn decrypt(ciphertext: &[u8], identity_path: Option<&Path>) -> Result<Vec<u8>> {
    let Some(path) = crate::fs::canonicalize_identity(identity_path)? else {
        return MissingIdentitySnafu.fail();
    };

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = path.metadata() {
            let mode = meta.permissions().mode();
            if mode & 0o077 != 0 {
                crate::ui::warn(format!(
                    "Identity file '{}' permissions are too open (0{:o}); expected 0600",
                    path.display(),
                    mode & 0o777
                ));
            }
        }
    }

    let args = vec![
        "-d".to_string(),
        "-i".to_string(),
        path.display().to_string(),
    ];

    let mut child = Command::new("age")
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .context(ProcessSpawnSnafu { command: "age" })
        .tap_err(|err| tracing::error!(%err, "failed to spawn age decryption"))?;

    if let Some(mut stdin) = child.stdin.take() {
        let data = ciphertext.to_vec();
        tokio::spawn(async move {
            let _ = stdin.write_all(&data).await;
        });
    }

    let output = child
        .wait_with_output()
        .await
        .context(ProcessSpawnSnafu { command: "age" })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let display_cmd = format!("age {}", args.join(" "));
        return match output.status.code() {
            Some(status) => CommandExecutionSnafu {
                command: display_cmd,
                status,
                stderr,
                log_path: None,
            }
            .fail(),
            None => CommandSignaledSnafu {
                command: display_cmd,
            }
            .fail(),
        };
    }

    output.stdout.pipe(Ok)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::RockyError;

    #[tokio::test]
    async fn test_encrypt_and_decrypt_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let id_file = dir.path().join("id_age");
        let output = std::process::Command::new("age-keygen").output().unwrap();
        assert!(output.status.success());
        std::fs::write(&id_file, &output.stdout).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&id_file, std::fs::Permissions::from_mode(0o600)).unwrap();
        }

        let pub_output = std::process::Command::new("age-keygen")
            .arg("-y")
            .arg(&id_file)
            .output()
            .unwrap();
        assert!(pub_output.status.success());
        let pubkey = String::from_utf8(pub_output.stdout)
            .unwrap()
            .trim()
            .to_string();

        let plaintext = b"Database password = supersecret123!";
        let ciphertext = encrypt(plaintext, &[pubkey]).await.unwrap();
        assert!(!ciphertext.is_empty());
        assert_ne!(ciphertext, plaintext);

        let decrypted = decrypt(&ciphertext, Some(&id_file)).await.unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[tokio::test]
    async fn test_decrypt_rejects_insecure_permissions() {
        let dir = tempfile::tempdir().unwrap();
        let id_file = dir.path().join("id_age_insecure");
        let output = std::process::Command::new("age-keygen").output().unwrap();
        assert!(output.status.success());
        std::fs::write(&id_file, &output.stdout).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&id_file, std::fs::Permissions::from_mode(0o644)).unwrap();
            let res = decrypt(b"dummy", Some(&id_file)).await;
            assert!(res.is_err());
            match res.unwrap_err() {
                RockyError::InsecurePath { path, .. } => {
                    assert_eq!(path, id_file);
                }
                other => panic!("expected InsecurePath, got: {:?}", other),
            }
        }
    }

    #[tokio::test]
    async fn test_encrypt_rejects_empty_recipients() {
        let plaintext = b"secret";
        let res = encrypt(plaintext, &[]).await;
        assert!(matches!(res, Err(RockyError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_encrypt_with_ssh_key_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let key_path = dir.path().join("id_ed25519");
        let status = std::process::Command::new("ssh-keygen")
            .args(["-t", "ed25519", "-N", "", "-f", key_path.to_str().unwrap()])
            .output()
            .unwrap();
        assert!(status.status.success());

        let pubkey = std::fs::read_to_string(dir.path().join("id_ed25519.pub")).unwrap();
        let plaintext = b"SSH encrypted payload 42";

        let ciphertext = encrypt(plaintext, &[pubkey.trim().to_string()])
            .await
            .unwrap();
        let decrypted = decrypt(&ciphertext, Some(&key_path)).await.unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[tokio::test]
    async fn test_decrypt_invalid_ciphertext_fails() {
        let dir = tempfile::tempdir().unwrap();
        let key_path = dir.path().join("id_ed25519");
        let status = std::process::Command::new("ssh-keygen")
            .args(["-t", "ed25519", "-N", "", "-f", key_path.to_str().unwrap()])
            .output()
            .unwrap();
        assert!(status.status.success());

        let invalid = b"this is not age ciphertext";
        let res = decrypt(invalid, Some(&key_path)).await;
        assert!(matches!(res, Err(RockyError::CommandExecution { .. })));
    }

    #[tokio::test]
    async fn test_decrypt_without_identity_resolves_or_fails_typed() {
        let invalid = b"this is not age ciphertext";
        let res = decrypt(invalid, None).await;
        assert!(matches!(res, Err(RockyError::MissingIdentity)));
    }
}
