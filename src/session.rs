//! Settings and session persistence.
//!
//! Everything lives under `%APPDATA%\MaterialNotepad` (or the platform's config
//! directory equivalent): `settings.json`, `session.json` and `backups/` for
//! unsaved buffers restored on the next start.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::document::{Encoding, Eol};
use crate::theme::SEEDS;
use eframe::egui::Color32;

pub const APP_NAME: &str = "MaterialNotepad";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub dark_mode: bool,
    pub seed: [u8; 3],
    pub ui_scale: f32,
    pub editor_font_size: f32,
    pub tab_width: usize,
    pub spaces_for_tabs: bool,
    pub word_wrap: bool,
    pub line_numbers: bool,
    pub restore_session: bool,
    pub recent: Vec<String>,
    pub last_folder: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            dark_mode: true,
            seed: [SEEDS[0].1.r(), SEEDS[0].1.g(), SEEDS[0].1.b()],
            ui_scale: 1.0,
            editor_font_size: 14.0,
            tab_width: 4,
            spaces_for_tabs: true,
            word_wrap: false,
            line_numbers: true,
            restore_session: true,
            recent: Vec::new(),
            last_folder: None,
        }
    }
}

impl Settings {
    pub fn seed_color(&self) -> Color32 {
        Color32::from_rgb(self.seed[0], self.seed[1], self.seed[2])
    }

    pub fn sanitize(&mut self) {
        self.ui_scale = self.ui_scale.clamp(0.75, 1.75);
        self.editor_font_size = self.editor_font_size.clamp(9.0, 32.0);
        self.tab_width = self.tab_width.clamp(1, 16);
        self.recent.truncate(12);
        let mut seen = std::collections::HashSet::new();
        self.recent.retain(|path| seen.insert(path.clone()));
    }

    pub fn push_recent(&mut self, path: &Path) {
        let text = path.to_string_lossy().into_owned();
        self.recent.retain(|p| p != &text);
        self.recent.insert(0, text);
        self.recent.truncate(12);
    }
}

pub fn config_dir() -> PathBuf {
    if let Ok(appdata) = std::env::var("APPDATA") {
        return PathBuf::from(appdata).join(APP_NAME);
    }
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(xdg).join(APP_NAME);
    }
    PathBuf::from(".config").join(APP_NAME)
}

fn settings_path() -> PathBuf {
    config_dir().join("settings.json")
}

fn session_path() -> PathBuf {
    config_dir().join("session.json")
}

fn backups_dir() -> PathBuf {
    config_dir().join("backups")
}

/// Read a JSON config file, ignoring a UTF-8 byte order mark. Editors and
/// Windows tooling often add one, and `serde_json` rejects it.
fn read_json(path: &std::path::Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes);
    String::from_utf8(bytes.to_vec()).ok()
}

pub fn load_settings() -> Settings {
    let path = settings_path();
    let mut settings = read_json(&path)
        .and_then(|text| serde_json::from_str::<Settings>(&text).ok())
        .unwrap_or_default();
    settings.sanitize();
    settings
}

pub fn save_settings(settings: &Settings) -> io::Result<()> {
    let dir = config_dir();
    fs::create_dir_all(&dir)?;
    let text = serde_json::to_string_pretty(settings).map_err(io::Error::other)?;
    write_atomic(&settings_path(), text.as_bytes())
}

fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(tmp, path)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionFile {
    pub path: Option<String>,
    pub backup: Option<String>,
    pub line: usize,
    pub column: usize,
    pub encoding: Encoding,
    pub eol: Eol,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Session {
    pub files: Vec<SessionFile>,
    pub active: usize,
    pub side_panel: bool,
}

pub fn load_session() -> Session {
    read_json(&session_path())
        .and_then(|text| serde_json::from_str::<Session>(&text).ok())
        .unwrap_or_default()
}

pub fn save_session(session: &Session) -> io::Result<()> {
    let dir = config_dir();
    fs::create_dir_all(&dir)?;
    let text = serde_json::to_string_pretty(session).map_err(io::Error::other)?;
    write_atomic(&session_path(), text.as_bytes())
}

/// Persist an unsaved buffer so it survives a restart.
pub fn write_backup(id: u64, text: &str) -> Option<String> {
    let dir = backups_dir();
    fs::create_dir_all(&dir).ok()?;
    let name = format!("{id}.txt");
    fs::write(dir.join(&name), text.as_bytes()).ok()?;
    Some(name)
}

pub fn read_backup(name: &str) -> Option<String> {
    let path = backups_dir().join(name);
    fs::read_to_string(path).ok()
}

pub fn remove_backup(name: &str) {
    let _ = fs::remove_file(backups_dir().join(name));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip() {
        let settings = Settings {
            seed: [1, 2, 3],
            recent: vec!["a.txt".into()],
            ..Default::default()
        };
        let text = serde_json::to_string(&settings).unwrap();
        let back: Settings = serde_json::from_str(&text).unwrap();
        assert_eq!(back.seed, [1, 2, 3]);
        assert_eq!(back.recent, vec!["a.txt".to_owned()]);
    }

    #[test]
    fn sanitize_clamps_values() {
        let mut settings = Settings {
            ui_scale: 99.0,
            tab_width: 0,
            editor_font_size: -2.0,
            ..Default::default()
        };
        settings.sanitize();
        assert_eq!(settings.ui_scale, 1.75);
        assert_eq!(settings.tab_width, 1);
        assert_eq!(settings.editor_font_size, 9.0);
    }

    #[test]
    fn recent_files_are_unique_and_ordered() {
        let mut settings = Settings::default();
        settings.push_recent(Path::new("a.txt"));
        settings.push_recent(Path::new("b.txt"));
        settings.push_recent(Path::new("a.txt"));
        assert_eq!(settings.recent, vec!["a.txt", "b.txt"]);
    }
}
