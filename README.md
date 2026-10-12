# d4c (Dirty Deeds Done Dirt Cheap)

A minimalist local infrastructure orchestrator in Rust designed to manage **Podman** containerized services and create atomic state branches using **Btrfs Copy-on-Write (CoW)** snapshots, with a fluid, full-screen **Ratatui TUI** rendered via Font Awesome icons.

`d4c` combines high-performance container orchestration with instant file system state branching:
- **d4c Engine**: Pure container orchestration, Btrfs subvolumes, atomic branching, merging, and networking.
- **Interactive TUI Dashboard**: Fluid full-screen terminal interface with two-pane inspector, real-time live logs, network topology visualizer, and keyboard shortcuts.

---

## Key Features

- **Fluid Terminal UI (TUI)**: Full-screen interactive dashboard built with Ratatui and Crossterm. Seamless keyboard navigation (`1..7`, `Tab`, `j`/`k`, arrows), live service metrics, connection URI helpers, and popup modals.
- **Font Awesome Glyph Styling**: Clean vector glyphs for containers, networks, metrics, and actions with zero emojis.
- **Atomic State Branching & Merging**: Instant Copy-on-Write snapshots of container storage volumes powered by Btrfs subvolumes. Branch a database in milliseconds without duplicating disk space, and safely merge changes back with automatic pre-merge backups.
- **Rootless Podman Support**: Automatically leverages rootless container user namespaces (`podman unshare`) to manage database subvolumes with container-mapped UIDs safely.
- **Automatic Port Discovery**: Automatically scans and binds free TCP host ports or checks requested port availability.
- **Curated Service Catalog**: Zero-config provisioning for `postgres`, `mysql`, `mongo`, `redis`, `kafka` (KRaft mode), `rabbitmq`, or arbitrary `custom` OCI images.
- **Advanced Bridge Networking**: Create custom Podman networks with custom subnet CIDRs, inspect attached containers and their assigned IP addresses, hot attach/detach containers (including detach from all), and stream multi-container aggregated logs with Panorama.
- **Live Resource Limits**: Configure `--cpus` and `--memory` limits upon creation or branching, and tune them live on running containers via `d4c update`.
- **JSON API Output**: Machine-readable JSON output via `--json` flags for `ps`, `snap list`, and `net list`.
- **Snapshot Export & Compose Generation**: Export atomic volume snapshots to compressed `.tar.gz` archives, or generate standard `docker-compose.yml` files (in `host` bind or `portable` named volume modes).

---

## Quick Start

### Installation

Ensure Rust, Podman, and Font Awesome are installed:

```bash
# Clone the repository
git clone https://github.com/your-username/d4c.git
cd d4c

# Build release binary
cargo build --release

# Run d4c
./target/release/d4c --help
```

---

## Interactive TUI Dashboard

To launch the full-screen interactive TUI dashboard:

```bash
# Launch TUI dashboard (running d4c without arguments or with 'ui')
d4c
# or
d4c ui
```

### Keyboard Navigation & Hotkeys

- **Tabs Navigation**:
  - `[1]` Services Dashboard
  - `[2]` Networks & Bridges Topology
  - `[3]` Btrfs Snapshots & Branches
  - `[4]` Panorama Multi-Container Logs
  - `[5]` Provision / Deploy Wizard
  - `[6]` Search & Filter
  - `[?]` / `[7]` Help Cheatsheet
  - `[Tab]` / `[Shift-Tab]` or `[Left]` / `[Right]`: Cycle tabs
  - `[Up]` / `[Down]` or `[k]` / `[j]`: Navigate items in lists
  - `[r]`: Force immediate refresh
  - `[q]` / `[Esc]`: Quit or dismiss popup

- **Services View Hotkeys**:
  - `[s]`: Start / Restart container
  - `[x]`: Stop container
  - `[c]`: Branch container (creates Btrfs CoW snapshot and launches branch on free port)
  - `[m]`: Merge branch changes back into target service
  - `[u]`: Tune CPU and Memory quotas on the fly
  - `[t]`: Edit or clear project tag metadata
  - `[b]`: Backup volume to `.tar.gz` archive
  - `[d]`: Delete container (volume preserved)
  - `[w]`: Wipe service (destroys container and Btrfs subvolume)
  - `[n]`: Hot-attach container to bridge network
  - `[X]`: Detach container from all networks

- **Networks View Hotkeys**:
  - `[c]`: Create new bridge network (custom name suffix, tag, subnet)
  - `[d]`: Remove bridge network
  - `[a]`: Attach container to network
  - `[x]`: Detach container from network
  - `[t]`: Tag network
  - `[p]`: Open Panorama live log aggregator for this network

---

## CLI Commands

### 1. Service Provisioning & Management

```bash
# Provision a PostgreSQL service (automatically binds free port, e.g., 5432)
d4c up postgres --name db-main --tag production

# Provision Redis with CPU quota and custom memory limit
d4c up redis --name cache --cpus "1.0" --memory "512m"

# Provision Apache Kafka (KRaft mode)
d4c up kafka --name kafka-broker

# Provision a custom container image
d4c up custom --image docker.io/library/nginx:alpine --name web --port 8080

# List all managed services (table format)
d4c ps

# List services in JSON format
d4c ps --json

# Start, stop, or remove a container (preserves Btrfs volume data)
d4c start db-main
d4c stop db-main
d4c rm db-main

# Stream live container logs
d4c logs db-main --follow --tail 100

# Update CPU and memory limits on the fly
d4c update db-main --cpus "2.0" --memory "2g"

# Completely wipe a container AND destroy all its Btrfs subvolumes and snapshots
d4c wipe db-main --force
```

### 2. Atomic Snapshots, Branching & Merging

```bash
# 1. Create a point-in-time snapshot of db-main
d4c snap create db-main --tag v1.0 --readonly

# 2. List all snapshots available for a service (table or JSON)
d4c snap list db-main
d4c snap list db-main --json

# 3. Branch db-main into an independent writable container instance
# (Clones volume state instantly via Btrfs CoW, overrides/inherits CPU/RAM limits, and starts container on a free port)
d4c snap branch db-main feat-auth --port 5433 --cpus "1.5" --memory "1g"

# 4. Merge changes from branch db-main_feat-auth back into db-main
# (Automatically creates a safety backup snapshot of db-main before merging)
d4c snap merge db-main_feat-auth db-main

# 5. Restore db-main to an earlier snapshot
d4c snap restore db-main --snapshot v1.0

# 6. Export an atomic snapshot as a compressed .tar.gz archive
d4c snap export db-main --destination /tmp/backups
```

### 3. Bridge Networks & Panorama Logging

```bash
# Create an isolated bridge network with custom subnet and metadata tag
d4c net create backend-mesh --tag vpc-1 --subnet "10.89.2.0/24"

# List bridge networks (shows IPAM subnet, gateway, container counts, and container IPs)
d4c net list
d4c net list --json

# Hot-attach / hot-detach running containers to the network
d4c net attach db-main backend-mesh
d4c net detach db-main backend-mesh

# Detach a container from all networks
d4c net detach db-main --all

# Stream aggregated real-time color-coded logs across all containers on a network
d4c net panorama backend-mesh
```

### 4. Topology Export & Search

```bash
# Export active topology to docker-compose.yml
d4c compose --output ./docker-compose.yml --mode host

# Search containers, networks, ports, and tags
d4c search auth
```

---

## Storage Architecture

All container data volumes and Btrfs snapshots are stored in the user's local XDG data path:

```
~/.local/share/d4c/
├── state.json                           # Metadata state store (tags, branch links)
└── volumes/
    ├── d4c_vol_<service>_data/          # Active writable Btrfs subvolume
    ├── d4c_vol_<service>_snap_<tag>/    # Atomic snapshot subvolume
    └── d4c_vol_<service>_<branch>_data/ # Branched writable subvolume
```

---

## Testing

Run the full automated unit and integration test suite:

```bash
cargo test
cargo clippy -- -D warnings
```
