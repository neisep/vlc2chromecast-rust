use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Config {
    pub chromecast_ip: String,
    pub vlc_path: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            chromecast_ip: String::new(),
            vlc_path: default_vlc_path(),
        }
    }
}

fn default_vlc_path() -> String {
    if cfg!(target_os = "windows") {
        r"C:\Program Files\VideoLAN\VLC\vlc.exe".to_string()
    } else {
        "vlc".to_string()
    }
}

fn config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("vlc2chromecast").join("settings.json"))
}

impl Config {
    pub fn load() -> Self {
        let Some(path) = config_path() else {
            return Config::default();
        };
        match fs::read_to_string(&path) {
            Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
            Err(_) => Config::default(),
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let path = config_path().ok_or("Could not determine config directory")?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config directory: {e}"))?;
        }
        let json =
            serde_json::to_string_pretty(self).map_err(|e| format!("Failed to serialize: {e}"))?;
        fs::write(&path, json).map_err(|e| format!("Failed to write config: {e}"))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_empty_ip() {
        let config = Config::default();
        assert!(config.chromecast_ip.is_empty());
        assert!(!config.vlc_path.is_empty());
    }

    #[test]
    fn serialize_deserialize_roundtrip() {
        let config = Config {
            chromecast_ip: "192.168.1.100".to_string(),
            vlc_path: "/usr/bin/vlc".to_string(),
        };
        let json = serde_json::to_string(&config).unwrap();
        let restored: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.chromecast_ip, "192.168.1.100");
        assert_eq!(restored.vlc_path, "/usr/bin/vlc");
    }

    #[test]
    fn deserialize_partial_json_fills_defaults() {
        let json = r#"{"chromecast_ip": "192.168.1.50"}"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.chromecast_ip, "192.168.1.50");
        assert_eq!(config.vlc_path, default_vlc_path());
    }

    #[test]
    fn deserialize_invalid_json_returns_default() {
        let result: Result<Config, _> = serde_json::from_str("not json");
        assert!(result.is_err());
    }
}
