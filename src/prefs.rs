use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const RECENT_MAX: usize = 12;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Preferences {
    #[serde(default)]
    pub last_workdir: Option<PathBuf>,
    #[serde(default)]
    pub last_project: Option<PathBuf>,
    #[serde(default)]
    pub recent_projects: Vec<PathBuf>,
    #[serde(default)]
    pub recent_workdirs: Vec<PathBuf>,
    /// "auto", "light" or "dark".
    #[serde(default = "default_theme")]
    pub theme: String,
}

fn default_theme() -> String {
    "auto".to_string()
}

impl Preferences {
    pub fn path() -> Option<PathBuf> {
        ProjectDirs::from("org", "opensourcephysics", "tracker-gpui")
            .map(|dirs| dirs.config_dir().join("preferences.json"))
    }

    pub fn load() -> Self {
        let Some(path) = Self::path() else {
            return Self::default();
        };
        let Ok(text) = fs::read_to_string(path) else {
            return Self::default();
        };
        serde_json::from_str(&text).unwrap_or_default()
    }

    pub fn save(&self) {
        let Some(path) = Self::path() else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(text) = serde_json::to_string_pretty(self) {
            let _ = fs::write(path, text);
        }
    }

    pub fn note_workdir(&mut self, path: &Path) {
        self.last_workdir = Some(path.to_path_buf());
        note_recent(&mut self.recent_workdirs, path);
        self.save();
    }

    pub fn note_project(&mut self, path: &Path) {
        self.last_project = Some(path.to_path_buf());
        note_recent(&mut self.recent_projects, path);
        self.save();
    }
}

fn note_recent(items: &mut Vec<PathBuf>, path: &Path) {
    items.retain(|item| item != path);
    items.insert(0, path.to_path_buf());
    items.truncate(RECENT_MAX);
}
