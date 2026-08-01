use std::path::{Path, PathBuf};
use std::fs;
use serde::{Deserialize, Serialize};
use crate::monitor::{Monitor, Position};

#[derive(Debug,Default, Clone, Deserialize)]
pub struct Configuration {
    pub monitors_config_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorState {
    pub name: String,
    pub position: Option<Position>,
    pub scale: Option<f32>,
}
impl Configuration {
    pub fn get() -> Self {
        let config_json_path = dirs::home_dir()
             .map(|p| p.join(".config/display-tui/config.json"))
             .unwrap_or_else(|| Path::new("~/.config/display-tui/config.json").to_path_buf());
        match !config_json_path.exists() {
            true => {
                Configuration::create_default_config(&config_json_path)
            },
            false => {
                Configuration::load_config()
            }
        }
    }

    pub fn load_monitor_state() -> Option<Vec<MonitorState>> {
        let state_path = dirs::home_dir()
            .map(|p| p.join(".config/display-tui/monitor_state.json"))
            .unwrap_or_else(|| Path::new("~/.config/display-tui/monitor_state.json").to_path_buf());
        
        if !state_path.exists() {
            return None;
        }

        let content = fs::read_to_string(&state_path).ok()?;
        serde_json::from_str(&content).ok()
    }

    pub fn save_monitor_state(monitors: &Vec<Monitor>) -> std::io::Result<()> {
        let state_path = dirs::home_dir()
            .map(|p| p.join(".config/display-tui/monitor_state.json"))
            .unwrap_or_else(|| Path::new("~/.config/display-tui/monitor_state.json").to_path_buf());
        
        fs::create_dir_all(state_path.parent().unwrap())?;
        
        let state: Vec<MonitorState> = monitors
            .iter()
            .map(|m| MonitorState {
                name: m.name.clone(),
                position: m.position.clone(),
                scale: m.scale,
            })
            .collect();
        
        let json = serde_json::to_string_pretty(&state)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        fs::write(state_path, json)?;
        
        Ok(())
    }

    fn create_default_config(config_json_path: &PathBuf) -> Self {
        let monitors_config_path = Configuration::detect_monitors_config_path();
        let default_config = format!("{{\n  \"monitors_config_path\": \"{}\"\n}}", monitors_config_path);
        fs::create_dir_all(config_json_path.parent().unwrap()).expect("Failed to create config directory");
        fs::write(config_json_path, default_config).expect("Failed to write default config file");
        Configuration {
            monitors_config_path,
        } 
    }

    fn detect_monitors_config_path() -> String {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("~"));

        // CachyOS / Lua config path
        let lua_path = home.join(".config/hypr/config/monitors.lua");
        if lua_path.exists() {
            return "~/.config/hypr/config/monitors.lua".to_string();
        }

        // Traditional Hyprland config path
        let traditional_path = home.join(".config/hypr/hyprland/monitors.conf");
        if traditional_path.exists() {
            return "~/.config/hypr/hyprland/monitors.conf".to_string();
        }

        // Default to traditional path if neither exists
        "~/.config/hypr/hyprland/monitors.conf".to_string()
    }
    fn load_config() -> Self {
        let config_json_path = dirs::home_dir()
            .map(|p| p.join(".config/display-tui/config.json"))
            .unwrap_or_else(|| Path::new("~/.config/display-tui/config.json").to_path_buf());
        
        let config_content = fs::read_to_string(config_json_path)
            .expect("Failed to read config file");
        
        let mut config: Configuration = serde_json::from_str(&config_content)
            .expect("Failed to parse config file");

        // If configured path doesn't exist, try to auto-detect
        let expanded = shellexpand::tilde(&config.monitors_config_path).to_string();
        if !std::path::Path::new(&expanded).exists() {
            let detected = Configuration::detect_monitors_config_path();
            let detected_expanded = shellexpand::tilde(&detected).to_string();
            if std::path::Path::new(&detected_expanded).exists() {
                config.monitors_config_path = detected;
            }
        }

        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitor::{Monitor, Position};

    #[test]
    fn test_save_and_load_monitor_state() {
        // Create mock monitors
        let monitors = vec![
            Monitor {
                name: "HDMI-A-1".to_string(),
                position: Some(Position { x: 100, y: 200 }),
                scale: Some(1.5),
                enabled: true,
                ..Default::default()
            },
            Monitor {
                name: "DP-1".to_string(),
                position: Some(Position { x: 300, y: 400 }),
                scale: Some(1.0),
                enabled: true,
                ..Default::default()
            },
        ];

        // Save
        Configuration::save_monitor_state(&monitors).expect("Failed to save");

        // Load
        let loaded = Configuration::load_monitor_state().expect("Failed to load");

        // Verify
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].name, "HDMI-A-1");
        assert_eq!(loaded[0].position, Some(Position { x: 100, y: 200 }));
        assert_eq!(loaded[0].scale, Some(1.5));

        assert_eq!(loaded[1].name, "DP-1");
        assert_eq!(loaded[1].position, Some(Position { x: 300, y: 400 }));
        assert_eq!(loaded[1].scale, Some(1.0));
    }
}
