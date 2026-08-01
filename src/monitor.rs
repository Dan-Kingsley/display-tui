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
    pub fn save_hyprland_config(path: &str, monitors: &Vec<Monitor>) -> std::io::Result<()> {
        if Monitor::is_lua_config(path) {
            Monitor::save_lua_config(path, monitors)
        } else {
            Monitor::save_traditional_config(path, monitors)
        }
    }

    fn save_traditional_config(path: &str, monitors: &Vec<Monitor>) -> std::io::Result<()> {
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

    fn save_lua_config(path: &str, monitors: &Vec<Monitor>) -> std::io::Result<()> {
        let expanded_path = shellexpand::tilde(path).to_string();
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(expanded_path)?;

        writeln!(file, "-- Monitor wiki https://wiki.hypr.land/Configuring/Basics/Monitors/")?;
        writeln!(file)?;

        for monitor in monitors {
            let block = monitor.to_lua_monitor_block();
            writeln!(file, "{}", block)?;
        }
        Ok(())
    }

    fn to_lua_monitor_block(&self) -> String {
        if self.enabled {
            let mode = match self.get_current_resolution() {
                Some(m) => format!("{}x{}@{}", m.width, m.height, m.refresh),
                None => "preferred".to_string(),
            };
            let pos = match &self.position {
                Some(p) => format!("{}x{}", p.x, p.y),
                None => "0x0".to_string(),
            };
            let scale = self.scale.unwrap_or(1.0);
            let scale_str = if scale == scale.floor() {
                format!("{}", scale as i32)
            } else {
                format!("{}", scale)
            };
            format!(
                "hl.monitor({{\n    output    = \"{}\",\n    mode      = \"{}\",\n    position  = \"{}\",\n    scale     = \"{}\",\n}})",
                self.name, mode, pos, scale_str
            )
        } else {
            format!(
                "hl.monitor({{\n    output    = \"{}\",\n    mode      = \"disabled\",\n}})",
                self.name
            )
        }
    }

    pub fn is_lua_config(path: &str) -> bool {
        path.ends_with(".lua")
    }

    pub fn load_from_hyprland_config(path: &str, monitors: &mut Vec<Monitor>) {
        let expanded_path = shellexpand::tilde(path).to_string();
        if !std::path::Path::new(&expanded_path).exists() {
            return;
        }
        if Monitor::is_lua_config(path) {
            Monitor::load_from_lua_config(path, monitors);
        } else {
            Monitor::load_from_traditional_config(path, monitors);
        }
    }

    fn load_from_traditional_config(path: &str, monitors: &mut Vec<Monitor>) {
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

    fn load_from_lua_config(path: &str, monitors: &mut Vec<Monitor>) {
        let expanded_path = shellexpand::tilde(path).to_string();
        let content = match std::fs::read_to_string(&expanded_path) {
            Ok(c) => c,
            Err(_) => return,
        };

        // Find all hl.monitor({...}) blocks
        let mut block_start = 0;
        while let Some(start) = content[block_start..].find("hl.monitor(") {
            let abs_start = block_start + start;
            // Find the matching closing paren
            let block_content = &content[abs_start..];
            let mut depth = 0;
            let mut block_end = 0;
            for (i, ch) in block_content.char_indices() {
                match ch {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            block_end = i + 1;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            if block_end == 0 {
                break;
            }

            let block = &content[abs_start..abs_start + block_end];
            Monitor::parse_lua_monitor_block(block, monitors);
            block_start = abs_start + block_end;
        }
    }

    fn parse_lua_monitor_block(block: &str, monitors: &mut Vec<Monitor>) {
        // Extract field values from the block
        let output = Monitor::extract_lua_field(block, "output");
        let mode = Monitor::extract_lua_field(block, "mode");
        let position = Monitor::extract_lua_field(block, "position");
        let scale = Monitor::extract_lua_field(block, "scale");

        let name = match output {
            Some(n) => n,
            None => return,
        };

        let monitor = match monitors.iter_mut().find(|m| m.name == name) {
            Some(m) => m,
            None => return,
        };

        // Mode
        if let Some(ref mode_val) = mode {
            if mode_val == "disabled" {
                monitor.enabled = false;
                return;
            }
            monitor.enabled = true;

            if mode_val == "preferred" || mode_val == "highres" {
                if let Some(pref_pos) = monitor.modes.iter().position(|m| m.preferred) {
                    monitor.set_current_resolution(pref_pos);
                }
            } else if let Some(pos) = monitor.modes.iter().position(|m| {
                let full = format!("{}x{}@{}", m.width, m.height, m.refresh);
                let short = format!("{}x{}", m.width, m.height);
                mode_val == &full || mode_val == &short
            }) {
                monitor.set_current_resolution(pos);
            }
        }

        // Position
        if let Some(ref pos_val) = position {
            if pos_val != "auto" {
                let coords: Vec<&str> = pos_val.split('x').collect();
                if coords.len() == 2 {
                    if let (Ok(x), Ok(y)) = (coords[0].parse::<i32>(), coords[1].parse::<i32>()) {
                        monitor.position = Some(Position { x, y });
                    }
                }
            }
        }

        // Scale
        if let Some(ref scale_val) = scale {
            if scale_val != "auto" {
                if let Ok(s) = scale_val.parse::<f32>() {
                    monitor.scale = Some(s);
                }
            }
        }
    }

    fn extract_lua_field(block: &str, field_name: &str) -> Option<String> {
        // Find the field name followed by = and extract the value
        for line in block.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix(field_name) {
                let rest = rest.trim();
                if let Some(eq_rest) = rest.strip_prefix('=') {
                    let value = eq_rest.trim();
                    // Take value up to comma or end of line
                    let value = value.split(|c: char| c == ',').next()?.trim();
                    // Remove quotes if present
                    if (value.starts_with('"') && value.ends_with('"'))
                        || (value.starts_with('\'') && value.ends_with('\''))
                    {
                        return Some(value[1..value.len() - 1].to_string());
                    } else {
                        return Some(value.to_string());
                    }
                }
            }
        }
        None
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
