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
    /// `None` / `"auto"` follows the OS locale; otherwise a BCP-47 tag.
    #[serde(default)]
    pub locale: Option<String>,
    /// Which modifier turns a wheel/two-finger scroll into zoom or sideways pan.
    #[serde(default)]
    pub wheel: WheelBindings,
    #[serde(default)]
    pub table_copy_format: TableCopyFormat,
    #[serde(default)]
    pub analysis_split_ratio: Option<f32>,
}

/// Excel accepts tab-separated clipboard text directly as worksheet cells.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TableCopyFormat {
    #[default]
    Excel,
    Csv,
}

impl TableCopyFormat {
    pub(crate) fn key(self) -> &'static str {
        match self {
            Self::Excel => "excel",
            Self::Csv => "csv",
        }
    }

    pub(crate) fn delimiter(self) -> char {
        match self {
            Self::Excel => '\t',
            Self::Csv => ',',
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Preferences, TableCopyFormat};

    #[test]
    fn table_copy_defaults_to_excel_and_preserves_saved_choices() {
        let old: Preferences = serde_json::from_str(r#"{"theme":"dark"}"#).unwrap();
        assert_eq!(old.table_copy_format, TableCopyFormat::Excel);
        assert_eq!(
            Preferences::default().table_copy_format,
            TableCopyFormat::Excel
        );
        for format in [TableCopyFormat::Excel, TableCopyFormat::Csv] {
            let prefs = Preferences {
                table_copy_format: format,
                ..Preferences::default()
            };
            let loaded: Preferences =
                serde_json::from_str(&serde_json::to_string(&prefs).unwrap()).unwrap();
            assert_eq!(loaded.table_copy_format, format);
        }
    }
}

/// Modifier per wheel action. Values are the canonical names used by
/// [`crate::wheel`]: `"none"` or a canonical `ctrl+alt+shift` subset.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WheelBindings {
    /// Plain wheel / touchpad vertical scroll.
    pub vertical: String,
    /// Modifier for horizontal panning.
    pub horizontal: String,
    /// Modifier for zooming around the cursor.
    pub zoom: String,
}

impl Default for WheelBindings {
    fn default() -> Self {
        Self {
            vertical: "none".to_string(),
            horizontal: "ctrl".to_string(),
            zoom: "alt".to_string(),
        }
    }
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
