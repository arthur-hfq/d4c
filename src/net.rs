use crate::model::{NetworkInfo, StateStore};
use anyhow::{Context, Result, bail};
use console::{Color, Style};
use serde_json::Value;
use std::net::TcpListener;
use std::process::{Command, Stdio};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command as TokioCommand;

/// Checks if a TCP port is currently available
pub fn is_port_free(port: u16) -> bool {
    if TcpListener::bind(("0.0.0.0", port)).is_err() {
        return false;
    }
    TcpListener::bind(("127.0.0.1", port)).is_ok()
}

/// Automatically finds an available host TCP port starting from the default catalog port
pub fn find_free_port(preferred: Option<u16>, default_catalog_port: u16) -> Result<u16> {
    if let Some(port) = preferred {
        if is_port_free(port) {
            return Ok(port);
        }
        bail!("Requested host port {} is already in use by another process or container", port);
    }

    if is_port_free(default_catalog_port) {
        return Ok(default_catalog_port);
    }

    // Scan upward from default port
    let max_port = 65535;
    let start_port = if default_catalog_port < 1024 { 8000 } else { default_catalog_port + 1 };

    for port in start_port..=max_port {
        if is_port_free(port) {
            return Ok(port);
        }
    }

    bail!("Could not find any free TCP port in range {}..{}", start_port, max_port);
}

/// Formats network name with standard d4c prefix
pub fn normalize_network_name(name: &str) -> String {
    if name.starts_with("d4c_net_") {
        name.to_string()
    } else {
        format!("d4c_net_{}", name)
    }
}

/// Ensures rootless Podman network configuration is compatible with netavark
pub fn ensure_rootless_network_config() {
    if let Ok(home) = std::env::var("HOME") {
        let conf_dir = std::path::PathBuf::from(home).join(".config").join("containers");
        let conf_file = conf_dir.join("containers.conf");
        if !conf_file.exists() {
            let _ = std::fs::create_dir_all(&conf_dir);
            let _ = std::fs::write(&conf_file, "[network]\nfirewall_driver = \"none\"\n");
        } else if let Ok(content) = std::fs::read_to_string(&conf_file)
            && !content.contains("firewall_driver")
        {
            let mut updated = content;
            if !updated.contains("[network]") {
                updated.push_str("\n[network]\nfirewall_driver = \"none\"\n");
            } else {
                updated = updated.replace("[network]", "[network]\nfirewall_driver = \"none\"");
            }
            let _ = std::fs::write(&conf_file, updated);
        }
    }
}

/// Creates a new isolated bridge network in Podman with d4c metadata labels
pub fn create_bridge_network(name: &str, tag: Option<&str>, subnet: Option<&str>) -> Result<String> {
    ensure_rootless_network_config();
    let net_name = normalize_network_name(name);

    let mut cmd = Command::new("podman");
    cmd.args(["network", "create"])
        .args(["--label", "d4c.managed=true"]);

    if let Some(t) = tag {
        cmd.args(["--label", &format!("d4c.tag={}", t)]);
    }

    if let Some(s) = subnet {
        cmd.args(["--subnet", s]);
    }

    // Disable DNS plugin if aardvark-dns is not installed to prevent netavark IO errors
    let has_aardvark = Command::new("which")
        .arg("aardvark-dns")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !has_aardvark {
        cmd.arg("--disable-dns");
    }

    cmd.arg(&net_name);

    let output = cmd.output().context("Failed to execute 'podman network create'")?;

    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        bail!("Failed to create bridge network '{}': {}", net_name, err_msg.trim());
    }

    // Persist tag in state store
    let mut store = StateStore::load().unwrap_or_default();
    let _ = store.set_network_tag(&net_name, tag.map(|s| s.to_string()));

    Ok(net_name)
}

/// Lists all bridge networks managed by d4c or existing in Podman
pub fn list_bridge_networks() -> Result<Vec<NetworkInfo>> {
    let output = Command::new("podman")
        .args(["network", "ls", "--format", "json"])
        .output()
        .context("Failed to execute 'podman network ls'")?;

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let json_str = String::from_utf8_lossy(&output.stdout);
    let raw_list: Vec<Value> = serde_json::from_str(&json_str).unwrap_or_default();
    let store = StateStore::load().unwrap_or_default();

    let mut networks = Vec::new();

    for item in raw_list {
        let name = item.get("name")
            .or_else(|| item.get("Name"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let id = item.get("id")
            .or_else(|| item.get("Id"))
            .or_else(|| item.get("network_interface"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let labels = item.get("labels").and_then(|v| v.as_object());
        let is_d4c = name.starts_with("d4c_net_")
            || labels.is_some_and(|l| l.contains_key("d4c.managed"));

        if !is_d4c {
            continue;
        }

        // Inspect detailed network settings
        let inspect_output = Command::new("podman")
            .args(["network", "inspect", &name, "--format", "json"])
            .output();

        let mut subnet = None;
        let mut gateway = None;
        let mut containers = Vec::new();

        if let Ok(insp_out) = inspect_output
            && insp_out.status.success()
        {
            let insp_json: Vec<Value> = serde_json::from_slice(&insp_out.stdout).unwrap_or_default();
            if let Some(first_sub) = insp_json.first()
                .and_then(|first| first.get("subnets"))
                .and_then(|v| v.as_array())
                .and_then(|subnets| subnets.first())
            {
                subnet = first_sub.get("subnet").and_then(|v| v.as_str()).map(String::from);
                gateway = first_sub.get("gateway").and_then(|v| v.as_str()).map(String::from);
            }
        }

        // Find connected containers by querying podman ps
        let ps_output = Command::new("podman")
            .args(["ps", "-a", "--filter", &format!("network={}", name), "--format", "{{.Names}}"])
            .output();

        let mut container_ips = std::collections::HashMap::new();
        if let Ok(ps_out) = ps_output {
            let names_str = String::from_utf8_lossy(&ps_out.stdout);
            for line in names_str.lines() {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    containers.push(trimmed.to_string());
                    // Inspect IP for this container on this network
                    let c_insp = Command::new("podman")
                        .args(["inspect", trimmed, "--format", &format!("{{{{.NetworkSettings.Networks.{}.IPAddress}}}}", name)])
                        .output();
                    if let Ok(c_out) = c_insp {
                        let ip = String::from_utf8_lossy(&c_out.stdout).trim().to_string();
                        if !ip.is_empty() && ip != "<no value>" {
                            container_ips.insert(trimmed.to_string(), ip);
                        }
                    }
                }
            }
        }

        let tag = labels
            .and_then(|l| l.get("d4c.tag"))
            .and_then(|v| v.as_str())
            .map(String::from)
            .or_else(|| store.networks.get(&name).cloned().flatten());

        let count = containers.len();
        networks.push(NetworkInfo {
            name,
            id,
            subnet,
            gateway,
            tag,
            containers_count: count,
            containers,
            container_ips,
        });
    }

    Ok(networks)
}

/// Removes a custom bridge network
pub fn remove_bridge_network(name: &str) -> Result<()> {
    let net_name = normalize_network_name(name);

    let output = Command::new("podman")
        .args(["network", "rm", &net_name])
        .output()
        .context("Failed to execute 'podman network rm'")?;

    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        bail!("Failed to remove network '{}': {}", net_name, err_msg.trim());
    }

    let mut store = StateStore::load().unwrap_or_default();
    let _ = store.remove_network(&net_name);

    Ok(())
}

/// Migrates a legacy container running in pasta mode to a bridge network without losing Btrfs volume data
pub fn migrate_pasta_container_to_bridge(service: &str, network: &str) -> Result<()> {
    let store = StateStore::load().unwrap_or_default();
    let service_state = store.get_service(service).cloned();

    let (catalog, image, host_port, container_port, volume_path, env_vars, cpus, memory, tag, branch_parent) = match service_state {
        Some(s) => {
            let env: Vec<(String, String)> = s.env.into_iter().collect();
            (s.catalog, s.image, s.host_port, s.container_port, s.volume_path, env, s.cpus, s.memory, s.tag, s.branch_parent)
        }
        None => {
            let insp = crate::podman::inspect_service(service)?
                .context(format!("Service '{}' not found for migration", service))?;
            let env = insp.catalog.default_env(None, insp.container_port);
            let p = insp.host_port.unwrap_or(insp.container_port);
            (insp.catalog, insp.image, p, insp.container_port, insp.volume_path, env, insp.cpus, insp.memory, insp.tag, insp.branch_parent)
        }
    };

    let was_running = crate::podman::inspect_service(service)?
        .map(|s| s.is_running())
        .unwrap_or(true);

    if was_running {
        let _ = crate::podman::stop_service(service);
    }

    crate::podman::remove_container(service, true)?;

    let container_mount = catalog.default_data_dir();
    crate::podman::run_service_container(crate::podman::RunContainerParams {
        name: service,
        catalog,
        image: &image,
        host_port,
        container_port,
        volume_path: &volume_path,
        container_mount,
        env: &env_vars,
        cpus: cpus.as_deref(),
        memory: memory.as_deref(),
        network: Some(network),
        tag: tag.as_deref(),
        branch_parent: branch_parent.as_deref(),
    })?;

    if !was_running {
        let _ = crate::podman::stop_service(service);
    }

    let mut store = StateStore::load().unwrap_or_default();
    if let Some(svc) = store.services.get_mut(service) {
        svc.network = Some(network.to_string());
        let _ = store.save();
    }

    Ok(())
}

/// Hot-attaches a service container to a bridge network
pub fn attach_container(service: &str, network: &str) -> Result<()> {
    ensure_rootless_network_config();
    let net_name = normalize_network_name(network);

    let output = Command::new("podman")
        .args(["network", "connect", &net_name, service])
        .output()
        .context("Failed to execute 'podman network connect'")?;

    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        if err_msg.contains("pasta") {
            // Container was started with pasta network mode.
            // Seamlessly migrate it to bridge network mode preserving its Btrfs subvolume!
            migrate_pasta_container_to_bridge(service, &net_name)?;
            return Ok(());
        }
        bail!("Failed to attach container '{}' to network '{}': {}", service, net_name, err_msg.trim());
    }

    let mut store = StateStore::load().unwrap_or_default();
    if let Some(svc) = store.services.get_mut(service) {
        if !svc.networks.contains(&net_name) {
            svc.networks.push(net_name.clone());
            svc.networks.sort();
        }
        svc.network = Some(net_name);
        let _ = store.save();
    }

    Ok(())
}

/// Hot-detaches a service container from a bridge network
pub fn detach_container(service: &str, network: &str) -> Result<()> {
    let net_name = normalize_network_name(network);

    let output = Command::new("podman")
        .args(["network", "disconnect", &net_name, service])
        .output()
        .context("Failed to execute 'podman network disconnect'")?;

    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        bail!("Failed to detach container '{}' from network '{}': {}", service, net_name, err_msg.trim());
    }

    let mut store = StateStore::load().unwrap_or_default();
    if let Some(svc) = store.services.get_mut(service) {
        svc.networks.retain(|n| n != &net_name);
        if svc.network.as_deref() == Some(&net_name) {
            svc.network = svc.networks.first().cloned();
        }
        let _ = store.save();
    }

    Ok(())
}

/// Streams aggregated real-time logs across all containers on a given network
pub async fn panorama_logs(network: &str) -> Result<()> {
    let net_name = normalize_network_name(network);

    // Get containers attached to this network
    let ps_output = Command::new("podman")
        .args(["ps", "--filter", &format!("network={}", net_name), "--format", "{{.Names}}"])
        .output()
        .context("Failed to list containers for panorama")?;

    let names_str = String::from_utf8_lossy(&ps_output.stdout);
    let container_names: Vec<String> = names_str
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if container_names.is_empty() {
        println!("No running containers attached to network '{}'", net_name);
        return Ok(());
    }

    println!("Streaming aggregated Panorama logs for network '{}' ({} containers)...", net_name, container_names.len());
    println!("Press Ctrl+C to stop.\n");

    let colors = [
        Color::Cyan,
        Color::Green,
        Color::Yellow,
        Color::Magenta,
        Color::Blue,
        Color::Red,
    ];

    let mut join_set = tokio::task::JoinSet::new();

    for (i, name) in container_names.into_iter().enumerate() {
        let color = colors[i % colors.len()];
        let tag_style = Style::new().fg(color).bold();
        let prefix = tag_style.apply_to(format!("[{}] ", name)).to_string();

        join_set.spawn(async move {
            let mut child = match TokioCommand::new("podman")
                .args(["logs", "-f", "--tail", "20", &name])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
            {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("{} Failed to attach log stream: {}", prefix, e);
                    return;
                }
            };

            let stdout = child.stdout.take();
            let stderr = child.stderr.take();

            let prefix_out = prefix.clone();
            let out_task = tokio::spawn(async move {
                if let Some(stdout) = stdout {
                    let mut reader = BufReader::new(stdout).lines();
                    while let Ok(Some(line)) = reader.next_line().await {
                        println!("{}{}", prefix_out, line);
                    }
                }
            });

            let prefix_err = prefix.clone();
            let err_task = tokio::spawn(async move {
                if let Some(stderr) = stderr {
                    let mut reader = BufReader::new(stderr).lines();
                    while let Ok(Some(line)) = reader.next_line().await {
                        eprintln!("{}{}", prefix_err, line);
                    }
                }
            });

            let _ = tokio::join!(out_task, err_task);
            let _ = child.wait().await;
        });
    }

    // Wait until Ctrl+C or all terminate
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            println!("\nTerminating Panorama stream.");
        }
        _ = async {
            while join_set.join_next().await.is_some() {}
        } => {}
    }

    Ok(())
}

/// Disconnects a container from all networks it is currently attached to
pub fn disconnect_all_networks(service: &str) -> Result<()> {
    let output = Command::new("podman")
        .args(["inspect", service, "--format", "{{range $net, $conf := .NetworkSettings.Networks}}{{$net}} {{end}}"])
        .output();

    if let Ok(out) = output {
        let nets_str = String::from_utf8_lossy(&out.stdout);
        for net in nets_str.split_whitespace() {
            let _ = detach_container(service, net);
        }
    }

    let mut store = StateStore::load().unwrap_or_default();
    if let Some(svc) = store.services.get_mut(service) {
        svc.networks.clear();
        svc.network = None;
        let _ = store.save();
    }

    Ok(())
}

/// Fetches recent logs from all containers on a given network for in-TUI panorama rendering
pub fn get_network_recent_logs(network: &str, tail: usize) -> Vec<(String, String)> {
    let net_name = normalize_network_name(network);
    let ps_output = Command::new("podman")
        .args(["ps", "--filter", &format!("network={}", net_name), "--format", "{{.Names}}"])
        .output();

    let mut all_logs = Vec::new();
    if let Ok(ps_out) = ps_output {
        let names_str = String::from_utf8_lossy(&ps_out.stdout);
        for line in names_str.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                let logs = crate::podman::get_recent_logs(trimmed, tail);
                for l in logs {
                    all_logs.push((trimmed.to_string(), l));
                }
            }
        }
    }
    all_logs
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_network_name() {
        assert_eq!(normalize_network_name("backend"), "d4c_net_backend");
        assert_eq!(normalize_network_name("d4c_net_backend"), "d4c_net_backend");
    }

    #[test]
    fn test_find_free_port() {
        let free_port = find_free_port(None, 49152).expect("Should find a free ephemeral port");
        assert!(free_port >= 49152);
        assert!(is_port_free(free_port));
    }
}
