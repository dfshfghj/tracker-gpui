use crate::coords::ImageCoordSystem;
use rust_i18n::t;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Footprint {
    Diamond,
    Triangle,
    Square,
    Circle,
    Crosshair,
}

impl Default for Footprint {
    fn default() -> Self {
        Self::Diamond
    }
}

impl Footprint {
    pub fn label(self) -> String {
        match self {
            Self::Diamond => t!("footprint.diamond"),
            Self::Triangle => t!("footprint.triangle"),
            Self::Square => t!("footprint.square"),
            Self::Circle => t!("footprint.circle"),
            Self::Crosshair => t!("footprint.crosshair"),
        }
        .to_string()
    }

    pub fn next(self) -> Self {
        match self {
            Self::Diamond => Self::Triangle,
            Self::Triangle => Self::Square,
            Self::Square => Self::Circle,
            Self::Circle => Self::Crosshair,
            Self::Crosshair => Self::Diamond,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointMassStep {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct PointMass {
    pub steps: BTreeMap<usize, PointMassStep>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RigidBodyMarker {
    pub id: u32,
    /// Marker corners in the rigid-body coordinate system, in marker-side units.
    pub corners: [[f32; 2]; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RigidMaskMode {
    #[default]
    FixedShape,
    PureMotion,
}

impl RigidMaskMode {
    pub fn label(self) -> String {
        match self {
            Self::FixedShape => t!("mask_mode.fixed"),
            Self::PureMotion => t!("mask_mode.pure"),
        }
        .to_string()
    }

    pub fn tag(self) -> &'static str {
        match self {
            Self::FixedShape => "fixed",
            Self::PureMotion => "pure",
        }
    }

    pub fn from_tag(tag: &str) -> Self {
        if tag == "pure" {
            Self::PureMotion
        } else {
            Self::FixedShape
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RigidBodyStep {
    /// Image-plane angle in radians.
    pub angle: f32,
    /// Characteristic body size in image pixels (marker side for ArUco, ROI size for ORB).
    pub scale: f32,
    /// Visible marker count for ArUco, inlier count for ORB.
    pub marker_count: u8,
    /// Per-frame motion contour for the pure-mask mode, normalized by `scale`.
    pub contour: Option<Vec<[f32; 2]>>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct RigidBody {
    pub dictionary: String,
    pub markers: Vec<RigidBodyMarker>,
    pub steps: BTreeMap<usize, RigidBodyStep>,
    /// Reference contour centered on the body, normalized by the characteristic size.
    pub contour: Vec<[f32; 2]>,
    pub mask_mode: RigidMaskMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MeasurementKind {
    Tape,
    Protractor,
    Circle,
}

impl MeasurementKind {
    pub fn label(self) -> String {
        match self {
            Self::Tape => t!("measurement.tape"),
            Self::Protractor => t!("measurement.protractor"),
            Self::Circle => t!("measurement.circle"),
        }
        .to_string()
    }

    pub fn tag(self) -> &'static str {
        match self {
            Self::Tape => "tape",
            Self::Protractor => "protractor",
            Self::Circle => "circle",
        }
    }

    pub fn from_tag(tag: &str) -> Option<Self> {
        match tag {
            "tape" => Some(Self::Tape),
            "protractor" => Some(Self::Protractor),
            "circle" => Some(Self::Circle),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MeasurementStep {
    pub handles: Vec<(f32, f32)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MeasurementTrack {
    pub kind: MeasurementKind,
    pub steps: BTreeMap<usize, MeasurementStep>,
}

impl MeasurementTrack {
    pub fn new(kind: MeasurementKind) -> Self {
        Self {
            kind,
            steps: BTreeMap::new(),
        }
    }

    pub fn default_step(kind: MeasurementKind, x: f32, y: f32) -> MeasurementStep {
        let radius = 72.0;
        let handles = match kind {
            MeasurementKind::Tape => vec![(x, y), (x + radius * 2.0, y)],
            MeasurementKind::Protractor => vec![
                (x, y),
                (x + radius, y),
                (x - radius * 0.5, y - radius * 0.866),
            ],
            MeasurementKind::Circle => vec![(x - radius, y), (x + radius, y), (x, y - radius)],
        };
        MeasurementStep { handles }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum TrackKind {
    PointMass(PointMass),
    Measurement(MeasurementTrack),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    pub id: u64,
    pub name: String,
    pub color: u32,
    pub visible: bool,
    pub locked: bool,
    pub step_size: usize,
    pub auto_advance: bool,
    pub footprint: Footprint,
    pub keyframes: BTreeSet<usize>,
    pub kind: TrackKind,
    pub rigid: Option<RigidBody>,
    pub variables: Vec<crate::variables::Definition>,
}

impl Track {
    pub fn point_mass(id: u64, name: impl Into<String>, color: u32) -> Self {
        Self {
            id,
            name: name.into(),
            color,
            visible: true,
            locked: false,
            step_size: 1,
            auto_advance: false,
            footprint: Footprint::Diamond,
            keyframes: BTreeSet::new(),
            kind: TrackKind::PointMass(PointMass::default()),
            rigid: None,
            variables: Vec::new(),
        }
    }

    pub fn rigid_body(
        id: u64,
        name: impl Into<String>,
        color: u32,
        dictionary: impl Into<String>,
        markers: Vec<RigidBodyMarker>,
    ) -> Self {
        let mut track = Self::point_mass(id, name, color);
        track.rigid = Some(RigidBody {
            dictionary: dictionary.into(),
            markers,
            steps: BTreeMap::new(),
            contour: Vec::new(),
            mask_mode: RigidMaskMode::FixedShape,
        });
        track
    }

    pub fn set_rigid_contour(&mut self, contour: Vec<[f32; 2]>) {
        if let Some(rigid) = &mut self.rigid {
            rigid.contour = contour;
        }
    }

    pub fn set_rigid_mask_mode(&mut self, mode: RigidMaskMode) {
        if let Some(rigid) = &mut self.rigid {
            rigid.mask_mode = mode;
        }
    }

    pub fn set_rigid_step_contour(&mut self, frame: usize, contour: Vec<[f32; 2]>) {
        if let Some(rigid) = &mut self.rigid {
            if let Some(step) = rigid.steps.get_mut(&frame) {
                step.contour = Some(contour);
            }
        }
    }

    /// Manually rotates one rigid-body step. The new angle is unwrapped
    /// relative to the stored angle so plots stay continuous.
    pub fn set_rigid_angle(&mut self, frame: usize, angle: f32) {
        if let Some(rigid) = &mut self.rigid {
            if let Some(step) = rigid.steps.get_mut(&frame) {
                let mut angle = angle;
                while angle - step.angle > std::f32::consts::PI {
                    angle -= std::f32::consts::TAU;
                }
                while angle - step.angle < -std::f32::consts::PI {
                    angle += std::f32::consts::TAU;
                }
                step.angle = angle;
            }
        }
    }

    pub fn measurement(
        id: u64,
        kind: MeasurementKind,
        name: impl Into<String>,
        color: u32,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            color,
            visible: true,
            locked: false,
            step_size: 1,
            auto_advance: false,
            footprint: Footprint::Crosshair,
            keyframes: BTreeSet::new(),
            kind: TrackKind::Measurement(MeasurementTrack::new(kind)),
            rigid: None,
            variables: Vec::new(),
        }
    }

    pub fn is_point_mass(&self) -> bool {
        matches!(self.kind, TrackKind::PointMass(_))
    }

    pub fn is_rigid_body(&self) -> bool {
        self.rigid.is_some() && self.is_point_mass()
    }

    pub fn rigid_step(&self, frame: usize) -> Option<&RigidBodyStep> {
        self.rigid.as_ref()?.steps.get(&frame)
    }

    pub fn set_rigid_step(
        &mut self,
        frame: usize,
        x: f32,
        y: f32,
        angle: f32,
        scale: f32,
        marker_count: u8,
    ) {
        if !self.is_point_mass() {
            return;
        }
        self.set_point(frame, x, y);
        if let Some(rigid) = &mut self.rigid {
            rigid.steps.insert(
                frame,
                RigidBodyStep {
                    angle,
                    scale,
                    marker_count,
                    contour: None,
                },
            );
        }
    }

    pub fn measurement_kind(&self) -> Option<MeasurementKind> {
        match &self.kind {
            TrackKind::PointMass(_) => None,
            TrackKind::Measurement(track) => Some(track.kind),
        }
    }

    pub fn measurement_steps(&self) -> Option<&BTreeMap<usize, MeasurementStep>> {
        match &self.kind {
            TrackKind::PointMass(_) => None,
            TrackKind::Measurement(track) => Some(&track.steps),
        }
    }

    pub fn kind_label(&self) -> String {
        if self.is_rigid_body() {
            return t!("track.kind.rigid").to_string();
        }
        match &self.kind {
            TrackKind::PointMass(_) => t!("track.kind.point").to_string(),
            TrackKind::Measurement(track) => track.kind.label(),
        }
    }

    pub fn points(&self) -> impl Iterator<Item = (usize, f32, f32)> + '_ {
        let steps = match &self.kind {
            TrackKind::PointMass(point_mass) => Some(&point_mass.steps),
            TrackKind::Measurement(_) => None,
        };
        steps
            .into_iter()
            .flat_map(|steps| steps.iter())
            .map(|(frame, step)| (*frame, step.x, step.y))
    }

    // BTreeMap neighbors avoid allocating/scanning all frames for each derivative.
    fn point_neighbors(&self, frame: usize) -> Option<(Option<usize>, Option<usize>)> {
        let TrackKind::PointMass(track) = &self.kind else {
            return None;
        };
        if !track.steps.contains_key(&frame) {
            return None;
        }
        let previous = track.steps.range(..frame).next_back().map(|(f, _)| *f);
        let next = track
            .steps
            .range((std::ops::Bound::Excluded(frame), std::ops::Bound::Unbounded))
            .next()
            .map(|(f, _)| *f);
        Some((previous, next))
    }

    pub fn point_at(&self, frame: usize) -> Option<(f32, f32)> {
        match &self.kind {
            TrackKind::PointMass(point_mass) => {
                point_mass.steps.get(&frame).map(|step| (step.x, step.y))
            }
            TrackKind::Measurement(_) => None,
        }
    }

    pub fn handles(&self, frame: usize) -> Vec<(f32, f32)> {
        match &self.kind {
            TrackKind::PointMass(point_mass) => point_mass
                .steps
                .get(&frame)
                .map(|step| vec![(step.x, step.y)])
                .unwrap_or_default(),
            TrackKind::Measurement(track) => track
                .steps
                .get(&frame)
                .map(|step| step.handles.clone())
                .unwrap_or_default(),
        }
    }

    pub fn measurement_step(&self, frame: usize) -> Option<&MeasurementStep> {
        match &self.kind {
            TrackKind::PointMass(_) => None,
            TrackKind::Measurement(track) => track.steps.get(&frame),
        }
    }

    pub fn set_measurement_step(&mut self, frame: usize, step: MeasurementStep) {
        if let TrackKind::Measurement(track) = &mut self.kind {
            track.steps.insert(frame, step);
        }
    }

    pub fn set_point(&mut self, frame: usize, x: f32, y: f32) {
        match &mut self.kind {
            TrackKind::PointMass(point_mass) => {
                point_mass.steps.insert(frame, PointMassStep { x, y });
            }
            TrackKind::Measurement(_) => {}
        }
    }

    pub fn set_handle(&mut self, frame: usize, index: usize, x: f32, y: f32) {
        match &mut self.kind {
            TrackKind::PointMass(point_mass) => {
                if index == 0 {
                    point_mass.steps.insert(frame, PointMassStep { x, y });
                }
            }
            TrackKind::Measurement(track) => {
                if let Some(step) = track.steps.get_mut(&frame) {
                    if let Some(handle) = step.handles.get_mut(index) {
                        *handle = (x, y);
                    }
                }
            }
        }
    }

    pub fn delete_point(&mut self, frame: usize) {
        match &mut self.kind {
            TrackKind::PointMass(point_mass) => {
                point_mass.steps.remove(&frame);
                self.keyframes.remove(&frame);
            }
            TrackKind::Measurement(track) => {
                track.steps.remove(&frame);
            }
        }
        if let Some(rigid) = &mut self.rigid {
            rigid.steps.remove(&frame);
        }
    }

    pub fn has_point(&self, frame: usize) -> bool {
        match &self.kind {
            TrackKind::PointMass(_) => self.point_at(frame).is_some(),
            TrackKind::Measurement(track) => track.steps.contains_key(&frame),
        }
    }

    pub fn clear_points(&mut self) {
        match &mut self.kind {
            TrackKind::PointMass(point_mass) => point_mass.steps.clear(),
            TrackKind::Measurement(track) => track.steps.clear(),
        }
        if let Some(rigid) = &mut self.rigid {
            rigid.steps.clear();
        }
        self.keyframes.clear();
    }

    pub fn add_keyframe(&mut self, frame: usize) {
        if self.is_point_mass() && self.has_point(frame) {
            self.keyframes.insert(frame);
        }
    }

    pub fn remove_keyframe(&mut self, frame: usize) {
        self.keyframes.remove(&frame);
    }

    pub fn keyframes(&self) -> impl Iterator<Item = usize> + '_ {
        self.keyframes.iter().copied()
    }

    pub fn nearest_keyframe(&self, frame: usize, backwards: bool) -> Option<usize> {
        if backwards {
            self.keyframes.range(..=frame).next_back().copied()
        } else {
            self.keyframes.range(frame..).next().copied()
        }
    }

    pub fn clear_points_after(&mut self, frame: usize, keep_keyframes: bool) {
        if let TrackKind::PointMass(point_mass) = &mut self.kind {
            let keep: Vec<usize> = self.keyframes.iter().copied().collect();
            point_mass
                .steps
                .retain(|key, _| *key <= frame || (keep_keyframes && keep.contains(key)));
            if keep_keyframes {
                self.keyframes
                    .retain(|key| *key <= frame || keep.contains(key));
            } else {
                self.keyframes.retain(|key| *key <= frame);
            }
        }
        if let Some(rigid) = &mut self.rigid {
            rigid.steps.retain(|key, _| *key <= frame);
        }
    }

    pub fn step_count(&self) -> usize {
        match &self.kind {
            TrackKind::PointMass(point_mass) => point_mass.steps.len(),
            TrackKind::Measurement(track) => track.steps.len(),
        }
    }

    pub fn frames(&self) -> Vec<usize> {
        match &self.kind {
            TrackKind::PointMass(point_mass) => point_mass.steps.keys().copied().collect(),
            TrackKind::Measurement(track) => track.steps.keys().copied().collect(),
        }
    }

    pub fn tape_values(&self, frame: usize) -> Option<(f32, f32)> {
        let handles = self.handles(frame);
        if handles.len() < 2 {
            return None;
        }
        let (a, b) = (handles[0], handles[1]);
        let length = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
        let angle = (b.1 - a.1).atan2(b.0 - a.0).to_degrees();
        Some((length, angle))
    }

    pub fn protractor_values(&self, frame: usize) -> Option<(f32, f32, f32)> {
        let handles = self.handles(frame);
        if handles.len() < 3 {
            return None;
        }
        let vertex = handles[0];
        let arm1 = (handles[1].0 - vertex.0, handles[1].1 - vertex.1);
        let arm2 = (handles[2].0 - vertex.0, handles[2].1 - vertex.1);
        let len1 = (arm1.0 * arm1.0 + arm1.1 * arm1.1).sqrt();
        let len2 = (arm2.0 * arm2.0 + arm2.1 * arm2.1).sqrt();
        if len1 < 1e-6 || len2 < 1e-6 {
            return None;
        }
        let cosine = ((arm1.0 * arm2.0 + arm1.1 * arm2.1) / (len1 * len2)).clamp(-1.0, 1.0);
        Some((cosine.acos().to_degrees(), len1, len2))
    }

    pub fn circle_values(&self, frame: usize) -> Option<((f32, f32), f32)> {
        let handles = self.handles(frame);
        if handles.len() < 3 {
            return None;
        }
        let (a, b, cc) = (handles[0], handles[1], handles[2]);
        let d = 2.0 * (a.0 * (b.1 - cc.1) + b.0 * (cc.1 - a.1) + cc.0 * (a.1 - b.1));
        if d.abs() < 1e-6 {
            return None;
        }
        let a2 = a.0 * a.0 + a.1 * a.1;
        let b2 = b.0 * b.0 + b.1 * b.1;
        let c2 = cc.0 * cc.0 + cc.1 * cc.1;
        let center = (
            (a2 * (b.1 - cc.1) + b2 * (cc.1 - a.1) + c2 * (a.1 - b.1)) / d,
            (a2 * (cc.0 - b.0) + b2 * (a.0 - cc.0) + c2 * (b.0 - a.0)) / d,
        );
        let radius = ((a.0 - center.0).powi(2) + (a.1 - center.1).powi(2)).sqrt();
        Some((center, radius))
    }

    pub fn world_position(&self, frame: usize, coords: &ImageCoordSystem) -> Option<(f32, f32)> {
        self.point_at(frame)
            .map(|(x, y)| coords.image_to_world(frame, x, y))
    }

    pub fn world_velocity(
        &self,
        frame: usize,
        coords: &ImageCoordSystem,
        frame_times: &[f64],
    ) -> Option<(f32, f32)> {
        let (previous, next) = self.point_neighbors(frame)?;
        let current_time = frame_times.get(frame).copied()?;
        let previous = previous.and_then(|f| Some((f, frame_times.get(f).copied()?)));
        let next = next.and_then(|f| Some((f, frame_times.get(f).copied()?)));
        let (world_x, world_y) = self.world_position(frame, coords)?;
        let (prev_x, prev_y) = previous
            .and_then(|(f, _)| self.world_position(f, coords))
            .map_or((world_x, world_y), |value| value);
        let (next_x, next_y) = next
            .and_then(|(f, _)| self.world_position(f, coords))
            .map_or((world_x, world_y), |value| value);
        let (start_time, end_time) = match (previous, next) {
            (Some((_, before)), Some((_, after))) => (before, after),
            (Some((_, before)), None) => (before, current_time),
            (None, Some((_, after))) => (current_time, after),
            (None, None) => return None,
        };
        let dt = end_time - start_time;
        if dt.abs() < 1e-9 {
            return None;
        }
        Some((
            (next_x - prev_x) as f32 / dt as f32,
            (next_y - prev_y) as f32 / dt as f32,
        ))
    }

    pub fn world_acceleration(
        &self,
        frame: usize,
        coords: &ImageCoordSystem,
        frame_times: &[f64],
    ) -> Option<(f32, f32)> {
        let (previous, next) = self.point_neighbors(frame)?;
        let (Some(previous), Some(next)) = (previous, next) else {
            return None;
        };
        let before_velocity = self.world_velocity(previous, coords, frame_times)?;
        let after_velocity = self.world_velocity(next, coords, frame_times)?;
        let dt = frame_times.get(next)? - frame_times.get(previous)?;
        if dt.abs() < 1e-9 {
            return None;
        }
        Some((
            (after_velocity.0 - before_velocity.0) / dt as f32,
            (after_velocity.1 - before_velocity.1) / dt as f32,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tape_measure_geometry() {
        let mut track = Track::measurement(1, MeasurementKind::Tape, "Tape", 0);
        track.set_measurement_step(
            0,
            MeasurementStep {
                handles: vec![(0.0, 0.0), (3.0, 4.0)],
            },
        );
        let (length, angle) = track.tape_values(0).unwrap();
        assert!((length - 5.0).abs() < 1e-4);
        assert!((angle - 53.1301).abs() < 1e-2);
    }

    #[test]
    fn protractor_measure_angle() {
        let mut track = Track::measurement(1, MeasurementKind::Protractor, "Angle", 0);
        track.set_measurement_step(
            0,
            MeasurementStep {
                handles: vec![(0.0, 0.0), (10.0, 0.0), (0.0, 10.0)],
            },
        );
        let (angle, len1, len2) = track.protractor_values(0).unwrap();
        assert!((angle - 90.0).abs() < 1e-3);
        assert!((len1 - 10.0).abs() < 1e-4);
        assert!((len2 - 10.0).abs() < 1e-4);
    }

    #[test]
    fn circle_fit_geometry() {
        let mut track = Track::measurement(1, MeasurementKind::Circle, "Circle", 0);
        track.set_measurement_step(
            0,
            MeasurementStep {
                handles: vec![(-5.0, 0.0), (5.0, 0.0), (0.0, 5.0)],
            },
        );
        let ((cx, cy), radius) = track.circle_values(0).unwrap();
        assert!(cx.abs() < 1e-3);
        assert!(cy.abs() < 1e-3);
        assert!((radius - 5.0).abs() < 1e-3);
    }

    #[test]
    fn keyframes_track_manual_marks_and_preserve_later_keys() {
        let mut track = Track::point_mass(1, "Track", 0);
        track.set_point(0, 10.0, 10.0);
        track.set_point(5, 20.0, 20.0);
        track.set_point(10, 30.0, 30.0);
        track.add_keyframe(0);
        track.add_keyframe(10);
        assert_eq!(track.nearest_keyframe(6, true), Some(0));
        assert_eq!(track.nearest_keyframe(6, false), Some(10));
        track.clear_points_after(0, true);
        assert!(track.has_point(0));
        assert!(!track.has_point(5));
        assert!(track.has_point(10));
        track.delete_point(10);
        assert!(!track.keyframes().any(|frame| frame == 10));
    }

    #[test]
    fn rigid_body_keeps_center_and_pose() {
        let mut track = Track::rigid_body(
            1,
            "Rigid",
            0,
            "DICT_4X4_50",
            vec![RigidBodyMarker {
                id: 0,
                corners: [[-0.5, -0.5], [0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]],
            }],
        );
        track.set_rigid_step(2, 10.0, 20.0, 0.5, 100.0, 2);
        assert!(track.is_point_mass());
        assert!(track.is_rigid_body());
        // Label text is localized, so compare against the active locale.
        assert_eq!(track.kind_label(), t!("track.kind.rigid"));
        assert_eq!(track.point_at(2), Some((10.0, 20.0)));
        let step = track.rigid_step(2).unwrap();
        assert!((step.angle - 0.5).abs() < 1e-6);
        assert_eq!(step.marker_count, 2);
        track.delete_point(2);
        assert!(!track.has_point(2));
        assert!(track.rigid_step(2).is_none());
    }

    #[test]
    fn manual_rigid_angle_stays_continuous() {
        let mut track = Track::rigid_body(1, "Rigid", 0, "MOTION", Vec::new());
        track.set_rigid_step(0, 10.0, 20.0, 3.0, 20.0, 100);
        track.set_rigid_angle(0, -3.0);
        let angle = track.rigid_step(0).unwrap().angle;
        assert!((angle - (std::f32::consts::TAU - 3.0)).abs() < 1e-5);
    }

    #[test]
    fn measurement_handles_are_editable() {
        let mut track = Track::measurement(1, MeasurementKind::Tape, "Tape", 0);
        track.set_measurement_step(
            3,
            MeasurementStep {
                handles: vec![(1.0, 1.0), (2.0, 2.0)],
            },
        );
        assert_eq!(track.handles(3), vec![(1.0, 1.0), (2.0, 2.0)]);
        track.set_handle(3, 1, 10.0, 20.0);
        assert_eq!(track.handles(3), vec![(1.0, 1.0), (10.0, 20.0)]);
        assert!(track.measurement_step(3).is_some());
        track.delete_point(3);
        assert!(track.handles(3).is_empty());
    }
    #[test]
    fn derivatives_use_sparse_neighbors_and_preserve_endpoint_rules() {
        let mut track = Track::point_mass(1, "Sparse", 0);
        let frames = [0, 2, 7, 20, 31];
        let times: Vec<_> = (0..32).map(|f| (f as f64 * 0.2).powi(2)).collect();
        for f in frames {
            track.set_point(f, (times[f] * times[f]) as f32, times[f] as f32);
        }
        let coords = ImageCoordSystem::default();
        let reference = |i: usize| {
            let a = i.saturating_sub(1);
            let b = (i + 1).min(frames.len() - 1);
            let pa = track.world_position(frames[a], &coords).unwrap();
            let pb = track.world_position(frames[b], &coords).unwrap();
            let dt = (times[frames[b]] - times[frames[a]]) as f32;
            ((pb.0 - pa.0) / dt, (pb.1 - pa.1) / dt)
        };
        for (i, f) in frames.into_iter().enumerate() {
            let v = track.world_velocity(f, &coords, &times).unwrap();
            assert_eq!(v, reference(i));
            if i > 0 && i + 1 < frames.len() {
                let a = reference(i - 1);
                let b = reference(i + 1);
                let dt = (times[frames[i + 1]] - times[frames[i - 1]]) as f32;
                assert_eq!(
                    track.world_acceleration(f, &coords, &times),
                    Some(((b.0 - a.0) / dt, (b.1 - a.1) / dt))
                );
            } else {
                assert!(track.world_acceleration(f, &coords, &times).is_none());
            }
        }
        assert!(track.world_velocity(3, &coords, &times).is_none());
        assert!(track.world_velocity(7, &coords, &[]).is_none());
        let mut one = Track::point_mass(2, "One", 0);
        one.set_point(7, 1., 2.);
        assert!(one.world_velocity(7, &coords, &times).is_none());
        assert!(track.world_velocity(7, &coords, &[0.; 32]).is_none());
    }
}
