use crate::rotation::Rotation;
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::io::Write;
use ratatui::layout::Rect;
#[derive(Debug,Default, Clone, Deserialize, Serialize)]
pub struct Monitor {
    pub name: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub modes: Vec<Resolution>,
    pub position: Option<Position>,
    pub scale: Option<f32>,
    pub transform: Option<String>,
    #[serde(skip)]
    pub saved_position: Option<Position>,
    #[serde(skip)]
    pub saved_scale: Option<f32>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Position{
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Resolution {
    pub width: i32,
    pub height: i32,
    pub refresh: f32,
    pub preferred: bool,
    pub current: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MonitorCanvas{
    pub top: i32,
    pub x_bounds: [f64; 2],
    pub y_bounds: [f64; 2],
    pub offset_y: i32,
}


impl Monitor {

    pub fn get_monitors() -> Vec<Monitor> {
        let output = Command::new("wlr-randr")
            .arg("--json")
            .output().expect("Failed to execute wlr-randr command");
        let stdout = String::from_utf8(output.stdout).expect("Failed to convert output to string");
        let new_monitors: Vec<Monitor> = match serde_json::from_str(&stdout) {
            Ok(monitors) => monitors,
            Err(e) => {
                eprintln!("Deserialization error: {}", e);
                Vec::new()
            }
        };

        new_monitors
    }
    pub fn get_monitors_canvas(monitors: &Vec<Monitor>, _area: &Rect) -> MonitorCanvas {
        let mut left = 10000.0;
        let mut bottom = 10000.0;
        let mut right = -10000.0;
        let mut top = -10000.0;

        for monitor in monitors {
            if !monitor.enabled {
                continue;
            }
            let mut mode = monitor.get_current_resolution();
            if mode.is_none() {
                mode = monitor.get_prefered_resolution();
            }

            let rotation = Rotation::from_transform(&monitor.transform);
            let (width, height) = if rotation == Rotation::Deg90 || rotation == Rotation::Deg270 {
                (mode.unwrap().height, mode.unwrap().width)
            } else {
                (mode.unwrap().width, mode.unwrap().height)
            };

            let monitor_left = monitor.position.clone().unwrap().x as f64;
            let monitor_right = monitor_left  + (width as f64 / monitor.scale.unwrap() as f64);

            let monitor_bottom = monitor.position.clone().unwrap().y as f64;
            let monitor_top = monitor_bottom + (height as f64 / monitor.scale.unwrap() as f64);
            
            if monitor_right > right {
                right= monitor_right;
            }
            if monitor_top > top {
                top= monitor_top;
            }
            if monitor_left < left {
                left= monitor_left;
            }
            if monitor_bottom < bottom {
                bottom= monitor_bottom;
            }
        }


        let margin = 50.0;
        left -= margin;
        bottom -= margin;
        right += margin;
        top += margin;

        let x_bounds = [left, right];
        let y_bounds = [bottom, top];

        let mut offset_y = 0.0;
        if bottom < 0.0 {
             offset_y = -bottom;
        }
       
        MonitorCanvas {
            top: top as i32,
            x_bounds,
            y_bounds,
            offset_y: offset_y as i32,
        }

    }

    pub fn get_current_resolution(&self) -> Option<&Resolution> {
        self.modes
            .iter()
            .find(|m| m.current)
    }

    pub fn get_prefered_resolution(&self) -> Option<&Resolution> {
        self.modes
            .iter()
            .find(|m| m.preferred)
    }
    
    pub fn set_current_resolution(&mut self, index: usize) {
        if index < self.modes.len() {
            for mode in &mut self.modes {
                mode.current = false;
            }
            self.modes[index].current = true;
        } else {
            eprintln!("Index out of bounds: {}", index);
        }
    }

    pub fn to_hyprland_config(&self) -> String {
        let mode = match self.get_current_resolution() {
            Some(m) => m,
            None => {
                self.get_prefered_resolution().expect("No preferred resolution found")
            }
        };
        if self.enabled {
            let rotation = Rotation::from_transform(&self.transform);
            format!(
                "monitor = {}, {}x{}@{}, {}x{}, {}, transform,{}",
                self.name,
                mode.width, mode.height, mode.refresh,
                self.position.clone().unwrap().x, self.position.clone().unwrap().y,
                self.scale.unwrap_or(1.0),
                rotation.to_hyprland()
            )
        } else {
            format!(
                "monitor = {}, disabled",
                self.name
            )
        }
        
    }
    pub fn save_hyprland_config(path:&String,monitors: &Vec<Monitor>) -> std::io::Result<()> {
        let expanded_path = shellexpand::tilde(path).to_string();
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(expanded_path)?;
        for monitor in monitors {
            let config_line = monitor.to_hyprland_config();
            writeln!(file, "{}", config_line)?;
        }
        Ok(())
    }

    pub fn load_from_hyprland_config(path: &str, monitors: &mut Vec<Monitor>) {
        let expanded_path = shellexpand::tilde(path).to_string();
        if let Ok(content) = std::fs::read_to_string(expanded_path) {
            for line in content.lines() {
                let line = line.trim();
                if !line.starts_with("monitor") {
                    continue;
                }
                let parts: Vec<&str> = line
                    .splitn(2, '=')
                    .nth(1)
                    .unwrap_or("")
                    .split(',')
                    .map(|s| s.trim())
                    .collect();

                if parts.len() < 2 {
                    continue;
                }

                let name = parts[0];
                if let Some(monitor) = monitors.iter_mut().find(|m| m.name == name) {
                    if parts[1] == "disabled" {
                        monitor.enabled = false;
                        continue;
                    }

                    monitor.enabled = true;

                    // Resolution: e.g. 1920x1080@60 or 1920x1080 or preferred
                    if let Some(res_part) = parts.get(1) {
                        if let Some(pos) = monitor.modes.iter().position(|m| {
                            let full = format!("{}x{}@{}", m.width, m.height, m.refresh);
                            let short = format!("{}x{}", m.width, m.height);
                            res_part == &full || res_part == &short
                        }) {
                            monitor.set_current_resolution(pos);
                        } else if *res_part == "preferred" || *res_part == "highres" {
                            if let Some(pref_pos) = monitor.modes.iter().position(|m| m.preferred) {
                                monitor.set_current_resolution(pref_pos);
                            }
                        }
                    }

                    // Position: e.g. 0x0 or 1920x0
                    if let Some(pos_part) = parts.get(2) {
                        let coords: Vec<&str> = pos_part.split('x').collect();
                        if coords.len() == 2 {
                            if let (Ok(x), Ok(y)) = (coords[0].parse::<i32>(), coords[1].parse::<i32>()) {
                                monitor.position = Some(Position { x, y });
                            }
                        }
                    }

                    // Scale: e.g. 1 or 1.5
                    if let Some(scale_part) = parts.get(3) {
                        if let Ok(scale) = scale_part.parse::<f32>() {
                            monitor.scale = Some(scale);
                        }
                    }

                    // Transform: e.g. transform, 1
                    if parts.len() >= 6 && parts[4] == "transform" {
                        if let Ok(rot_id) = parts[5].parse::<i32>() {
                            monitor.transform = Some(match rot_id {
                                1 => "90".to_string(),
                                2 => "180".to_string(),
                                3 => "270".to_string(),
                                _ => "normal".to_string(),
                            });
                        }
                    }
                }
            }
        }
    }

    pub fn move_vertical(&mut self, direction: i32) {
        if let Some(ref mut pos) = self.position { pos.y += direction};
    }

    pub fn move_horizontal(&mut self, direction: i32) {
        if let Some(ref mut pos) = self.position { pos.x += direction};
    }

    pub fn get_geometry(&self) -> (f64, f64, f64, f64) {
        let mut mode = self.get_current_resolution();
        if mode.is_none() {
            mode = self.get_prefered_resolution();
        }
        
        if mode.is_none() { return (0.0,0.0,0.0,0.0); }

        let rotation = Rotation::from_transform(&self.transform);
        let (width, height) = if rotation == Rotation::Deg90 || rotation == Rotation::Deg270 {
            (mode.unwrap().height, mode.unwrap().width)
        } else {
            (mode.unwrap().width, mode.unwrap().height)
        };

        let scale = self.scale.unwrap_or(1.0);
        let logical_width = width as f64 / scale as f64;
        let logical_height = height as f64 / scale as f64;
        let x = self.position.clone().unwrap().x as f64;
        let y = self.position.clone().unwrap().y as f64;

        (x, y, logical_width, logical_height)
    }
}
