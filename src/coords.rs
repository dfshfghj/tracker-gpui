use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Per-frame coordinate transform. scale is image pixels per world unit.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrameCoords {
    pub origin: [f64; 2],
    pub scale: [f64; 2],
    /// World x-axis angle CCW from image x-axis, in radians.
    pub angle: f64,
}

impl Default for FrameCoords {
    fn default() -> Self {
        Self {
            origin: [0.0, 0.0],
            scale: [1.0, 1.0],
            angle: 0.0,
        }
    }
}

impl FrameCoords {
    pub fn image_to_world(&self, x: f64, y: f64) -> (f64, f64) {
        let dx = x - self.origin[0];
        let dy = y - self.origin[1];
        let cos = self.angle.cos();
        let sin = self.angle.sin();
        let wx = (dx * cos - dy * sin) / self.scale[0];
        let wy = -(dx * sin + dy * cos) / self.scale[1];
        (wx, wy)
    }

    #[allow(dead_code)]
    pub fn world_to_image(&self, wx: f64, wy: f64) -> (f64, f64) {
        let cos = self.angle.cos();
        let sin = self.angle.sin();
        let x = self.scale[0] * cos * wx - self.scale[1] * sin * wy + self.origin[0];
        let y = -self.scale[0] * sin * wx - self.scale[1] * cos * wy + self.origin[1];
        (x, y)
    }
}

/// Mirrors OSP ImageCoordSystem: per-frame values with fixed/locked options.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageCoordSystem {
    pub base: FrameCoords,
    #[serde(default)]
    pub frames: BTreeMap<usize, FrameCoords>,
    #[serde(default = "default_true")]
    pub fixed_origin: bool,
    #[serde(default = "default_true")]
    pub fixed_angle: bool,
    #[serde(default = "default_true")]
    pub fixed_scale: bool,
    #[serde(default)]
    pub locked: bool,
}

fn default_true() -> bool {
    true
}

impl Default for ImageCoordSystem {
    fn default() -> Self {
        Self {
            base: FrameCoords::default(),
            frames: BTreeMap::new(),
            fixed_origin: true,
            fixed_angle: true,
            fixed_scale: true,
            locked: false,
        }
    }
}

impl ImageCoordSystem {
    pub fn frame(&self, n: usize) -> FrameCoords {
        let mut coords = self
            .frames
            .range(..=n)
            .next_back()
            .map(|(_, coords)| *coords)
            .unwrap_or(self.base);
        if self.fixed_origin {
            coords.origin = self.base.origin;
        }
        if self.fixed_angle {
            coords.angle = self.base.angle;
        }
        if self.fixed_scale {
            coords.scale = self.base.scale;
        }
        coords
    }

    pub fn origin(&self, n: usize) -> [f64; 2] {
        self.frame(n).origin
    }

    pub fn scale_x(&self, n: usize) -> f64 {
        self.frame(n).scale[0]
    }

    pub fn angle(&self, n: usize) -> f64 {
        self.frame(n).angle
    }

    pub fn image_to_world(&self, n: usize, x: f32, y: f32) -> (f32, f32) {
        let (x, y) = self.frame(n).image_to_world(x as f64, y as f64);
        (x as f32, y as f32)
    }

    #[allow(dead_code)]
    pub fn world_to_image(&self, n: usize, x: f64, y: f64) -> (f32, f32) {
        let (x, y) = self.frame(n).world_to_image(x, y);
        (x as f32, y as f32)
    }

    pub fn set_scale(&mut self, n: usize, scale: f64) {
        if self.locked {
            return;
        }
        if self.fixed_scale {
            self.base.scale = [scale, scale];
        } else {
            let mut coords = self.frame(n);
            coords.scale = [scale, scale];
            self.frames.insert(n, coords);
        }
    }

    /// Sets scale for calibration sticks. Coordinate locking applies to the
    /// axes, not to independent calibration-stick objects.
    pub fn set_scale_forced(&mut self, n: usize, scale: f64) {
        if self.fixed_scale {
            self.base.scale = [scale, scale];
        } else {
            let mut coords = self.frame(n);
            coords.scale = [scale, scale];
            self.frames.insert(n, coords);
        }
    }

    pub fn set_angle(&mut self, n: usize, angle: f64) {
        if self.locked {
            return;
        }
        if self.fixed_angle {
            self.base.angle = angle;
        } else {
            let mut coords = self.frame(n);
            coords.angle = angle;
            self.frames.insert(n, coords);
        }
    }

    pub fn set_origin(&mut self, n: usize, origin: [f64; 2]) {
        if self.locked {
            return;
        }
        if self.fixed_origin {
            self.base.origin = origin;
        } else {
            let mut coords = self.frame(n);
            coords.origin = origin;
            self.frames.insert(n, coords);
        }
    }

    pub fn set_all_values_to_frame(&mut self, n: usize) {
        if self.locked {
            return;
        }
        self.base = self.frame(n);
        self.frames.clear();
    }

    pub fn set_fixed_origin(&mut self, fixed: bool, n: usize) {
        if self.locked || self.fixed_origin == fixed {
            return;
        }
        if fixed {
            let value = self.frame(n).origin;
            self.base.origin = value;
        } else {
            let coords = self.frame(n);
            self.frames.insert(n, coords);
        }
        self.fixed_origin = fixed;
    }

    pub fn set_fixed_angle(&mut self, fixed: bool, n: usize) {
        if self.locked || self.fixed_angle == fixed {
            return;
        }
        if fixed {
            let value = self.frame(n).angle;
            self.base.angle = value;
        } else {
            let coords = self.frame(n);
            self.frames.insert(n, coords);
        }
        self.fixed_angle = fixed;
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CalibrationStep {
    pub a: [f64; 2],
    pub b: [f64; 2],
    pub world_a: [f64; 2],
    pub world_b: [f64; 2],
}

impl Default for CalibrationStep {
    fn default() -> Self {
        Self {
            a: [0.0, 0.0],
            b: [1.0, 0.0],
            world_a: [0.0, 0.0],
            world_b: [1.0, 0.0],
        }
    }
}

/// Additional fixed calibration sticks. All sticks share the coordinate scale
/// but keep their own world lengths.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExtraStick {
    pub a: [f64; 2],
    pub b: [f64; 2],
    pub length: f64,
    #[serde(default = "default_true")]
    pub visible: bool,
}

/// Calibration stick. Step 0 is the fixed/default stick; later frames can override.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Calibration {
    #[serde(default)]
    pub steps: BTreeMap<usize, CalibrationStep>,
    #[serde(default)]
    pub extra_sticks: Vec<ExtraStick>,
    #[serde(default = "default_true")]
    pub primary_visible: bool,
    #[serde(default = "default_true")]
    pub fixed_coordinates: bool,
    #[serde(default = "default_unit")]
    pub unit: String,
    #[serde(default = "default_decimals")]
    pub decimals: usize,
}

fn default_unit() -> String {
    "m".to_string()
}

fn default_decimals() -> usize {
    3
}

impl Default for Calibration {
    fn default() -> Self {
        let mut steps = BTreeMap::new();
        steps.insert(0, CalibrationStep::default());
        Self {
            steps,
            extra_sticks: Vec::new(),
            primary_visible: true,
            fixed_coordinates: true,
            unit: default_unit(),
            decimals: default_decimals(),
        }
    }
}

impl Calibration {
    pub fn new(width: f32, height: f32) -> Self {
        let width = width.max(2.0) as f64;
        let height = height.max(2.0) as f64;
        let step = CalibrationStep {
            a: [width * 0.25, height * 0.85],
            b: [width * 0.75, height * 0.85],
            world_a: [0.0, 0.0],
            world_b: [1.0, 0.0],
        };
        let mut steps = BTreeMap::new();
        steps.insert(0, step);
        Self {
            steps,
            extra_sticks: Vec::new(),
            primary_visible: true,
            fixed_coordinates: true,
            unit: default_unit(),
            decimals: default_decimals(),
        }
    }

    pub fn step(&self, n: usize) -> CalibrationStep {
        if self.fixed_coordinates {
            return self
                .steps
                .get(&0)
                .cloned()
                .unwrap_or_else(CalibrationStep::default);
        }
        self.steps
            .range(..=n)
            .next_back()
            .map(|(_, step)| step.clone())
            .or_else(|| self.steps.get(&0).cloned())
            .unwrap_or_else(CalibrationStep::default)
    }

    pub fn set_point(&mut self, n: usize, index: usize, x: f32, y: f32) {
        let key = if self.fixed_coordinates { 0 } else { n };
        let mut step = self.step(key);
        if index == 0 {
            step.a = [x as f64, y as f64];
        } else {
            step.b = [x as f64, y as f64];
        }
        self.steps.insert(key, step);
    }

    pub fn length(&self, n: usize) -> f64 {
        let step = self.step(n);
        let dx = step.world_b[0] - step.world_a[0];
        let dy = step.world_b[1] - step.world_a[1];
        (dx * dx + dy * dy).sqrt()
    }

    pub fn set_length(&mut self, n: usize, length: f64) {
        let key = if self.fixed_coordinates { 0 } else { n };
        let mut step = self.step(key);
        step.world_b = [step.world_a[0] + length.max(1e-9), step.world_a[1]];
        self.steps.insert(key, step);
    }

    /// Set scale, angle and origin in `coords` so the stick maps to its world points.
    pub fn apply(&self, n: usize, coords: &mut ImageCoordSystem) {
        if coords.locked {
            return;
        }
        let step = self.step(n);
        let ax = step.a[0];
        let ay = step.a[1];
        let bx = step.b[0];
        let by = step.b[1];
        let image_dx = bx - ax;
        let image_dy = by - ay;
        let image_distance = (image_dx * image_dx + image_dy * image_dy).sqrt();
        let world_dx = step.world_b[0] - step.world_a[0];
        let world_dy = step.world_b[1] - step.world_a[1];
        let world_distance = (world_dx * world_dx + world_dy * world_dy).sqrt();
        if image_distance <= 1e-9 || world_distance <= 1e-9 {
            return;
        }
        let scale = image_distance / world_distance;
        coords.fixed_scale = self.fixed_coordinates;
        coords.set_scale(n, scale);
    }

    pub fn stick(&self, n: usize, index: usize) -> Option<([f64; 2], [f64; 2], f64)> {
        if index == 0 {
            let step = self.step(n);
            Some((step.a, step.b, self.length(n)))
        } else {
            self.extra_sticks
                .get(index - 1)
                .map(|stick| (stick.a, stick.b, stick.length))
        }
    }

    pub fn stick_count(&self) -> usize {
        1 + self.extra_sticks.len()
    }

    pub fn stick_visible(&self, index: usize) -> bool {
        if index == 0 {
            self.primary_visible
        } else {
            self.extra_sticks
                .get(index - 1)
                .map(|stick| stick.visible)
                .unwrap_or(false)
        }
    }

    pub fn set_stick_visible(&mut self, index: usize, visible: bool) {
        if index == 0 {
            self.primary_visible = visible;
        } else if let Some(stick) = self.extra_sticks.get_mut(index - 1) {
            stick.visible = visible;
        }
    }

    pub fn show_all_sticks(&mut self) {
        self.primary_visible = true;
        for stick in &mut self.extra_sticks {
            stick.visible = true;
        }
    }

    pub fn remove_stick(&mut self, index: usize) {
        if index == 0 {
            if let Some(extra) = self.extra_sticks.first().cloned() {
                self.extra_sticks.remove(0);
                self.steps.clear();
                self.steps.insert(
                    0,
                    CalibrationStep {
                        a: extra.a,
                        b: extra.b,
                        world_a: [0.0, 0.0],
                        world_b: [extra.length, 0.0],
                    },
                );
                self.primary_visible = extra.visible;
            } else {
                self.primary_visible = false;
            }
        } else if index - 1 < self.extra_sticks.len() {
            self.extra_sticks.remove(index - 1);
        }
    }

    pub fn add_extra_stick(&mut self, a: [f64; 2], b: [f64; 2], length: f64) {
        self.extra_sticks.push(ExtraStick {
            a,
            b,
            length: length.max(1e-9),
            visible: true,
        });
    }

    pub fn set_stick_point(&mut self, n: usize, index: usize, end: usize, x: f32, y: f32) {
        if index == 0 {
            self.set_point(n, end, x, y);
        } else if let Some(stick) = self.extra_sticks.get_mut(index - 1) {
            if end == 0 {
                stick.a = [x as f64, y as f64];
            } else {
                stick.b = [x as f64, y as f64];
            }
        }
    }

    pub fn set_stick_length(&mut self, n: usize, index: usize, length: f64) {
        let length = length.max(1e-9);
        if index == 0 {
            self.set_length(n, length);
        } else if let Some(stick) = self.extra_sticks.get_mut(index - 1) {
            stick.length = length;
        }
    }

    /// Resize all sticks except `dragged` around their midpoints so every stick
    /// keeps its own world length at the current coordinate scale.
    pub fn sync_sticks(&mut self, n: usize, coords: &ImageCoordSystem, dragged: Option<usize>) {
        let scale = coords.scale_x(n);
        if scale <= 1e-9 {
            return;
        }
        if dragged != Some(0) {
            let step = self.step(n);
            let length = self.length(n);
            let mid = [(step.a[0] + step.b[0]) * 0.5, (step.a[1] + step.b[1]) * 0.5];
            let dx = step.b[0] - step.a[0];
            let dy = step.b[1] - step.a[1];
            let current = (dx * dx + dy * dy).sqrt();
            let (ux, uy) = if current > 1e-9 {
                (dx / current, dy / current)
            } else {
                (1.0, 0.0)
            };
            let half = length * scale * 0.5;
            self.set_point(
                n,
                0,
                (mid[0] - ux * half) as f32,
                (mid[1] - uy * half) as f32,
            );
            self.set_point(
                n,
                1,
                (mid[0] + ux * half) as f32,
                (mid[1] + uy * half) as f32,
            );
        }
        for (i, stick) in self.extra_sticks.iter_mut().enumerate() {
            if dragged == Some(i + 1) {
                continue;
            }
            let mid = [
                (stick.a[0] + stick.b[0]) * 0.5,
                (stick.a[1] + stick.b[1]) * 0.5,
            ];
            let dx = stick.b[0] - stick.a[0];
            let dy = stick.b[1] - stick.a[1];
            let current = (dx * dx + dy * dy).sqrt();
            let (ux, uy) = if current > 1e-9 {
                (dx / current, dy / current)
            } else {
                (1.0, 0.0)
            };
            let half = stick.length * scale * 0.5;
            stick.a = [mid[0] - ux * half, mid[1] - uy * half];
            stick.b = [mid[0] + ux * half, mid[1] + uy * half];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calibration_maps_stick_to_world_axis() {
        let mut coords = ImageCoordSystem::default();
        let mut calibration = Calibration::new(1000.0, 1000.0);
        coords.set_origin(0, [100.0, 900.0]);
        calibration.set_point(0, 0, 100.0, 900.0);
        calibration.set_point(0, 1, 600.0, 900.0);
        calibration.set_length(0, 2.0);
        calibration.apply(0, &mut coords);

        let (x, y) = coords.image_to_world(0, 100.0, 900.0);
        assert!((x - 0.0).abs() < 1e-6 && (y - 0.0).abs() < 1e-6);
        let (x, y) = coords.image_to_world(0, 600.0, 900.0);
        assert!((x - 2.0).abs() < 1e-6 && y.abs() < 1e-6);
        let (ix, iy) = coords.world_to_image(0, 1.0, 0.0);
        assert!((ix - 350.0).abs() < 1e-4 && (iy - 900.0).abs() < 1e-4);
        assert_eq!(coords.origin(0), [100.0, 900.0]);
    }

    #[test]
    fn extra_sticks_keep_their_own_length_when_scale_changes() {
        let mut coords = ImageCoordSystem::default();
        let mut calibration = Calibration::new(1000.0, 1000.0);
        calibration.set_point(0, 0, 100.0, 900.0);
        calibration.set_point(0, 1, 600.0, 900.0);
        calibration.set_length(0, 2.0);
        calibration.apply(0, &mut coords);
        calibration.add_extra_stick([100.0, 700.0], [200.0, 700.0], 1.0);

        let snapshot = coords.clone();
        calibration.sync_sticks(0, &snapshot, Some(0));
        let (a, b, length) = calibration.stick(0, 1).unwrap();
        assert!((length - 1.0).abs() < 1e-9);
        assert!(((b[0] - a[0]) - 250.0).abs() < 1e-6);

        calibration.set_length(0, 1.0);
        calibration.apply(0, &mut coords);
        let snapshot = coords.clone();
        calibration.sync_sticks(0, &snapshot, Some(0));
        let (a, b, _) = calibration.stick(0, 1).unwrap();
        assert!(((b[0] - a[0]) - 500.0).abs() < 1e-6);
    }

    #[test]
    fn fixed_scale_applies_to_all_frames() {
        let mut coords = ImageCoordSystem::default();
        coords.set_scale(5, 10.0);
        assert!((coords.scale_x(0) - 10.0).abs() < 1e-9);
        assert!((coords.scale_x(100) - 10.0).abs() < 1e-9);
        coords.fixed_scale = false;
        coords.set_scale(5, 20.0);
        assert!((coords.scale_x(5) - 20.0).abs() < 1e-9);
        assert!((coords.scale_x(4) - 10.0).abs() < 1e-9);
    }

    #[test]
    fn fixed_origin_and_angle_toggle_per_frame() {
        let mut coords = ImageCoordSystem::default();
        coords.set_origin(0, [10.0, 20.0]);
        assert_eq!(coords.origin(5), [10.0, 20.0]);
        coords.set_fixed_origin(false, 5);
        coords.set_origin(5, [30.0, 40.0]);
        assert_eq!(coords.origin(5), [30.0, 40.0]);
        assert_eq!(coords.origin(0), [10.0, 20.0]);
        coords.set_fixed_origin(true, 5);
        assert_eq!(coords.origin(0), [30.0, 40.0]);

        coords.set_angle(0, 0.1);
        assert!((coords.angle(5) - 0.1).abs() < 1e-9);
        coords.set_fixed_angle(false, 5);
        coords.set_angle(5, 0.5);
        assert!((coords.angle(5) - 0.5).abs() < 1e-9);
        assert!((coords.angle(0) - 0.1).abs() < 1e-9);
        coords.set_fixed_angle(true, 5);
        assert!((coords.angle(0) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn remove_extra_stick_promotes_next_stick() {
        let mut calibration = Calibration::new(1000.0, 1000.0);
        calibration.add_extra_stick([10.0, 10.0], [110.0, 10.0], 2.0);
        calibration.add_extra_stick([20.0, 20.0], [220.0, 20.0], 4.0);
        calibration.remove_stick(0);
        assert_eq!(calibration.stick_count(), 2);
        let (a, b, length) = calibration.stick(0, 0).unwrap();
        assert_eq!(a, [10.0, 10.0]);
        assert_eq!(b, [110.0, 10.0]);
        assert!((length - 2.0).abs() < 1e-9);
        calibration.remove_stick(1);
        assert_eq!(calibration.stick_count(), 1);
    }

    #[test]
    fn per_frame_calibration_applies_at_its_frame() {
        let mut calibration = Calibration::new(1000.0, 1000.0);
        calibration.fixed_coordinates = false;
        calibration.set_point(10, 0, 100.0, 100.0);
        calibration.set_point(10, 1, 200.0, 100.0);
        calibration.set_length(10, 2.0);
        let mut coords = ImageCoordSystem::default();
        calibration.apply(10, &mut coords);
        assert!((coords.scale_x(10) - 50.0).abs() < 1e-9);
        assert!((coords.scale_x(0) - 1.0).abs() < 1e-9);
        assert!(coords.fixed_origin);
        assert!(coords.fixed_angle);
        assert!(!coords.fixed_scale);
    }
}
