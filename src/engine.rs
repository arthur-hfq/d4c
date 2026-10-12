use crate::btrfs;
use crate::cli::{NetCommands, ServiceCatalog, SnapCommands};
use crate::model::StateStore;
use crate::net;
use crate::podman::{self, RunContainerParams};
use anyhow::{Context, Result};
use console::Style;
use dialoguer::{Confirm, Input, Select};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

pub struct Engine;

impl Engine {
    /// Provision and launch a service from catalog or custom image
    #[allow(clippy::too_many_arguments)]
    pub fn up(
        service_type: ServiceCatalog,
        port: Option<u16>,
        name: Option<String>,
        password: Option<String>,
        tag: Option<String>,
        cpus: Option<String>,
        memory: Option<String>,
        network: Option<String>,
        image: Option<String>,
    ) -> Result<()> {
        let bold = Style::new().bold();
        let green = Style::new().green().bold();
        let cyan = Style::new().cyan();

        let service_name = name.unwrap_or_else(|| service_type.to_string());
        println!("{}", bold.apply_to(format!("==> Provisioning service '{}'...", service_name)));

        // Discover or validate port
        let host_port = net::find_free_port(port, service_type.default_port())?;
        let container_port = service_type.default_port();

        // Ensure Btrfs data subvolume exists
        let volume_path = btrfs::ensure_service_volume(&service_name)?;

        // If custom network requested, ensure it exists or create it
        let resolved_network = if let Some(net_name) = network {
            let normalized = net::normalize_network_name(&net_name);
            let existing_nets = net::list_bridge_networks()?;
            if !existing_nets.iter().any(|n| n.name == normalized) {
                println!("    Creating bridge network '{}'...", normalized);
                net::create_bridge_network(&normalized, tag.as_deref())?;
            }
            Some(normalized)
        } else {
            None
        };

        let container_image = service_type.default_image(image.as_deref());
        let container_mount = service_type.default_data_dir();
        let env_vars = service_type.default_env(password.as_deref(), host_port);

        println!("    Selected image: {}", cyan.apply_to(&container_image));
        println!("    Host port binding: {}", cyan.apply_to(format!("{} -> {}/tcp", host_port, container_port)));
        println!("    Btrfs data volume: {}", cyan.apply_to(format!("{:?}", volume_path)));

        let container_id = podman::run_service_container(RunContainerParams {
            name: &service_name,
            catalog: service_type,
            image: &container_image,
            host_port,
            container_port,
            volume_path: &volume_path,
            container_mount,
            env: &env_vars,
            cpus: cpus.as_deref(),
            memory: memory.as_deref(),
            network: resolved_network.as_deref(),
            tag: tag.as_deref(),
            branch_parent: None,
        })?;

        println!(
            "\n{}",
            green.apply_to(format!("[OK] Service '{}' is up and running! (ID: {})", service_name, &container_id[..12.min(container_id.len())]))
        );
        Ok(())
    }

    /// List all managed services in a formatted dashboard table or JSON
    pub fn ps(json: bool) -> Result<()> {
        let services = podman::list_services()?;

        if json {
            let json_str = serde_json::to_string_pretty(&services)
                .context("Failed to serialize services to JSON")?;
            println!("{}", json_str);
            return Ok(());
        }

        if services.is_empty() {
            println!("No managed services found. Run 'd4c up <service_type>' to provision one.");
            return Ok(());
        }

        let header_style = Style::new().bold().underlined();
        let green = Style::new().green();
        let red = Style::new().red();
        let yellow = Style::new().yellow();
        let cyan = Style::new().cyan();

        println!(
            "{:<18} {:<12} {:<10} {:<20} {:<12} {:<16} {:<15}",
            header_style.apply_to("NAME"),
            header_style.apply_to("STATUS"),
            header_style.apply_to("CATALOG"),
            header_style.apply_to("PORTS"),
            header_style.apply_to("TAG"),
            header_style.apply_to("NETWORK"),
            header_style.apply_to("LIMITS")
        );

        for svc in services {
            let status_styled = if svc.status.to_lowercase().contains("running") || svc.status.to_lowercase().contains("up") {
                green.apply_to(&svc.status)
            } else if svc.status.to_lowercase().contains("exit") || svc.status.to_lowercase().contains("stop") {
                red.apply_to(&svc.status)
            } else {
                yellow.apply_to(&svc.status)
            };

            let port_str = match svc.host_port {
                Some(p) => format!("{}:{}", p, svc.container_port),
                None => format!("- :{}", svc.container_port),
            };

            let tag_str = svc.tag.clone().unwrap_or_else(|| "-".to_string());
            let net_str = svc.network.clone().unwrap_or_else(|| "default".to_string());

            let mut limits = Vec::new();
            if let Some(c) = &svc.cpus {
                limits.push(format!("cpu:{}", c));
            }
            if let Some(m) = &svc.memory {
                limits.push(format!("mem:{}", m));
            }
            let limits_str = if limits.is_empty() {
                "-".to_string()
            } else {
                limits.join(", ")
            };

            println!(
                "{:<18} {:<12} {:<10} {:<20} {:<12} {:<16} {:<15}",
                cyan.apply_to(&svc.name),
                status_styled,
                svc.catalog.to_string(),
                port_str,
                tag_str,
                net_str,
                limits_str
            );
        }

        Ok(())
    }

    /// Start or restart a service
    pub fn start(service: &str) -> Result<()> {
        let green = Style::new().green().bold();
        println!("Starting service '{}'...", service);
        podman::start_service(service)?;
        println!("{}", green.apply_to(format!("[OK] Service '{}' started.", service)));
        Ok(())
    }

    /// Gracefully stop a running service
    pub fn stop(service: &str) -> Result<()> {
        let yellow = Style::new().yellow().bold();
        println!("Stopping service '{}'...", service);
        podman::stop_service(service)?;
        println!("{}", yellow.apply_to(format!("[OK] Service '{}' stopped.", service)));
        Ok(())
    }

    /// Remove a service container while preserving Btrfs data volume
    pub fn rm(service: &str) -> Result<()> {
        let yellow = Style::new().yellow().bold();
        println!("Removing container for service '{}' (volume will be preserved)...", service);
        podman::remove_container(service, true)?;
        println!("{}", yellow.apply_to(format!("[OK] Container '{}' removed. Btrfs volume preserved.", service)));
        Ok(())
    }

    /// Destroy container, state tags, AND Btrfs volume subvolume (destructive)
    pub fn wipe(service: &str, force: bool) -> Result<()> {
        let red = Style::new().red().bold();

        if !force {
            let confirmation = Confirm::new()
                .with_prompt(format!("[WARN] Are you sure you want to WIPE '{}' and DESTROY all its data subvolumes?", service))
                .default(false)
                .interact()?;

            if !confirmation {
                println!("Operation cancelled.");
                return Ok(());
            }
        }

        println!("Wiping container and storage for service '{}'...", service);
        let _ = podman::stop_service(service);
        let _ = podman::remove_container(service, true);

        // Wipe Btrfs volumes and snapshots
        btrfs::wipe_service_storage(service)?;

        // Remove from state store
        let mut store = StateStore::load().unwrap_or_default();
        let _ = store.remove_service(service);

        println!("{}", red.apply_to(format!("[OK] Service '{}' and all associated Btrfs volumes were wiped.", service)));
        Ok(())
    }

    /// Stream live container logs
    pub fn logs(service: &str, follow: bool, tail: usize) -> Result<()> {
        println!("Streaming logs for service '{}' (tail: {}, follow: {})...", service, tail, follow);
        podman::stream_service_logs(service, follow, tail)
    }

    /// Update CPU and memory limits of an existing service on the fly
    pub fn update(service: &str, cpus: Option<String>, memory: Option<String>) -> Result<()> {
        let green = Style::new().green().bold();
        println!("Updating resource limits for service '{}'...", service);
        podman::update_service_resources(service, cpus.as_deref(), memory.as_deref())?;
        println!("{}", green.apply_to(format!("[OK] Resource limits updated for '{}'.", service)));
        Ok(())
    }

    /// Set or update the project tag metadata for a service
    pub fn tag(service: &str, tag: Option<String>) -> Result<()> {
        let green = Style::new().green().bold();
        let mut store = StateStore::load().unwrap_or_default();
        store.set_service_tag(service, tag.clone())?;
        match tag {
            Some(t) => println!("{}", green.apply_to(format!("[OK] Service '{}' tagged with '{}'.", service, t))),
            None => println!("{}", green.apply_to(format!("[OK] Removed tag from service '{}'.", service))),
        }
        Ok(())
    }

    /// Handle snap subcommands
    pub fn snap(cmd: SnapCommands) -> Result<()> {
        match cmd {
            SnapCommands::Create { service, tag, readonly } => {
                let green = Style::new().green().bold();
                println!("Creating atomic snapshot for service '{}'...", service);
                let snap_path = btrfs::create_service_snapshot(&service, tag.as_deref(), readonly)?;
                println!(
                    "{}",
                    green.apply_to(format!(
                        "[OK] Snapshot created at: {:?} (readonly: {})",
                        snap_path, readonly
                    ))
                );
            }
            SnapCommands::Branch { service, suffix, port } => {
                let green = Style::new().green().bold();
                let branch_name = format!("{}_{}", service, suffix);
                println!("Branching service '{}' into '{}'...", service, branch_name);

                // Check parent service details
                let store = StateStore::load().unwrap_or_default();
                let parent_state = store.get_service(&service).cloned();

                let (catalog, image, default_port, container_mount, env_vars, cpus, memory, network) = match parent_state {
                    Some(s) => {
                        let c_mount = s.catalog.default_data_dir().to_string();
                        let env: Vec<(String, String)> = s.env.into_iter().collect();
                        (s.catalog, s.image, s.container_port, c_mount, env, s.cpus, s.memory, s.network)
                    }
                    None => {
                        // Inspect container
                        let insp = podman::inspect_service(&service)?
                            .context(format!("Parent service '{}' not found in podman or state store", service))?;
                        let c_mount = insp.catalog.default_data_dir().to_string();
                        let env = insp.catalog.default_env(None, insp.container_port);
                        (insp.catalog, insp.image, insp.container_port, c_mount, env, insp.cpus, insp.memory, insp.network)
                    }
                };

                // Branch Btrfs volume
                let branch_vol = btrfs::branch_service_volume(&service, &branch_name)?;

                // Find free port for branch container
                let branch_port = net::find_free_port(port, default_port + 100)?;

                // Run branch container instance
                let cid = podman::run_service_container(RunContainerParams {
                    name: &branch_name,
                    catalog,
                    image: &image,
                    host_port: branch_port,
                    container_port: default_port,
                    volume_path: &branch_vol,
                    container_mount: &container_mount,
                    env: &env_vars,
                    cpus: cpus.as_deref(),
                    memory: memory.as_deref(),
                    network: network.as_deref(),
                    tag: Some(&format!("branch_of_{}", service)),
                    branch_parent: Some(&service),
                })?;

                println!(
                    "{}",
                    green.apply_to(format!(
                        "[OK] Cloned branch '{}' ready on port {}! (ID: {})",
                        branch_name,
                        branch_port,
                        &cid[..12.min(cid.len())]
                    ))
                );
            }
            SnapCommands::Restore { service, snapshot } => {
                let green = Style::new().green().bold();
                println!("Restoring service '{}' from snapshot '{}'...", service, snapshot);

                // Stop service if running
                let was_running = podman::inspect_service(&service)?
                    .map(|s| s.status.to_lowercase().contains("running"))
                    .unwrap_or(false);

                if was_running {
                    println!("    Stopping service '{}' before restore...", service);
                    let _ = podman::stop_service(&service);
                }

                // Restore snapshot
                let restored_path = btrfs::restore_service_snapshot(&service, &snapshot)?;

                if was_running {
                    println!("    Restarting service '{}'...", service);
                    let _ = podman::start_service(&service);
                }

                println!(
                    "{}",
                    green.apply_to(format!("[OK] Service '{}' restored from snapshot at {:?}.", service, restored_path))
                );
            }
            SnapCommands::Merge { source, target } => {
                let green = Style::new().green().bold();
                println!("Merging branch service '{}' into target service '{}'...", source, target);

                // Stop target service if running
                let target_was_running = podman::inspect_service(&target)?
                    .map(|s| s.status.to_lowercase().contains("running"))
                    .unwrap_or(false);

                if target_was_running {
                    println!("    Stopping target service '{}' before merge...", target);
                    let _ = podman::stop_service(&target);
                }

                // Execute volume merge
                btrfs::merge_service_volumes(&source, &target)?;

                if target_was_running {
                    println!("    Restarting target service '{}'...", target);
                    let _ = podman::start_service(&target);
                }

                println!(
                    "{}",
                    green.apply_to(format!("[OK] Successfully merged '{}' into '{}' with safety snapshot created.", source, target))
                );
            }
            SnapCommands::Export { service, destination } => {
                let green = Style::new().green().bold();
                println!("Exporting snapshot archive for service '{}'...", service);
                let tarball = btrfs::export_service_snapshot(&service, destination.as_deref())?;
                println!(
                    "{}",
                    green.apply_to(format!("[OK] Snapshot exported successfully to: {:?}", tarball))
                );
            }
            SnapCommands::List { service, json } => {
                let snapshots = btrfs::list_service_snapshots(&service)?;

                if json {
                    let json_str = serde_json::to_string_pretty(&snapshots)
                        .context("Failed to serialize snapshots to JSON")?;
                    println!("{}", json_str);
                    return Ok(());
                }

                if snapshots.is_empty() {
                    println!("No snapshots found for service '{}'.", service);
                    return Ok(());
                }

                let header_style = Style::new().bold().underlined();
                let cyan = Style::new().cyan();

                println!(
                    "{:<25} {:<10} {:<12} {:<24} {:<30}",
                    header_style.apply_to("SNAPSHOT TAG"),
                    header_style.apply_to("MODE"),
                    header_style.apply_to("SIZE"),
                    header_style.apply_to("CREATED AT"),
                    header_style.apply_to("PATH")
                );

                for snap in snapshots {
                    let mode = if snap.readonly { "ro" } else { "rw" };
                    let size_mb = format!("{:.2} MB", snap.size_bytes as f64 / 1_048_576.0);
                    let created = snap.created_at.unwrap_or_else(|| "-".to_string());

                    println!(
                        "{:<25} {:<10} {:<12} {:<24} {:<30}",
                        cyan.apply_to(&snap.tag),
                        mode,
                        size_mb,
                        created,
                        snap.path.display()
                    );
                }
            }
        }
        Ok(())
    }

    /// Handle net subcommands
    pub async fn net(cmd: NetCommands) -> Result<()> {
        match cmd {
            NetCommands::Create { name, tag } => {
                let green = Style::new().green().bold();
                println!("Creating bridge network '{}'...", name);
                let created_name = net::create_bridge_network(&name, tag.as_deref())?;
                println!("{}", green.apply_to(format!("[OK] Bridge network '{}' created.", created_name)));
            }
            NetCommands::List { json } => {
                let networks = net::list_bridge_networks()?;

                if json {
                    let json_str = serde_json::to_string_pretty(&networks)
                        .context("Failed to serialize networks to JSON")?;
                    println!("{}", json_str);
                    return Ok(());
                }

                if networks.is_empty() {
                    println!("No d4c bridge networks found. Use 'd4c net create <name>' to create one.");
                    return Ok(());
                }

                let header_style = Style::new().bold().underlined();
                let cyan = Style::new().cyan();

                println!(
                    "{:<22} {:<18} {:<16} {:<14} {:<12}",
                    header_style.apply_to("NETWORK NAME"),
                    header_style.apply_to("SUBNET"),
                    header_style.apply_to("GATEWAY"),
                    header_style.apply_to("TAG"),
                    header_style.apply_to("CONTAINERS")
                );

                for n in networks {
                    let subnet = n.subnet.unwrap_or_else(|| "-".to_string());
                    let gateway = n.gateway.unwrap_or_else(|| "-".to_string());
                    let tag = n.tag.unwrap_or_else(|| "-".to_string());
                    let count_str = format!("{} active", n.containers_count);

                    println!(
                        "{:<22} {:<18} {:<16} {:<14} {:<12}",
                        cyan.apply_to(&n.name),
                        subnet,
                        gateway,
                        tag,
                        count_str
                    );
                }
            }
            NetCommands::Rm { name } => {
                let yellow = Style::new().yellow().bold();
                println!("Removing bridge network '{}'...", name);
                net::remove_bridge_network(&name)?;
                println!("{}", yellow.apply_to(format!("[OK] Bridge network '{}' removed.", name)));
            }
            NetCommands::Attach { service, network } => {
                let green = Style::new().green().bold();
                println!("Attaching container '{}' to network '{}'...", service, network);
                net::attach_container(&service, &network)?;
                println!("{}", green.apply_to(format!("[OK] Container '{}' attached to '{}'.", service, network)));
            }
            NetCommands::Detach { service, network } => {
                let yellow = Style::new().yellow().bold();
                println!("Detaching container '{}' from network '{}'...", service, network);
                net::detach_container(&service, &network)?;
                println!("{}", yellow.apply_to(format!("[OK] Container '{}' detached from '{}'.", service, network)));
            }
            NetCommands::Panorama { network } => {
                net::panorama_logs(&network).await?;
            }
        }
        Ok(())
    }

    /// Export active grid topology to a docker-compose.yml file
    pub fn compose(output: PathBuf, mode: &str) -> Result<()> {
        let green = Style::new().green().bold();
        let services = podman::list_services()?;

        if services.is_empty() {
            println!("No active services to export.");
            return Ok(());
        }

        let is_portable = mode.to_lowercase() == "portable";
        let mut compose_content = String::new();
        compose_content.push_str("version: '3.8'\n\nservices:\n");

        let mut named_volumes = Vec::new();
        let mut custom_networks = Vec::new();

        for s in &services {
            compose_content.push_str(&format!("  {}:\n", s.name));
            compose_content.push_str(&format!("    image: {}\n", s.image));
            compose_content.push_str(&format!("    container_name: {}\n", s.name));
            compose_content.push_str("    restart: unless-stopped\n");

            if let Some(port) = s.host_port {
                compose_content.push_str("    ports:\n");
                compose_content.push_str(&format!("      - \"{}:{}\"\n", port, s.container_port));
            }

            compose_content.push_str("    volumes:\n");
            let container_mount = s.catalog.default_data_dir();
            if is_portable {
                let vol_name = format!("{}_data", s.name);
                compose_content.push_str(&format!("      - {}:{}\n", vol_name, container_mount));
                named_volumes.push(vol_name);
            } else {
                compose_content.push_str(&format!("      - {}:{}\n", s.volume_path.display(), container_mount));
            }

            if let Some(net) = &s.network {
                compose_content.push_str("    networks:\n");
                compose_content.push_str(&format!("      - {}\n", net));
                if !custom_networks.contains(net) {
                    custom_networks.push(net.clone());
                }
            }

            if s.cpus.is_some() || s.memory.is_some() {
                compose_content.push_str("    deploy:\n      resources:\n        limits:\n");
                if let Some(c) = &s.cpus {
                    compose_content.push_str(&format!("          cpus: '{}'\n", c));
                }
                if let Some(m) = &s.memory {
                    compose_content.push_str(&format!("          memory: {}\n", m));
                }
            }

            compose_content.push('\n');
        }

        if !custom_networks.is_empty() {
            compose_content.push_str("networks:\n");
            for net in custom_networks {
                compose_content.push_str(&format!("  {}:\n    driver: bridge\n", net));
            }
            compose_content.push('\n');
        }

        if is_portable && !named_volumes.is_empty() {
            compose_content.push_str("volumes:\n");
            for vol in named_volumes {
                compose_content.push_str(&format!("  {}:\n", vol));
            }
        }

        fs::write(&output, compose_content)
            .with_context(|| format!("Failed to write docker-compose file to {:?}", output))?;

        println!(
            "{}",
            green.apply_to(format!("[OK] Exported docker-compose spec to {:?} (mode: {})", output, mode))
        );
        Ok(())
    }

    /// Search containers and bridge networks by tag, name, port, or subnet
    pub fn search(query: &str) -> Result<()> {
        let q = query.to_lowercase();
        let cyan = Style::new().cyan();
        let bold = Style::new().bold();

        println!("Searching for '{}' across containers and networks...\n", query);

        let services = podman::list_services()?;
        let networks = net::list_bridge_networks()?;

        let mut matched_services = Vec::new();
        for s in services {
            let port_str = s.host_port.map(|p| p.to_string()).unwrap_or_default();
            let tag_str = s.tag.clone().unwrap_or_default();
            let net_str = s.network.clone().unwrap_or_default();

            if s.name.to_lowercase().contains(&q)
                || s.catalog.to_string().contains(&q)
                || s.image.to_lowercase().contains(&q)
                || port_str.contains(&q)
                || tag_str.to_lowercase().contains(&q)
                || net_str.to_lowercase().contains(&q)
            {
                matched_services.push(s);
            }
        }

        let mut matched_networks = Vec::new();
        for n in networks {
            let tag_str = n.tag.clone().unwrap_or_default();
            let sub_str = n.subnet.clone().unwrap_or_default();
            if n.name.to_lowercase().contains(&q)
                || tag_str.to_lowercase().contains(&q)
                || sub_str.contains(&q)
            {
                matched_networks.push(n);
            }
        }

        if matched_services.is_empty() && matched_networks.is_empty() {
            println!("No results found matching query '{}'.", query);
            return Ok(());
        }

        if !matched_services.is_empty() {
            println!("{}", bold.apply_to(format!("Found {} matching container(s):", matched_services.len())));
            for s in matched_services {
                let port = s.host_port.map(|p| p.to_string()).unwrap_or_else(|| "-".to_string());
                let tag = s.tag.unwrap_or_else(|| "-".to_string());
                println!("  * {} [catalog: {}, port: {}, tag: {}, status: {}]",
                    cyan.apply_to(&s.name), s.catalog, port, tag, s.status
                );
            }
            println!();
        }

        if !matched_networks.is_empty() {
            println!("{}", bold.apply_to(format!("Found {} matching network(s):", matched_networks.len())));
            for n in matched_networks {
                let sub = n.subnet.unwrap_or_else(|| "-".to_string());
                println!("  * {} [subnet: {}, containers: {}]",
                    cyan.apply_to(&n.name), sub, n.containers_count
                );
            }
        }

        Ok(())
    }

    /// Launch UI: Quickshell graphical window by default, or terminal dashboard if --tui
    pub async fn ui(tui: bool) -> Result<()> {
        if !tui {
            // Find quickshell path
            let qs_locations = [
                PathBuf::from("./quickshell/shell.qml"),
                dirs_or_env_var().unwrap_or_default().join("d4c").join("quickshell").join("shell.qml"),
                PathBuf::from("/home/mepper/Projects/d4c/quickshell/shell.qml"),
            ];

            let found_qml = qs_locations.iter().find(|p| p.exists());

            if let Some(qml_path) = found_qml {
                let which_qs = Command::new("which").arg("quickshell").output();
                let has_quickshell = which_qs.is_ok_and(|out| out.status.success());

                if has_quickshell {
                    println!("[INFO] Launching QuickShell dashboard ({:?})...", qml_path);
                    let mut cmd = Command::new("quickshell");
                    if let Ok(exe) = std::env::current_exe() {
                        cmd.env("D4C_BIN", exe);
                    }
                    let status = cmd
                        .args(["-p", &qml_path.to_string_lossy()])
                        .status();

                    match status {
                        Ok(s) if s.success() => return Ok(()),
                        Ok(_) => {
                            eprintln!("[WARN] QuickShell exited with non-zero status. Falling back to terminal mode.");
                        }
                        Err(e) => {
                            eprintln!("[WARN] Failed to spawn QuickShell: {}. Falling back to terminal mode.", e);
                        }
                    }
                } else {
                    eprintln!("[WARN] 'quickshell' binary not found on PATH. Falling back to terminal mode.");
                }
            } else {
                eprintln!("[WARN] quickshell/shell.qml not found. Falling back to terminal mode.");
            }
        }

        Self::tui().await
    }

    /// Clean text-only terminal UI dashboard
    pub async fn tui() -> Result<()> {
        let bold = Style::new().bold();

        loop {
            println!("\n{}", bold.apply_to("=== d4c: Podman & Btrfs Orchestrator ==="));
            println!("Select an action:");

            let options = &[
                "[1] Service Dashboard (Ps)",
                "[2] Provision New Service (Up)",
                "[3] Start Service",
                "[4] Stop Service",
                "[5] Branch Service (Atomic CoW clone)",
                "[6] Create Volume Snapshot",
                "[7] Restore Snapshot",
                "[8] Merge Branch into Target",
                "[9] Export Snapshot Archive",
                "[10] Bridge Networks",
                "[11] View Logs",
                "[12] Remove / Wipe Service",
                "[0] Exit",
            ];

            let selection = Select::new()
                .items(options)
                .default(0)
                .interact()?;

            match selection {
                0 => {
                    Self::ps(false)?;
                }
                1 => {
                    let catalogs = &[
                        "Postgres",
                        "Mysql",
                        "Mongo",
                        "Redis",
                        "Kafka",
                        "Rabbitmq",
                        "Custom",
                    ];
                    let cat_idx = Select::new()
                        .with_prompt("Select service catalog")
                        .items(catalogs)
                        .default(0)
                        .interact()?;

                    let cat = match cat_idx {
                        0 => ServiceCatalog::Postgres,
                        1 => ServiceCatalog::Mysql,
                        2 => ServiceCatalog::Mongo,
                        3 => ServiceCatalog::Redis,
                        4 => ServiceCatalog::Kafka,
                        5 => ServiceCatalog::Rabbitmq,
                        _ => ServiceCatalog::Custom,
                    };

                    let default_name = cat.to_string();
                    let name: String = Input::new()
                        .with_prompt("Service name")
                        .default(default_name)
                        .interact_text()?;

                    let tag_input: String = Input::new()
                        .with_prompt("Tag metadata (optional, press Enter to skip)")
                        .allow_empty(true)
                        .interact_text()?;
                    let tag = if tag_input.trim().is_empty() { None } else { Some(tag_input.trim().to_string()) };

                    Self::up(cat, None, Some(name), None, tag, None, None, None, None)?;
                }
                2 => {
                    let services = podman::list_services()?;
                    if services.is_empty() {
                        println!("No services available.");
                        continue;
                    }
                    let names: Vec<String> = services.into_iter().map(|s| s.name).collect();
                    let idx = Select::new()
                        .with_prompt("Select service to start")
                        .items(&names)
                        .default(0)
                        .interact()?;
                    Self::start(&names[idx])?;
                }
                3 => {
                    let services = podman::list_services()?;
                    if services.is_empty() {
                        println!("No services available.");
                        continue;
                    }
                    let names: Vec<String> = services.into_iter().map(|s| s.name).collect();
                    let idx = Select::new()
                        .with_prompt("Select service to stop")
                        .items(&names)
                        .default(0)
                        .interact()?;
                    Self::stop(&names[idx])?;
                }
                4 => {
                    let services = podman::list_services()?;
                    if services.is_empty() {
                        println!("No services available.");
                        continue;
                    }
                    let names: Vec<String> = services.into_iter().map(|s| s.name).collect();
                    let idx = Select::new()
                        .with_prompt("Select service to branch")
                        .items(&names)
                        .default(0)
                        .interact()?;

                    let suffix: String = Input::new()
                        .with_prompt("Branch suffix (e.g., 'dev', 'feat')")
                        .interact_text()?;

                    Self::snap(SnapCommands::Branch {
                        service: names[idx].clone(),
                        suffix,
                        port: None,
                    })?;
                }
                5 => {
                    let services = podman::list_services()?;
                    if services.is_empty() {
                        println!("No services available.");
                        continue;
                    }
                    let names: Vec<String> = services.into_iter().map(|s| s.name).collect();
                    let idx = Select::new()
                        .with_prompt("Select service to snapshot")
                        .items(&names)
                        .default(0)
                        .interact()?;

                    let tag_input: String = Input::new()
                        .with_prompt("Snapshot tag (optional, press Enter for timestamp)")
                        .allow_empty(true)
                        .interact_text()?;
                    let tag = if tag_input.trim().is_empty() { None } else { Some(tag_input.trim().to_string()) };

                    Self::snap(SnapCommands::Create {
                        service: names[idx].clone(),
                        tag,
                        readonly: true,
                    })?;
                }
                6 => {
                    let services = podman::list_services()?;
                    if services.is_empty() {
                        println!("No services available.");
                        continue;
                    }
                    let names: Vec<String> = services.into_iter().map(|s| s.name).collect();
                    let idx = Select::new()
                        .with_prompt("Select service to restore")
                        .items(&names)
                        .default(0)
                        .interact()?;

                    let snapshots = btrfs::list_service_snapshots(&names[idx])?;
                    if snapshots.is_empty() {
                        println!("No snapshots found for '{}'.", names[idx]);
                        continue;
                    }

                    let snap_tags: Vec<String> = snapshots.into_iter().map(|s| s.tag).collect();
                    let snap_idx = Select::new()
                        .with_prompt("Select snapshot to restore from")
                        .items(&snap_tags)
                        .default(0)
                        .interact()?;

                    Self::snap(SnapCommands::Restore {
                        service: names[idx].clone(),
                        snapshot: snap_tags[snap_idx].clone(),
                    })?;
                }
                7 => {
                    let services = podman::list_services()?;
                    if services.len() < 2 {
                        println!("You need at least 2 services (source branch and target) to perform a merge.");
                        continue;
                    }
                    let names: Vec<String> = services.into_iter().map(|s| s.name).collect();
                    let src_idx = Select::new()
                        .with_prompt("Select SOURCE branch service")
                        .items(&names)
                        .default(0)
                        .interact()?;

                    let target_names: Vec<String> = names.iter().filter(|n| *n != &names[src_idx]).cloned().collect();
                    let tgt_idx = Select::new()
                        .with_prompt("Select TARGET service to receive changes")
                        .items(&target_names)
                        .default(0)
                        .interact()?;

                    Self::snap(SnapCommands::Merge {
                        source: names[src_idx].clone(),
                        target: target_names[tgt_idx].clone(),
                    })?;
                }
                8 => {
                    let services = podman::list_services()?;
                    if services.is_empty() {
                        println!("No services available.");
                        continue;
                    }
                    let names: Vec<String> = services.into_iter().map(|s| s.name).collect();
                    let idx = Select::new()
                        .with_prompt("Select service to export")
                        .items(&names)
                        .default(0)
                        .interact()?;

                    Self::snap(SnapCommands::Export {
                        service: names[idx].clone(),
                        destination: None,
                    })?;
                }
                9 => {
                    Self::net(NetCommands::List { json: false }).await?;
                }
                10 => {
                    let services = podman::list_services()?;
                    if services.is_empty() {
                        println!("No services available.");
                        continue;
                    }
                    let names: Vec<String> = services.into_iter().map(|s| s.name).collect();
                    let idx = Select::new()
                        .with_prompt("Select service to view logs")
                        .items(&names)
                        .default(0)
                        .interact()?;

                    Self::logs(&names[idx], false, 50)?;
                }
                11 => {
                    let services = podman::list_services()?;
                    if services.is_empty() {
                        println!("No services available.");
                        continue;
                    }
                    let names: Vec<String> = services.into_iter().map(|s| s.name).collect();
                    let idx = Select::new()
                        .with_prompt("Select service to wipe")
                        .items(&names)
                        .default(0)
                        .interact()?;

                    Self::wipe(&names[idx], false)?;
                }
                _ => {
                    println!("Goodbye!");
                    break;
                }
            }
        }

        Ok(())
    }
}

fn dirs_or_env_var() -> Option<PathBuf> {
    if let Ok(xdg_data) = std::env::var("XDG_DATA_HOME") {
        Some(PathBuf::from(xdg_data))
    } else {
        std::env::var("HOME").ok().map(|home| PathBuf::from(home).join(".local").join("share"))
    }
}
