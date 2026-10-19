use std::path::Path;
use std::process::Command;

fn eval_nix_test(relative_path: &str) {
    let full_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    let output = Command::new("nix-instantiate")
        .args(["--eval", "--strict"])
        .arg(&full_path)
        .output()
        .expect("failed to execute nix-instantiate");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "{relative_path} failed:\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stdout.contains("passed = true;"),
        "expected 'passed = true;', got: {stdout}"
    );
}

#[test]
fn test_nix_secrets_scoping() {
    eval_nix_test("tests/nix/secrets.nix");
}

#[test]
fn test_nix_upstreams_and_addressing() {
    eval_nix_test("tests/nix/upstreams.nix");
}
