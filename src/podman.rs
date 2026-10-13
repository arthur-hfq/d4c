use crate::cli::ServiceCatalog;
use crate::model::{ServiceInfo, ServiceState, StateStore};
use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Copy)]
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

fn find_crun_runtime() -> Option<PathBuf> {
    if let Ok(home) = std::env::var("HOME") {
        let p = PathBuf::from(home).join(".local/bin/crun");
        if p.exists() {
            return Some(p);
        }
    }
    for p in ["/usr/bin/crun", "/usr/local/bin/crun", "/usr/sbin/crun"] {
        let path = PathBuf::from(p);
        if path.exists() {
            return Some(path);
        }
    }
    None
}

pub fn normalize_memory_limit(mem: &str) -> String {
    let trimmed = mem.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    // If user typed only digits (e.g. "512", "1024"), default to megabytes (m)
    if trimmed.chars().all(|c| c.is_ascii_digit()) {
        return format!("{}m", trimmed);
    }
    let lower = trimmed.to_lowercase();
    if lower.ends_with("mb") {
        return format!("{}m", &lower[..lower.len() - 2]);
    }
    if lower.ends_with("gb") {
        return format!("{}g", &lower[..lower.len() - 2]);
    }
    if lower.ends_with("kb") {
        return format!("{}k", &lower[..lower.len() - 2]);
    }
    trimmed.to_string()
}

pub fn normalize_cpu_limit(cpu: &str) -> String {
    cpu.trim().to_string()
}

fn build_run_command(params: &RunContainerParams, include_limits: bool) -> Command {
    let mut cmd = Command::new("podman");
    cmd.args(["run", "-d"]);
    cmd.args(["--name", params.name]);

    // Use crun runtime if available for native rootless cgroup limit support
    if let Some(crun_path) = find_crun_runtime() {
        cmd.args(["--runtime", crun_path.to_str().unwrap_or("crun")]);
    }

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

    // Resource limits (if enabled and requested)
    if include_limits {
        if let Some(cpus) = params.cpus {
            let n_cpu = normalize_cpu_limit(cpus);
            if !n_cpu.is_empty() {
                cmd.args(["--cpus", &n_cpu]);
            }
        }
        if let Some(mem) = params.memory {
            let n_mem = normalize_memory_limit(mem);
            if !n_mem.is_empty() {
                cmd.args(["--memory", &n_mem]);
            }
        }
    }

    // Network connection: only pass --network if a custom network was specified.
    // In rootless mode, omitting --network allows Podman to use default rootless networking (pasta/slirp4netns),
    // which does not require root/iptables NAT permissions.
    if let Some(net) = params.network {
        cmd.args(["--network", net]);
    }

    // Environment variables
    for (k, v) in params.env {
        cmd.args(["-e", &format!("{}={}", k, v)]);
    }

    // Image
    cmd.arg(params.image);

    cmd
}

/// Provisions and launches a container with Podman
pub fn run_service_container(params: RunContainerParams) -> Result<String> {
    // Proactively remove any stale or stopped container instance with the same name
    // to prevent container name conflicts or orphaned state
    let _ = remove_container(params.name, true);

    let normalized_cpus = params.cpus.map(normalize_cpu_limit).filter(|s| !s.is_empty());
    let normalized_mem = params.memory.map(normalize_memory_limit).filter(|s| !s.is_empty());
    let has_limits = normalized_cpus.is_some() || normalized_mem.is_some();
    let mut applied_limits = has_limits;

    let mut cmd = build_run_command(&params, true);
    let output = cmd.output().context("Failed to execute 'podman run'")?;

    let container_id = if output.status.success() {
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    } else {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        // Ensure failed half-created container is removed from Podman
        let _ = remove_container(params.name, true);

        // Check if failure is due to rootless cgroup delegation limitations or bridge network iptables failure
        let is_cgroup_err = err_msg.contains("cgroup")
            || err_msg.contains("rootless needs no limits")
            || err_msg.contains("permission denied: OCI permission denied");
        let is_net_err = err_msg.contains("netavark") || err_msg.contains("ip_tables");

        if has_limits && is_cgroup_err {
            applied_limits = false;
            let mut retry_cmd = build_run_command(&params, false);
            let retry_out = retry_cmd.output().context("Failed to execute fallback 'podman run'")?;
            if retry_out.status.success() {
                String::from_utf8_lossy(&retry_out.stdout).trim().to_string()
            } else {
                let retry_err = String::from_utf8_lossy(&retry_out.stderr);
                let _ = remove_container(params.name, true);
                bail!("Failed to start container '{}': {}", params.name, retry_err.trim());
            }
        } else if is_net_err && params.network.is_some() {
            // Retry without custom bridge network
            let mut retry_params = params;
            retry_params.network = None;
            let mut retry_cmd = build_run_command(&retry_params, applied_limits);
            let retry_out = retry_cmd.output().context("Failed to execute network fallback 'podman run'")?;
            if retry_out.status.success() {
                String::from_utf8_lossy(&retry_out.stdout).trim().to_string()
            } else {
                let retry_err = String::from_utf8_lossy(&retry_out.stderr);
                let _ = remove_container(params.name, true);
                bail!("Failed to start container '{}': {}", params.name, retry_err.trim());
            }
        } else {
            bail!("Failed to start container '{}': {}", params.name, err_msg.trim());
        }
    };

    // Persist to local state store
    let mut env_map = HashMap::new();
    for (k, v) in params.env {
        env_map.insert(k.clone(), v.clone());
    }

    let now_str = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();

    let initial_networks = params.network.map(|s| vec![s.to_string()]).unwrap_or_default();
    let state = ServiceState {
        name: params.name.to_string(),
        catalog: params.catalog,
        image: params.image.to_string(),
        host_port: params.host_port,
        container_port: params.container_port,
        tag: params.tag.map(|s| s.to_string()),
        volume_path: params.volume_path.to_path_buf(),
        networks: initial_networks,
        network: params.network.map(|s| s.to_string()),
        cpus: if applied_limits { normalized_cpus } else { None },
        memory: if applied_limits { normalized_mem } else { None },
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
        let is_missing = err.contains("no such container") || err.contains("no container with name");
        let is_cgroup_err = err.contains("cgroup")
            || err.contains("rootless needs no limits")
            || err.contains("permission denied: OCI permission denied");
        let is_net_err = err.contains("netavark") || err.contains("ip_tables");

        if is_missing || is_cgroup_err || is_net_err {
            let store = StateStore::load().unwrap_or_default();
            if let Some(s) = store.services.get(name) {
                // If it was a cgroup or network failure on an existing container, remove the broken container first
                if is_cgroup_err || is_net_err {
                    let _ = remove_container(name, true);
                }
                let cat = s.catalog;
                let c_mount = cat.default_data_dir();
                let env_vec: Vec<(String, String)> = s.env.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                let _ = run_service_container(RunContainerParams {
                    name,
                    catalog: cat,
                    image: &s.image,
                    host_port: s.host_port,
                    container_port: s.container_port,
                    volume_path: &s.volume_path,
                    container_mount: c_mount,
                    env: &env_vec,
                    cpus: if is_cgroup_err { None } else { s.cpus.as_deref() },
                    memory: if is_cgroup_err { None } else { s.memory.as_deref() },
                    network: if is_net_err { None } else { s.network.as_deref() },
                    tag: s.tag.as_deref(),
                    branch_parent: s.branch_parent.as_deref(),
                })?;
                return Ok(());
            }
        }

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

            let networks: Vec<String> = item.get("Networks")
                .and_then(|v| {
                    if let Some(arr) = v.as_array() {
                        let mut list: Vec<String> = arr.iter().filter_map(|s| s.as_str().map(String::from)).collect();
                        list.sort();
                        Some(list)
                    } else {
                        v.as_str().map(|s| vec![s.to_string()])
                    }
                })
                .unwrap_or_else(|| {
                    store.services.get(&name)
                        .and_then(|s| s.network.clone())
                        .map(|n| vec![n])
                        .unwrap_or_default()
                });

            let network = networks.first().cloned();

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
                networks,
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
            let net_list = if !s.networks.is_empty() {
                s.networks.clone()
            } else {
                s.network.clone().map(|n| vec![n]).unwrap_or_default()
            };
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
                    networks: net_list,
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
    let norm_cpu = cpus.map(normalize_cpu_limit).filter(|c| !c.is_empty());
    let norm_mem = memory.map(normalize_memory_limit).filter(|m| !m.is_empty());

    if norm_cpu.is_none() && norm_mem.is_none() {
        return Ok(());
    }

    let mut cmd = Command::new("podman");
    cmd.args(["update"]);

    if let Some(ref c) = norm_cpu {
        cmd.args(["--cpus", c]);
    }
    if let Some(ref m) = norm_mem {
        cmd.args(["--memory", m]);
    }
    cmd.arg(name);

    let output = cmd.output().context("Failed to execute 'podman update'")?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        if err.contains("cgroup") || err.contains("cpu.max") || err.contains("exit status 1") {
            bail!(
                "Failed to update resources for '{}': Rootless cgroup delegation is not enabled on this host. Dynamic limits cannot be updated without root/cgroup privileges.",
                name
            );
        }
        bail!("Failed to update resources for '{}': {}", name, err.trim());
    }

    // Update in state store
    let mut store = StateStore::load().unwrap_or_default();
    if let Some(svc) = store.services.get_mut(name) {
        if let Some(c) = norm_cpu {
            svc.cpus = Some(c);
        }
        if let Some(m) = norm_mem {
            svc.memory = Some(m);
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

    let networks: Vec<String> = val
        .get("NetworkSettings")
        .and_then(|n| n.get("Networks"))
        .and_then(|n| n.as_object())
        .map(|nets| {
            let mut list: Vec<String> = nets.keys().cloned().collect();
            list.sort();
            list
        })
        .unwrap_or_else(|| {
            state_opt
                .and_then(|s| s.network.clone())
                .map(|n| vec![n])
                .unwrap_or_default()
        });

    let network = networks.first().cloned();
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
        networks,
        network,
        cpus,
        memory,
        created_at,
        branch_parent,
    }
}

/// Retrieve the most recent log lines of a container
pub fn get_recent_logs(name: &str, tail: usize) -> Vec<String> {
    let output = Command::new("podman")
        .args(["logs", "--tail", &tail.to_string(), name])
        .output();

    if let Ok(out) = output {
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        let mut lines: Vec<String> = stdout
            .lines()
            .chain(stderr.lines())
            .map(|s| s.to_string())
            .collect();
        if lines.len() > tail {
            lines = lines.split_off(lines.len() - tail);
        }
        lines
    } else {
        Vec::new()
    }
}

/// Retrieve assigned IP addresses for each attached network of a container
pub fn get_container_network_ips(name: &str) -> Vec<(String, String)> {
    let output = Command::new("podman")
        .args(["inspect", name, "--format", "json"])
        .output();

    let mut result = Vec::new();
    if let Ok(out) = output
        && out.status.success()
    {
        let val: Vec<Value> = serde_json::from_slice(&out.stdout).unwrap_or_default();
        if let Some(first) = val.first()
            && let Some(nets) = first
                .get("NetworkSettings")
                .and_then(|n| n.get("Networks"))
                .and_then(|n| n.as_object())
        {
            for (net_name, conf) in nets {
                let ip = conf.get("IPAddress").and_then(|v| v.as_str()).unwrap_or("").to_string();
                if !ip.is_empty() {
                    result.push((net_name.clone(), ip));
                }
            }
        }
    }
    result
}
