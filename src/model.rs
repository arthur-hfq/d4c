use crate::cli::ServiceCatalog;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

impl ServiceCatalog {
    /// Return the canonical default OCI container image for this catalog service
    pub fn default_image(&self, custom: Option<&str>) -> String {
        match self {
            ServiceCatalog::Postgres => "docker.io/library/postgres:16-alpine".to_string(),
            ServiceCatalog::Mysql => "docker.io/library/mysql:8.0".to_string(),
            ServiceCatalog::Mongo => "docker.io/library/mongo:7.0".to_string(),
            ServiceCatalog::Redis => "docker.io/library/redis:7-alpine".to_string(),
            ServiceCatalog::Kafka => "docker.io/apache/kafka:latest".to_string(),
            ServiceCatalog::Rabbitmq => "docker.io/library/rabbitmq:3-management-alpine".to_string(),
            ServiceCatalog::Custom => custom
                .unwrap_or("docker.io/library/alpine:latest")
                .to_string(),
        }
    }

    /// Return the standard internal container listening port
    pub fn default_port(&self) -> u16 {
        match self {
            ServiceCatalog::Postgres => 5432,
            ServiceCatalog::Mysql => 3306,
            ServiceCatalog::Mongo => 27017,
            ServiceCatalog::Redis => 6379,
            ServiceCatalog::Kafka => 9092,
            ServiceCatalog::Rabbitmq => 5672,
            ServiceCatalog::Custom => 8080,
        }
    }

    /// Return the standard internal data storage directory where volume is mounted
    pub fn default_data_dir(&self) -> &'static str {
        match self {
            ServiceCatalog::Postgres => "/var/lib/postgresql/data",
            ServiceCatalog::Mysql => "/var/lib/mysql",
            ServiceCatalog::Mongo => "/data/db",
            ServiceCatalog::Redis => "/data",
            ServiceCatalog::Kafka => "/tmp/kraft-combined-logs",
            ServiceCatalog::Rabbitmq => "/var/lib/rabbitmq",
            ServiceCatalog::Custom => "/data",
        }
    }

    /// Generate default environment variables and credentials
    pub fn default_env(&self, password: Option<&str>, host_port: u16) -> Vec<(String, String)> {
        match self {
            ServiceCatalog::Postgres => {
                let pass = password.unwrap_or("postgres");
                vec![
                    ("POSTGRES_USER".to_string(), "postgres".to_string()),
                    ("POSTGRES_PASSWORD".to_string(), pass.to_string()),
                    ("POSTGRES_DB".to_string(), "d4c_db".to_string()),
                ]
            }
            ServiceCatalog::Mysql => {
                let pass = password.unwrap_or("mysql");
                vec![
                    ("MYSQL_ROOT_PASSWORD".to_string(), pass.to_string()),
                    ("MYSQL_DATABASE".to_string(), "d4c_db".to_string()),
                ]
            }
            ServiceCatalog::Mongo => {
                if let Some(pass) = password {
                    vec![
                        ("MONGO_INITDB_ROOT_USERNAME".to_string(), "admin".to_string()),
                        ("MONGO_INITDB_ROOT_PASSWORD".to_string(), pass.to_string()),
                    ]
                } else {
                    vec![]
                }
            }
            ServiceCatalog::Redis => {
                // Redis accepts password via cli flags or env
                vec![]
            }
            ServiceCatalog::Kafka => vec![
                ("KAFKA_NODE_ID".to_string(), "1".to_string()),
                (
                    "KAFKA_PROCESS_ROLES".to_string(),
                    "broker,controller".to_string(),
                ),
                (
                    "KAFKA_LISTENERS".to_string(),
                    "PLAINTEXT://0.0.0.0:9092,CONTROLLER://0.0.0.0:9093".to_string(),
                ),
                (
                    "KAFKA_ADVERTISED_LISTENERS".to_string(),
                    format!("PLAINTEXT://localhost:{}", host_port),
                ),
                (
                    "KAFKA_CONTROLLER_LISTENER_NAMES".to_string(),
                    "CONTROLLER".to_string(),
                ),
                (
                    "KAFKA_LISTENER_SECURITY_PROTOCOL_MAP".to_string(),
                    "CONTROLLER:PLAINTEXT,PLAINTEXT:PLAINTEXT".to_string(),
                ),
                (
                    "KAFKA_CONTROLLER_QUORUM_VOTERS".to_string(),
                    "1@localhost:9093".to_string(),
                ),
                (
                    "KAFKA_OFFSETS_TOPIC_REPLICATION_FACTOR".to_string(),
                    "1".to_string(),
                ),
                (
                    "KAFKA_TRANSACTION_STATE_LOG_REPLICATION_FACTOR".to_string(),
                    "1".to_string(),
                ),
                (
                    "KAFKA_TRANSACTION_STATE_LOG_MIN_ISR".to_string(),
                    "1".to_string(),
                ),
                (
                    "KAFKA_GROUP_INITIAL_REBALANCE_DELAY_MS".to_string(),
                    "0".to_string(),
                ),
                ("KAFKA_NUM_PARTITIONS".to_string(), "1".to_string()),
            ],
            ServiceCatalog::Rabbitmq => {
                let pass = password.unwrap_or("admin");
                vec![
                    ("RABBITMQ_DEFAULT_USER".to_string(), "admin".to_string()),
                    ("RABBITMQ_DEFAULT_PASS".to_string(), pass.to_string()),
                ]
            }
            ServiceCatalog::Custom => vec![],
        }
    }

    /// Parse loose string into catalog enum
    pub fn parse_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "postgres" | "postgresql" | "pgsql" => Some(ServiceCatalog::Postgres),
            "mysql" | "mariadb" => Some(ServiceCatalog::Mysql),
            "mongo" | "mongodb" => Some(ServiceCatalog::Mongo),
            "redis" | "valkey" => Some(ServiceCatalog::Redis),
            "kafka" => Some(ServiceCatalog::Kafka),
            "rabbitmq" | "rabbit" => Some(ServiceCatalog::Rabbitmq),
            "custom" => Some(ServiceCatalog::Custom),
            _ => None,
        }
    }
}

/// Information describing a managed service container
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInfo {
    pub name: String,
    pub container_id: Option<String>,
    pub status: String,
    pub catalog: ServiceCatalog,
    pub image: String,
    pub host_port: Option<u16>,
    pub container_port: u16,
    pub tag: Option<String>,
    pub volume_path: PathBuf,
    pub network: Option<String>,
    pub cpus: Option<String>,
    pub memory: Option<String>,
    pub created_at: Option<String>,
    pub branch_parent: Option<String>,
}

/// Metadata describing a Btrfs volume snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotInfo {
    pub name: String,
    pub service: String,
    pub tag: String,
    pub path: PathBuf,
    pub readonly: bool,
    pub size_bytes: u64,
    pub created_at: Option<String>,
}

/// Metadata describing a bridge network
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInfo {
    pub name: String,
    pub id: String,
    pub subnet: Option<String>,
    pub gateway: Option<String>,
    pub tag: Option<String>,
    pub containers_count: usize,
    pub containers: Vec<String>,
}

/// Persistent service state entry stored on disk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceState {
    pub name: String,
    pub catalog: ServiceCatalog,
    pub image: String,
    pub host_port: u16,
    pub container_port: u16,
    pub tag: Option<String>,
    pub volume_path: PathBuf,
    pub network: Option<String>,
    pub cpus: Option<String>,
    pub memory: Option<String>,
    pub branch_parent: Option<String>,
    pub created_at: String,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

/// Global local state storage for d4c orchestrator
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StateStore {
    #[serde(default)]
    pub services: HashMap<String, ServiceState>,
    #[serde(default)]
    pub networks: HashMap<String, Option<String>>, // network_name -> tag
}

impl StateStore {
    fn state_file_path() -> Result<PathBuf> {
        let base_dir = crate::btrfs::get_volumes_dir()?.parent().map(Path::to_path_buf).context("Invalid parent directory")?;
        Ok(base_dir.join("state.json"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::state_file_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read state file at {:?}", path))?;
        let store: StateStore = serde_json::from_str(&content)
            .unwrap_or_default();
        Ok(store)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::state_file_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)
            .context("Failed to serialize state store to JSON")?;
        fs::write(&path, content)
            .with_context(|| format!("Failed to write state file at {:?}", path))?;
        Ok(())
    }

    pub fn get_service(&self, name: &str) -> Option<&ServiceState> {
        self.services.get(name)
    }

    pub fn set_service(&mut self, state: ServiceState) -> Result<()> {
        self.services.insert(state.name.clone(), state);
        self.save()
    }

    pub fn remove_service(&mut self, name: &str) -> Result<()> {
        self.services.remove(name);
        self.save()
    }

    pub fn set_service_tag(&mut self, name: &str, tag: Option<String>) -> Result<()> {
        if let Some(service) = self.services.get_mut(name) {
            service.tag = tag;
            self.save()?;
        }
        Ok(())
    }

    pub fn set_network_tag(&mut self, name: &str, tag: Option<String>) -> Result<()> {
        self.networks.insert(name.to_string(), tag);
        self.save()
    }

    pub fn remove_network(&mut self, name: &str) -> Result<()> {
        self.networks.remove(name);
        self.save()
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_catalog_defaults() {
        assert_eq!(ServiceCatalog::Postgres.default_port(), 5432);
        assert_eq!(ServiceCatalog::Mysql.default_port(), 3306);
        assert_eq!(ServiceCatalog::Mongo.default_port(), 27017);
        assert_eq!(ServiceCatalog::Redis.default_port(), 6379);
        assert_eq!(ServiceCatalog::Kafka.default_port(), 9092);
        assert_eq!(ServiceCatalog::Rabbitmq.default_port(), 5672);
        assert_eq!(ServiceCatalog::Custom.default_port(), 8080);

        assert_eq!(ServiceCatalog::Postgres.default_data_dir(), "/var/lib/postgresql/data");
        assert_eq!(ServiceCatalog::Mysql.default_data_dir(), "/var/lib/mysql");
        assert_eq!(ServiceCatalog::Mongo.default_data_dir(), "/data/db");
        assert_eq!(ServiceCatalog::Redis.default_data_dir(), "/data");
    }

    #[test]
    fn test_parse_loose() {
        assert_eq!(ServiceCatalog::parse_loose("postgres"), Some(ServiceCatalog::Postgres));
        assert_eq!(ServiceCatalog::parse_loose("PGSQL"), Some(ServiceCatalog::Postgres));
        assert_eq!(ServiceCatalog::parse_loose("mariadb"), Some(ServiceCatalog::Mysql));
        assert_eq!(ServiceCatalog::parse_loose("valkey"), Some(ServiceCatalog::Redis));
        assert_eq!(ServiceCatalog::parse_loose("unknown_svc"), None);
    }

    #[test]
    fn test_service_catalog_env() {
        let pg_env = ServiceCatalog::Postgres.default_env(Some("secret123"), 5432);
        assert!(pg_env.iter().any(|(k, v)| k == "POSTGRES_PASSWORD" && v == "secret123"));

        let kafka_env = ServiceCatalog::Kafka.default_env(None, 9092);
        assert!(kafka_env.iter().any(|(k, v)| k == "KAFKA_NODE_ID" && v == "1"));
        assert!(kafka_env.iter().any(|(k, v)| k == "KAFKA_ADVERTISED_LISTENERS" && v == "PLAINTEXT://localhost:9092"));
    }

    #[test]
    fn test_state_store_serialization() {
        let mut store = StateStore::default();
        let svc = ServiceState {
            name: "test-db".to_string(),
            catalog: ServiceCatalog::Postgres,
            image: "postgres:16-alpine".to_string(),
            host_port: 5432,
            container_port: 5432,
            tag: Some("dev".to_string()),
            volume_path: PathBuf::from("/tmp/d4c_vol_test"),
            network: None,
            cpus: Some("1.0".to_string()),
            memory: Some("512m".to_string()),
            branch_parent: None,
            created_at: "2026-10-12 12:00:00 UTC".to_string(),
            env: HashMap::new(),
        };

        store.services.insert("test-db".to_string(), svc);
        let serialized = serde_json::to_string(&store).expect("Failed to serialize");
        let deserialized: StateStore = serde_json::from_str(&serialized).expect("Failed to deserialize");

        assert!(deserialized.services.contains_key("test-db"));
        assert_eq!(deserialized.services["test-db"].catalog, ServiceCatalog::Postgres);
    }
}
