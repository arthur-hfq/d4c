use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "d4c", version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Launch the interactive terminal dashboard (TUI)
    Ui,

    /// Provision and run a preconfigured catalog service or custom image
    Up {
        /// Service catalog selection
        #[arg(value_enum)]
        service_type: ServiceCatalog,

        /// Host port to bind (auto-discovers next free port if omitted)
        #[arg(short, long)]
        port: Option<u16>,

        /// Custom container name suffix (defaults to service type)
        #[arg(short, long)]
        name: Option<String>,

        /// Root/admin password override
        #[arg(long)]
        password: Option<String>,

        /// Project or metadata tag
        #[arg(short, long)]
        tag: Option<String>,

        /// CPU limit quota (e.g., '1.0', '0.5')
        #[arg(long)]
        cpus: Option<String>,

        /// Memory allocation quota (e.g., '512m', '1g')
        #[arg(long)]
        memory: Option<String>,

        /// Custom bridge network name to connect during provisioning
        #[arg(long)]
        network: Option<String>,

        /// Arbitrary OCI container image (used only with 'custom' service type)
        #[arg(long)]
        image: Option<String>,
    },

    /// List all managed services, health status, and port bindings
    Ps,

    /// Start or restart a managed service
    Start {
        /// Name of the service container
        service: String,
    },

    /// Gracefully stop a running service container
    Stop {
        /// Name of the service container
        service: String,
    },

    /// Remove a service container while preserving its Btrfs volume
    Rm {
        /// Name of the service container
        service: String,
    },

    /// Destroy container, state tags, AND Btrfs volume subvolume (destructive)
    Wipe {
        /// Name of the service container to destroy
        service: String,

        /// Bypass manual confirmation prompt
        #[arg(short, long)]
        force: bool,
    },

    /// Stream live standard output and error logs from a service container
    Logs {
        /// Target service container
        service: String,

        /// Follow log stream in real time
        #[arg(short, long, default_value_t = false)]
        follow: bool,

        /// Number of lines to show from the end of the logs
        #[arg(short, long, default_value_t = 100)]
        tail: usize,
    },

    /// Update CPU and memory limits of an existing service on the fly
    Update {
        /// Target service container
        service: String,

        /// Updated CPU limit quota (e.g., '2.0')
        #[arg(long)]
        cpus: Option<String>,

        /// Updated memory allocation quota (e.g., '2g')
        #[arg(long)]
        memory: Option<String>,
    },

    /// Set or update the project tag metadata for a service
    Tag {
        /// Target service container
        service: String,

        /// Tag value (leave empty to delete existing tag)
        tag: Option<String>,
    },

    /// Btrfs subvolume snapshotting and atomic state branching
    #[command(subcommand)]
    Snap(SnapCommands),

    /// Network topology management and multi-container log aggregation
    #[command(subcommand)]
    Net(NetCommands),

    /// Export active grid topology to a docker-compose.yml file
    Compose {
        /// Output destination path
        #[arg(short, long, default_value = "./d4c-compose.yml")]
        output: PathBuf,

        /// Export strategy: 'host' (absolute paths) or 'portable' (named volumes)
        #[arg(long, default_value = "host")]
        mode: String,
    },

    /// Search containers and bridge networks by tag, name, port, or subnet
    Search {
        /// Search keyword query
        query: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum SnapCommands {
    /// Create an atomic copy-on-write snapshot of a service volume
    Create {
        /// Target service name
        service: String,

        /// Custom snapshot tag suffix (defaults to current UNIX timestamp)
        #[arg(short, long)]
        tag: Option<String>,

        /// Mark the generated snapshot subvolume as read-only
        #[arg(short, long)]
        readonly: bool,
    },

    /// Branch a service by cloning its volume into a new independent container instance
    Branch {
        /// Source service container to fork from
        service: String,

        /// Suffix tag for the new branched service
        suffix: String,

        /// Dedicated host port for the branch container
        #[arg(short, long)]
        port: Option<u16>,
    },

    /// Create an atomic snapshot and export it as a compressed archive (.tar.gz)
    Export {
        /// Target service volume to export
        service: String,

        /// Output tarball destination directory
        #[arg(short, long)]
        destination: Option<PathBuf>,
    },

    /// List all point-in-time snapshots available for a specific service
    List {
        /// Target service name
        service: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum NetCommands {
    /// Create a new custom isolated bridge network
    Create {
        /// Network name suffix (prefixed automatically with d4c_net_)
        name: String,

        /// Project or metadata tag
        #[arg(short, long)]
        tag: Option<String>,
    },

    /// List custom networks with container attachment counts
    List,

    /// Remove a custom bridge network
    Rm {
        /// Network name to delete
        name: String,
    },

    /// Hot-attach a running service to a network
    Attach {
        /// Target service container
        service: String,

        /// Network to join
        network: String,
    },

    /// Hot-detach a service from a network
    Detach {
        /// Target service container
        service: String,

        /// Network to leave
        network: String,
    },

    /// Stream live aggregated and color-coded logs across all containers on a network
    Panorama {
        /// Target bridge network
        network: String,
    },
}

#[derive(ValueEnum, Clone, Debug)]
pub enum ServiceCatalog {
    Postgres,
    Mysql,
    Mongo,
    Redis,
    Kafka,
    Rabbitmq,
    Custom,
}
