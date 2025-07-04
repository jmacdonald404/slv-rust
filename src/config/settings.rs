use crate::ui::PreferencesState;
use std::fs;
use std::path::PathBuf;
use directories::ProjectDirs;
use toml;
use serde::{Serialize, Deserialize};

const CONFIG_FILE: &str = "preferences.toml";

#[derive(Serialize, Deserialize)]
pub struct PreferencesToml {
    pub enable_sound: bool,
    pub volume: f32,
    pub graphics_api: String,
    pub vsync: bool,
    pub render_distance: u32,
    pub max_bandwidth: u32,
    pub timeout: u32,
}

impl From<&PreferencesState> for PreferencesToml {
    fn from(p: &PreferencesState) -> Self {
        Self {
            enable_sound: p.enable_sound,
            volume: p.volume,
            graphics_api: p.graphics_api.clone(),
            vsync: p.vsync,
            render_distance: p.render_distance,
            max_bandwidth: p.max_bandwidth,
            timeout: p.timeout,
        }
    }
}

impl Into<PreferencesState> for PreferencesToml {
    fn into(self) -> PreferencesState {
        PreferencesState {
            enable_sound: self.enable_sound,
            volume: self.volume,
            graphics_api: self.graphics_api,
            vsync: self.vsync,
            render_distance: self.render_distance,
            max_bandwidth: self.max_bandwidth,
            timeout: self.timeout,
        }
    }
}

fn config_path() -> Option<PathBuf> {
    ProjectDirs::from("com", "slv", "slv-rust")
        .map(|proj| proj.config_dir().join(CONFIG_FILE))
}

pub fn save_preferences(prefs: &PreferencesState) -> std::io::Result<()> {
    if let Some(path) = config_path() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let toml = toml::to_string_pretty(&PreferencesToml::from(prefs)).unwrap();
        fs::write(path, toml)?;
    }
    Ok(())
}

pub fn load_preferences() -> Option<PreferencesState> {
    if let Some(path) = config_path() {
        if let Ok(data) = fs::read_to_string(path) {
            if let Ok(toml) = toml::from_str::<PreferencesToml>(&data) {
                return Some(toml.into());
            }
        }
    }
    None
}
