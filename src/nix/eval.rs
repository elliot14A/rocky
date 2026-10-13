use std::path::Path;

use snafu::OptionExt;
use tap::prelude::*;

use crate::error::{Kind, NotFoundSnafu, Result};
use crate::process;

pub async fn eval(expr: &str) -> Result<String> {
    process::exec(
        "nix",
        ["eval", "--offline", "--json", "--impure", "--expr", expr],
        None,
    )
    .await
}

pub async fn eval_default_provider(fleet_path: &Path) -> Result<String> {
    let abs_path = std::fs::canonicalize(fleet_path).unwrap_or_else(|_| fleet_path.to_path_buf());
    let expr = format!(
        "(builtins.getFlake (toString \"{}\")).config.defaultProvider",
        abs_path.display()
    );

    eval(&expr)
        .await
        .map_err(|_| {
            NotFoundSnafu {
                kind: Kind::Provider,
                name: "defaultProvider".to_string(),
                path: fleet_path.join("flake.nix"),
            }
            .build()
        })?
        .pipe(|raw| raw.trim().trim_matches('"').trim().to_string())
        .pipe(|provider| {
            (!provider.is_empty())
                .then_some(provider)
                .context(NotFoundSnafu {
                    kind: Kind::Provider,
                    name: "defaultProvider".to_string(),
                    path: fleet_path.join("flake.nix"),
                })
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_eval_expression() {
        let res = eval("1 + 1").await;
        assert_eq!(res.unwrap().trim(), "2");
    }

    #[tokio::test]
    async fn test_eval_complex_expression() {
        let res = eval(r#"{ a = 1; b = "rocky"; }"#).await;
        assert!(res.is_ok());
        let json_str = res.unwrap();
        assert!(json_str.contains("\"a\":1"));
        assert!(json_str.contains("\"b\":\"rocky\""));
    }

    #[tokio::test]
    async fn test_eval_invalid_expression() {
        let res = eval("throw \"deliberate evaluation failure\"").await;
        match res {
            Err(crate::error::RockyError::CommandExecution { command, .. }) => {
                assert!(command.starts_with("nix"));
            }
            other => panic!("expected CommandExecution error, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_eval_default_provider_success() {
        let dir = tempdir().unwrap();
        let flake_content = r#"{
  description = "test";
  outputs = inputs: {
    config = {
      defaultProvider = "hetzner";
    };
  };
}
"#;
        std::fs::write(dir.path().join("flake.nix"), flake_content).unwrap();

        let res = eval_default_provider(dir.path()).await;
        assert_eq!(res.unwrap(), "hetzner");
    }

    #[tokio::test]
    async fn test_eval_default_provider_missing() {
        let dir = tempdir().unwrap();
        let flake_content = r#"{
  description = "test";
  outputs = inputs: {};
}
"#;
        std::fs::write(dir.path().join("flake.nix"), flake_content).unwrap();

        let res = eval_default_provider(dir.path()).await;
        match res {
            Err(crate::error::RockyError::NotFound { kind, name, .. }) => {
                assert_eq!(kind, Kind::Provider);
                assert_eq!(name, "defaultProvider");
            }
            other => panic!("expected NotFound Provider, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_eval_default_provider_empty_string() {
        let dir = tempdir().unwrap();
        let flake_content = r#"{
  description = "test";
  outputs = inputs: {
    config = {
      defaultProvider = "";
    };
  };
}
"#;
        std::fs::write(dir.path().join("flake.nix"), flake_content).unwrap();

        let res = eval_default_provider(dir.path()).await;
        match res {
            Err(crate::error::RockyError::NotFound { kind, name, .. }) => {
                assert_eq!(kind, Kind::Provider);
                assert_eq!(name, "defaultProvider");
            }
            other => panic!("expected NotFound Provider, got {:?}", other),
        }
    }
}
