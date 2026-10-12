use crate::cli::ServiceCatalog;
use crate::model::{NetworkInfo, ServiceInfo, SnapshotInfo, StateStore};
use crate::net;
use crate::podman;
use std::collections::HashMap;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Services,
    Networks,
    Snapshots,
    Panorama,
    Deploy,
    Search,
    Help,
}

impl Tab {
    pub fn all() -> &'static [Tab] {
        &[
            Tab::Services,
            Tab::Networks,
            Tab::Snapshots,
            Tab::Panorama,
            Tab::Deploy,
            Tab::Search,
            Tab::Help,
        ]
    }

    pub fn title(&self) -> &'static str {
        match self {
            Tab::Services => "\u{f233}  Services",
            Tab::Networks => "\u{f0e8}  Networks",
            Tab::Snapshots => "\u{f0c2}  Snapshots",
            Tab::Panorama => "\u{f06e}  Panorama",
            Tab::Deploy => "\u{f067}  Deploy",
            Tab::Search => "\u{f002}  Search",
            Tab::Help => "\u{f059}  Help",
        }
    }
}

#[derive(Debug, Clone)]
pub enum PendingAction {
    DeleteService(String),
    DeleteNetwork(String),
    DeleteSnapshot { service: String, tag: String },
    BackupService(String),
    RestoreSnapshot { service: String, snapshot: String },
    DetachNetwork { service: String, network: String },
    DetachAllNetworks(String),
}

#[derive(Debug, Clone)]
pub enum Modal {
    None,
    Branch {
        service: String,
        suffix: String,
        port: String,
        cpus: String,
        mem: String,
        field_idx: usize,
    },
    TuneLimits {
        service: String,
        cpus: String,
        mem: String,
        field_idx: usize,
    },
    Merge {
        source: String,
        target_idx: usize,
        targets: Vec<String>,
    },
    Tag {
        target: String,
        is_network: bool,
        tag_input: String,
    },
    Wipe {
        service: String,
        confirm_input: String,
    },
    NetworkAttach {
        service: String,
        network_idx: usize,
        networks: Vec<String>,
    },
    NetworkAttachContainer {
        network: String,
        container_idx: usize,
        containers: Vec<String>,
    },
    NetworkDetachContainer {
        network: String,
        container_idx: usize,
        containers: Vec<String>,
    },
    CreateNetwork {
        name_suffix: String,
        tag: String,
        subnet: String,
        field_idx: usize,
    },
    ConfirmAction {
        title: String,
        prompt: String,
        action: PendingAction,
    },
    Message {
        title: String,
        message: String,
        is_error: bool,
    },
}

#[derive(Debug, Clone)]
pub struct DeployForm {
    pub catalog_idx: usize,
    pub custom_image: String,
    pub name_suffix: String,
    pub host_port: String,
    pub password: String,
    pub tag: String,
    pub cpus: String,
    pub memory: String,
    pub network_idx: usize,
    pub field_idx: usize,
}

impl Default for DeployForm {
    fn default() -> Self {
        Self {
            catalog_idx: 0,
            custom_image: String::new(),
            name_suffix: "postgres".to_string(),
            host_port: "5432".to_string(),
            password: "password".to_string(),
            tag: String::new(),
            cpus: String::new(),
            memory: String::new(),
            network_idx: 0,
            field_idx: 0,
        }
    }
}

pub struct App {
    pub tab: Tab,
    pub services: Vec<ServiceInfo>,
    pub networks: Vec<NetworkInfo>,
    pub snapshots: Vec<SnapshotInfo>,
    pub recent_logs: Vec<String>,
    pub panorama_logs: Vec<(String, String)>,

    // Cached container IP mapping (container_name -> ip_address)
    pub container_ips: HashMap<String, String>,

    // List selections
    pub selected_service_idx: usize,
    pub selected_network_idx: usize,
    pub selected_snapshot_idx: usize,
    pub logs_scroll: usize,

    // Deploy form
    pub deploy_form: DeployForm,

    // Search query
    pub search_query: String,

    // Modals
    pub modal: Modal,

    // Toast notifications
    pub toast: Option<(String, bool)>, // (message, is_error)
    pub toast_time: Option<Instant>,

    // Auto-refresh control
    pub last_refresh: Instant,
    pub should_quit: bool,
}

impl App {
    pub fn new() -> Self {
        let mut app = Self {
            tab: Tab::Services,
            services: Vec::new(),
            networks: Vec::new(),
            snapshots: Vec::new(),
            recent_logs: Vec::new(),
            panorama_logs: Vec::new(),
            container_ips: HashMap::new(),
            selected_service_idx: 0,
            selected_network_idx: 0,
            selected_snapshot_idx: 0,
            logs_scroll: 0,
            deploy_form: DeployForm::default(),
            search_query: String::new(),
            modal: Modal::None,
            toast: None,
            toast_time: None,
            last_refresh: Instant::now(),
            should_quit: false,
        };

        app.refresh_all();
        app.update_deploy_defaults();
        app
    }

    /// Refresh all background data from Podman and Btrfs
    pub fn refresh_all(&mut self) {
        self.last_refresh = Instant::now();

        // 1. Refresh services
        if let Ok(svcs) = podman::list_services() {
            self.services = svcs;
            if self.selected_service_idx >= self.services.len() && !self.services.is_empty() {
                self.selected_service_idx = self.services.len() - 1;
            }
        }

        // 2. Refresh networks
        if let Ok(nets) = net::list_bridge_networks() {
            self.networks = nets;
            if self.selected_network_idx >= self.networks.len() && !self.networks.is_empty() {
                self.selected_network_idx = self.networks.len() - 1;
            }
        }

        // 3. Refresh snapshots
        if let Ok(snaps) = crate::btrfs::list_all_snapshots() {
            self.snapshots = snaps;
            if self.selected_snapshot_idx >= self.snapshots.len() && !self.snapshots.is_empty() {
                self.selected_snapshot_idx = self.snapshots.len() - 1;
            }
        }

        // 4. Refresh selected service logs and IPs
        self.refresh_selected_service_details();

        // 5. Refresh panorama logs if active
        if self.tab == Tab::Panorama {
            self.refresh_panorama_logs();
        }
    }

    /// Refresh details and recent logs for the currently selected service
    pub fn refresh_selected_service_details(&mut self) {
        if let Some(svc) = self.selected_service() {
            let name = svc.name.clone();
            self.recent_logs = podman::get_recent_logs(&name, 20);

            let ips = podman::get_container_network_ips(&name);
            for (net, ip) in ips {
                self.container_ips.insert(format!("{}:{}", name, net), ip);
            }
        } else {
            self.recent_logs.clear();
        }
    }

    /// Refresh panorama logs for selected network
    pub fn refresh_panorama_logs(&mut self) {
        if let Some(net_info) = self.selected_network() {
            let logs = net::get_network_recent_logs(&net_info.name, 15);
            self.panorama_logs = logs;
        } else if !self.networks.is_empty() {
            let first_net = self.networks[0].name.clone();
            self.panorama_logs = net::get_network_recent_logs(&first_net, 15);
        } else {
            self.panorama_logs.clear();
        }
    }

    pub fn selected_service(&self) -> Option<&ServiceInfo> {
        self.services.get(self.selected_service_idx)
    }

    pub fn selected_network(&self) -> Option<&NetworkInfo> {
        self.networks.get(self.selected_network_idx)
    }

    pub fn selected_snapshot(&self) -> Option<&SnapshotInfo> {
        self.snapshots.get(self.selected_snapshot_idx)
    }

    pub fn set_toast(&mut self, message: &str, is_error: bool) {
        self.toast = Some((message.to_string(), is_error));
        self.toast_time = Some(Instant::now());
    }

    pub fn clear_toast_if_expired(&mut self) {
        if let Some(t) = self.toast_time
            && t.elapsed().as_secs() > 4
        {
            self.toast = None;
            self.toast_time = None;
        }
    }

    /// Update deploy form default ports and names based on catalog choice
    pub fn update_deploy_defaults(&mut self) {
        let catalogs = [
            ServiceCatalog::Postgres,
            ServiceCatalog::Mysql,
            ServiceCatalog::Mongo,
            ServiceCatalog::Redis,
            ServiceCatalog::Kafka,
            ServiceCatalog::Rabbitmq,
            ServiceCatalog::Custom,
        ];

        let cat = catalogs[self.deploy_form.catalog_idx.min(catalogs.len() - 1)];
        let default_p = cat.default_port();
        let free_port = net::find_free_port(None, default_p).unwrap_or(default_p);

        self.deploy_form.host_port = free_port.to_string();
        self.deploy_form.name_suffix = format!("{}_{}", cat, free_port);
    }

    /// Cycle to next tab
    pub fn next_tab(&mut self) {
        let tabs = Tab::all();
        let idx = tabs.iter().position(|t| *t == self.tab).unwrap_or(0);
        self.tab = tabs[(idx + 1) % tabs.len()];
        self.on_tab_switched();
    }

    /// Cycle to previous tab
    pub fn prev_tab(&mut self) {
        let tabs = Tab::all();
        let idx = tabs.iter().position(|t| *t == self.tab).unwrap_or(0);
        let new_idx = if idx == 0 { tabs.len() - 1 } else { idx - 1 };
        self.tab = tabs[new_idx];
        self.on_tab_switched();
    }

    pub fn switch_tab(&mut self, tab: Tab) {
        self.tab = tab;
        self.on_tab_switched();
    }

    fn on_tab_switched(&mut self) {
        match self.tab {
            Tab::Services => self.refresh_selected_service_details(),
            Tab::Panorama => self.refresh_panorama_logs(),
            _ => {}
        }
    }

    pub fn select_next_item(&mut self) {
        match self.tab {
            Tab::Services => {
                if !self.services.is_empty() {
                    self.selected_service_idx = (self.selected_service_idx + 1) % self.services.len();
                    self.refresh_selected_service_details();
                }
            }
            Tab::Networks => {
                if !self.networks.is_empty() {
                    self.selected_network_idx = (self.selected_network_idx + 1) % self.networks.len();
                    self.refresh_panorama_logs();
                }
            }
            Tab::Snapshots => {
                if !self.snapshots.is_empty() {
                    self.selected_snapshot_idx = (self.selected_snapshot_idx + 1) % self.snapshots.len();
                }
            }
            Tab::Panorama => {
                self.logs_scroll = self.logs_scroll.saturating_add(1);
            }
            _ => {}
        }
    }

    pub fn select_prev_item(&mut self) {
        match self.tab {
            Tab::Services => {
                if !self.services.is_empty() {
                    self.selected_service_idx = if self.selected_service_idx == 0 {
                        self.services.len() - 1
                    } else {
                        self.selected_service_idx - 1
                    };
                    self.refresh_selected_service_details();
                }
            }
            Tab::Networks => {
                if !self.networks.is_empty() {
                    self.selected_network_idx = if self.selected_network_idx == 0 {
                        self.networks.len() - 1
                    } else {
                        self.selected_network_idx - 1
                    };
                    self.refresh_panorama_logs();
                }
            }
            Tab::Snapshots => {
                if !self.snapshots.is_empty() {
                    self.selected_snapshot_idx = if self.selected_snapshot_idx == 0 {
                        self.snapshots.len() - 1
                    } else {
                        self.selected_snapshot_idx - 1
                    };
                }
            }
            Tab::Panorama => {
                self.logs_scroll = self.logs_scroll.saturating_sub(1);
            }
            _ => {}
        }
    }

    // Modal launchers
    pub fn open_branch_modal(&mut self) {
        if let Some(svc) = self.selected_service() {
            let next_free = net::find_free_port(None, svc.host_port.unwrap_or(svc.container_port) + 1).unwrap_or(5435);
            self.modal = Modal::Branch {
                service: svc.name.clone(),
                suffix: "branch".to_string(),
                port: next_free.to_string(),
                cpus: svc.cpus.clone().unwrap_or_default(),
                mem: svc.memory.clone().unwrap_or_default(),
                field_idx: 0,
            };
        }
    }

    pub fn open_tune_limits_modal(&mut self) {
        if let Some(svc) = self.selected_service() {
            self.modal = Modal::TuneLimits {
                service: svc.name.clone(),
                cpus: svc.cpus.clone().unwrap_or_default(),
                mem: svc.memory.clone().unwrap_or_default(),
                field_idx: 0,
            };
        }
    }

    pub fn open_merge_modal(&mut self) {
        if let Some(svc) = self.selected_service() {
            let source = svc.name.clone();
            let targets: Vec<String> = self
                .services
                .iter()
                .filter(|s| s.name != source)
                .map(|s| s.name.clone())
                .collect();

            if targets.is_empty() {
                self.set_toast("No other services available to merge into", true);
                return;
            }

            self.modal = Modal::Merge {
                source,
                target_idx: 0,
                targets,
            };
        }
    }

    pub fn open_tag_modal(&mut self, is_network: bool) {
        if is_network {
            if let Some(net) = self.selected_network() {
                self.modal = Modal::Tag {
                    target: net.name.clone(),
                    is_network: true,
                    tag_input: net.tag.clone().unwrap_or_default(),
                };
            }
        } else if let Some(svc) = self.selected_service() {
            self.modal = Modal::Tag {
                target: svc.name.clone(),
                is_network: false,
                tag_input: svc.tag.clone().unwrap_or_default(),
            };
        }
    }

    pub fn open_wipe_modal(&mut self) {
        if let Some(svc) = self.selected_service() {
            self.modal = Modal::Wipe {
                service: svc.name.clone(),
                confirm_input: String::new(),
            };
        }
    }

    pub fn open_network_attach_modal(&mut self) {
        if let Some(svc) = self.selected_service() {
            let nets: Vec<String> = self.networks.iter().map(|n| n.name.clone()).collect();
            if nets.is_empty() {
                self.set_toast("No custom networks available. Create one first [c] in Networks tab", true);
                return;
            }
            self.modal = Modal::NetworkAttach {
                service: svc.name.clone(),
                network_idx: 0,
                networks: nets,
            };
        }
    }

    pub fn open_network_attach_container_modal(&mut self) {
        if let Some(net) = self.selected_network() {
            let net_name = net.name.clone();
            let all_services: Vec<String> = self.services.iter().map(|s| s.name.clone()).collect();
            if all_services.is_empty() {
                self.set_toast("No services available to attach to network", true);
                return;
            }
            self.modal = Modal::NetworkAttachContainer {
                network: net_name,
                container_idx: 0,
                containers: all_services,
            };
        }
    }

    pub fn open_network_detach_container_modal(&mut self) {
        if let Some(net) = self.selected_network() {
            let net_name = net.name.clone();
            let attached_containers = net.containers.clone();
            if attached_containers.is_empty() {
                self.set_toast(&format!("No containers currently attached to network '{}'", net_name), true);
                return;
            }
            self.modal = Modal::NetworkDetachContainer {
                network: net_name,
                container_idx: 0,
                containers: attached_containers,
            };
        }
    }

    pub fn open_create_network_modal(&mut self) {
        self.modal = Modal::CreateNetwork {
            name_suffix: "backend".to_string(),
            tag: String::new(),
            subnet: String::new(),
            field_idx: 0,
        };
    }

    // Modal submit handlers
    pub fn submit_branch_modal(&mut self) {
        if let Modal::Branch { service, suffix, port, cpus, mem, .. } = &self.modal {
            let port_parsed = port.parse::<u16>().ok();
            let cpus_opt = if cpus.trim().is_empty() { None } else { Some(cpus.trim().to_string()) };
            let mem_opt = if mem.trim().is_empty() { None } else { Some(mem.trim().to_string()) };

            let res = crate::engine::Engine::branch_service(
                service,
                suffix,
                port_parsed,
                cpus_opt,
                mem_opt,
            );

            match res {
                Ok((b_name, _, b_port)) => {
                    self.set_toast(&format!("[OK] Branch '{}' created on port {}", b_name, b_port), false);
                    self.refresh_all();
                }
                Err(e) => {
                    self.set_toast(&format!("[ERROR] Branch failed: {}", e), true);
                }
            }
        }
        self.modal = Modal::None;
    }

    pub fn submit_tune_limits_modal(&mut self) {
        if let Modal::TuneLimits { service, cpus, mem, .. } = &self.modal {
            let cpus_opt = if cpus.trim().is_empty() { None } else { Some(cpus.trim().to_string()) };
            let mem_opt = if mem.trim().is_empty() { None } else { Some(mem.trim().to_string()) };

            let res = podman::update_service_resources(
                service,
                cpus_opt.as_deref(),
                mem_opt.as_deref(),
            );

            match res {
                Ok(_) => {
                    self.set_toast(&format!("[OK] Limits updated for '{}'", service), false);
                    self.refresh_all();
                }
                Err(e) => {
                    self.set_toast(&format!("[ERROR] Update limits failed: {}", e), true);
                }
            }
        }
        self.modal = Modal::None;
    }

    pub fn submit_merge_modal(&mut self) {
        if let Modal::Merge { source, targets, target_idx } = &self.modal
            && let Some(target) = targets.get(*target_idx)
        {
            let res = crate::engine::Engine::merge_service_branches(source, target);

            match res {
                Ok(_) => {
                    self.set_toast(&format!("[OK] Merged '{}' into '{}'", source, target), false);
                    self.refresh_all();
                }
                Err(e) => {
                    self.set_toast(&format!("[ERROR] Merge failed: {}", e), true);
                }
            }
        }
        self.modal = Modal::None;
    }

    pub fn submit_tag_modal(&mut self) {
        if let Modal::Tag { target, is_network, tag_input } = &self.modal {
            let tag_opt = if tag_input.trim().is_empty() { None } else { Some(tag_input.trim().to_string()) };

            if *is_network {
                let mut store = StateStore::load().unwrap_or_default();
                let _ = store.set_network_tag(target, tag_opt.clone());
                self.set_toast(&format!("[OK] Tag updated for network '{}'", target), false);
            } else {
                let mut store = StateStore::load().unwrap_or_default();
                let res = store.set_service_tag(target, tag_opt);
                match res {
                    Ok(_) => self.set_toast(&format!("[OK] Tag updated for service '{}'", target), false),
                    Err(e) => self.set_toast(&format!("[ERROR] Tag failed: {}", e), true),
                }
            }
            self.refresh_all();
        }
        self.modal = Modal::None;
    }

    pub fn submit_wipe_modal(&mut self) {
        if let Modal::Wipe { service, confirm_input } = &self.modal {
            if confirm_input.trim() == "DESTROY" {
                let res = crate::engine::Engine::wipe_service_storage(service);
                match res {
                    Ok(_) => {
                        self.set_toast(&format!("[OK] Service '{}' and data completely destroyed", service), false);
                        self.refresh_all();
                    }
                    Err(e) => {
                        self.set_toast(&format!("[ERROR] Wipe failed: {}", e), true);
                    }
                }
            } else {
                self.set_toast("Wipe aborted: Must type 'DESTROY' to confirm", true);
            }
        }
        self.modal = Modal::None;
    }

    pub fn submit_network_attach_modal(&mut self) {
        if let Modal::NetworkAttach { service, network_idx, networks } = &self.modal
            && let Some(net) = networks.get(*network_idx)
        {
            let res = net::attach_container(service, net);
            match res {
                Ok(_) => {
                    self.set_toast(&format!("[OK] Attached '{}' to network '{}'", service, net), false);
                    self.refresh_all();
                }
                Err(e) => {
                    self.set_toast(&format!("[ERROR] Attach failed: {}", e), true);
                }
            }
        }
        self.modal = Modal::None;
    }

    pub fn submit_network_attach_container_modal(&mut self) {
        if let Modal::NetworkAttachContainer { network, container_idx, containers } = &self.modal
            && let Some(c_name) = containers.get(*container_idx)
        {
            let res = net::attach_container(c_name, network);
            match res {
                Ok(_) => {
                    self.set_toast(&format!("[OK] Attached container '{}' to network '{}'", c_name, network), false);
                    self.refresh_all();
                }
                Err(e) => {
                    self.set_toast(&format!("[ERROR] Attach failed: {}", e), true);
                }
            }
        }
        self.modal = Modal::None;
    }

    pub fn submit_network_detach_container_modal(&mut self) {
        if let Modal::NetworkDetachContainer { network, container_idx, containers } = &self.modal
            && let Some(c_name) = containers.get(*container_idx)
        {
            let res = net::detach_container(c_name, network);
            match res {
                Ok(_) => {
                    self.set_toast(&format!("[OK] Detached container '{}' from network '{}'", c_name, network), false);
                    self.refresh_all();
                }
                Err(e) => {
                    self.set_toast(&format!("[ERROR] Detach failed: {}", e), true);
                }
            }
        }
        self.modal = Modal::None;
    }

    pub fn submit_create_network_modal(&mut self) {
        if let Modal::CreateNetwork { name_suffix, tag, subnet, .. } = &self.modal {
            let tag_opt = if tag.trim().is_empty() { None } else { Some(tag.trim()) };
            let sub_opt = if subnet.trim().is_empty() { None } else { Some(subnet.trim()) };

            let res = net::create_bridge_network(name_suffix, tag_opt, sub_opt);
            match res {
                Ok(net_name) => {
                    self.set_toast(&format!("[OK] Bridge network '{}' created", net_name), false);
                    self.refresh_all();
                }
                Err(e) => {
                    self.set_toast(&format!("[ERROR] Network create failed: {}", e), true);
                }
            }
        }
        self.modal = Modal::None;
    }

    pub fn execute_pending_action(&mut self, action: PendingAction) {
        match action {
            PendingAction::DeleteService(name) => {
                let res = podman::remove_container(&name, true);
                match res {
                    Ok(_) => self.set_toast(&format!("[OK] Container '{}' deleted (data preserved)", name), false),
                    Err(e) => self.set_toast(&format!("[ERROR] Delete failed: {}", e), true),
                }
            }
            PendingAction::DeleteNetwork(name) => {
                let res = net::remove_bridge_network(&name);
                match res {
                    Ok(_) => self.set_toast(&format!("[OK] Bridge network '{}' removed", name), false),
                    Err(e) => self.set_toast(&format!("[ERROR] Remove network failed: {}", e), true),
                }
            }
            PendingAction::DeleteSnapshot { service, tag } => {
                let res = crate::btrfs::delete_snapshot(&service, &tag);
                match res {
                    Ok(_) => self.set_toast(&format!("[OK] Snapshot '{}' deleted", tag), false),
                    Err(e) => self.set_toast(&format!("[ERROR] Delete snapshot failed: {}", e), true),
                }
            }
            PendingAction::BackupService(name) => {
                let res = crate::btrfs::export_service_snapshot(&name, None);
                match res {
                    Ok(path) => self.set_toast(&format!("[OK] Volume backup exported: {:?}", path), false),
                    Err(e) => self.set_toast(&format!("[ERROR] Backup export failed: {}", e), true),
                }
            }
            PendingAction::RestoreSnapshot { service, snapshot } => {
                let res = crate::engine::Engine::restore_service_snapshot(&service, &snapshot);
                match res {
                    Ok(_) => self.set_toast(&format!("[OK] Service '{}' restored to snapshot '{}'", service, snapshot), false),
                    Err(e) => self.set_toast(&format!("[ERROR] Restore failed: {}", e), true),
                }
            }
            PendingAction::DetachNetwork { service, network } => {
                let res = net::detach_container(&service, &network);
                match res {
                    Ok(_) => self.set_toast(&format!("[OK] Detached '{}' from network '{}'", service, network), false),
                    Err(e) => self.set_toast(&format!("[ERROR] Detach failed: {}", e), true),
                }
            }
            PendingAction::DetachAllNetworks(service) => {
                let res = net::disconnect_all_networks(&service);
                match res {
                    Ok(_) => self.set_toast(&format!("[OK] Detached '{}' from all networks", service), false),
                    Err(e) => self.set_toast(&format!("[ERROR] Detach all failed: {}", e), true),
                }
            }
        }
        self.modal = Modal::None;
        self.refresh_all();
    }

    /// Submit deploy form and launch new container
    pub fn submit_deploy_form(&mut self) {
        let catalogs = [
            ServiceCatalog::Postgres,
            ServiceCatalog::Mysql,
            ServiceCatalog::Mongo,
            ServiceCatalog::Redis,
            ServiceCatalog::Kafka,
            ServiceCatalog::Rabbitmq,
            ServiceCatalog::Custom,
        ];

        let cat = catalogs[self.deploy_form.catalog_idx.min(catalogs.len() - 1)];
        let port_parsed = self.deploy_form.host_port.trim().parse::<u16>().ok();
        let name_opt = if self.deploy_form.name_suffix.trim().is_empty() { None } else { Some(self.deploy_form.name_suffix.trim().to_string()) };
        let pass_opt = if self.deploy_form.password.trim().is_empty() { None } else { Some(self.deploy_form.password.trim().to_string()) };
        let tag_opt = if self.deploy_form.tag.trim().is_empty() { None } else { Some(self.deploy_form.tag.trim().to_string()) };
        let cpu_opt = if self.deploy_form.cpus.trim().is_empty() { None } else { Some(self.deploy_form.cpus.trim().to_string()) };
        let mem_opt = if self.deploy_form.memory.trim().is_empty() { None } else { Some(self.deploy_form.memory.trim().to_string()) };
        let img_opt = if self.deploy_form.custom_image.trim().is_empty() { None } else { Some(self.deploy_form.custom_image.trim().to_string()) };

        let net_opt = if self.deploy_form.network_idx > 0 && self.deploy_form.network_idx <= self.networks.len() {
            Some(self.networks[self.deploy_form.network_idx - 1].name.clone())
        } else {
            None
        };

        let res = crate::engine::Engine::provision_service(
            cat,
            port_parsed,
            name_opt,
            pass_opt,
            tag_opt,
            cpu_opt,
            mem_opt,
            net_opt,
            img_opt,
        );

        match res {
            Ok((svc_name, _)) => {
                self.set_toast(&format!("[SUCCESS] Service '{}' deployed and running", svc_name), false);
                self.tab = Tab::Services;
                self.refresh_all();
            }
            Err(e) => {
                self.set_toast(&format!("[ERROR] Deploy failed: {}", e), true);
            }
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
