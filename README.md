# d4c

A minimalist local infrastructure orchestrator in Rust designed to manage **Podman** containerized services and create atomic state branches using **Btrfs Copy-on-Write (CoW)** snapshots, featuring a fluid full-screen **Ratatui TUI** styled with Font Awesome icons.

`d4c` bridges local container orchestration with filesystem-level state branching:
- **d4c Engine**: Pure Rust container orchestration, Btrfs subvolumes, atomic branching, safe merging, and bridge networking.
- **Interactive TUI Dashboard**: Fluid full-screen terminal interface with two-pane inspector, multi-tab navigation, live logs, network topology visualizer, and hotkey workflows.

---

## Key Features

- **Fluid Terminal UI (TUI)**: Full-screen interactive dashboard built with Ratatui and Crossterm. Fluid keyboard navigation (`1..7`, `Tab`, `j`/`k`, arrow keys), live container metrics, connection URI helpers, and interactive modals.
- **Font Awesome Glyph Styling**: Clean vector glyphs for containers, networks, metrics, and actions with zero emojis.
- **Atomic State Branching & Merging**: Instant Copy-on-Write snapshots of container storage volumes powered by Btrfs subvolumes. Branch a database in milliseconds without duplicating disk space, and safely merge changes back with automatic pre-merge safety backups.
- **Rootless Podman Support**: Automatically leverages rootless container user namespaces (`podman unshare`) to manage database subvolumes with container-mapped UIDs safely (e.g., PostgreSQL UID 70 / subuid 100069).
- **Auto-Provisioned Visible Private Bridges**: Services created without an explicit network automatically receive a dedicated, isolated bridge network (`d4c_net_private_<service>`) tagged as `private`. These networks are fully visible and manageable in both the Networks and Panorama tabs.
- **Safe Network Migration & Container Protection**: When removing a bridge network, d4c automatically disconnects any attached containers first, preventing container removal or network-in-use errors.
- **Automatic Free Port Discovery**: Scans and binds the next free TCP host port or validates requested port availability automatically.
- **Curated Service Catalog**: Zero-config provisioning for `postgres`, `mysql`, `mongo`, `redis`, `kafka` (KRaft mode), `rabbitmq`, or arbitrary `custom` OCI images.
- **Live Resource Limits**: Configure `--cpus` and `--memory` limits upon creation or branching, and adjust them live on running containers via `d4c update` or the `[u]` modal in the TUI.
- **JSON API Output**: Machine-readable JSON output via `--json` flags for `ps`, `snap list`, and `net list`.
- **Snapshot Export & Compose Generation**: Export atomic volume snapshots to compressed `.tar.gz` archives, or generate standard `docker-compose.yml` files (in `host` bind or `portable` named volume modes).

---

## Prerequisites

1. **Linux OS** with a **Btrfs** filesystem mounted at `~/.local/share/` (or your chosen storage path).
2. **Podman** (rootless or root).
3. A terminal font supporting **Font Awesome** glyphs (e.g., Nerd Fonts).

---

## Installation

```bash
# Clone the repository
git clone https://github.com/your-username/d4c.git
cd d4c

# Build release binary
cargo build --release

# Run d4c
./target/release/d4c --help
```

Optionally install `d4c` to your local path:

```bash
install -Dm755 target/release/d4c ~/.local/bin/d4c
```

---

## Interactive TUI Dashboard

Launch the full-screen interactive TUI dashboard:

```bash
# Launch TUI dashboard (running d4c without arguments or with 'ui')
d4c
# or
d4c ui
```

### Dashboard Tabs

| Key | Tab | Description |
|---|---|---|
| `[1]` | **Services** | List containers, inspect status, view connection URIs, manage ports and limits |
| `[2]` | **Networks** | Manage bridge networks, view IPAM subnets/gateways, attach/detach containers |
| `[3]` | **Snapshots** | View Btrfs snapshots, create point-in-time tags, restore states, export archives |
| `[4]` | **Panorama** | Stream real-time aggregated logs across multiple containers on a bridge network |
| `[5]` | **Deploy** | Interactive form to provision catalog services with custom ports, limits, and networks |
| `[6]` | **Search** | Filter containers, networks, ports, and tags with instant matching |
| `[?]` / `[7]` | **Help** | Keyboard shortcuts reference and navigation cheatsheet |

### Global Navigation Hotkeys

- `[1]`..`[7]` or `[Tab]` / `[Shift-Tab]`: Switch between tabs.
- `[Left]` / `[Right]`: Switch tabs horizontally.
- `[Up]` / `[Down]` or `[k]` / `[j]`: Navigate items in lists and tables.
- `[r]`: Force immediate refresh of container states and networks.
- `[q]` / `[Esc]`: Quit d4c or dismiss open modal overlay.

### Services View Hotkeys

- `[s]`: Start or restart selected service container.
- `[x]`: Stop running service container.
- `[c]`: Branch container via Btrfs CoW snapshot (opens modal to set suffix, port, and limits).
- `[m]`: Merge branch changes back into target service (with pre-merge backup).
- `[u]`: Update CPU and memory limits on the fly.
- `[t]`: Set or clear project tag metadata.
- `[b]`: Backup volume to compressed `.tar.gz` archive.
- `[d]`: Delete container definition (Btrfs storage volume preserved).
- `[w]`: Wipe service completely (destroys container, state, and Btrfs subvolumes).
- `[n]`: Hot-attach container to bridge network.
- `[X]`: Detach container from all networks.

### Networks View Hotkeys

- `[c]`: Create new bridge network (custom name, tag, subnet CIDR).
- `[d]`: Remove bridge network (cleanly disconnects all containers first).
- `[a]`: Attach container to network.
- `[x]`: Detach container from network.
- `[t]`: Update network tag metadata.
- `[p]`: Open Panorama live log aggregator for the selected network.

---

## CLI Reference

### 1. Service Provisioning & Management

```bash
# Provision a PostgreSQL service (automatically binds free port, e.g., 5432)
d4c up postgres --name db-main --tag production

# Provision Redis with CPU quota and custom memory limit
d4c up redis --name cache --cpus "1.0" --memory "512m"

# Provision Apache Kafka (KRaft mode)
d4c up kafka --name kafka-broker

# Provision a custom OCI container image
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

# Update CPU and memory limits live on running containers
d4c update db-main --cpus "2.0" --memory "2g"

# Completely wipe a container AND destroy all its Btrfs subvolumes and snapshots
d4c wipe db-main
```

### 2. Atomic Snapshots, Branching & Merging

```bash
# 1. Create a point-in-time snapshot of db-main
d4c snap create db-main --tag v1.0 --readonly

# 2. List all snapshots available for a service (table or JSON)
d4c snap list db-main
d4c snap list db-main --json

# 3. Branch db-main into an independent writable container instance
# Clones volume state instantly via Btrfs CoW, inherits or overrides CPU/RAM limits, and binds a free port
d4c snap branch db-main feat-auth --port 5433 --cpus "1.5" --memory "1g"

# 4. Merge changes from branch db-main_feat-auth back into db-main
# Automatically stops containers safely, creates a safety backup snapshot, merges filesystem state, and restarts
d4c snap merge db-main_feat-auth db-main

# 5. Restore db-main to an earlier snapshot
d4c snap restore db-main -s v1.0

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
# Export active topology to docker-compose.yml (host bind mode or portable volume mode)
d4c compose --output ./docker-compose.yml --mode host

# Search containers, networks, ports, and tags
d4c search auth
```

---

## Storage Architecture

All container data volumes and Btrfs snapshots are stored in the user local XDG data path:

```
~/.local/share/d4c/
├── state.json                                  # Metadata state store (tags, branch parents, networks)
└── volumes/
    ├── d4c_vol_<service>_data/                 # Active writable Btrfs subvolume
    ├── d4c_vol_<service>_snap_<tag>/           # Point-in-time snapshot subvolume
    ├── d4c_vol_<service>_snap_pre_merge_<ts>/  # Automatic safety backup before merge
    └── d4c_vol_<service>_<branch>_data/        # Independent writable branch subvolume
```

- **Zero-Copy Branching**: Creating a branch takes milliseconds and initially consumes 0 bytes of disk space. Only modified data blocks consume additional storage as changes are written.
- **Safety Pre-Merge Snapshots**: Every merge operation automatically creates a `pre_merge_<source>_<timestamp>` snapshot before applying changes, allowing seamless rollback via `d4c snap restore` if needed.
- **Rootless Sub-UID Safety**: Directory operations and Btrfs snapshot management execute within `podman unshare` namespaces when handling container-mapped sub-UIDs, preserving permission fidelity without root host privileges.

---

## Testing & Quality

Run the test suite and static analysis:

```bash
# Run unit and integration tests
cargo test

# Run Clippy linter
cargo clippy -- -D warnings
```

---

## License

MIT License. See [LICENSE](LICENSE) for details.
