use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credentials {
    pub token: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub lastfm_session_key: Option<String>,
    #[serde(default)]
    pub stream_format: Option<String>,
    #[serde(default)]
    pub stream_bitrate: Option<u32>,
    #[serde(default = "default_volume")]
    pub volume: u8,
}

fn default_volume() -> u8 {
    100
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            url: None,
            token: None,
            lastfm_session_key: None,
            stream_format: None,
            stream_bitrate: None,
            volume: 100,
        }
    }
}

pub fn config_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config")
        .join("supernova")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.json")
}

pub fn legacy_credentials_path() -> PathBuf {
    config_dir().join("credentials.json")
}

/// Loads `~/.config/supernova/config.json`, falling back to `~/.config/supernova/credentials.json`
/// for `url`/`token` if `config.json` does not exist or lacks auth fields.
pub fn load_config() -> ClientConfig {
    let mut cfg: ClientConfig = fs::read_to_string(config_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    if cfg.token.is_none() || cfg.url.is_none() {
        if let Some(creds) = fs::read_to_string(legacy_credentials_path())
            .ok()
            .and_then(|s| serde_json::from_str::<Credentials>(&s).ok())
        {
            if cfg.url.is_none() && !creds.url.is_empty() {
                cfg.url = Some(creds.url);
            }
            if cfg.token.is_none() && !creds.token.is_empty() {
                cfg.token = Some(creds.token);
            }
        }
    }

    cfg
}

pub fn save_config(cfg: &ClientConfig) -> Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("create supernova config dir")?;
    }
    let data = serde_json::to_string_pretty(cfg)?;
    fs::write(&path, data).context("write config.json")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}
