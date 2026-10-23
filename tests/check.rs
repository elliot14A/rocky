mod common;

use std::fs;

use common::{setup_fleet, TestWorkspace};
use rocky::commands::check::{self, CheckArgs};
use rocky::crypto::generate_mock_secrets;

#[tokio::test]
async fn test_check_mock_secrets_creates_throwaway_ciphertexts() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);

    let env_dir = fleet_path.join("environments").join("staging");
    fs::create_dir_all(&env_dir).unwrap();

    let secrets_nix = r#"{
  "jwt.age".publicKeys = [ ];
  "database.age".publicKeys = [ ];
}
"#;
    fs::write(env_dir.join("secrets.nix"), secrets_nix).unwrap();

    let jwt_file = env_dir.join("jwt.age");
    let db_file = env_dir.join("database.age");
    assert!(!jwt_file.exists());
    assert!(!db_file.exists());

    let mocked = generate_mock_secrets(&fleet_path).await.unwrap();
    assert_eq!(mocked.len(), 2);
    assert!(jwt_file.is_file());
    assert!(db_file.is_file());

    let jwt_content = fs::read_to_string(&jwt_file).unwrap();
    assert!(jwt_content.contains("age-encryption.org/v1"));
}

#[tokio::test]
async fn test_check_mock_secrets_idempotent_when_files_exist() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);

    let env_dir = fleet_path.join("environments").join("prod");
    fs::create_dir_all(&env_dir).unwrap();

    let secrets_nix = r#"{
  "existing.age".publicKeys = [ ];
}
"#;
    fs::write(env_dir.join("secrets.nix"), secrets_nix).unwrap();
    fs::write(env_dir.join("existing.age"), "existing-ciphertext").unwrap();

    let mocked = generate_mock_secrets(&fleet_path).await.unwrap();
    assert!(mocked.is_empty());

    let content = fs::read_to_string(env_dir.join("existing.age")).unwrap();
    assert_eq!(content, "existing-ciphertext");
}

#[tokio::test]
async fn test_check_command_adapter_execution() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);

    let env_dir = fleet_path.join("environments").join("dev");
    fs::create_dir_all(&env_dir).unwrap();

    let secrets_nix = r#"{
  "dev-token.age".publicKeys = [ ];
}
"#;
    fs::write(env_dir.join("secrets.nix"), secrets_nix).unwrap();

    let args = CheckArgs { mock_secrets: true };
    // Running check on test fleet
    let dev_token = env_dir.join("dev-token.age");
    assert!(!dev_token.exists());

    let res = check::run(args, Some(&fleet_path)).await;
    assert!(res.is_ok(), "check::run failed: {:?}", res);
    // Mock secret was automatically cleaned up by RAII guard
    assert!(!dev_token.exists());
}
