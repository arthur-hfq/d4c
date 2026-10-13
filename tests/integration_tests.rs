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

#[test]
fn test_integration_service_lifecycle_and_recreation() {
    let test_svc = "d4c_int_test_redis";
    // Ensure clean state before test
    let _ = Engine::wipe_service_storage(test_svc);

    // 1. Provision service with cpus limit (tests rootless cgroup fallback if unprivileged)
    let prov_res = Engine::provision_service(
        d4c::cli::ServiceCatalog::Redis,
        None,
        Some(test_svc.to_string()),
        None,
        Some("lifecycle_test".to_string()),
        Some("1".to_string()),
        None,
        None,
        None,
    );
    assert!(prov_res.is_ok(), "Provisioning service failed: {:?}", prov_res.err());

    // 2. Check service is listed and running
    let svcs = d4c::podman::list_services().expect("Failed to list services");
    let found = svcs.iter().find(|s| s.name == test_svc);
    assert!(found.is_some(), "Service was not found after provisioning");
    assert!(found.unwrap().is_running(), "Service should be in running state");

    // 3. Remove container (rm) - preserves volume on disk
    let rm_res = Engine::rm(test_svc);
    assert!(rm_res.is_ok(), "Engine::rm failed: {:?}", rm_res.err());

    // 4. Verify service is NO LONGER in active services list
    let svcs_after_rm = d4c::podman::list_services().expect("Failed to list services");
    assert!(
        !svcs_after_rm.iter().any(|s| s.name == test_svc),
        "Service should not be present in list_services after Engine::rm"
    );

    // 5. Recreate service with the EXACT same name
    let recreate_res = Engine::provision_service(
        d4c::cli::ServiceCatalog::Redis,
        None,
        Some(test_svc.to_string()),
        None,
        Some("recreated_tag".to_string()),
        None,
        None,
        None,
        None,
    );
    assert!(recreate_res.is_ok(), "Recreation failed: {:?}", recreate_res.err());

    // 6. Verify recreated service is up and running
    let svcs_recreated = d4c::podman::list_services().expect("Failed to list services");
    let recreated_found = svcs_recreated.iter().find(|s| s.name == test_svc);
    assert!(recreated_found.is_some(), "Recreated service not found in list");
    assert!(recreated_found.unwrap().is_running(), "Recreated service should be running");

    // 7. Cleanup
    let wipe_res = Engine::wipe_service_storage(test_svc);
    assert!(wipe_res.is_ok(), "Wipe cleanup failed: {:?}", wipe_res.err());
}
