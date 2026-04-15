use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server_url: String,
    pub device_name: String,
    pub server_id: Option<String>,
    /// OAuth access token. Stored in the config file (mode 0600) instead of
    /// the OS keychain because keychain ACLs can deny background-thread
    /// access on macOS for unsigned dev builds. Will move back to keychain
    /// once the app is properly code-signed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        let hostname = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "Unknown".to_string());

        Self {
            server_url: "https://useflash.com".to_string(),
            device_name: hostname,
            server_id: None,
            access_token: None,
        }
    }
}

impl Config {
    fn config_path() -> PathBuf {
        let dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("flash-desktop");
        fs::create_dir_all(&dir).ok();
        dir.join("config.json")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        let raw = match fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[config] read failed at {:?}: {}", path, e);
                return Self::default();
            }
        };
        match serde_json::from_str::<Self>(&raw) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[config] parse failed: {} (raw: {})", e, raw);
                Self::default()
            }
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_path();
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        eprintln!("[config] saving to {:?}", path);
        fs::write(&path, json).map_err(|e| e.to_string())?;
        // Restrict to user-only since the file holds the OAuth token
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).ok();
        }
        Ok(())
    }
}
