use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "d4c",
    version,
    about = "Minimalist local infrastructure orchestrator for Podman containers with atomic Btrfs snapshots"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Launch the QuickShell graphical dashboard (or terminal UI with --tui)
    Ui {
        /// Launch in text terminal mode instead of QuickShell
        #[arg(long)]
        tui: bool,
    },

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
    Ps {
        /// Output results in JSON format
        #[arg(short, long)]
        json: bool,
    },

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

        /// Output results in JSON format
        #[arg(short, long)]
        json: bool,
    },

    /// Restore a service volume to a previous snapshot
    Restore {
        /// Target service container
        service: String,

        /// Snapshot tag or name to restore from
        #[arg(short, long)]
        snapshot: String,
    },

    /// Merge changes from a branch service volume back into a target service
    Merge {
        /// Source branch service container
        source: String,

        /// Target service container to receive the merged state
        target: String,
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
    List {
        /// Output results in JSON format
        #[arg(short, long)]
        json: bool,
    },

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

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServiceCatalog {
    Postgres,
    Mysql,
    Mongo,
    Redis,
    Kafka,
    Rabbitmq,
    Custom,
}

impl std::fmt::Display for ServiceCatalog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServiceCatalog::Postgres => write!(f, "postgres"),
            ServiceCatalog::Mysql => write!(f, "mysql"),
            ServiceCatalog::Mongo => write!(f, "mongo"),
            ServiceCatalog::Redis => write!(f, "redis"),
            ServiceCatalog::Kafka => write!(f, "kafka"),
            ServiceCatalog::Rabbitmq => write!(f, "rabbitmq"),
            ServiceCatalog::Custom => write!(f, "custom"),
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parse_up() {
        let args = vec!["d4c", "up", "postgres", "-p", "5433", "--name", "my-pg", "--tag", "prod"];
        let cli = Cli::try_parse_from(args).expect("Failed to parse 'd4c up'");
        match cli.command {
            Some(Commands::Up { service_type, port, name, tag, .. }) => {
                assert_eq!(service_type, ServiceCatalog::Postgres);
                assert_eq!(port, Some(5433));
                assert_eq!(name, Some("my-pg".to_string()));
                assert_eq!(tag, Some("prod".to_string()));
            }
            _ => panic!("Expected Commands::Up"),
        }
    }

    #[test]
    fn test_cli_parse_snap_branch_and_merge() {
        let branch_args = vec!["d4c", "snap", "branch", "my-pg", "test-branch", "--port", "5435"];
        let branch_cli = Cli::try_parse_from(branch_args).expect("Failed to parse 'd4c snap branch'");
        match branch_cli.command {
            Some(Commands::Snap(SnapCommands::Branch { service, suffix, port })) => {
                assert_eq!(service, "my-pg");
                assert_eq!(suffix, "test-branch");
                assert_eq!(port, Some(5435));
            }
            _ => panic!("Expected SnapCommands::Branch"),
        }

        let merge_args = vec!["d4c", "snap", "merge", "my-pg_test-branch", "my-pg"];
        let merge_cli = Cli::try_parse_from(merge_args).expect("Failed to parse 'd4c snap merge'");
        match merge_cli.command {
            Some(Commands::Snap(SnapCommands::Merge { source, target })) => {
                assert_eq!(source, "my-pg_test-branch");
                assert_eq!(target, "my-pg");
            }
            _ => panic!("Expected SnapCommands::Merge"),
        }
    }

    #[test]
    fn test_cli_parse_snap_restore() {
        let args = vec!["d4c", "snap", "restore", "my-pg", "-s", "snap_1234"];
        let cli = Cli::try_parse_from(args).expect("Failed to parse 'd4c snap restore'");
        match cli.command {
            Some(Commands::Snap(SnapCommands::Restore { service, snapshot })) => {
                assert_eq!(service, "my-pg");
                assert_eq!(snapshot, "snap_1234");
            }
            _ => panic!("Expected SnapCommands::Restore"),
        }
    }

    #[test]
    fn test_cli_parse_net() {
        let args = vec!["d4c", "net", "create", "dev-bridge", "--tag", "network-tag"];
        let cli = Cli::try_parse_from(args).expect("Failed to parse 'd4c net create'");
        match cli.command {
            Some(Commands::Net(NetCommands::Create { name, tag })) => {
                assert_eq!(name, "dev-bridge");
                assert_eq!(tag, Some("network-tag".to_string()));
            }
            _ => panic!("Expected NetCommands::Create"),
        }
    }
}
