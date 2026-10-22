//! Forward tracker for the two motion-mask modes.

use opencv::core::Mat;

use crate::contour_math::{distance, nearest_axis, search_rect, unwrap_angle};
use crate::contour_util::{
    find_contours_in, make_template, match_template_in, ContourInfo, Template,
};
use crate::rigid::RigidPoint;
use crate::shape_model::{
    normalized_window_contour, ShapeModel, ShapePose,
};

pub(crate) enum TrackMode {
    Fixed(ShapeModel),
    Pure { template: Option<Template> },
}

pub(crate) struct TrackState {
    mode: TrackMode,
    pose: ShapePose,
    velocity: (f32, f32),
    previous_angle: Option<f32>,
    last_contour: Vec<[f32; 2]>,
    lost: usize,
}

pub(crate) struct TrackSample {
    pub point: RigidPoint,
    pub contour: Option<Vec<[f32; 2]>>,
}

impl TrackState {
    pub(crate) fn new_fixed(
        shape: ShapeModel,
        pose: ShapePose,
        velocity: (f32, f32),
        previous_angle: Option<f32>,
    ) -> Self {
        Self {
            mode: TrackMode::Fixed(shape),
            pose,
            velocity,
            previous_angle,
            last_contour: Vec::new(),
            lost: 0,
        }
    }

    pub(crate) fn new_pure(
        reference: &ContourInfo,
        reference_gray: &Mat,
        pose: ShapePose,
        velocity: (f32, f32),
        previous_angle: Option<f32>,
    ) -> Self {
        let template = make_template(reference_gray, reference).ok();
        Self {
            mode: TrackMode::Pure { template },
            pose,
            velocity,
            previous_angle,
            last_contour: normalized_window_contour(reference, pose.angle),
            lost: 0,
        }
    }

    pub(crate) fn lost_count(&self) -> usize {
        self.lost
    }

    pub(crate) fn step(
        &mut self,
        mask: &Mat,
        gray: &Mat,
        frame: usize,
        width: u32,
        height: u32,
    ) -> Result<Option<TrackSample>, String> {
        let predicted = (
            self.pose.center.0 + self.velocity.0,
            self.pose.center.1 + self.velocity.1,
        );
        let search = search_rect(predicted, self.pose.scale, width, height);

        if let TrackMode::Fixed(shape) = &self.mode {
            if let Some(pose) = shape.match_candidates(mask, search, predicted, self.pose)? {
                let old = self.pose.center;
                let angle = unwrap_angle(pose.angle, self.previous_angle);
                self.previous_angle = Some(angle);
                self.pose = ShapePose {
                    center: pose.center,
                    angle,
                    scale: pose.scale,
                    area: pose.area,
                    iou: pose.iou,
                };
                self.velocity = (
                    0.5 * self.velocity.0 + 0.5 * (pose.center.0 - old.0),
                    0.5 * self.velocity.1 + 0.5 * (pose.center.1 - old.1),
                );
                self.lost = 0;
                return Ok(Some(TrackSample {
                    point: RigidPoint {
                        frame,
                        x: pose.center.0,
                        y: pose.center.1,
                        angle,
                        scale: pose.scale,
                        marker_count: (pose.iou * 100.0).clamp(1.0, 100.0) as u8,
                    },
                    contour: None,
                }));
            }
            self.lost += 1;
            return Ok(None);
        }

        let candidates = find_contours_in(mask, search)?;
        let mut best: Option<(f32, ContourInfo)> = None;
        for candidate in candidates {
            let area_ratio = candidate.area / self.pose.area.max(1.0);
            if !(0.35..=2.8).contains(&area_ratio) {
                continue;
            }
            let scale = candidate.area.sqrt().max(1.0);
            let scale_ratio = scale / self.pose.scale.max(1e-4);
            if !(0.5..=2.0).contains(&scale_ratio) {
                continue;
            }
            let jump = distance(candidate.center, predicted);
            if jump > self.pose.scale * 1.5 + 80.0 {
                continue;
            }
            let cost = jump - candidate.area.min(2000.0) * 0.001;
            if best.as_ref().map_or(true, |(best_cost, _)| cost < *best_cost) {
                best = Some((cost, candidate));
            }
        }

        if let Some((_, info)) = best {
            let old = self.pose.center;
            let angle = unwrap_angle(
                nearest_axis(info.angle, self.previous_angle.unwrap_or(info.angle)),
                self.previous_angle,
            );
            self.previous_angle = Some(angle);
            let scale = info.area.sqrt().max(1.0);
            self.pose = ShapePose {
                center: info.center,
                angle,
                scale,
                area: info.area,
                iou: 1.0,
            };
            self.velocity = (
                0.5 * self.velocity.0 + 0.5 * (info.center.0 - old.0),
                0.5 * self.velocity.1 + 0.5 * (info.center.1 - old.1),
            );
            self.last_contour = normalized_window_contour(&info, angle);
            if let TrackMode::Pure { template } = &mut self.mode {
                if let Ok(new_template) = make_template(gray, &info) {
                    *template = Some(new_template);
                }
            }
            self.lost = 0;
            return Ok(Some(TrackSample {
                point: RigidPoint {
                    frame,
                    x: info.center.0,
                    y: info.center.1,
                    angle,
                    scale,
                    marker_count: 100,
                },
                contour: Some(self.last_contour.clone()),
            }));
        }

        if let TrackMode::Pure {
            template: Some(template),
        } = &self.mode
        {
            if let Some((center, score)) = match_template_in(gray, search, template)? {
                if distance(center, predicted) <= self.pose.scale * 1.5 + 80.0 {
                    let old = self.pose.center;
                    self.pose.center = center;
                    self.velocity = (
                        0.5 * self.velocity.0 + 0.5 * (center.0 - old.0),
                        0.5 * self.velocity.1 + 0.5 * (center.1 - old.1),
                    );
                    self.lost = 0;
                    return Ok(Some(TrackSample {
                        point: RigidPoint {
                            frame,
                            x: center.0,
                            y: center.1,
                            angle: self.pose.angle,
                            scale: self.pose.scale,
                            marker_count: (score * 100.0).clamp(1.0, 100.0) as u8,
                        },
                        contour: Some(self.last_contour.clone()),
                    }));
                }
            }
        }
        self.lost += 1;
        Ok(None)
    }
}
