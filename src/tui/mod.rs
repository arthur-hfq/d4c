pub mod app;
pub mod ui;

use anyhow::Result;
use app::{App, Modal, PendingAction, Tab};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io::stdout;
use std::time::{Duration, Instant};

/// Launch the interactive full-screen Ratatui terminal UI
pub async fn run() -> Result<()> {
    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(out);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();

    let res = run_loop(&mut terminal, &mut app).await;

    // Restore terminal state
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    res
}

async fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
) -> Result<()> {
    let tick_rate = Duration::from_millis(100);
    let mut last_tick = Instant::now();
    let mut was_modal = false;

    loop {
        let is_modal = !matches!(app.modal, Modal::None);
        if was_modal && !is_modal {
            terminal.clear()?;
        }
        was_modal = is_modal;

        terminal.draw(|f| ui::draw(f, app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_millis(0));

        if event::poll(timeout)?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            handle_key_event(app, key.code, key.modifiers);
        }

        if last_tick.elapsed() >= tick_rate {
            app.clear_toast_if_expired();

            // Auto-refresh states every 4 seconds if no modal is active
            if matches!(app.modal, Modal::None) && app.last_refresh.elapsed() >= Duration::from_secs(4) {
                app.refresh_all();
            }

            last_tick = Instant::now();
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

fn handle_key_event(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
    // Global Ctrl+C handler
    if modifiers.contains(KeyModifiers::CONTROL) && code == KeyCode::Char('c') {
        app.should_quit = true;
        return;
    }

    // Modal Active: route keys to modal
    if !matches!(app.modal, Modal::None) {
        handle_modal_keys(app, code);
        return;
    }

    // Deploy Tab typing handler
    if app.tab == Tab::Deploy && handle_deploy_keys(app, code) {
        return;
    }

    // Search Tab typing handler
    if app.tab == Tab::Search && handle_search_keys(app, code) {
        return;
    }

    // General navigation keys
    match code {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char('1') => app.switch_tab(Tab::Services),
        KeyCode::Char('2') => app.switch_tab(Tab::Networks),
        KeyCode::Char('3') => app.switch_tab(Tab::Snapshots),
        KeyCode::Char('4') => app.switch_tab(Tab::Panorama),
        KeyCode::Char('5') => app.switch_tab(Tab::Deploy),
        KeyCode::Char('6') => app.switch_tab(Tab::Search),
        KeyCode::Char('7') | KeyCode::Char('?') => app.switch_tab(Tab::Help),

        KeyCode::Tab => app.next_tab(),
        KeyCode::BackTab => app.prev_tab(),
        KeyCode::Left | KeyCode::Char('h') => app.prev_tab(),
        KeyCode::Right | KeyCode::Char('l') => app.next_tab(),

        KeyCode::Down | KeyCode::Char('j') => app.select_next_item(),
        KeyCode::Up | KeyCode::Char('k') => app.select_prev_item(),

        KeyCode::Char('r') => {
            app.refresh_all();
            app.set_toast("[OK] Refreshed containers & volumes", false);
        }

        _ => handle_tab_specific_keys(app, code),
    }
}

fn handle_tab_specific_keys(app: &mut App, code: KeyCode) {
    match app.tab {
        Tab::Services => match code {
            KeyCode::Char('s') => {
                if let Some(svc) = app.selected_service() {
                    let name = svc.name.clone();
                    let res = crate::podman::start_service(&name);
                    match res {
                        Ok(_) => app.set_toast(&format!("[OK] Service '{}' started", name), false),
                        Err(e) => app.set_toast(&format!("[ERROR] Start failed: {}", e), true),
                    }
                    app.refresh_all();
                }
            }
            KeyCode::Char('x') => {
                if let Some(svc) = app.selected_service() {
                    let name = svc.name.clone();
                    let res = crate::podman::stop_service(&name);
                    match res {
                        Ok(_) => app.set_toast(&format!("[OK] Service '{}' stopped", name), false),
                        Err(e) => app.set_toast(&format!("[ERROR] Stop failed: {}", e), true),
                    }
                    app.refresh_all();
                }
            }
            KeyCode::Char('c') => app.open_branch_modal(),
            KeyCode::Char('u') => app.open_tune_limits_modal(),
            KeyCode::Char('m') => app.open_merge_modal(),
            KeyCode::Char('t') => app.open_tag_modal(false),
            KeyCode::Char('w') => app.open_wipe_modal(),
            KeyCode::Char('n') => app.open_network_attach_modal(),
            KeyCode::Char('X') => {
                if let Some(svc) = app.selected_service() {
                    let name = svc.name.clone();
                    app.modal = Modal::ConfirmAction {
                        title: " Detach from All Networks ".to_string(),
                        prompt: format!("Disconnect container '{}' from all bridge networks?", name),
                        action: PendingAction::DetachAllNetworks(name),
                    };
                }
            }
            KeyCode::Char('b') => {
                if let Some(svc) = app.selected_service() {
                    let name = svc.name.clone();
                    app.modal = Modal::ConfirmAction {
                        title: " Backup Service Volume ".to_string(),
                        prompt: format!("Create atomic Btrfs snapshot and export as tar.gz archive for '{}'?", name),
                        action: PendingAction::BackupService(name),
                    };
                }
            }
            KeyCode::Char('d') => {
                if let Some(svc) = app.selected_service() {
                    let name = svc.name.clone();
                    app.modal = Modal::ConfirmAction {
                        title: " Delete Container ".to_string(),
                        prompt: format!("Delete container '{}'? Btrfs volume data will be PRESERVED.", name),
                        action: PendingAction::DeleteService(name),
                    };
                }
            }
            _ => {}
        },
        Tab::Networks => match code {
            KeyCode::Char('c') => app.open_create_network_modal(),
            KeyCode::Char('d') => {
                if let Some(net) = app.selected_network() {
                    let name = net.name.clone();
                    app.modal = Modal::ConfirmAction {
                        title: " Delete Bridge Network ".to_string(),
                        prompt: format!("Remove network '{}'? Ensure no running services depend on it.", name),
                        action: PendingAction::DeleteNetwork(name),
                    };
                }
            }
            KeyCode::Char('t') => app.open_tag_modal(true),
            KeyCode::Char('p') => {
                app.tab = Tab::Panorama;
                app.refresh_panorama_logs();
            }
            KeyCode::Char('a') => app.open_network_attach_container_modal(),
            KeyCode::Char('x') => app.open_network_detach_container_modal(),
            _ => {}
        },
        Tab::Snapshots => match code {
            KeyCode::Char('b') => {
                if let Some(snap) = app.selected_snapshot() {
                    let next_free = crate::net::find_free_port(None, 5433).unwrap_or(5433);
                    app.modal = Modal::Branch {
                        service: snap.service.clone(),
                        suffix: format!("{}_branch", snap.tag),
                        port: next_free.to_string(),
                        cpus: String::new(),
                        mem: String::new(),
                        field_idx: 0,
                    };
                }
            }
            KeyCode::Char('r') => {
                if let Some(snap) = app.selected_snapshot() {
                    let s_name = snap.service.clone();
                    let s_tag = snap.tag.clone();
                    app.modal = Modal::ConfirmAction {
                        title: " Restore Snapshot ".to_string(),
                        prompt: format!("Restore service '{}' to snapshot '{}'? Current active state will be replaced.", s_name, s_tag),
                        action: PendingAction::RestoreSnapshot { service: s_name, snapshot: s_tag },
                    };
                }
            }
            KeyCode::Char('e') => {
                if let Some(snap) = app.selected_snapshot() {
                    let name = snap.service.clone();
                    app.execute_pending_action(PendingAction::BackupService(name));
                }
            }
            KeyCode::Char('d') => {
                if let Some(snap) = app.selected_snapshot() {
                    let s_name = snap.service.clone();
                    let s_tag = snap.tag.clone();
                    app.modal = Modal::ConfirmAction {
                        title: " Delete Snapshot ".to_string(),
                        prompt: format!("Delete snapshot '{}' for service '{}'?", s_tag, s_name),
                        action: PendingAction::DeleteSnapshot { service: s_name, tag: s_tag },
                    };
                }
            }
            _ => {}
        },
        Tab::Panorama => match code {
            KeyCode::Char('n') => {
                if !app.networks.is_empty() {
                    app.selected_network_idx = (app.selected_network_idx + 1) % app.networks.len();
                    app.refresh_panorama_logs();
                }
            }
            KeyCode::Char('c') => {
                app.panorama_logs.clear();
            }
            _ => {}
        },
        _ => {}
    }
}

fn handle_deploy_keys(app: &mut App, code: KeyCode) -> bool {
    let form = &mut app.deploy_form;

    match code {
        KeyCode::Char(c @ '1'..='7') => {
            let idx = (c as usize) - ('1' as usize);
            form.catalog_idx = idx;
            app.update_deploy_defaults();
            true
        }
        KeyCode::Tab | KeyCode::Down => {
            form.field_idx = (form.field_idx + 1) % 8;
            true
        }
        KeyCode::BackTab | KeyCode::Up => {
            form.field_idx = if form.field_idx == 0 { 7 } else { form.field_idx - 1 };
            true
        }
        KeyCode::Char('n') if form.field_idx == 7 => {
            // Cycle network selection
            form.network_idx = (form.network_idx + 1) % (app.networks.len() + 1);
            true
        }
        KeyCode::Enter => {
            app.submit_deploy_form();
            true
        }
        KeyCode::Backspace => {
            match form.field_idx {
                0 => { form.name_suffix.pop(); }
                1 => { form.host_port.pop(); }
                2 => { form.password.pop(); }
                3 => { form.tag.pop(); }
                4 => { form.cpus.pop(); }
                5 => { form.memory.pop(); }
                6 => { form.custom_image.pop(); }
                _ => {}
            }
            true
        }
        KeyCode::Char(ch) => {
            match form.field_idx {
                0 => form.name_suffix.push(ch),
                1 => form.host_port.push(ch),
                2 => form.password.push(ch),
                3 => form.tag.push(ch),
                4 => form.cpus.push(ch),
                5 => form.memory.push(ch),
                6 => form.custom_image.push(ch),
                _ => {}
            }
            true
        }
        _ => false,
    }
}

fn handle_search_keys(app: &mut App, code: KeyCode) -> bool {
    match code {
        KeyCode::Esc => {
            app.search_query.clear();
            app.tab = Tab::Services;
            true
        }
        KeyCode::Backspace => {
            app.search_query.pop();
            true
        }
        KeyCode::Char(ch) => {
            app.search_query.push(ch);
            true
        }
        _ => false,
    }
}

fn handle_modal_keys(app: &mut App, code: KeyCode) {
    match &mut app.modal {
        Modal::None => {}
        Modal::Branch { suffix, port, cpus, mem, field_idx, .. } => {
            match code {
                KeyCode::Esc => app.modal = Modal::None,
                KeyCode::Enter => app.submit_branch_modal(),
                KeyCode::Tab | KeyCode::Down => *field_idx = (*field_idx + 1) % 4,
                KeyCode::BackTab | KeyCode::Up => *field_idx = if *field_idx == 0 { 3 } else { *field_idx - 1 },
                KeyCode::Backspace => {
                    match *field_idx {
                        0 => { suffix.pop(); }
                        1 => { port.pop(); }
                        2 => { cpus.pop(); }
                        3 => { mem.pop(); }
                        _ => {}
                    }
                }
                KeyCode::Char(ch) => {
                    match *field_idx {
                        0 => suffix.push(ch),
                        1 => port.push(ch),
                        2 => cpus.push(ch),
                        3 => mem.push(ch),
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        Modal::TuneLimits { cpus, mem, field_idx, .. } => {
            match code {
                KeyCode::Esc => app.modal = Modal::None,
                KeyCode::Enter => app.submit_tune_limits_modal(),
                KeyCode::Tab | KeyCode::Down => *field_idx = (*field_idx + 1) % 2,
                KeyCode::BackTab | KeyCode::Up => *field_idx = if *field_idx == 0 { 1 } else { *field_idx - 1 },
                KeyCode::Backspace => {
                    if *field_idx == 0 { cpus.pop(); } else { mem.pop(); }
                }
                KeyCode::Char(ch) => {
                    if *field_idx == 0 { cpus.push(ch); } else { mem.push(ch); }
                }
                _ => {}
            }
        }
        Modal::Merge { target_idx, targets, .. } => {
            match code {
                KeyCode::Esc => app.modal = Modal::None,
                KeyCode::Enter => app.submit_merge_modal(),
                KeyCode::Down | KeyCode::Tab if !targets.is_empty() => {
                    *target_idx = (*target_idx + 1) % targets.len();
                }
                KeyCode::Up | KeyCode::BackTab if !targets.is_empty() => {
                    *target_idx = if *target_idx == 0 { targets.len() - 1 } else { *target_idx - 1 };
                }
                _ => {}
            }
        }
        Modal::Tag { tag_input, .. } => {
            match code {
                KeyCode::Esc => app.modal = Modal::None,
                KeyCode::Enter => app.submit_tag_modal(),
                KeyCode::Backspace => { tag_input.pop(); }
                KeyCode::Char(ch) => tag_input.push(ch),
                _ => {}
            }
        }
        Modal::Wipe { confirm_input, .. } => {
            match code {
                KeyCode::Esc => app.modal = Modal::None,
                KeyCode::Enter => app.submit_wipe_modal(),
                KeyCode::Backspace => { confirm_input.pop(); }
                KeyCode::Char(ch) => confirm_input.push(ch),
                _ => {}
            }
        }
        Modal::NetworkAttach { network_idx, networks, .. } => {
            match code {
                KeyCode::Esc => app.modal = Modal::None,
                KeyCode::Enter => app.submit_network_attach_modal(),
                KeyCode::Down | KeyCode::Tab if !networks.is_empty() => {
                    *network_idx = (*network_idx + 1) % networks.len();
                }
                KeyCode::Up | KeyCode::BackTab if !networks.is_empty() => {
                    *network_idx = if *network_idx == 0 { networks.len() - 1 } else { *network_idx - 1 };
                }
                _ => {}
            }
        }
        Modal::NetworkAttachContainer { container_idx, containers, .. } => {
            match code {
                KeyCode::Esc => app.modal = Modal::None,
                KeyCode::Enter => app.submit_network_attach_container_modal(),
                KeyCode::Down | KeyCode::Tab if !containers.is_empty() => {
                    *container_idx = (*container_idx + 1) % containers.len();
                }
                KeyCode::Up | KeyCode::BackTab if !containers.is_empty() => {
                    *container_idx = if *container_idx == 0 { containers.len() - 1 } else { *container_idx - 1 };
                }
                _ => {}
            }
        }
        Modal::NetworkDetachContainer { container_idx, containers, .. } => {
            match code {
                KeyCode::Esc => app.modal = Modal::None,
                KeyCode::Enter => app.submit_network_detach_container_modal(),
                KeyCode::Down | KeyCode::Tab if !containers.is_empty() => {
                    *container_idx = (*container_idx + 1) % containers.len();
                }
                KeyCode::Up | KeyCode::BackTab if !containers.is_empty() => {
                    *container_idx = if *container_idx == 0 { containers.len() - 1 } else { *container_idx - 1 };
                }
                _ => {}
            }
        }
        Modal::CreateNetwork { name_suffix, tag, subnet, field_idx } => {
            match code {
                KeyCode::Esc => app.modal = Modal::None,
                KeyCode::Enter => app.submit_create_network_modal(),
                KeyCode::Tab | KeyCode::Down => *field_idx = (*field_idx + 1) % 3,
                KeyCode::BackTab | KeyCode::Up => *field_idx = if *field_idx == 0 { 2 } else { *field_idx - 1 },
                KeyCode::Backspace => {
                    match *field_idx {
                        0 => { name_suffix.pop(); }
                        1 => { tag.pop(); }
                        2 => { subnet.pop(); }
                        _ => {}
                    }
                }
                KeyCode::Char(ch) => {
                    match *field_idx {
                        0 => name_suffix.push(ch),
                        1 => tag.push(ch),
                        2 => subnet.push(ch),
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        Modal::ConfirmAction { action, .. } => {
            match code {
                KeyCode::Esc => app.modal = Modal::None,
                KeyCode::Enter => {
                    let act = action.clone();
                    app.execute_pending_action(act);
                }
                _ => {}
            }
        }
        Modal::Message { .. } => {
            if matches!(code, KeyCode::Esc | KeyCode::Enter | KeyCode::Char(' ')) {
                app.modal = Modal::None;
            }
        }
    }
}
