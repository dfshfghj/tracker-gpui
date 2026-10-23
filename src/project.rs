use crate::coords::{Calibration, ImageCoordSystem};
use crate::plot::PlotState;
use crate::track::{Footprint, MeasurementKind};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const FORMAT: &str = "tracker-gpui-project";
// v4 adds derived-variable definitions and custom plot-axis references.
pub const VERSION: u32 = 4;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectFile {
    pub format: String,
    pub version: u32,
    #[serde(default)]
    pub workdir: Option<PathBuf>,
    #[serde(default)]
    pub active_doc: usize,
    #[serde(default)]
    pub docs: Vec<DocProject>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DocProject {
    pub path: PathBuf,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub frame: usize,
    #[serde(default)]
    pub tracks: Vec<TrackProject>,
    #[serde(default)]
    pub active_track: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coords: Option<ImageCoordSystem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calibration: Option<Calibration>,
    // v1 legacy fields; accepted on load, not written anymore.
    #[serde(default, skip_serializing)]
    pub cal_a: [f32; 2],
    #[serde(default, skip_serializing)]
    pub cal_b: [f32; 2],
    #[serde(default = "one", skip_serializing)]
    pub cal_m: f32,
    #[serde(default = "one")]
    pub scale: f32,
    #[serde(default)]
    pub tx: f32,
    #[serde(default)]
    pub ty: f32,
    #[serde(default)]
    pub user_adjusted: bool,
    #[serde(default)]
    pub axes_visible: bool,
    #[serde(default = "default_true")]
    pub trail_visible: bool,
    #[serde(default = "default_trail_length")]
    pub trail_length: usize,
    #[serde(default = "default_true")]
    pub labels_visible: bool,
    #[serde(default)]
    pub trace_visible: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plot: Option<PlotState>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrackProject {
    #[serde(default)]
    pub variables: Vec<crate::variables::Definition>,
    pub name: String,
    #[serde(default)]
    pub color: u32,
    #[serde(default = "default_true")]
    pub visible: bool,
    #[serde(default)]
    pub locked: bool,
    #[serde(default = "one_usize")]
    pub step_size: usize,
    #[serde(default)]
    pub auto_advance: bool,
    #[serde(default)]
    pub footprint: Footprint,
    #[serde(default)]
    pub keyframes: Vec<usize>,
    #[serde(default)]
    pub points: Vec<PointProject>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measurement: Option<MeasurementProject>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rigid_body: Option<RigidBodyProject>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RigidBodyProject {
    #[serde(default)]
    pub dictionary: String,
    #[serde(default)]
    pub markers: Vec<RigidBodyMarkerProject>,
    #[serde(default)]
    pub steps: Vec<RigidBodyStepProject>,
    #[serde(default)]
    pub contour: Vec<[f32; 2]>,
    #[serde(default)]
    pub mask_mode: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RigidBodyMarkerProject {
    pub id: u32,
    pub corners: [[f32; 2]; 4],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RigidBodyStepProject {
    pub frame: usize,
    #[serde(default)]
    pub angle: f32,
    #[serde(default)]
    pub scale: f32,
    #[serde(default)]
    pub marker_count: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contour: Option<Vec<[f32; 2]>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MeasurementProject {
    pub kind: String,
    #[serde(default)]
    pub steps: Vec<MeasurementStepProject>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MeasurementStepProject {
    pub frame: usize,
    pub handles: Vec<[f32; 2]>,
}

impl MeasurementProject {
    pub fn kind(&self) -> Option<MeasurementKind> {
        MeasurementKind::from_tag(&self.kind)
    }
}

impl MeasurementStepProject {
    pub fn from_handles(frame: usize, handles: &[(f32, f32)]) -> Self {
        Self {
            frame,
            handles: handles.iter().map(|(x, y)| [*x, *y]).collect(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PointProject {
    pub frame: usize,
    pub x: f32,
    pub y: f32,
}

fn one() -> f32 {
    1.0
}

fn one_usize() -> usize {
    1
}

fn default_trail_length() -> usize {
    15
}

fn default_true() -> bool {
    true
}

impl ProjectFile {
    pub fn new(workdir: Option<PathBuf>, active_doc: usize, docs: Vec<DocProject>) -> Self {
        Self {
            format: FORMAT.to_string(),
            version: VERSION,
            workdir,
            active_doc,
            docs,
        }
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let text = fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
        let project: Self =
            serde_json::from_str(&text).map_err(|e| format!("parse {}: {e}", path.display()))?;
        if project.format != FORMAT {
            return Err(format!("unsupported project format: {}", project.format));
        }
        if project.version > VERSION {
            return Err(format!(
                "project version {} is newer than supported {}",
                project.version, VERSION
            ));
        }
        Ok(project)
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("create {}: {e}", parent.display()))?;
            }
        }
        let text =
            serde_json::to_string_pretty(self).map_err(|e| format!("serialize project: {e}"))?;
        fs::write(path, text).map_err(|e| format!("write {}: {e}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_track_fields_get_defaults() {
        let text = r#"{
            "format": "tracker-gpui-project",
            "version": 2,
            "docs": [{
                "path": "a.mp4",
                "tracks": [{"name": "Track 1", "color": 1, "points": []}]
            }]
        }"#;
        let project: ProjectFile = serde_json::from_str(text).unwrap();
        let track = &project.docs[0].tracks[0];
        assert!(track.visible);
        assert!(!track.locked);
        assert_eq!(track.step_size, 1);
        assert!(!track.auto_advance);
        assert_eq!(track.footprint, Footprint::Diamond);
    }

    #[test]
    fn track_metadata_roundtrip() {
        let track = TrackProject {
            variables: Vec::new(),
            name: "Track 7".into(),
            color: 0x123456,
            visible: false,
            locked: true,
            step_size: 3,
            auto_advance: true,
            footprint: Footprint::Circle,
            keyframes: vec![12],
            points: vec![PointProject {
                frame: 12,
                x: 1.5,
                y: -2.5,
            }],
            measurement: None,
            rigid_body: None,
        };
        let text = serde_json::to_string(&track).unwrap();
        let restored: TrackProject = serde_json::from_str(&text).unwrap();
        assert_eq!(restored.name, track.name);
        assert_eq!(restored.color, track.color);
        assert_eq!(restored.visible, track.visible);
        assert_eq!(restored.locked, track.locked);
        assert_eq!(restored.step_size, track.step_size);
        assert_eq!(restored.auto_advance, track.auto_advance);
        assert_eq!(restored.footprint, track.footprint);
        assert_eq!(restored.keyframes, vec![12]);
        assert_eq!(restored.points.len(), 1);
        assert!(restored.measurement.is_none());
    }

    #[test]
    fn measurement_roundtrip() {
        let track = TrackProject {
            variables: Vec::new(),
            name: "Tape 2".into(),
            color: 0,
            visible: true,
            locked: false,
            step_size: 1,
            auto_advance: false,
            footprint: Footprint::Crosshair,
            keyframes: Vec::new(),
            points: Vec::new(),
            measurement: Some(MeasurementProject {
                kind: "tape".into(),
                steps: vec![MeasurementStepProject {
                    frame: 5,
                    handles: vec![[1.0, 2.0], [3.0, 4.0]],
                }],
            }),
            rigid_body: None,
        };
        let text = serde_json::to_string(&track).unwrap();
        let restored: TrackProject = serde_json::from_str(&text).unwrap();
        let measurement = restored.measurement.unwrap();
        assert_eq!(measurement.kind(), Some(MeasurementKind::Tape));
        assert_eq!(measurement.steps.len(), 1);
        assert_eq!(measurement.steps[0].handles, vec![[1.0, 2.0], [3.0, 4.0]]);
    }

    #[test]
    fn rigid_body_roundtrip() {
        let track = TrackProject {
            variables: Vec::new(),
            name: "Rigid".into(),
            color: 1,
            visible: true,
            locked: false,
            step_size: 1,
            auto_advance: false,
            footprint: Footprint::Diamond,
            keyframes: Vec::new(),
            points: vec![PointProject {
                frame: 0,
                x: 20.0,
                y: 30.0,
            }],
            measurement: None,
            rigid_body: Some(RigidBodyProject {
                dictionary: "DICT_4X4_50".into(),
                markers: vec![RigidBodyMarkerProject {
                    id: 0,
                    corners: [[-0.5, -0.5], [0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]],
                }],
                steps: vec![RigidBodyStepProject {
                    frame: 0,
                    angle: 0.5,
                    scale: 99.0,
                    marker_count: 1,
                    contour: None,
                }],
                contour: Vec::new(),
                mask_mode: "fixed".into(),
            }),
        };
        let text = serde_json::to_string(&track).unwrap();
        let restored: TrackProject = serde_json::from_str(&text).unwrap();
        let rigid = restored.rigid_body.unwrap();
        assert_eq!(rigid.dictionary, "DICT_4X4_50");
        assert_eq!(rigid.markers.len(), 1);
        assert_eq!(rigid.markers[0].id, 0);
        assert_eq!(rigid.steps.len(), 1);
        assert!((rigid.steps[0].angle - 0.5).abs() < 1e-6);
        assert_eq!(rigid.steps[0].marker_count, 1);
    }
}
