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

#[test]
fn test_nix_tailscale_configuration() {
    eval_nix_test("tests/nix/tailscale.nix");
}

#[test]
fn test_nix_service_replication() {
    eval_nix_test("tests/nix/service.nix");
}

#[test]
fn test_nix_nftables_load_balancer() {
    eval_nix_test("tests/nix/nftables.nix");
}

#[test]
fn test_nix_mesh_syntax_evaluates() {
    let full_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/nix/mesh.nix");
    let output = Command::new("nix-instantiate")
        .args(["--eval", "-E"])
        .arg(format!("(import {}).name", full_path.display()))
        .output()
        .expect("failed to execute nix-instantiate");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "mesh.nix evaluation failed:\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stdout.contains("vm-test-run-rocky-mesh-test"),
        "expected vm-test-run derivation, got: {stdout}"
    );
}

#[test]
#[ignore]
fn test_nix_mesh_vm() {
    let full_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/nix/mesh.nix");
    let output = Command::new("nix-build")
        .args(["--no-out-link"])
        .arg(&full_path)
        .output()
        .expect("failed to execute nix-build");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "mesh vm test failed:\nstdout: {stdout}\nstderr: {stderr}"
    );
}
