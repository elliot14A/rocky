mod common;

use std::fs;

use common::{setup_fleet, TestWorkspace};
use rocky::commands::secret::new::{self, NewArgs};
use rocky::commands::secret::rekey::{self, RekeyArgs};
use rocky::commands::secret::show::{self, ShowArgs};
use rocky::error::{Kind, RockyError};

fn setup_fleet_with_secrets_env(
    ws: &TestWorkspace,
) -> (std::path::PathBuf, std::path::PathBuf, String) {
    let fleet_path = setup_fleet(ws);
    let env_dir = fleet_path.join("environments").join("prod");
    fs::create_dir_all(&env_dir).expect("failed to create environment directory");

    let key_path = ws.path.join("id_ed25519");
    let status = std::process::Command::new("ssh-keygen")
        .args(["-t", "ed25519", "-N", "", "-f", key_path.to_str().unwrap()])
        .output()
        .expect("failed to generate test ssh key");
    assert!(status.status.success());

    let pubkey = fs::read_to_string(ws.path.join("id_ed25519.pub"))
        .expect("failed to read test ssh pubkey")
        .trim()
        .to_string();

    let secrets_nix_content = format!(
        r#"let
  operators = [ "{pubkey}" ];
  recipientsFor = services: operators;
  allHosts = operators;
in
{{
}}
"#
    );
    fs::write(env_dir.join("secrets.nix"), secrets_nix_content)
        .expect("failed to write secrets.nix");

    (fleet_path, key_path, pubkey)
}

#[tokio::test]
async fn test_secret_new_creates_age_file_and_scopes_recipients() {
    let ws = TestWorkspace::new();
    let (fleet_path, _key_path, _pubkey) = setup_fleet_with_secrets_env(&ws);

    let args = NewArgs {
        name: "jwt".to_string(),
        env: Some("prod".to_string()),
        service: vec![],
        all_hosts: true,
        from_file: None,
        value: Some("JWT_SECRET=supersecret123\n".to_string()),
    };

    let res = new::run(args, Some(&fleet_path)).await;
    assert!(res.is_ok(), "secret::new failed: {:?}", res);

    let env_dir = fleet_path.join("environments").join("prod");
    let secret_path = env_dir.join("jwt.age");
    assert!(secret_path.is_file(), "jwt.age was not created");

    let secrets_nix = fs::read_to_string(env_dir.join("secrets.nix")).unwrap();
    assert!(
        secrets_nix.contains(r#""jwt.age".publicKeys = allHosts;"#),
        "secrets.nix missing declared entry: {secrets_nix}"
    );
}

#[tokio::test]
async fn test_secret_show_displays_key_value_and_filtered_key() {
    let ws = TestWorkspace::new();
    let (fleet_path, key_path, _pubkey) = setup_fleet_with_secrets_env(&ws);

    let new_args = NewArgs {
        name: "db-creds".to_string(),
        env: None, // Test auto-selecting single environment
        service: vec![],
        all_hosts: true,
        from_file: None,
        value: Some("DB_USER=postgres\nDB_PASS=mypassword\n".to_string()),
    };
    new::run(new_args, Some(&fleet_path)).await.unwrap();

    let show_args = ShowArgs {
        name: "db-creds".to_string(),
        env: None,
        key: None,
        identity: Some(key_path.clone()),
    };
    let res = show::run(show_args, Some(&fleet_path)).await;
    assert!(res.is_ok(), "secret::show failed: {:?}", res);

    let env_dir = fleet_path.join("environments").join("prod");
    let full_content = rocky::crypto::show_secret(&env_dir, "db-creds", Some(&key_path), None)
        .await
        .unwrap();
    assert_eq!(full_content, "DB_USER=postgres\nDB_PASS=mypassword\n");

    let show_key_args = ShowArgs {
        name: "db-creds.age".to_string(),
        env: Some("prod".to_string()),
        key: Some("DB_PASS".to_string()),
        identity: Some(key_path.clone()),
    };
    let res_key = show::run(show_key_args, Some(&fleet_path)).await;
    assert!(
        res_key.is_ok(),
        "secret::show key filter failed: {:?}",
        res_key
    );

    let pass_only =
        rocky::crypto::show_secret(&env_dir, "db-creds", Some(&key_path), Some("DB_PASS"))
            .await
            .unwrap();
    assert_eq!(pass_only, "mypassword");

    let user_only =
        rocky::crypto::show_secret(&env_dir, "db-creds", Some(&key_path), Some("DB_USER"))
            .await
            .unwrap();
    assert_eq!(user_only, "postgres");
}

#[tokio::test]
async fn test_secret_new_from_file() {
    let ws = TestWorkspace::new();
    let (fleet_path, key_path, _pubkey) = setup_fleet_with_secrets_env(&ws);

    let raw_file = ws.path.join("secret.env");
    fs::write(&raw_file, "API_KEY=xyz987\n").unwrap();

    let args = NewArgs {
        name: "api".to_string(),
        env: Some("prod".to_string()),
        service: vec![],
        all_hosts: true,
        from_file: Some(raw_file),
        value: None,
    };
    new::run(args, Some(&fleet_path)).await.unwrap();

    let env_dir = fleet_path.join("environments").join("prod");
    let secret_path = env_dir.join("api.age");
    assert!(secret_path.is_file());

    let show_args = ShowArgs {
        name: "api".to_string(),
        env: Some("prod".to_string()),
        key: Some("API_KEY".to_string()),
        identity: Some(key_path.clone()),
    };
    assert!(show::run(show_args, Some(&fleet_path)).await.is_ok());

    let full = rocky::crypto::show_secret(&env_dir, "api", Some(&key_path), None)
        .await
        .unwrap();
    assert_eq!(full, "API_KEY=xyz987\n");

    let filtered = rocky::crypto::show_secret(&env_dir, "api", Some(&key_path), Some("API_KEY"))
        .await
        .unwrap();
    assert_eq!(filtered, "xyz987");
}

#[tokio::test]
async fn test_secret_new_rejects_duplicate() {
    let ws = TestWorkspace::new();
    let (fleet_path, _key_path, _pubkey) = setup_fleet_with_secrets_env(&ws);

    let args1 = NewArgs {
        name: "token".to_string(),
        env: Some("prod".to_string()),
        service: vec![],
        all_hosts: true,
        from_file: None,
        value: Some("KEY=1\n".to_string()),
    };
    new::run(args1, Some(&fleet_path)).await.unwrap();

    let args2 = NewArgs {
        name: "token".to_string(),
        env: Some("prod".to_string()),
        service: vec![],
        all_hosts: true,
        from_file: None,
        value: Some("KEY=2\n".to_string()),
    };
    let res = new::run(args2, Some(&fleet_path)).await;
    assert!(matches!(
        res,
        Err(RockyError::AlreadyExists {
            kind: Kind::Secret,
            ..
        })
    ));
}

#[tokio::test]
async fn test_secret_new_rejects_nonexistent_service() {
    let ws = TestWorkspace::new();
    let (fleet_path, _key_path, _pubkey) = setup_fleet_with_secrets_env(&ws);

    // Create services dir with only 'caddy.nix'
    let services_dir = fleet_path.join("services");
    fs::create_dir_all(&services_dir).unwrap();
    fs::write(services_dir.join("caddy.nix"), "# caddy\n").unwrap();

    let args = NewArgs {
        name: "postgres-secret".to_string(),
        env: Some("prod".to_string()),
        service: vec!["postgres".to_string()],
        all_hosts: false,
        from_file: None,
        value: Some("POSTGRES_PASSWORD=secret\n".to_string()),
    };

    let res = new::run(args, Some(&fleet_path)).await;
    assert!(matches!(
        res,
        Err(RockyError::NotFound {
            kind: Kind::Service,
            ..
        })
    ));
}

#[tokio::test]
async fn test_secret_new_with_valid_service_scopes_recipient() {
    let ws = TestWorkspace::new();
    let (fleet_path, _key_path, _pubkey) = setup_fleet_with_secrets_env(&ws);

    let services_dir = fleet_path.join("services");
    fs::create_dir_all(&services_dir).unwrap();
    fs::write(services_dir.join("postgrest.nix"), "# postgrest\n").unwrap();

    let args = NewArgs {
        name: "pgrst".to_string(),
        env: Some("prod".to_string()),
        service: vec!["postgrest".to_string()],
        all_hosts: false,
        from_file: None,
        value: Some("PGRST_JWT=myjwt\n".to_string()),
    };

    let res = new::run(args, Some(&fleet_path)).await;
    assert!(res.is_ok(), "secret::new with service failed: {:?}", res);

    let env_dir = fleet_path.join("environments").join("prod");
    let secrets_nix = fs::read_to_string(env_dir.join("secrets.nix")).unwrap();
    assert!(secrets_nix.contains(r#""pgrst.age".publicKeys = recipientsFor [ "postgrest" ];"#));
}

#[tokio::test]
async fn test_secret_rekey_executes_successfully() {
    let ws = TestWorkspace::new();
    let (fleet_path, key_path, _pubkey) = setup_fleet_with_secrets_env(&ws);

    let new_args = NewArgs {
        name: "rekey-test".to_string(),
        env: Some("prod".to_string()),
        service: vec![],
        all_hosts: true,
        from_file: None,
        value: Some("FOO=bar\n".to_string()),
    };
    new::run(new_args, Some(&fleet_path)).await.unwrap();

    let rekey_args = RekeyArgs {
        env: Some("prod".to_string()),
        identity: Some(key_path),
    };
    let res = rekey::run(rekey_args, Some(&fleet_path)).await;
    match res {
        Ok(()) => {}
        Err(RockyError::ProcessSpawn { ref command, .. }) if command == "agenix" => {
            // agenix binary is provided via nix flake devShell, not ambient host PATH
        }
        Err(err) => panic!("secret::rekey failed with unexpected error: {err}"),
    }
}

#[tokio::test]
async fn test_secret_rekey_with_nested_secrets_dir() {
    let ws = TestWorkspace::new();
    let fleet_path = setup_fleet(&ws);
    let env_dir = fleet_path.join("environments").join("prod");
    let secrets_dir = env_dir.join("secrets");
    fs::create_dir_all(&secrets_dir).expect("failed to create secrets dir");

    let key_path = ws.path.join("id_ed25519");
    let status = std::process::Command::new("ssh-keygen")
        .args(["-t", "ed25519", "-N", "", "-f", key_path.to_str().unwrap()])
        .output()
        .expect("failed to generate test ssh key");
    assert!(status.status.success());

    let pubkey = fs::read_to_string(ws.path.join("id_ed25519.pub"))
        .expect("failed to read test ssh pubkey")
        .trim()
        .to_string();

    let secrets_nix_content = format!(
        r#"let
  operators = [ "{pubkey}" ];
  recipientsFor = services: operators;
  allHosts = operators;
in
{{
}}
"#
    );
    fs::write(secrets_dir.join("secrets.nix"), secrets_nix_content)
        .expect("failed to write secrets.nix");

    let new_args = NewArgs {
        name: "nested-rekey".to_string(),
        env: Some("prod".to_string()),
        service: vec![],
        all_hosts: true,
        from_file: None,
        value: Some("NESTED=val\n".to_string()),
    };
    new::run(new_args, Some(&fleet_path)).await.unwrap();

    let rekey_args = RekeyArgs {
        env: Some("prod".to_string()),
        identity: Some(key_path),
    };
    let res = rekey::run(rekey_args, Some(&fleet_path)).await;
    match res {
        Ok(()) => {}
        Err(RockyError::ProcessSpawn { ref command, .. }) if command == "agenix" => {}
        Err(err) => panic!("secret::rekey with nested secrets dir failed: {err}"),
    }
}
