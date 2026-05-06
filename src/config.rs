use serde::Deserialize;
use std::path::Path;

pub const DEFAULT_CONFIG_FILE: &str = "config/default";

#[derive(Debug, Deserialize, Clone)]
pub struct ServerSettings {
    pub http_port: u16,
    pub https_port: u16,
    pub http_enabled: bool,
    pub https_enabled: bool,
    pub behind_proxy: bool,
    #[serde(default)]
    pub public_base_url: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RegistrySettings {
    #[serde(default = "default_registry_name")]
    pub default: String,
    #[serde(default)]
    pub upstream_registry: Option<String>,
    #[serde(default)]
    pub auth_realm: Option<String>,
    #[serde(default)]
    pub auth_service: Option<String>,
    #[serde(default = "default_true")]
    pub auto_library_prefix: bool,
    #[serde(default)]
    pub public_base_url: Option<String>,
    #[serde(default)]
    pub upstreams: Vec<RegistryUpstreamSettings>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RegistryUpstreamSettings {
    pub name: String,
    #[serde(default)]
    pub hosts: Vec<String>,
    pub upstream_registry: String,
    pub auth_realm: String,
    pub auth_service: String,
    #[serde(default)]
    pub auto_library_prefix: bool,
    #[serde(default)]
    pub public_base_url: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TlsSettings {
    pub cert_path: String,
    pub key_path: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
    pub server: ServerSettings,
    pub registry: RegistrySettings,
    pub tls: TlsSettings,
}

impl Settings {
    pub fn new() -> Result<Self, config::ConfigError> {
        Self::from_file(DEFAULT_CONFIG_FILE)
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, config::ConfigError> {
        let builder = config::Config::builder().add_source(config::File::from(path.as_ref()));
        builder.build()?.try_deserialize()
    }
}

fn default_registry_name() -> String {
    "dockerhub".to_string()
}

fn default_true() -> bool {
    true
}
