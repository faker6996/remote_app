use serde::{Deserialize, Serialize};
use figment::{Figment, providers::{Format, Toml, Env}};
use anyhow::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub device_id: String,
    pub server_url: String,
    pub max_fps: u8,
    pub encoder_quality: u8,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            device_id: hostname::get()
                .ok()
                .and_then(|h| h.into_string().ok())
                .unwrap_or_else(|| "unknown-device".to_string()),
            server_url: "127.0.0.1:4433".to_string(),
            max_fps: 30,
            encoder_quality: 80,
        }
    }
}

impl AgentConfig {
    pub fn load() -> Result<Self> {
        let config: AgentConfig = Figment::new()
            .merge(Toml::file("config/agent.toml"))
            .merge(Env::prefixed("RD_AGENT_"))
            .extract()
            .unwrap_or_default();
        
        Ok(config)
    }
}
