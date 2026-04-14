use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Top-level config, deserialized from config.toml.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub organization: OrganizationConfig,
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub graph: GraphConfig,
    #[serde(default)]
    pub embeddings: EmbeddingsConfig,
    #[serde(default)]
    pub repos: Vec<RepoConfig>,
    #[serde(default)]
    pub deps: Vec<DepMapping>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationConfig {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_server_mode")]
    pub mode: String,
    #[serde(default = "default_server_port")]
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            mode: default_server_mode(),
            port: default_server_port(),
        }
    }
}

fn default_server_mode() -> String {
    "stdio".to_string()
}

fn default_server_port() -> u16 {
    3100
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphConfig {
    #[serde(default = "default_store")]
    pub store: String,
    #[serde(default = "default_max_hop_depth")]
    pub max_hop_depth: u32,
    #[serde(default = "default_branch_ttl")]
    pub branch_ttl: String,
}

impl Default for GraphConfig {
    fn default() -> Self {
        Self {
            store: default_store(),
            max_hop_depth: default_max_hop_depth(),
            branch_ttl: default_branch_ttl(),
        }
    }
}

fn default_store() -> String {
    "sqlite".to_string()
}

fn default_max_hop_depth() -> u32 {
    4
}

fn default_branch_ttl() -> String {
    "30d".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingsConfig {
    #[serde(default)]
    pub enabled: bool,
    pub store: Option<String>,
    pub model: Option<String>,
    pub model_path: Option<String>,
}

impl Default for EmbeddingsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            store: None,
            model: None,
            model_path: None,
        }
    }
}

/// A repo entry from [[repos]] in config.toml.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoConfig {
    pub name: String,
    pub root: PathBuf,
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
}

/// A dependency mapping from [[deps]] in config.toml.
/// Maps a package name to the repo that provides it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepMapping {
    pub package: String,
    pub repo: String,
}
