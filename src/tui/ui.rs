use crate::tui::app::{App, Modal, Tab};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Cell, Clear, List, ListItem, Paragraph, Row, Table, Tabs, Wrap,
    },
};

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header & Tabs
            Constraint::Min(10),   // Content View
            Constraint::Length(2), // Footer Action Bar
        ])
        .split(f.area());

    draw_header(f, app, chunks[0]);

    match app.tab {
        Tab::Services => draw_services_tab(f, app, chunks[1]),
        Tab::Networks => draw_networks_tab(f, app, chunks[1]),
        Tab::Snapshots => draw_snapshots_tab(f, app, chunks[1]),
        Tab::Panorama => draw_panorama_tab(f, app, chunks[1]),
        Tab::Deploy => draw_deploy_tab(f, app, chunks[1]),
        Tab::Search => draw_search_tab(f, app, chunks[1]),
        Tab::Help => draw_help_tab(f, app, chunks[1]),
    }

    draw_footer(f, app, chunks[2]);

    if !matches!(app.modal, Modal::None) {
        draw_modal(f, app);
    }
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(38), Constraint::Min(20)])
        .split(area);

    // Title Block
    let title_line = Line::from(vec![
        Span::styled(" \u{f0e7} [D4C] ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled("NODE CONTROL ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::styled("v0.1 ", Style::default().fg(Color::DarkGray)),
    ]);

    let title_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray));
    f.render_widget(Paragraph::new(title_line).block(title_block), header_chunks[0]);

    // Tab Bar
    let tab_titles: Vec<Line> = Tab::all()
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let num = format!("[{}] ", i + 1);
            Line::from(vec![
                Span::styled(num, Style::default().fg(Color::DarkGray)),
                Span::raw(t.title()),
            ])
        })
        .collect();

    let active_idx = Tab::all().iter().position(|t| *t == app.tab).unwrap_or(0);

    let tabs = Tabs::new(tab_titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .select(active_idx)
        .style(Style::default().fg(Color::Gray))
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
                .add_modifier(Modifier::UNDERLINED),
        )
        .divider(Span::styled(" | ", Style::default().fg(Color::DarkGray)));

    f.render_widget(tabs, header_chunks[1]);
}

fn draw_services_tab(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);

    // Left: Services List Table
    let rows: Vec<Row> = app
        .services
        .iter()
        .enumerate()
        .map(|(idx, svc)| {
            let is_sel = idx == app.selected_service_idx;
            let status_icon = if svc.is_running() {
                Span::styled(" \u{f04b} ", Style::default().fg(Color::Green))
            } else {
                Span::styled(" \u{f04d} ", Style::default().fg(Color::Red))
            };

            let name_style = if is_sel {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let tag_text = svc.tag.as_deref().unwrap_or("-");
            let tag_style = if svc.tag.is_some() {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            let port_str = svc.host_port.map(|p| format!(":{}", p)).unwrap_or_else(|| "-".to_string());

            let row_style = if is_sel {
                Style::default().bg(Color::Rgb(26, 32, 48))
            } else {
                Style::default()
            };

            Row::new(vec![
                Cell::from(status_icon),
                Cell::from(Span::styled(&svc.name, name_style)),
                Cell::from(Span::styled(tag_text, tag_style)),
                Cell::from(Span::styled(port_str, Style::default().fg(Color::LightCyan))),
            ])
            .style(row_style)
        })
        .collect();

    let widths = [
        Constraint::Length(4),
        Constraint::Percentage(45),
        Constraint::Percentage(25),
        Constraint::Percentage(25),
    ];

    let header_row = Row::new(vec![
        Cell::from(""),
        Cell::from(Span::styled("SERVICE NAME", Style::default().fg(Color::Gray).add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("TAG", Style::default().fg(Color::Gray).add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("PORT", Style::default().fg(Color::Gray).add_modifier(Modifier::BOLD))),
    ])
    .style(Style::default().bg(Color::Rgb(15, 20, 30)));

    let empty_hint = if app.services.is_empty() {
        " [No services running. Press [5] Deploy to launch a service] "
    } else {
        " Managed Containers "
    };

    let table = Table::new(rows, widths)
        .header(header_row)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Cyan))
                .title(Span::styled(
                    format!(" \u{f233}{} (Total: {}) ", empty_hint, app.services.len()),
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                )),
        );

    f.render_widget(table, chunks[0]);

    // Right: Selected Service Inspector Panel
    if let Some(svc) = app.selected_service() {
        draw_service_detail_card(f, app, svc, chunks[1]);
    } else {
        let empty_card = Paragraph::new("\n\n  No service selected.")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::DarkGray))
                    .title(" Service Inspector "),
            );
        f.render_widget(empty_card, chunks[1]);
    }
}

fn draw_service_detail_card(f: &mut Frame, app: &App, svc: &crate::model::ServiceInfo, area: Rect) {
    let top_height = if svc.networks.len() > 1 {
        (9 + svc.networks.len() as u16).min(area.height.saturating_sub(4)).max(10)
    } else {
        10.min(area.height.saturating_sub(4)).max(8)
    };

    let card_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(top_height), Constraint::Min(4)])
        .split(area);

    let status_badge = if svc.is_running() {
        Span::styled(" \u{f04b} RUNNING ", Style::default().fg(Color::Black).bg(Color::Green).add_modifier(Modifier::BOLD))
    } else {
        Span::styled(" \u{f04d} STOPPED ", Style::default().fg(Color::White).bg(Color::Red).add_modifier(Modifier::BOLD))
    };

    let tag_span = if let Some(tag) = &svc.tag {
        Span::styled(format!("\u{f02b} {}", tag), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
    } else {
        Span::styled("-", Style::default().fg(Color::DarkGray))
    };

    let (cpus, mem) = svc.formatted_limits();
    let port_str = format!("{}:{}", svc.host_port.unwrap_or(svc.container_port), svc.container_port);

    let branch_info = if let Some(parent) = &svc.branch_parent {
        format!(" \u{f126} Branch of: {}", parent)
    } else {
        " Standard Service".to_string()
    };

    let conn_url = svc.connection_url();

    let get_ip = |net_name: &str| -> &str {
        let key = format!("{}:{}", svc.name, net_name);
        if let Some(ip) = app.container_ips.get(&key) {
            ip.as_str()
        } else if let Some(net) = app.networks.iter().find(|n| n.name == net_name) {
            net.container_ips.get(&svc.name).map(|s| s.as_str()).unwrap_or("-")
        } else {
            "-"
        }
    };

    let mut details_lines = vec![
        Line::from(vec![
            Span::styled(" \u{f013} ", Style::default().fg(Color::Cyan)),
            Span::styled(&svc.name, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::raw("   "),
            status_badge,
            Span::raw(" "),
            Span::styled(branch_info, Style::default().fg(Color::LightMagenta)),
        ]),
        Line::from(vec![
            Span::raw("  Catalog: "),
            Span::styled(svc.catalog.to_string(), Style::default().fg(Color::Cyan)),
            Span::raw("  | Image: "),
            Span::styled(&svc.image, Style::default().fg(Color::Gray)),
            Span::raw("  | ID: "),
            Span::styled(svc.short_id(), Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::raw("  Port Mapping: "),
            Span::styled(format!("\u{f0ec} {}", port_str), Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD)),
            Span::raw("   Tag: "),
            tag_span,
        ]),
        Line::from(vec![
            Span::raw("  Resources:    \u{f2db} CPU: "),
            Span::styled(cpus, Style::default().fg(Color::Yellow)),
            Span::raw("  |  \u{f080} RAM: "),
            Span::styled(mem, Style::default().fg(Color::Yellow)),
        ]),
        Line::from(vec![
            Span::raw("  Storage:      \u{f0a0} Subvolume: "),
            Span::styled(svc.volume_path.display().to_string(), Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled("  \u{f0c1} URL: ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled(conn_url, Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        ]),
    ];

    if svc.networks.len() > 1 {
        details_lines.push(Line::from(vec![
            Span::styled(format!("  \u{f0e8} Attached Networks ({}):", svc.networks.len()), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ]));
        for net_name in &svc.networks {
            let ip_str = get_ip(net_name);
            details_lines.push(Line::from(vec![
                Span::raw("    \u{f0da} "),
                Span::styled(format!("{:<20}", net_name), Style::default().fg(Color::LightCyan)),
                Span::raw("  IP: "),
                Span::styled(ip_str, Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD)),
            ]));
        }
    } else {
        let net_name = svc.networks.first().map(|s| s.as_str()).or(svc.network.as_deref()).unwrap_or("podman (default bridge)");
        let ip_str = get_ip(net_name);
        details_lines.push(Line::from(vec![
            Span::raw("  Networking:   \u{f0e8} Bridge: "),
            Span::styled(net_name, Style::default().fg(Color::Cyan)),
            Span::raw("  |  IP: "),
            Span::styled(ip_str, Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD)),
        ]));
    }

    let detail_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::White))
        .title(Span::styled(" \u{f013} Service Details & Metrics ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)));

    f.render_widget(Paragraph::new(details_lines).block(detail_block), card_chunks[0]);

    // Bottom of card: Recent Logs Preview
    let log_items: Vec<ListItem> = if app.recent_logs.is_empty() {
        vec![ListItem::new("  (No logs recorded yet or service is stopped)")]
    } else {
        app.recent_logs
            .iter()
            .map(|l| ListItem::new(Span::styled(l, Style::default().fg(Color::Gray))))
            .collect()
    };

    let logs_list = List::new(log_items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(Span::styled(
                format!(" \u{f06e} Live Logs Preview ({}) ", svc.name),
                Style::default().fg(Color::LightBlue),
            )),
    );

    f.render_widget(logs_list, card_chunks[1]);
}

fn draw_networks_tab(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    // Left: Bridge Networks List
    let items: Vec<ListItem> = app
        .networks
        .iter()
        .enumerate()
        .map(|(idx, net)| {
            let is_sel = idx == app.selected_network_idx;
            let name_style = if is_sel {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let tag_str = net.tag.as_deref().unwrap_or("-");
            let line = Line::from(vec![
                Span::styled(" \u{f0e8} ", Style::default().fg(Color::LightBlue)),
                Span::styled(&net.name, name_style),
                Span::raw("  "),
                Span::styled(format!("[{} containers]", net.containers_count), Style::default().fg(Color::DarkGray)),
                Span::raw("  "),
                Span::styled(format!("tag: {}", tag_str), Style::default().fg(Color::Yellow)),
            ]);

            let item = ListItem::new(line);
            if is_sel {
                item.style(Style::default().bg(Color::Rgb(26, 32, 48)))
            } else {
                item
            }
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Cyan))
            .title(Span::styled(
                format!(" \u{f0e8} Bridge Networks & Topologies ({}) ", app.networks.len()),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            )),
    );

    f.render_widget(list, chunks[0]);

    // Right: Network Inspector & Attached Containers with IP addresses
    if let Some(net) = app.selected_network() {
        let sub = net.subnet.as_deref().unwrap_or("auto");
        let gw = net.gateway.as_deref().unwrap_or("auto");
        let tag = net.tag.as_deref().unwrap_or("none");

        let mut lines = vec![
            Line::from(vec![
                Span::styled(" \u{f0e8} Network: ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                Span::styled(&net.name, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::raw("   Tag: "),
                Span::styled(tag, Style::default().fg(Color::Yellow)),
            ]),
            Line::from(vec![
                Span::raw("  Driver: bridge   |  Subnet: "),
                Span::styled(sub, Style::default().fg(Color::LightGreen)),
                Span::raw("   |  Gateway: "),
                Span::styled(gw, Style::default().fg(Color::LightGreen)),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                format!(" \u{f233} Attached Service Containers ({}):", net.containers.len()),
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            )),
            Line::from(" ─────────────────────────────────────────────────────────────"),
        ];

        if net.containers.is_empty() {
            lines.push(Line::from(Span::styled(
                "   (No containers currently attached to this bridge network)",
                Style::default().fg(Color::DarkGray),
            )));
        } else {
            for c in &net.containers {
                let ip = net.container_ips.get(c).map(|s| s.as_str()).unwrap_or("10.89.x.x");
                lines.push(Line::from(vec![
                    Span::styled("   \u{f0da} ", Style::default().fg(Color::Cyan)),
                    Span::styled(c, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                    Span::raw("  \u{f0ec} IP Address: "),
                    Span::styled(ip, Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD)),
                ]));
            }
        }

        let card = Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::White))
                .title(" Network Topology & IPAM "),
        );

        f.render_widget(card, chunks[1]);
    } else {
        let empty = Paragraph::new("\n\n  No network selected.")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::DarkGray))
                    .title(" Network Inspector "),
            );
        f.render_widget(empty, chunks[1]);
    }
}

fn draw_snapshots_tab(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let rows: Vec<Row> = app
        .snapshots
        .iter()
        .enumerate()
        .map(|(idx, snap)| {
            let is_sel = idx == app.selected_snapshot_idx;
            let name_style = if is_sel {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let lock_icon = if snap.readonly {
                Span::styled("\u{f023} ro", Style::default().fg(Color::Yellow))
            } else {
                Span::styled("\u{f09c} rw", Style::default().fg(Color::Green))
            };

            let size_kb = snap.size_bytes / 1024;
            let size_str = if size_kb > 1024 {
                format!("{:.1} MB", size_kb as f64 / 1024.0)
            } else {
                format!("{} KB", size_kb)
            };

            let row_style = if is_sel {
                Style::default().bg(Color::Rgb(26, 32, 48))
            } else {
                Style::default()
            };

            Row::new(vec![
                Cell::from(Span::styled(&snap.service, name_style)),
                Cell::from(Span::styled(&snap.tag, Style::default().fg(Color::Yellow))),
                Cell::from(lock_icon),
                Cell::from(Span::styled(size_str, Style::default().fg(Color::LightCyan))),
            ])
            .style(row_style)
        })
        .collect();

    let widths = [
        Constraint::Percentage(35),
        Constraint::Percentage(30),
        Constraint::Percentage(15),
        Constraint::Percentage(20),
    ];

    let header_row = Row::new(vec![
        Cell::from(Span::styled("SERVICE", Style::default().fg(Color::Gray).add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("SNAPSHOT TAG", Style::default().fg(Color::Gray).add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("MODE", Style::default().fg(Color::Gray).add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("SIZE", Style::default().fg(Color::Gray).add_modifier(Modifier::BOLD))),
    ])
    .style(Style::default().bg(Color::Rgb(15, 20, 30)));

    let empty_title = if app.snapshots.is_empty() {
        " [No snapshots taken yet. Press [b] in Services to snapshot] "
    } else {
        " Btrfs Subvolume Snapshots "
    };

    let table = Table::new(rows, widths)
        .header(header_row)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Cyan))
                .title(Span::styled(
                    format!(" \u{f0c2}  {} ({}) ", empty_title.trim(), app.snapshots.len()),
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                )),
        );

    f.render_widget(table, chunks[0]);

    // Right: Snapshot Inspector
    if let Some(snap) = app.selected_snapshot() {
        let created = snap.created_at.as_deref().unwrap_or("unknown");
        let lines = vec![
            Line::from(vec![
                Span::styled(" \u{f0c2}  Snapshot: ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                Span::styled(&snap.name, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::raw("  Parent Service: "),
                Span::styled(&snap.service, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                Span::raw("   |  Tag: "),
                Span::styled(&snap.tag, Style::default().fg(Color::Yellow)),
            ]),
            Line::from(vec![
                Span::raw("  Mode: "),
                if snap.readonly {
                    Span::styled("Read-Only (\u{f023})", Style::default().fg(Color::Yellow))
                } else {
                    Span::styled("Read-Write (\u{f09c})", Style::default().fg(Color::Green))
                },
                Span::raw("   |  Created: "),
                Span::styled(created, Style::default().fg(Color::Gray)),
            ]),
            Line::from(vec![
                Span::raw("  Path: "),
                Span::styled(snap.path.display().to_string(), Style::default().fg(Color::DarkGray)),
            ]),
            Line::from(""),
            Line::from(Span::styled(" \u{f0e7} CoW Branch Actions:", Style::default().fg(Color::White).add_modifier(Modifier::BOLD))),
            Line::from("   [b] Branch this snapshot into a new standalone container"),
            Line::from("   [r] Restore service volume to this snapshot"),
            Line::from("   [e] Export as compressed archive (.tar.gz)"),
            Line::from("   [d] Delete this snapshot subvolume"),
        ];

        let detail = Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::White))
                .title(" Snapshot Inspector & Actions "),
        );

        f.render_widget(detail, chunks[1]);
    } else {
        let empty = Paragraph::new("\n\n  No snapshot selected.")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::DarkGray))
                    .title(" Snapshot Inspector "),
            );
        f.render_widget(empty, chunks[1]);
    }
}

fn draw_panorama_tab(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(8)])
        .split(area);

    let net_name = app.selected_network().map(|n| n.name.as_str()).unwrap_or("all");

    let header_line = Line::from(vec![
        Span::styled(" \u{f06e} Panorama Streaming Network: ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::styled(net_name, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled("   (Aggregating live logs across all connected containers)", Style::default().fg(Color::DarkGray)),
    ]);

    let header_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan));
    f.render_widget(Paragraph::new(header_line).block(header_block), chunks[0]);

    let log_lines: Vec<ListItem> = if app.panorama_logs.is_empty() {
        vec![ListItem::new("  No logs captured yet for this network. Ensure containers are running on this bridge.")]
    } else {
        let colors = [Color::Cyan, Color::Green, Color::Yellow, Color::Magenta, Color::LightBlue];
        app.panorama_logs
            .iter()
            .enumerate()
            .map(|(i, (svc, log))| {
                let color = colors[i % colors.len()];
                let tag = format!("[{}] ", svc);
                let line = Line::from(vec![
                    Span::styled(tag, Style::default().fg(color).add_modifier(Modifier::BOLD)),
                    Span::styled(log, Style::default().fg(Color::Gray)),
                ]);
                ListItem::new(line)
            })
            .collect()
    };

    let list = List::new(log_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::White))
            .title(" Multi-Container Panorama Stream "),
    );

    f.render_widget(list, chunks[1]);
}

fn draw_deploy_tab(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(32), Constraint::Min(35)])
        .split(area);

    // Left: Service Catalog Presets
    let catalogs = [
        "1) PostgreSQL (16-alpine)",
        "2) MySQL (8.0)",
        "3) MongoDB (7.0)",
        "4) Redis (7-alpine)",
        "5) Kafka (KRaft)",
        "6) RabbitMQ (3-management)",
        "7) Custom Image...",
    ];

    let catalog_items: Vec<ListItem> = catalogs
        .iter()
        .enumerate()
        .map(|(i, cat)| {
            let is_sel = i == app.deploy_form.catalog_idx;
            let style = if is_sel {
                Style::default().fg(Color::Cyan).bg(Color::Rgb(26, 32, 48)).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(Span::styled(*cat, style))
        })
        .collect();

    let cat_list = List::new(catalog_items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Cyan))
            .title(Span::styled(" \u{f067} Catalog Presets [1-7] ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
    );

    f.render_widget(cat_list, chunks[0]);

    // Right: Deploy Form Fields
    let form = &app.deploy_form;
    let fields = [
        ("Container Name Suffix", &form.name_suffix),
        ("Host Port", &form.host_port),
        ("Password / Secret", &form.password),
        ("Project Tag (optional)", &form.tag),
        ("CPU Limit (e.g. 1.0)", &form.cpus),
        ("RAM Limit (e.g. 512m)", &form.memory),
        ("Custom Image (if custom)", &form.custom_image),
    ];

    let mut form_lines = vec![
        Line::from(Span::styled(" Configure and Launch New Container:", Style::default().fg(Color::White).add_modifier(Modifier::BOLD))),
        Line::from(" ─────────────────────────────────────────────────────────────"),
    ];

    for (i, (label, val)) in fields.iter().enumerate() {
        let is_focused = i == form.field_idx;
        let prefix = if is_focused { " ▸ " } else { "   " };
        let label_style = if is_focused {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        let val_display = if val.is_empty() { "<none>" } else { val.as_str() };
        let val_style = if is_focused {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        form_lines.push(Line::from(vec![
            Span::styled(prefix, Style::default().fg(Color::Cyan)),
            Span::styled(format!("{:<25}: ", label), label_style),
            Span::styled(val_display, val_style),
        ]));
    }

    // Network selector row
    let net_display = if form.network_idx == 0 {
        "Default Bridge (podman)".to_string()
    } else if form.network_idx <= app.networks.len() {
        app.networks[form.network_idx - 1].name.clone()
    } else {
        "Default".to_string()
    };

    let is_net_focused = form.field_idx == fields.len();
    let net_prefix = if is_net_focused { " ▸ " } else { "   " };
    form_lines.push(Line::from(vec![
        Span::styled(net_prefix, Style::default().fg(Color::Cyan)),
        Span::styled(format!("{:<25}: ", "Bridge Network [n to cycle]"), if is_net_focused { Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::Gray) }),
        Span::styled(net_display, Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD)),
    ]));

    form_lines.push(Line::from(""));
    form_lines.push(Line::from(vec![
        Span::styled("  [ Press ENTER to Deploy & Start Container ]", Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled("   (Tab/Down: Next field | BackTab/Up: Prev field)", Style::default().fg(Color::DarkGray)),
    ]));

    let form_paragraph = Paragraph::new(form_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::White))
            .title(" Provisioning Parameters "),
    );

    f.render_widget(form_paragraph, chunks[1]);
}

fn draw_search_tab(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(8)])
        .split(area);

    let query_line = Line::from(vec![
        Span::styled(" \u{f002} Keyword Query: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled(&app.search_query, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::styled("_", Style::default().fg(Color::White)),
        Span::styled("  (Type to search across services, tags, networks, ports)", Style::default().fg(Color::DarkGray)),
    ]);

    let search_bar = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan));
    f.render_widget(Paragraph::new(query_line).block(search_bar), chunks[0]);

    // Matching Results
    let q = app.search_query.to_lowercase();
    let mut results = Vec::new();

    results.push(Line::from(Span::styled(" Matching Services:", Style::default().fg(Color::White).add_modifier(Modifier::BOLD))));
    let mut svc_count = 0;
    for s in &app.services {
        let tag_match = s.tag.as_deref().unwrap_or("").to_lowercase().contains(&q);
        let name_match = s.name.to_lowercase().contains(&q);
        let port_match = s.host_port.map(|p| p.to_string()).unwrap_or_default().contains(&q);

        if q.is_empty() || tag_match || name_match || port_match {
            svc_count += 1;
            results.push(Line::from(vec![
                Span::styled("   \u{f233} ", Style::default().fg(Color::Cyan)),
                Span::styled(&s.name, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                Span::raw("  [Tag: "),
                Span::styled(s.tag.as_deref().unwrap_or("-"), Style::default().fg(Color::Yellow)),
                Span::raw("]  [Port: "),
                Span::styled(s.host_port.map(|p| p.to_string()).unwrap_or_else(|| "-".to_string()), Style::default().fg(Color::LightCyan)),
                Span::raw("]  Status: "),
                Span::styled(&s.status, if s.is_running() { Style::default().fg(Color::Green) } else { Style::default().fg(Color::Red) }),
            ]));
        }
    }
    if svc_count == 0 {
        results.push(Line::from(Span::styled("   (No matching services found)", Style::default().fg(Color::DarkGray))));
    }

    results.push(Line::from(""));
    results.push(Line::from(Span::styled(" Matching Bridge Networks:", Style::default().fg(Color::White).add_modifier(Modifier::BOLD))));
    let mut net_count = 0;
    for n in &app.networks {
        let name_match = n.name.to_lowercase().contains(&q);
        let tag_match = n.tag.as_deref().unwrap_or("").to_lowercase().contains(&q);
        let sub_match = n.subnet.as_deref().unwrap_or("").to_lowercase().contains(&q);

        if q.is_empty() || name_match || tag_match || sub_match {
            net_count += 1;
            results.push(Line::from(vec![
                Span::styled("   \u{f0e8} ", Style::default().fg(Color::LightBlue)),
                Span::styled(&n.name, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                Span::raw("  [Tag: "),
                Span::styled(n.tag.as_deref().unwrap_or("-"), Style::default().fg(Color::Yellow)),
                Span::raw("]  [Subnet: "),
                Span::styled(n.subnet.as_deref().unwrap_or("-"), Style::default().fg(Color::LightGreen)),
                Span::raw(format!("]  [{} containers]", n.containers_count)),
            ]));
        }
    }
    if net_count == 0 {
        results.push(Line::from(Span::styled("   (No matching networks found)", Style::default().fg(Color::DarkGray))));
    }

    let results_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::White))
        .title(" Filtered Search Results ");

    f.render_widget(Paragraph::new(results).block(results_block).wrap(Wrap { trim: false }), chunks[1]);
}

fn draw_help_tab(f: &mut Frame, _app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let left_lines = vec![
        Line::from(Span::styled(" GLOBAL CONTROLS & NAVIGATION:", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::from("  [1..7] Switch views       [Tab] Next tab"),
        Line::from("  [↑/↓/j/k] Move cursor     [r] Refresh states"),
        Line::from("  [q / Esc] Quit D4C or close active modal"),
        Line::from(""),
        Line::from(Span::styled(" SERVICE ACTIONS [Tab 1 - Services]:", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::from("  [s] Start / Restart       [x] Stop container"),
        Line::from("  [c] Branch via Btrfs CoW  [m] Merge branch"),
        Line::from("  [u] Tune CPU & RAM limits [t] Edit tag"),
        Line::from("  [b] Backup volume tar.gz  [n] Attach to bridge"),
        Line::from("  [d] Delete container      [X] Detach all nets"),
        Line::from("  [w] Wipe (DESTROY container & volume)"),
    ];

    let right_lines = vec![
        Line::from(Span::styled(" NETWORKS & SNAPSHOTS [Tabs 2 & 3]:", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::from("  [c] Create bridge network [d] Delete bridge / snapshot"),
        Line::from("  [a] Attach container      [x] Detach container"),
        Line::from("  [t] Edit network tag      [p] Panorama log stream"),
        Line::from("  [b] Branch snapshot       [r] Restore volume"),
        Line::from("  [e] Export snapshot tar.gz"),
        Line::from(""),
        Line::from(Span::styled(" FONT AWESOME ICONS LEGEND:", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::from("  \u{f233} Container   \u{f0e8} Bridge     \u{f0c2}  Snapshot   \u{f126} Branch"),
        Line::from("  \u{f2db} CPU Core    \u{f080} RAM Quota   \u{f0a0} Storage     \u{f0ec} Port"),
        Line::from("  \u{f04b} Running     \u{f04d} Stopped     \u{f02b} Tag         \u{f0c1} URL"),
    ];

    let left_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan))
        .title(Span::styled(" \u{f059} [D4C] Keyboard Controls ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));

    let right_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::White))
        .title(Span::styled(" Topology & Icons Cheatsheet ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)));

    f.render_widget(Paragraph::new(left_lines).block(left_block), chunks[0]);
    f.render_widget(Paragraph::new(right_lines).block(right_block), chunks[1]);
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(40), Constraint::Length(45)])
        .split(area);

    let hints = match app.tab {
        Tab::Services => "[s]Start [x]Stop [c]Branch [m]Merge [u]Limits [t]Tag [b]Backup [w]Wipe [n]Bridge [q]Quit",
        Tab::Networks => "[c]Create Net [d]Delete Net [a]Attach [x]Detach [t]Tag [p]Panorama [q]Quit",
        Tab::Snapshots => "[b]Branch [r]Restore [e]Export .tar.gz [d]Delete Snapshot [q]Quit",
        Tab::Panorama => "[Up/Down]Scroll [n]Select Net [Space]Pause Stream [q]Quit",
        Tab::Deploy => "[1-7]Catalog [Tab/Down]Next [Enter]Deploy & Start [q]Quit",
        Tab::Search => "[Type]Filter query [Esc]Clear query [q]Quit",
        Tab::Help => "[Tab]Switch View [q]Quit D4C",
    };

    let hints_p = Paragraph::new(Span::styled(hints, Style::default().fg(Color::DarkGray)));
    f.render_widget(hints_p, chunks[0]);

    // Toast status notification
    if let Some((msg, is_err)) = &app.toast {
        let style = if *is_err {
            Style::default().fg(Color::White).bg(Color::Red).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Black).bg(Color::Green).add_modifier(Modifier::BOLD)
        };
        let toast_p = Paragraph::new(Span::styled(format!(" {} ", msg), style)).alignment(Alignment::Right);
        f.render_widget(toast_p, chunks[1]);
    } else {
        f.render_widget(Paragraph::new(""), chunks[1]);
    }
}

fn draw_modal(f: &mut Frame, app: &App) {
    let area = centered_rect(65, 50, f.area());
    f.render_widget(Clear, area);

    match &app.modal {
        Modal::Branch { service, suffix, port, cpus, mem, field_idx } => {
            let fields = [
                ("Branch Suffix (e.g. dev)", suffix.as_str()),
                ("New Host Port", port.as_str()),
                ("CPU Limit (optional)", cpus.as_str()),
                ("RAM Limit (optional)", mem.as_str()),
            ];

            let mut lines = vec![
                Line::from(vec![
                    Span::raw(" Branch service: "),
                    Span::styled(service, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                ]),
                Line::from(" Creates an atomic Btrfs CoW snapshot and launches cloned container."),
                Line::from(" ─────────────────────────────────────────────────────────────"),
            ];

            for (i, (lbl, val)) in fields.iter().enumerate() {
                let is_foc = i == *field_idx;
                let prefix = if is_foc { " ▸ " } else { "   " };
                let lbl_style = if is_foc { Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::Gray) };
                let val_style = if is_foc { Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::White) };
                lines.push(Line::from(vec![
                    Span::styled(prefix, Style::default().fg(Color::Cyan)),
                    Span::styled(format!("{:<25}: ", lbl), lbl_style),
                    Span::styled(*val, val_style),
                ]));
            }

            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled(" [ Enter: Create Branch ] ", Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::raw("   "),
                Span::styled(" [ Esc: Cancel ] ", Style::default().fg(Color::White).bg(Color::DarkGray)),
            ]));

            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Cyan))
                .style(Style::default().bg(Color::Rgb(15, 18, 26)))
                .title(" \u{f126}  Branch Service via Btrfs CoW ");

            f.render_widget(Paragraph::new(lines).block(block), area);
        }
        Modal::TuneLimits { service, cpus, mem, field_idx } => {
            let fields = [
                ("CPU Quota (e.g. 1.0, 0.5)", cpus.as_str()),
                ("Memory Allocation (e.g. 512m, 2g)", mem.as_str()),
            ];

            let mut lines = vec![
                Line::from(vec![
                    Span::raw(" Live update resource quotas for: "),
                    Span::styled(service, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                ]),
                Line::from(" Applies immediately to the running container via podman update."),
                Line::from(" ─────────────────────────────────────────────────────────────"),
            ];

            for (i, (lbl, val)) in fields.iter().enumerate() {
                let is_foc = i == *field_idx;
                let prefix = if is_foc { " ▸ " } else { "   " };
                lines.push(Line::from(vec![
                    Span::styled(prefix, Style::default().fg(Color::Cyan)),
                    Span::styled(format!("{:<32}: ", lbl), if is_foc { Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::Gray) }),
                    Span::styled(*val, if is_foc { Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::White) }),
                ]));
            }

            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled(" [ Enter: Update Limits ] ", Style::default().fg(Color::Black).bg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::raw("   "),
                Span::styled(" [ Esc: Cancel ] ", Style::default().fg(Color::White).bg(Color::DarkGray)),
            ]));

            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Yellow))
                .style(Style::default().bg(Color::Rgb(15, 18, 26)))
                .title(" \u{f2db}  Tune CPU & Memory Limits ");

            f.render_widget(Paragraph::new(lines).block(block), area);
        }
        Modal::Merge { source, target_idx, targets } => {
            let mut lines = vec![
                Line::from(vec![
                    Span::raw(" Merge branch: "),
                    Span::styled(source, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                ]),
                Line::from(" An automatic backup snapshot of target will be taken first."),
                Line::from(" Select target service to receive the state:"),
                Line::from(" ─────────────────────────────────────────────────────────────"),
            ];

            for (i, t) in targets.iter().enumerate() {
                let is_foc = i == *target_idx;
                let prefix = if is_foc { " ▸ " } else { "   " };
                lines.push(Line::from(vec![
                    Span::styled(prefix, Style::default().fg(Color::Cyan)),
                    Span::styled(t, if is_foc { Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::White) }),
                ]));
            }

            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled(" [ Enter: Confirm Merge ] ", Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::raw("   "),
                Span::styled(" [ Esc: Cancel ] ", Style::default().fg(Color::White).bg(Color::DarkGray)),
            ]));

            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Cyan))
                .style(Style::default().bg(Color::Rgb(15, 18, 26)))
                .title(" Merge Branch into Target Service ");

            f.render_widget(Paragraph::new(lines).block(block), area);
        }
        Modal::Tag { target, is_network, tag_input } => {
            let entity = if *is_network { "Network" } else { "Service" };
            let lines = vec![
                Line::from(vec![
                    Span::raw(format!(" Edit metadata tag for {}: ", entity)),
                    Span::styled(target, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                ]),
                Line::from(" (Leave blank and press Enter to clear existing tag)"),
                Line::from(" ─────────────────────────────────────────────────────────────"),
                Line::from(vec![
                    Span::styled(" ▸ Tag Value: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                    Span::styled(tag_input, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                    Span::styled("_", Style::default().fg(Color::White)),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled(" [ Enter: Save Tag ] ", Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD)),
                    Span::raw("   "),
                    Span::styled(" [ Esc: Cancel ] ", Style::default().fg(Color::White).bg(Color::DarkGray)),
                ]),
            ];

            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Yellow))
                .style(Style::default().bg(Color::Rgb(15, 18, 26)))
                .title(" \u{f02b}  Edit Project Tag ");

            f.render_widget(Paragraph::new(lines).block(block), area);
        }
        Modal::Wipe { service, confirm_input } => {
            let lines = vec![
                Line::from(Span::styled(" \u{f071} DANGER: PERMANENT DATA DESTRUCTION \u{f071}", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))),
                Line::from(" ─────────────────────────────────────────────────────────────"),
                Line::from(vec![
                    Span::raw(" This will delete container "),
                    Span::styled(service, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                    Span::raw(" AND ALL BTRFS VOLUMES!"),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::raw(" Type "),
                    Span::styled("'DESTROY'", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                    Span::raw(" to confirm: "),
                    Span::styled(confirm_input, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                    Span::styled("_", Style::default().fg(Color::Red)),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled(" [ Enter: Confirm Wipe ] ", Style::default().fg(Color::White).bg(Color::Red).add_modifier(Modifier::BOLD)),
                    Span::raw("   "),
                    Span::styled(" [ Esc: Abort ] ", Style::default().fg(Color::White).bg(Color::DarkGray)),
                ]),
            ];

            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Red))
                .style(Style::default().bg(Color::Rgb(15, 18, 26)))
                .title(" \u{f014}  Wipe Service & Subvolume ");

            f.render_widget(Paragraph::new(lines).block(block), area);
        }
        Modal::NetworkAttach { service, network_idx, networks } => {
            let mut lines = vec![
                Line::from(vec![
                    Span::raw(" Connect container "),
                    Span::styled(service, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                    Span::raw(" to bridge network:"),
                ]),
                Line::from(" ─────────────────────────────────────────────────────────────"),
            ];

            for (i, n) in networks.iter().enumerate() {
                let is_foc = i == *network_idx;
                let prefix = if is_foc { " ▸ " } else { "   " };
                lines.push(Line::from(vec![
                    Span::styled(prefix, Style::default().fg(Color::Cyan)),
                    Span::styled(n, if is_foc { Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::White) }),
                ]));
            }

            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled(" [ Enter: Attach ] ", Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::raw("   "),
                Span::styled(" [ Esc: Cancel ] ", Style::default().fg(Color::White).bg(Color::DarkGray)),
            ]));

            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Cyan))
                .style(Style::default().bg(Color::Rgb(15, 18, 26)))
                .title(" \u{f0e8}  Hot-Attach Network ");

            f.render_widget(Paragraph::new(lines).block(block), area);
        }
        Modal::NetworkAttachContainer { network, container_idx, containers } => {
            let mut lines = vec![
                Line::from(vec![
                    Span::raw(" Attach service container to network: "),
                    Span::styled(network, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                ]),
                Line::from(" Select container to connect to this isolated bridge:"),
                Line::from(" ─────────────────────────────────────────────────────────────"),
            ];

            for (i, c) in containers.iter().enumerate() {
                let is_foc = i == *container_idx;
                let prefix = if is_foc { " ▸ " } else { "   " };
                lines.push(Line::from(vec![
                    Span::styled(prefix, Style::default().fg(Color::Cyan)),
                    Span::styled(c, if is_foc { Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::White) }),
                ]));
            }

            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled(" [ Enter: Attach Container ] ", Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::raw("   "),
                Span::styled(" [ Esc: Cancel ] ", Style::default().fg(Color::White).bg(Color::DarkGray)),
            ]));

            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Cyan))
                .style(Style::default().bg(Color::Rgb(15, 18, 26)))
                .title(" \u{f0e8}  Attach Container to Network ");

            f.render_widget(Paragraph::new(lines).block(block), area);
        }
        Modal::NetworkDetachContainer { network, container_idx, containers } => {
            let mut lines = vec![
                Line::from(vec![
                    Span::raw(" Detach service container from network: "),
                    Span::styled(network, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                ]),
                Line::from(" Select container to disconnect from this bridge:"),
                Line::from(" ─────────────────────────────────────────────────────────────"),
            ];

            for (i, c) in containers.iter().enumerate() {
                let is_foc = i == *container_idx;
                let prefix = if is_foc { " ▸ " } else { "   " };
                lines.push(Line::from(vec![
                    Span::styled(prefix, Style::default().fg(Color::Cyan)),
                    Span::styled(c, if is_foc { Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::White) }),
                ]));
            }

            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled(" [ Enter: Detach Container ] ", Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw("   "),
                Span::styled(" [ Esc: Cancel ] ", Style::default().fg(Color::White).bg(Color::DarkGray)),
            ]));

            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Yellow))
                .style(Style::default().bg(Color::Rgb(15, 18, 26)))
                .title(" \u{f0e8}  Detach Container from Network ");

            f.render_widget(Paragraph::new(lines).block(block), area);
        }
        Modal::CreateNetwork { name_suffix, tag, subnet, field_idx } => {
            let fields = [
                ("Network Name Suffix", name_suffix.as_str()),
                ("Project Tag (optional)", tag.as_str()),
                ("Subnet CIDR (e.g. 10.89.2.0/24)", subnet.as_str()),
            ];

            let mut lines = vec![
                Line::from(" Create a new isolated Podman bridge network (d4c_net_<suffix>):"),
                Line::from(" ─────────────────────────────────────────────────────────────"),
            ];

            for (i, (lbl, val)) in fields.iter().enumerate() {
                let is_foc = i == *field_idx;
                let prefix = if is_foc { " ▸ " } else { "   " };
                lines.push(Line::from(vec![
                    Span::styled(prefix, Style::default().fg(Color::Cyan)),
                    Span::styled(format!("{:<30}: ", lbl), if is_foc { Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::Gray) }),
                    Span::styled(*val, if is_foc { Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::White) }),
                ]));
            }

            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled(" [ Enter: Create Bridge ] ", Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::raw("   "),
                Span::styled(" [ Esc: Cancel ] ", Style::default().fg(Color::White).bg(Color::DarkGray)),
            ]));

            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Cyan))
                .style(Style::default().bg(Color::Rgb(15, 18, 26)))
                .title(" \u{f0e8}  Create Bridge Network ");

            f.render_widget(Paragraph::new(lines).block(block), area);
        }
        Modal::ConfirmAction { title, prompt, .. } => {
            let lines = vec![
                Line::from(Span::styled(prompt, Style::default().fg(Color::White).add_modifier(Modifier::BOLD))),
                Line::from(""),
                Line::from(vec![
                    Span::styled(" [ Enter: Yes / Confirm ] ", Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)),
                    Span::raw("   "),
                    Span::styled(" [ Esc: No / Cancel ] ", Style::default().fg(Color::White).bg(Color::DarkGray)),
                ]),
            ];

            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Yellow))
                .style(Style::default().bg(Color::Rgb(15, 18, 26)))
                .title(title.as_str());

            f.render_widget(Paragraph::new(lines).block(block), area);
        }
        Modal::Message { title, message, is_error } => {
            let style = if *is_error { Style::default().fg(Color::Red) } else { Style::default().fg(Color::Green) };
            let lines = vec![
                Line::from(Span::styled(message, style.add_modifier(Modifier::BOLD))),
                Line::from(""),
                Line::from(Span::styled(" [ Press Enter or Esc to dismiss ] ", Style::default().fg(Color::DarkGray))),
            ];

            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(style)
                .style(Style::default().bg(Color::Rgb(15, 18, 26)))
                .title(title.as_str());

            f.render_widget(Paragraph::new(lines).block(block), area);
        }
        Modal::None => {}
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
