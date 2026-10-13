use std::path::{Path, PathBuf};

use snafu::{OptionExt, ResultExt};
use tap::prelude::*;

use crate::error::{IoSnafu, Kind, NotFoundSnafu, Result};

async fn verify_fleet(dir: &Path) -> Result<PathBuf> {
    let flake_path = dir.join("flake.nix");
    flake_path
        .is_file()
        .then_some(&flake_path)
        .context(NotFoundSnafu {
            kind: Kind::Fleet,
            name: "flake.nix".to_string(),
            path: dir.to_path_buf(),
        })?;

    let abs_path = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    let expr = format!(
        "(builtins.getFlake (toString \"{}\")).rockyFleet or false",
        abs_path.display()
    );

    super::eval::eval(&expr)
        .await
        .map(|out| out.trim() == "true")
        .unwrap_or(false)
        .then_some(abs_path)
        .context(NotFoundSnafu {
            kind: Kind::Fleet,
            name: "rocky".to_string(),
            path: dir.to_path_buf(),
        })
}

pub async fn with_fleet<F, Fut, T>(target: Option<&Path>, f: F) -> Result<T>
where
    F: FnOnce(PathBuf) -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let target_path = target.map(|p| Ok(p.to_path_buf())).unwrap_or_else(|| {
        std::env::current_dir().context(IoSnafu {
            path: PathBuf::from("."),
        })
    })?;

    verify_fleet(&target_path)
        .await
        .tap_ok(|root| tracing::debug!(fleet_root = %root.display(), "verified fleet root"))?
        .pipe(f)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_verify_fleet_success() {
        let dir = tempdir().unwrap();
        let flake = r#"{
  description = "rocky fleet";
  outputs = inputs: {
    rockyFleet = true;
  };
}
"#;
        std::fs::write(dir.path().join("flake.nix"), flake).unwrap();

        let verified = verify_fleet(dir.path()).await;
        assert!(verified.is_ok());
        assert_eq!(verified.unwrap(), dir.path().canonicalize().unwrap());
    }

    #[tokio::test]
    async fn test_verify_fleet_missing_flake() {
        let dir = tempdir().unwrap();
        let res = verify_fleet(dir.path()).await;
        match res {
            Err(crate::error::RockyError::NotFound { kind, name, .. }) => {
                assert_eq!(kind, Kind::Fleet);
                assert_eq!(name, "flake.nix");
            }
            other => panic!("expected NotFound flake.nix, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_verify_fleet_not_a_rocky_fleet() {
        let dir = tempdir().unwrap();
        let foreign_flake = r#"{
  description = "some other rust project";
  outputs = inputs: {};
}
"#;
        std::fs::write(dir.path().join("flake.nix"), foreign_flake).unwrap();

        let res = verify_fleet(dir.path()).await;
        match res {
            Err(crate::error::RockyError::NotFound { kind, name, .. }) => {
                assert_eq!(kind, Kind::Fleet);
                assert_eq!(name, "rocky");
            }
            other => panic!("expected NotFound rocky, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_with_fleet_some_executes_closure_with_fleet_root() {
        let dir = tempdir().unwrap();
        let flake = r#"{
  outputs = _: {
    rockyFleet = true;
  };
}
"#;
        std::fs::write(dir.path().join("flake.nix"), flake).unwrap();

        let expected = dir.path().canonicalize().unwrap();
        let executed = with_fleet(Some(dir.path()), |root| async move {
            assert_eq!(root, expected);
            Ok(42)
        })
        .await;

        assert_eq!(executed.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_with_fleet_some_rejects_non_fleet() {
        let dir = tempdir().unwrap();
        let res = with_fleet(Some(dir.path()), |_| async { Ok(()) }).await;
        match res {
            Err(crate::error::RockyError::NotFound { kind, .. }) => {
                assert_eq!(kind, Kind::Fleet);
            }
            other => panic!("expected NotFound Fleet, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_with_fleet_none_checks_current_working_directory() {
        // Current directory is the rocky repo root, which is not a deployed fleet (lacks rockyFleet = true)
        let res = with_fleet(None, |_| async { Ok(()) }).await;
        match res {
            Err(crate::error::RockyError::NotFound { kind, name, .. }) => {
                assert_eq!(kind, Kind::Fleet);
                assert_eq!(name, "rocky");
            }
            other => panic!("expected NotFound rocky fleet, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_with_fleet_propagates_closure_error() {
        let dir = tempdir().unwrap();
        let flake = r#"{
  outputs = _: {
    rockyFleet = true;
  };
}
"#;
        std::fs::write(dir.path().join("flake.nix"), flake).unwrap();

        let res = with_fleet(Some(dir.path()), |root| async move {
            crate::error::NotFoundSnafu {
                kind: Kind::Host,
                name: "dummy-host".to_string(),
                path: root,
            }
            .fail::<()>()
        })
        .await;

        match res {
            Err(crate::error::RockyError::NotFound { kind, name, .. }) => {
                assert_eq!(kind, Kind::Host);
                assert_eq!(name, "dummy-host");
            }
            other => panic!("expected NotFound Host from closure, got {:?}", other),
        }
    }
}
