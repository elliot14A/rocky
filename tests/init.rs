mod common;

use common::TestWorkspace;
use rocky::RockyError;

#[tokio::test]
async fn scaffold_fleet_base_template_default_directory() {
    let ws = TestWorkspace::new();
    let res = ws.init_default().await;

    assert!(res.is_ok());
    assert!(ws.file_exists("rocky/flake.nix"));
    assert!(ws.file_exists("rocky/keys.nix"));
    assert!(ws.file_exists("rocky/lib/default.nix"));
    assert!(ws.file_exists("rocky/system/baseline.nix"));
    assert!(ws.file_exists("rocky/services/caddy.nix"));
}

#[tokio::test]
async fn scaffold_fleet_custom_path() {
    let ws = TestWorkspace::new();
    let res = ws.init_path("custom", "base", false).await;

    assert!(res.is_ok());
    assert!(ws.file_exists("custom/flake.nix"));
    assert!(ws.file_exists("custom/keys.nix"));
    assert!(ws.file_exists("custom/lib/default.nix"));
    assert!(ws.file_exists("custom/system/baseline.nix"));
    assert!(ws.file_exists("custom/services/caddy.nix"));
}

#[tokio::test]
async fn reject_non_empty_target_without_force() {
    let ws = TestWorkspace::new();
    let rocky_dir = ws.path.join("rocky");
    std::fs::create_dir_all(&rocky_dir).unwrap();
    let sample = rocky_dir.join("file.txt");
    std::fs::write(&sample, "data").unwrap();

    let res = ws.init("base", false).await;
    match res {
        Err(RockyError::TargetNotEmpty { path }) => {
            assert_eq!(path, rocky_dir);
        }
        other => panic!("expected RockyError::TargetNotEmpty, got {:?}", other),
    }
}

#[tokio::test]
async fn allow_non_empty_target_with_force() {
    let ws = TestWorkspace::new();
    let rocky_dir = ws.path.join("rocky");
    std::fs::create_dir_all(&rocky_dir).unwrap();
    let sample = rocky_dir.join("file.txt");
    std::fs::write(&sample, "data").unwrap();

    let res = ws.init("base", true).await;
    assert!(res.is_ok());
    assert!(ws.file_exists("rocky/flake.nix"));
}

#[tokio::test]
async fn reject_unknown_template() {
    let ws = TestWorkspace::new();
    let res = ws.init("nonexistent-template", false).await;

    assert!(res.is_err());
}
