use d4c::engine::Engine;
use std::fs;

#[test]
fn test_integration_cli_help() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_d4c"))
        .arg("--help")
        .output()
        .expect("Failed to execute d4c binary");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Minimalist"));
    assert!(stdout.contains("snap"));
    assert!(stdout.contains("net"));
    assert!(stdout.contains("compose"));
}

#[test]
fn test_integration_snap_subcommands_help() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_d4c"))
        .args(["snap", "--help"])
        .output()
        .expect("Failed to execute d4c snap --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("create"));
    assert!(stdout.contains("branch"));
    assert!(stdout.contains("restore"));
    assert!(stdout.contains("merge"));
    assert!(stdout.contains("export"));
    assert!(stdout.contains("list"));
}

#[test]
fn test_integration_net_subcommands_help() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_d4c"))
        .args(["net", "--help"])
        .output()
        .expect("Failed to execute d4c net --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("create"));
    assert!(stdout.contains("list"));
    assert!(stdout.contains("rm"));
    assert!(stdout.contains("attach"));
    assert!(stdout.contains("detach"));
    assert!(stdout.contains("panorama"));
}

#[test]
fn test_integration_compose_export() {
    let out_dir = std::env::temp_dir().join("d4c_compose_test");
    fs::create_dir_all(&out_dir).expect("Failed to create test directory");
    let compose_file = out_dir.join("docker-compose.yml");

    let result = Engine::compose(compose_file.clone(), "host");
    // Result is Ok whether there are services or not
    assert!(result.is_ok());

    let _ = fs::remove_dir_all(&out_dir);
}

#[test]
fn test_integration_search_empty() {
    let result = Engine::search("non_existent_service_query_123");
    assert!(result.is_ok());
}
