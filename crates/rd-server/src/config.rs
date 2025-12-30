use serde::{Deserialize, Serialize};
use figment::{Figment, providers::{Format, Toml, Env}};
use anyhow::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub bind_address: String,
    pub max_sessions: usize,
    pub relay_enabled: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_address: "0.0.0.0:4433".to_string(),
            max_sessions: 100,
            relay_enabled: true,
        }
    }
}

impl ServerConfig {
    pub fn load() -> Result<Self> {
        let config: ServerConfig = Figment::new()
            .merge(Toml::file("config/server.toml"))
            .merge(Env::prefixed("RD_SERVER_"))
            .extract()
            .unwrap_or_default();
        
        Ok(config)
    }
}
