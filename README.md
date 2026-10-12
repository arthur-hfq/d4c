# d4c (Dirty Deeds Done Dirt Cheap)

A minimalist local infrastructure orchestrator in Rust designed to manage **Podman** containerized services and create atomic state branches using **Btrfs Copy-on-Write (CoW)** snapshots, with a dedicated **QuickShell** desktop frontend rendered via Font Awesome icons.

`d4c` separates concerns cleanly:
- **d4c core**: Pure container orchestration, Btrfs subvolumes, atomic branching, merging, and networking.
- **QuickShell UI**: Desktop interface for live management, monitoring, branching, and snapshots without terminal clutter.

---

## Key Features

- **Atomic State Branching & Merging**: Instant Copy-on-Write snapshots of container storage volumes powered by Btrfs subvolumes. Branch a database in milliseconds without duplicating disk space, and safely merge changes back.
- **Rootless Podman Support**: Automatically leverages rootless container user namespaces (`podman unshare`) to manage database subvolumes with container-mapped UIDs safely.
- **Automatic Port Discovery**: Automatically scans and binds free TCP host ports or checks requested port availability.
- **Curated Service Catalog**: Zero-config provisioning for `postgres`, `mysql`, `mongo`, `redis`, `kafka` (KRaft mode), `rabbitmq`, or arbitrary `custom` OCI images.
- **Isolated Bridge Networks**: Create custom Podman networks, attach/detach containers on the fly, and stream multi-container aggregated logs with Panorama.
- **QuickShell Desktop Interface**: Dedicated graphical dashboard in `quickshell/shell.qml` using Font Awesome icons (zero emojis) for browsing services, branching, merging, taking snapshots, and viewing live logs.
- **JSON API Output**: Machine-readable JSON output via `--json` flags for `ps`, `snap list`, and `net list`.
- **Snapshot Export & Compose Generation**: Export atomic volume snapshots to compressed `.tar.gz` archives, or generate standard `docker-compose.yml` files (in `host` bind or `portable` named volume modes).

---

## Quick Start

### Installation

Ensure Rust, Podman, Quickshell, and Font Awesome are installed:

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

## QuickShell UI

To launch the graphical QuickShell dashboard:

```bash
# Launch QuickShell dashboard (automatically passes binary path)
./target/release/d4c ui

# Or launch directly with quickshell
quickshell -p quickshell/shell.qml

# For headless or text terminal mode:
./target/release/d4c ui --tui
```

The QuickShell interface features:
- Services manager with status, port mapping, and quick actions (Start, Stop, Branch, Snapshot, Logs, Wipe).
- Btrfs snapshot and branch center with one-click cloning and safe merging.
- Bridge networks topology viewer and manager.
- Live streaming container log viewer.
- Rendered with **Font Awesome** typography and dark theme styling (zero emojis).

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

# List services in JSON format (for QuickShell / scripts)
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
# (Clones volume state instantly via Btrfs CoW and starts container on a free port)
d4c snap branch db-main feat-auth --port 5433

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
# Create an isolated bridge network
d4c net create backend-mesh --tag vpc-1

# List bridge networks (table or JSON)
d4c net list
d4c net list --json

# Hot-attach / hot-detach running containers to the network
d4c net attach db-main backend-mesh
d4c net detach db-main backend-mesh

# Stream aggregated real-time logs across all containers on a network
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
