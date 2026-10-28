pub mod create;
pub mod edit;
pub mod format;
pub mod mock;
pub mod show;

use std::path::Path;

use crate::error::{Kind, NixParseSnafu, Result};
use crate::process;
use crate::utils::validate::validate_name;

pub use create::create_secret;
pub use edit::edit_secret;
pub use format::{
    detect_secret_format, extract_key, has_key_value_entries, parse_key_values, validate_key_value,
    SecretFormat,
};
pub use mock::generate_mock_secrets;
pub use show::show_secret;

pub(crate) async fn eval_secret_recipients(
    secrets_dir: &Path,
    file_name: &str,
) -> Result<Vec<String>> {
    let clean_name = file_name.strip_suffix(".age").unwrap_or(file_name);
    validate_name(Kind::Secret, clean_name)?;

    let expr = "{ secretFile }: (import ./secrets.nix).${secretFile}.publicKeys";
    let output = process::exec(
        "nix-instantiate",
        [
            "--eval",
            "--json",
            "--strict",
            "--argstr",
            "secretFile",
            file_name,
            "--expr",
            expr,
        ],
        Some(secrets_dir),
    )
    .await?;

    serde_json::from_str::<Vec<String>>(output.trim()).map_err(|e| {
        NixParseSnafu {
            path: secrets_dir.join("secrets.nix"),
            reason: format!("Failed to parse recipient public keys from Nix: {e}"),
        }
        .build()
    })
}

pub(crate) async fn eval_all_secret_recipients(
    secrets_dir: &Path,
) -> Result<std::collections::HashMap<String, usize>> {
    let secrets_nix = secrets_dir.join("secrets.nix");
    if !secrets_nix.is_file() {
        return Ok(std::collections::HashMap::new());
    }

    let expr = "let s = import ./secrets.nix; in builtins.mapAttrs (_: val: builtins.length (val.publicKeys or [])) s";
    let output = process::exec(
        "nix-instantiate",
        [
            "--eval",
            "--json",
            "--strict",
            "--expr",
            expr,
        ],
        Some(secrets_dir),
    )
    .await?;

    serde_json::from_str::<std::collections::HashMap<String, usize>>(output.trim()).map_err(|e| {
        NixParseSnafu {
            path: secrets_nix,
            reason: format!("Failed to parse recipient counts from Nix: {e}"),
        }
        .build()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_eval_secret_recipients_rejects_injection_in_file_name() {
        let dir = tempfile::tempdir().unwrap();
        let secrets_dir = dir.path();
        let res = eval_secret_recipients(
            secrets_dir,
            r#"test".publicKeys or (builtins.abort "pwn") #.age"#,
        )
        .await;
        assert!(matches!(
            res,
            Err(crate::error::RockyError::InvalidName {
                kind: Kind::Secret,
                ..
            })
        ));
    }
}
