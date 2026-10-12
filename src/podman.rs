use crate::cli::ServiceCatalog;
use crate::model::{ServiceInfo, ServiceState, StateStore};
use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub struct RunContainerParams<'a> {
    pub name: &'a str,
    pub catalog: ServiceCatalog,
    pub image: &'a str,
    pub host_port: u16,
    pub container_port: u16,
    pub volume_path: &'a Path,
    pub container_mount: &'a str,
    pub env: &'a [(String, String)],
    pub cpus: Option<&'a str>,
    pub memory: Option<&'a str>,
    pub network: Option<&'a str>,
    pub tag: Option<&'a str>,
    pub branch_parent: Option<&'a str>,
}

/// Provisions and launches a container with Podman
pub fn run_service_container(params: RunContainerParams) -> Result<String> {
    let mut cmd = Command::new("podman");
    cmd.args(["run", "-d"]);
    cmd.args(["--name", params.name]);

    // Standard d4c orchestration labels
    cmd.args(["--label", "d4c.managed=true"]);
    cmd.args(["--label", &format!("d4c.service={}", params.name)]);
    cmd.args(["--label", &format!("d4c.catalog={}", params.catalog)]);
    cmd.args(["--label", &format!("d4c.port={}", params.host_port)]);
    cmd.args(["--label", &format!("d4c.cport={}", params.container_port)]);
    cmd.args(["--label", &format!("d4c.volume={}", params.volume_path.display())]);

    if let Some(tag) = params.tag {
        cmd.args(["--label", &format!("d4c.tag={}", tag)]);
    }
    if let Some(parent) = params.branch_parent {
        cmd.args(["--label", &format!("d4c.parent={}", parent)]);
    }

    // Port mapping
    cmd.args(["-p", &format!("{}:{}", params.host_port, params.container_port)]);

    // Volume mounting with :Z flag for SELinux / rootless permissions
    cmd.args([
        "-v",
        &format!("{}:{}:Z", params.volume_path.display(), params.container_mount),
    ]);

    // Resource limits
    if let Some(cpus) = params.cpus {
        cmd.args(["--cpus", cpus]);
    }
    if let Some(mem) = params.memory {
        cmd.args(["--memory", mem]);
    }

    // Network connection
    if let Some(net) = params.network {
        cmd.args(["--network", net]);
    }

    // Environment variables
    for (k, v) in params.env {
        cmd.args(["-e", &format!("{}={}", k, v)]);
    }

    // Image
    cmd.arg(params.image);

    let output = cmd.output().context("Failed to execute 'podman run'")?;

    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        bail!("Failed to start container '{}': {}", params.name, err_msg.trim());
    }

    let container_id = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // Persist to local state store
    let mut env_map = HashMap::new();
    for (k, v) in params.env {
        env_map.insert(k.clone(), v.clone());
    }

    let now_str = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();

    let state = ServiceState {
        name: params.name.to_string(),
        catalog: params.catalog,
        image: params.image.to_string(),
        host_port: params.host_port,
        container_port: params.container_port,
        tag: params.tag.map(|s| s.to_string()),
        volume_path: params.volume_path.to_path_buf(),
        network: params.network.map(|s| s.to_string()),
        cpus: params.cpus.map(|s| s.to_string()),
        memory: params.memory.map(|s| s.to_string()),
        branch_parent: params.branch_parent.map(|s| s.to_string()),
        created_at: now_str,
        env: env_map,
    };

    let mut store = StateStore::load().unwrap_or_default();
    let _ = store.set_service(state);

    Ok(container_id)
}

/// Starts or restarts a managed service
pub fn start_service(name: &str) -> Result<()> {
    let output = Command::new("podman")
        .args(["start", name])
        .output()
        .context("Failed to execute 'podman start'")?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        bail!("Failed to start container '{}': {}", name, err.trim());
    }

    Ok(())
}

/// Gracefully stops a running service container
pub fn stop_service(name: &str) -> Result<()> {
    let output = Command::new("podman")
        .args(["stop", "-t", "10", name])
        .output()
        .context("Failed to execute 'podman stop'")?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        bail!("Failed to stop container '{}': {}", name, err.trim());
    }

    Ok(())
}

/// Removes a container instance
pub fn remove_container(name: &str, force: bool) -> Result<()> {
    let mut cmd = Command::new("podman");
    cmd.arg("rm");
    if force {
        cmd.arg("-f");
    }
    cmd.arg(name);

    let output = cmd.output().context("Failed to execute 'podman rm'")?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        bail!("Failed to remove container '{}': {}", name, err.trim());
    }

    Ok(())
}

/// Inspects a single service container
pub fn inspect_service(name: &str) -> Result<Option<ServiceInfo>> {
    let output = Command::new("podman")
        .args(["inspect", name, "--format", "json"])
        .output()
        .context("Failed to execute 'podman inspect'")?;

    if !output.status.success() {
        return Ok(None);
    }

    let json_bytes = output.stdout;
    let list: Vec<Value> = serde_json::from_slice(&json_bytes).unwrap_or_default();

    if let Some(item) = list.first() {
        let store = StateStore::load().unwrap_or_default();
        let state_opt = store.get_service(name);
        return Ok(Some(parse_inspect_json(item, state_opt)));
    }

    Ok(None)
}

/// Lists all d4c managed services, merging live Podman container states with the local state store
pub fn list_services() -> Result<Vec<ServiceInfo>> {
    let output = Command::new("podman")
        .args(["ps", "-a", "--filter", "label=d4c.managed=true", "--format", "json"])
        .output()
        .context("Failed to execute 'podman ps'")?;

    let store = StateStore::load().unwrap_or_default();
    let mut services_map: HashMap<String, ServiceInfo> = HashMap::new();

    if output.status.success() {
        let raw_list: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap_or_default();

        for item in raw_list {
            let name = item.get("Names")
                .and_then(|v| {
                    if let Some(arr) = v.as_array() {
                        arr.first().and_then(|s| s.as_str())
                    } else {
                        v.as_str()
                    }
                })
                .unwrap_or("")
                .to_string();

            if name.is_empty() {
                continue;
            }

            let id = item.get("Id").and_then(|v| v.as_str()).map(String::from);
            let state_str = item.get("State")
                .or_else(|| item.get("Status"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            let image = item.get("Image").and_then(|v| v.as_str()).unwrap_or("").to_string();

            let labels = item.get("Labels").and_then(|v| v.as_object());

            let catalog_str = labels
                .and_then(|l| l.get("d4c.catalog"))
                .and_then(|v| v.as_str())
                .unwrap_or("custom");
            let catalog = ServiceCatalog::parse_loose(catalog_str).unwrap_or(ServiceCatalog::Custom);

            let host_port = labels
                .and_then(|l| l.get("d4c.port"))
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<u16>().ok());

            let container_port = labels
                .and_then(|l| l.get("d4c.cport"))
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<u16>().ok())
                .unwrap_or_else(|| catalog.default_port());

            let vol_str = labels
                .and_then(|l| l.get("d4c.volume"))
                .and_then(|v| v.as_str());

            let volume_path = match vol_str {
                Some(v) => PathBuf::from(v),
                None => crate::btrfs::get_service_volume_path(&name).unwrap_or_default(),
            };

            let tag = labels
                .and_then(|l| l.get("d4c.tag"))
                .and_then(|v| v.as_str())
                .map(String::from)
                .or_else(|| store.services.get(&name).and_then(|s| s.tag.clone()));

            let branch_parent = labels
                .and_then(|l| l.get("d4c.parent"))
                .and_then(|v| v.as_str())
                .map(String::from)
                .or_else(|| store.services.get(&name).and_then(|s| s.branch_parent.clone()));

            let network = item.get("Networks")
                .and_then(|v| {
                    if let Some(arr) = v.as_array() {
                        arr.first().and_then(|s| s.as_str())
                    } else {
                        v.as_str()
                    }
                })
                .map(String::from)
                .or_else(|| store.services.get(&name).and_then(|s| s.network.clone()));

            let created_at = item.get("CreatedAt")
                .or_else(|| item.get("Created"))
                .and_then(|v| v.as_str())
                .map(String::from)
                .or_else(|| store.services.get(&name).map(|s| s.created_at.clone()));

            let cpus = store.services.get(&name).and_then(|s| s.cpus.clone());
            let memory = store.services.get(&name).and_then(|s| s.memory.clone());

            let svc = ServiceInfo {
                name: name.clone(),
                container_id: id,
                status: state_str,
                catalog,
                image,
                host_port,
                container_port,
                tag,
                volume_path,
                network,
                cpus,
                memory,
                created_at,
                branch_parent,
            };

            services_map.insert(name, svc);
        }
    }

    // Include stopped/uncreated services stored in state store that may not be active in podman
    for (name, s) in &store.services {
        if !services_map.contains_key(name) {
            services_map.insert(
                name.clone(),
                ServiceInfo {
                    name: name.clone(),
                    container_id: None,
                    status: "stopped".to_string(),
                    catalog: s.catalog,
                    image: s.image.clone(),
                    host_port: Some(s.host_port),
                    container_port: s.container_port,
                    tag: s.tag.clone(),
                    volume_path: s.volume_path.clone(),
                    network: s.network.clone(),
                    cpus: s.cpus.clone(),
                    memory: s.memory.clone(),
                    created_at: Some(s.created_at.clone()),
                    branch_parent: s.branch_parent.clone(),
                },
            );
        }
    }

    let mut result: Vec<ServiceInfo> = services_map.into_values().collect();
    result.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(result)
}

/// Updates resource limits (CPU and memory) of a running or existing container
pub fn update_service_resources(
    name: &str,
    cpus: Option<&str>,
    memory: Option<&str>,
) -> Result<()> {
    if cpus.is_none() && memory.is_none() {
        return Ok(());
    }

    let mut cmd = Command::new("podman");
    cmd.args(["update"]);

    if let Some(c) = cpus {
        cmd.args(["--cpus", c]);
    }
    if let Some(m) = memory {
        cmd.args(["--memory", m]);
    }
    cmd.arg(name);

    let output = cmd.output().context("Failed to execute 'podman update'")?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        bail!("Failed to update resources for '{}': {}", name, err.trim());
    }

    // Update in state store
    let mut store = StateStore::load().unwrap_or_default();
    if let Some(svc) = store.services.get_mut(name) {
        if let Some(c) = cpus {
            svc.cpus = Some(c.to_string());
        }
        if let Some(m) = memory {
            svc.memory = Some(m.to_string());
        }
        let _ = store.save();
    }

    Ok(())
}

/// Streams service container logs directly to stdout/stderr
pub fn stream_service_logs(name: &str, follow: bool, tail: usize) -> Result<()> {
    let mut cmd = Command::new("podman");
    cmd.arg("logs");
    cmd.args(["--tail", &tail.to_string()]);
    if follow {
        cmd.arg("-f");
    }
    cmd.arg(name);

    cmd.stdout(Stdio::inherit());
    cmd.stderr(Stdio::inherit());

    let status = cmd.status().context("Failed to stream podman logs")?;

    if !status.success() {
        bail!("podman logs exited with non-zero status");
    }

    Ok(())
}

fn parse_inspect_json(val: &Value, state_opt: Option<&ServiceState>) -> ServiceInfo {
    let name_raw = val.get("Name").and_then(|v| v.as_str()).unwrap_or("");
    let name = name_raw.trim_start_matches('/').to_string();

    let id = val.get("Id").and_then(|v| v.as_str()).map(String::from);

    let status = val.get("State")
        .and_then(|s| s.get("Status"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let image = val.get("Config")
        .and_then(|c| c.get("Image"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let labels = val.get("Config")
        .and_then(|c| c.get("Labels"))
        .and_then(|v| v.as_object());

    let catalog_str = labels
        .and_then(|l| l.get("d4c.catalog"))
        .and_then(|v| v.as_str())
        .unwrap_or("custom");
    let catalog = ServiceCatalog::parse_loose(catalog_str).unwrap_or(ServiceCatalog::Custom);

    let host_port = labels
        .and_then(|l| l.get("d4c.port"))
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<u16>().ok())
        .or_else(|| state_opt.map(|s| s.host_port));

    let container_port = labels
        .and_then(|l| l.get("d4c.cport"))
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<u16>().ok())
        .or_else(|| state_opt.map(|s| s.container_port))
        .unwrap_or_else(|| catalog.default_port());

    let volume_path = labels
        .and_then(|l| l.get("d4c.volume"))
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .or_else(|| state_opt.map(|s| s.volume_path.clone()))
        .unwrap_or_else(|| crate::btrfs::get_service_volume_path(&name).unwrap_or_default());

    let tag = labels
        .and_then(|l| l.get("d4c.tag"))
        .and_then(|v| v.as_str())
        .map(String::from)
        .or_else(|| state_opt.and_then(|s| s.tag.clone()));

    let branch_parent = labels
        .and_then(|l| l.get("d4c.parent"))
        .and_then(|v| v.as_str())
        .map(String::from)
        .or_else(|| state_opt.and_then(|s| s.branch_parent.clone()));

    let network = state_opt.and_then(|s| s.network.clone());
    let cpus = state_opt.and_then(|s| s.cpus.clone());
    let memory = state_opt.and_then(|s| s.memory.clone());

    let created_at = val.get("Created")
        .and_then(|v| v.as_str())
        .map(String::from)
        .or_else(|| state_opt.map(|s| s.created_at.clone()));

    ServiceInfo {
        name,
        container_id: id,
        status,
        catalog,
        image,
        host_port,
        container_port,
        tag,
        volume_path,
        network,
        cpus,
        memory,
        created_at,
        branch_parent,
    }
}
