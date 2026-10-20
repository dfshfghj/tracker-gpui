//! Plot data model aligned with Tracker's PlotTView: each track exposes a set
//! of data variables that can be plotted against each other.

use crate::coords::ImageCoordSystem;
use crate::track::{MeasurementKind, Track};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlotVar {
    Frame,
    Time,
    X,
    Y,
    Vx,
    Vy,
    Ax,
    Ay,
    Speed,
    Length,
    Angle,
    Arm1,
    Arm2,
    Radius,
    CenterX,
    CenterY,
}

impl PlotVar {
    pub fn label(self) -> &'static str {
        match self {
            Self::Frame => "帧",
            Self::Time => "t",
            Self::X => "x",
            Self::Y => "y",
            Self::Vx => "vx",
            Self::Vy => "vy",
            Self::Ax => "ax",
            Self::Ay => "ay",
            Self::Speed => "|v|",
            Self::Length => "长度",
            Self::Angle => "角度",
            Self::Arm1 => "臂1",
            Self::Arm2 => "臂2",
            Self::Radius => "半径",
            Self::CenterX => "圆心 x",
            Self::CenterY => "圆心 y",
        }
    }

    pub fn unit(self, length_unit: &str) -> String {
        match self {
            Self::Frame => String::new(),
            Self::Time => "s".to_string(),
            Self::X
            | Self::Y
            | Self::Length
            | Self::Arm1
            | Self::Arm2
            | Self::Radius
            | Self::CenterX
            | Self::CenterY => length_unit.to_string(),
            Self::Vx | Self::Vy | Self::Speed => format!("{length_unit}/s"),
            Self::Ax | Self::Ay => format!("{length_unit}/s²"),
            Self::Angle => "°".to_string(),
        }
    }

    pub fn axis_label(self, length_unit: &str) -> String {
        let unit = self.unit(length_unit);
        if unit.is_empty() {
            self.label().to_string()
        } else {
            format!("{} ({})", self.label(), unit)
        }
    }

    pub fn available(self, track: &Track) -> bool {
        let measurement = track.measurement_kind();
        match self {
            Self::Frame | Self::Time => true,
            Self::X | Self::Y | Self::Vx | Self::Vy | Self::Ax | Self::Ay | Self::Speed => {
                track.is_point_mass()
            }
            Self::Length => measurement == Some(MeasurementKind::Tape),
            Self::Angle => matches!(
                measurement,
                Some(MeasurementKind::Tape | MeasurementKind::Protractor)
            ),
            Self::Arm1 | Self::Arm2 => measurement == Some(MeasurementKind::Protractor),
            Self::Radius | Self::CenterX | Self::CenterY => {
                measurement == Some(MeasurementKind::Circle)
            }
        }
    }

    pub fn all_for(track: &Track) -> Vec<Self> {
        const ALL: [PlotVar; 16] = [
            PlotVar::Frame,
            PlotVar::Time,
            PlotVar::X,
            PlotVar::Y,
            PlotVar::Vx,
            PlotVar::Vy,
            PlotVar::Ax,
            PlotVar::Ay,
            PlotVar::Speed,
            PlotVar::Length,
            PlotVar::Angle,
            PlotVar::Arm1,
            PlotVar::Arm2,
            PlotVar::Radius,
            PlotVar::CenterX,
            PlotVar::CenterY,
        ];
        ALL.into_iter().filter(|var| var.available(track)).collect()
    }

    pub fn value(
        self,
        track: &Track,
        frame: usize,
        coords: &ImageCoordSystem,
        frame_times: &[f64],
    ) -> Option<f64> {
        match self {
            Self::Frame => Some(frame as f64),
            Self::Time => frame_times.get(frame).copied(),
            Self::X => track
                .world_position(frame, coords)
                .map(|point| point.0 as f64),
            Self::Y => track
                .world_position(frame, coords)
                .map(|point| point.1 as f64),
            Self::Vx => track
                .world_velocity(frame, coords, frame_times)
                .map(|value| value.0 as f64),
            Self::Vy => track
                .world_velocity(frame, coords, frame_times)
                .map(|value| value.1 as f64),
            Self::Ax => track
                .world_acceleration(frame, coords, frame_times)
                .map(|value| value.0 as f64),
            Self::Ay => track
                .world_acceleration(frame, coords, frame_times)
                .map(|value| value.1 as f64),
            Self::Speed => track
                .world_velocity(frame, coords, frame_times)
                .map(|value| ((value.0 as f64).powi(2) + (value.1 as f64).powi(2)).sqrt()),
            Self::Length => {
                if track.measurement_kind() != Some(MeasurementKind::Tape) {
                    return None;
                }
                let handles = track.handles(frame);
                if handles.len() < 2 {
                    return None;
                }
                let a = coords.image_to_world(frame, handles[0].0, handles[0].1);
                let b = coords.image_to_world(frame, handles[1].0, handles[1].1);
                Some(((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt() as f64)
            }
            Self::Angle => match track.measurement_kind()? {
                MeasurementKind::Tape => {
                    let handles = track.handles(frame);
                    if handles.len() < 2 {
                        return None;
                    }
                    let a = coords.image_to_world(frame, handles[0].0, handles[0].1);
                    let b = coords.image_to_world(frame, handles[1].0, handles[1].1);
                    Some((b.1 - a.1).atan2(b.0 - a.0).to_degrees() as f64)
                }
                MeasurementKind::Protractor => {
                    let handles = track.handles(frame);
                    if handles.len() < 3 {
                        return None;
                    }
                    let vertex = coords.image_to_world(frame, handles[0].0, handles[0].1);
                    let arm1 = coords.image_to_world(frame, handles[1].0, handles[1].1);
                    let arm2 = coords.image_to_world(frame, handles[2].0, handles[2].1);
                    let v1 = (arm1.0 - vertex.0, arm1.1 - vertex.1);
                    let v2 = (arm2.0 - vertex.0, arm2.1 - vertex.1);
                    let len1 = (v1.0 * v1.0 + v1.1 * v1.1).sqrt();
                    let len2 = (v2.0 * v2.0 + v2.1 * v2.1).sqrt();
                    if len1 < 1e-6 || len2 < 1e-6 {
                        return None;
                    }
                    Some(
                        ((v1.0 * v2.0 + v1.1 * v2.1) / (len1 * len2))
                            .clamp(-1.0, 1.0)
                            .acos()
                            .to_degrees() as f64,
                    )
                }
                MeasurementKind::Circle => None,
            },
            Self::Arm1 | Self::Arm2 => {
                if track.measurement_kind() != Some(MeasurementKind::Protractor) {
                    return None;
                }
                let handles = track.handles(frame);
                if handles.len() < 3 {
                    return None;
                }
                let vertex = coords.image_to_world(frame, handles[0].0, handles[0].1);
                let index = if matches!(self, Self::Arm1) { 1 } else { 2 };
                let arm = coords.image_to_world(frame, handles[index].0, handles[index].1);
                Some(((arm.0 - vertex.0).powi(2) + (arm.1 - vertex.1).powi(2)).sqrt() as f64)
            }
            Self::Radius | Self::CenterX | Self::CenterY => {
                if track.measurement_kind() != Some(MeasurementKind::Circle) {
                    return None;
                }
                let (center, _) = track.circle_values(frame)?;
                let handles = track.handles(frame);
                if handles.is_empty() {
                    return None;
                }
                let world_center = coords.image_to_world(frame, center.0, center.1);
                match self {
                    Self::CenterX => Some(world_center.0 as f64),
                    Self::CenterY => Some(world_center.1 as f64),
                    _ => {
                        let edge = coords.image_to_world(frame, handles[0].0, handles[0].1);
                        Some(
                            ((edge.0 - world_center.0).powi(2) + (edge.1 - world_center.1).powi(2))
                                .sqrt() as f64,
                        )
                    }
                }
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlotConfig {
    pub x_var: PlotVar,
    pub y_var: PlotVar,
    #[serde(default = "default_true")]
    pub points: bool,
    #[serde(default = "default_true")]
    pub lines: bool,
    #[serde(default)]
    pub x_range: Option<(f64, f64)>,
    #[serde(default)]
    pub y_range: Option<(f64, f64)>,
}

fn default_true() -> bool {
    true
}

impl Default for PlotConfig {
    fn default() -> Self {
        Self {
            x_var: PlotVar::Time,
            y_var: PlotVar::X,
            points: true,
            lines: true,
            x_range: None,
            y_range: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlotState {
    pub configs: Vec<PlotConfig>,
    #[serde(default)]
    pub linked_x: bool,
}

impl Default for PlotState {
    fn default() -> Self {
        Self {
            configs: vec![PlotConfig::default()],
            linked_x: false,
        }
    }
}

impl PlotState {
    pub fn for_track(track: &Track) -> Self {
        let count = if track.is_point_mass() { 2 } else { 1 };
        let mut state = Self {
            configs: (0..count)
                .map(|index| Self::default_config(track, index))
                .collect(),
            linked_x: false,
        };
        state.ensure_for_track(track);
        state
    }

    fn default_config(track: &Track, index: usize) -> PlotConfig {
        if track.is_point_mass() {
            PlotConfig {
                x_var: PlotVar::Time,
                y_var: if index == 0 { PlotVar::X } else { PlotVar::Y },
                ..PlotConfig::default()
            }
        } else {
            let y_var = match (track.measurement_kind(), index) {
                (Some(MeasurementKind::Tape), 0) => PlotVar::Length,
                (Some(MeasurementKind::Tape), _) => PlotVar::Angle,
                (Some(MeasurementKind::Protractor), 0) => PlotVar::Angle,
                (Some(MeasurementKind::Protractor), _) => PlotVar::Arm1,
                (Some(MeasurementKind::Circle), 0) => PlotVar::Radius,
                (Some(MeasurementKind::Circle), _) => PlotVar::CenterX,
                _ => PlotVar::X,
            };
            PlotConfig {
                x_var: PlotVar::Time,
                y_var,
                ..PlotConfig::default()
            }
        }
    }

    pub fn ensure_for_track(&mut self, track: &Track) {
        let default_count = if track.is_point_mass() { 2 } else { 1 };
        if self.configs.is_empty() {
            self.configs = (0..default_count)
                .map(|index| Self::default_config(track, index))
                .collect();
            return;
        }
        let track_changed = self
            .configs
            .iter()
            .any(|config| !config.x_var.available(track) || !config.y_var.available(track));
        if track_changed {
            self.configs = (0..default_count)
                .map(|index| Self::default_config(track, index))
                .collect();
            return;
        }
        if self.configs.len() > 2 {
            self.configs.truncate(2);
        }
    }

    pub fn set_count(&mut self, count: usize, track: &Track) {
        let count = count.clamp(1, 2);
        while self.configs.len() < count {
            let index = self.configs.len();
            self.configs.push(Self::default_config(track, index));
        }
        self.configs.truncate(count);
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PlotPoint {
    pub frame: usize,
    pub x: f64,
    pub y: f64,
}

pub struct PlotSeries {
    pub points: Vec<PlotPoint>,
    pub x_range: (f64, f64),
    pub y_range: (f64, f64),
}

pub fn series(
    track: &Track,
    config: &PlotConfig,
    coords: &ImageCoordSystem,
    frame_times: &[f64],
) -> PlotSeries {
    let mut points = Vec::new();
    for frame in track.frames() {
        let Some(x) = config.x_var.value(track, frame, coords, frame_times) else {
            continue;
        };
        let Some(y) = config.y_var.value(track, frame, coords, frame_times) else {
            continue;
        };
        if !x.is_finite() || !y.is_finite() {
            continue;
        }
        points.push(PlotPoint { frame, x, y });
    }
    let (x_range, y_range) = ranges(config, &points);
    PlotSeries {
        points,
        x_range,
        y_range,
    }
}

pub fn ranges(config: &PlotConfig, points: &[PlotPoint]) -> ((f64, f64), (f64, f64)) {
    let mut x_min = f64::INFINITY;
    let mut x_max = f64::NEG_INFINITY;
    let mut y_min = f64::INFINITY;
    let mut y_max = f64::NEG_INFINITY;
    for point in points {
        x_min = x_min.min(point.x);
        x_max = x_max.max(point.x);
        y_min = y_min.min(point.y);
        y_max = y_max.max(point.y);
    }
    let mut x_range = if x_min.is_finite() && x_max.is_finite() {
        padded(x_min, x_max, config.x_var)
    } else {
        (0.0, 1.0)
    };
    let y_range = if y_min.is_finite() && y_max.is_finite() {
        padded(y_min, y_max, config.y_var)
    } else {
        (0.0, 1.0)
    };
    if matches!(config.x_var, PlotVar::Time | PlotVar::Frame) && x_range.0 < 0.0 {
        x_range.0 = 0.0;
    }
    (
        config.x_range.unwrap_or(x_range),
        config.y_range.unwrap_or(y_range),
    )
}

fn padded(min: f64, max: f64, var: PlotVar) -> (f64, f64) {
    let mut min = min;
    let mut max = max;
    if (max - min).abs() < 1e-9 {
        let pad = if min.abs() < 1e-9 {
            1.0
        } else {
            min.abs() * 0.1
        };
        min -= pad;
        max += pad;
    } else {
        let pad = (max - min) * 0.05;
        min -= pad;
        max += pad;
    }
    if matches!(var, PlotVar::Time | PlotVar::Frame) {
        min = min.max(0.0);
    }
    (min, max)
}

pub fn nice_ticks(min: f64, max: f64, target: usize) -> Vec<f64> {
    if !min.is_finite() || !max.is_finite() || max <= min {
        return vec![min];
    }
    let raw_step = (max - min) / target.max(1) as f64;
    let exponent = raw_step.log10().floor();
    let base = 10f64.powf(exponent);
    let mut step = base * 10.0;
    for factor in [1.0, 2.0, 5.0, 10.0] {
        if base * factor >= raw_step {
            step = base * factor;
            break;
        }
    }
    let start = (min / step).ceil() * step;
    let mut ticks = Vec::new();
    let mut value = start;
    while value <= max + step * 1e-6 && ticks.len() < 64 {
        ticks.push(value);
        value += step;
    }
    ticks
}

pub fn nearest_point(points: &[PlotPoint], x: f64) -> Option<PlotPoint> {
    points
        .iter()
        .min_by(|a, b| {
            (a.x - x)
                .abs()
                .partial_cmp(&(b.x - x).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::track::{MeasurementStep, Track};

    #[test]
    fn point_mass_series_uses_time_and_world_values() {
        let mut track = Track::point_mass(1, "Track", 0);
        track.set_point(0, 0.0, 0.0);
        track.set_point(1, 10.0, 5.0);
        let coords = ImageCoordSystem::default();
        let config = PlotConfig {
            x_var: PlotVar::Time,
            y_var: PlotVar::X,
            ..PlotConfig::default()
        };
        let data = series(&track, &config, &coords, &[0.0, 0.5]);
        assert_eq!(data.points.len(), 2);
        assert_eq!(data.points[0].frame, 0);
        assert!((data.points[1].x - 0.5).abs() < 1e-9);
        assert!((data.points[1].y - 10.0).abs() < 1e-6);
    }

    #[test]
    fn tape_defaults_to_length_plot() {
        let mut track = Track::measurement(1, MeasurementKind::Tape, "Tape", 0);
        track.set_measurement_step(
            0,
            MeasurementStep {
                handles: vec![(0.0, 0.0), (30.0, 40.0)],
            },
        );
        let mut state = PlotState::for_track(&track);
        state.ensure_for_track(&track);
        assert_eq!(state.configs.len(), 1);
        assert_eq!(state.configs[0].y_var, PlotVar::Length);
        let data = series(
            &track,
            &state.configs[0],
            &ImageCoordSystem::default(),
            &[0.0],
        );
        assert!((data.points[0].y - 50.0).abs() < 1e-4);
    }

    #[test]
    fn switching_tracks_resets_unavailable_variables() {
        let point = Track::point_mass(1, "Point", 0);
        let tape = Track::measurement(2, MeasurementKind::Tape, "Tape", 0);
        let mut state = PlotState::for_track(&point);
        assert_eq!(state.configs.len(), 2);
        assert_eq!(state.configs[0].y_var, PlotVar::X);
        assert_eq!(state.configs[1].y_var, PlotVar::Y);
        state.ensure_for_track(&tape);
        assert!(state.configs[0].y_var.available(&tape));
        assert_eq!(state.configs[0].y_var, PlotVar::Length);
    }

    #[test]
    fn ticks_cover_range() {
        let ticks = nice_ticks(0.0, 10.0, 5);
        assert!(!ticks.is_empty());
        assert!(ticks.first().unwrap() >= &0.0);
        assert!(ticks.last().unwrap() <= &10.0);
        assert!(ticks.len() >= 3);
    }

    #[test]
    fn plot_state_roundtrip() {
        let point = Track::point_mass(1, "Point", 0);
        let mut state = PlotState::for_track(&point);
        state.configs[0].y_var = PlotVar::Vx;
        state.configs[0].points = false;
        state.linked_x = true;
        state.configs[1].x_range = Some((-1.0, 5.0));
        let text = serde_json::to_string(&state).unwrap();
        let restored: PlotState = serde_json::from_str(&text).unwrap();
        assert_eq!(restored.configs.len(), 2);
        assert_eq!(restored.configs[0].y_var, PlotVar::Vx);
        assert!(!restored.configs[0].points);
        assert!(restored.linked_x);
        assert_eq!(restored.configs[1].x_range, Some((-1.0, 5.0)));
    }
}
