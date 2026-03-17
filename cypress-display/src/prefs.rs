// Required Notice: Copyright (c) 2026 Omair Kamil
// See LICENSE file in root directory for license terms.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const PREFS_FILENAME: &str = "cb_prefs.json";

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct AppPrefs {
    pub brightness: Option<u8>,
    pub rotation: Option<u16>,
}

pub fn get_prefs_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut path = std::env::current_exe()?;
    path.pop();
    path.push(PREFS_FILENAME);
    Ok(path)
}

fn load_prefs() -> AppPrefs {
    if let Ok(path) = get_prefs_path() {
        if let Ok(contents) = std::fs::read_to_string(path) {
            return serde_json::from_str::<AppPrefs>(&contents).unwrap_or_default();
        }
    }
    AppPrefs::default()
}

fn save_prefs(prefs: &AppPrefs) {
    if let Ok(path) = get_prefs_path() {
        if let Ok(data) = serde_json::to_string_pretty(prefs) {
            let _ = std::fs::write(path, data);
        }
    }
}

pub fn load_brightness() -> u8 {
    load_prefs().brightness.unwrap_or(0x80)
}

pub fn save_brightness(brightness: u8) {
    let mut prefs = load_prefs();
    prefs.brightness = Some(brightness);
    save_prefs(&prefs);
}

pub fn load_rotation() -> u16 {
    load_prefs().rotation.unwrap_or(0)
}

pub fn save_rotation(rotation: u16) {
    let mut prefs = load_prefs();
    prefs.rotation = Some(rotation);
    save_prefs(&prefs);
}
