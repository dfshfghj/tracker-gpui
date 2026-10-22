//! Frozen silhouette model used by the fixed-shape motion tracker.
//!
//! The model is built from a short window around the reference frame. Each
//! observed contour is mapped into a pose-normalized canonical frame, the
//! aligned masks are fused, and the resulting contour is never updated while
//! tracking.

use crate::i18n::tr;
use crate::i18n::cv;
use opencv::core::{self, Mat, Point, Point2f, Scalar, Size, Vector};
use opencv::imgproc;
use opencv::prelude::*;

use crate::contour_math::distance;
use crate::contour_util::{find_contours_in, ContourInfo};

pub(crate) const CANON_SIZE: i32 = 96;
pub(crate) const CANON_CENTER: f32 = 48.0;
pub(crate) const CANON_RADIUS: f32 = 22.0;
pub(crate) const MIN_SHAPE_IOU: f32 = 0.72;
/// Peaks weaker than this fraction of the strongest peak are not hypotheses.
const PEAK_RELATIVE_THRESHOLD: f32 = 0.90;
/// Maximum continuity cost at a 180° branch difference. Small enough that a
/// genuinely better observation can still switch branches.
const CONTINUITY_COST: f32 = 0.05;

fn angular_delta(a: f32, b: f32) -> f32 {
    let two_pi = 2.0 * std::f32::consts::PI;
    let mut delta = (a - b) % two_pi;
    if delta > std::f32::consts::PI {
        delta -= two_pi;
    } else if delta < -std::f32::consts::PI {
        delta += two_pi;
    }
    delta
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ShapePose {
    pub center: (f32, f32),
    pub angle: f32,
    pub scale: f32,
    pub area: f32,
    pub iou: f32,
}

pub(crate) struct ShapeModel {
    /// Frozen contour in the canonical 96×96 frame.
    canonical: Vec<Point2f>,
    /// Contour normalized by CANON_RADIUS, ready for Track::rigid.contour.
    normalized: Vec<[f32; 2]>,
    pub reference_scale: f32,
}

pub(crate) struct WindowTrack {
    pub reference: ContourInfo,
    pub poses: Vec<(usize, ContourInfo)>,
}

pub(crate) fn collect_reference(
    mask: &Mat,
    roi: core::Rect,
    center: (f32, f32),
) -> Result<Option<ContourInfo>, String> {
    let x = (roi.x - 8).max(0);
    let y = (roi.y - 8).max(0);
    let right = (roi.x + roi.width + 8).min(mask.cols()).max(x + 4);
    let bottom = (roi.y + roi.height + 8).min(mask.rows()).max(y + 4);
    let expanded = core::Rect::new(x, y, right - x, bottom - y);
    let candidates = find_contours_in(mask, expanded)?;
    Ok(candidates.into_iter().min_by(|a, b| {
        distance(a.center, center)
            .partial_cmp(&distance(b.center, center))
            .unwrap_or(std::cmp::Ordering::Equal)
    }))
}

pub(crate) fn collect_window_poses(
    window_masks: &[(usize, Mat)],
    reference_frame: usize,
    roi: core::Rect,
    center: (f32, f32),
    width: u32,
    height: u32,
) -> Result<WindowTrack, String> {
    let reference_index = window_masks
        .iter()
        .position(|(frame, _)| *frame == reference_frame)
        .ok_or_else(|| tr("msg.reference_outside_window"))?;
    let reference = collect_reference(&window_masks[reference_index].1, roi, center)?
        .ok_or_else(|| tr("msg.no_contour_near_reference"))?;
    let mut poses: Vec<(usize, ContourInfo)> =
        vec![(reference_frame, reference.clone())];

    let mut previous = reference.clone();
    for (frame, mask) in window_masks.iter().skip(reference_index + 1) {
        if let Some(next) = nearest_contour(mask, &previous, width, height)? {
            poses.push((*frame, next.clone()));
            previous = next;
        }
    }

    let mut previous = reference.clone();
    for (frame, mask) in window_masks.iter().take(reference_index).rev() {
        if let Some(next) = nearest_contour(mask, &previous, width, height)? {
            poses.push((*frame, next.clone()));
            previous = next;
        }
    }
    poses.sort_by_key(|(frame, _)| *frame);
    Ok(WindowTrack { reference, poses })
}

fn nearest_contour(
    mask: &Mat,
    previous: &ContourInfo,
    width: u32,
    height: u32,
) -> Result<Option<ContourInfo>, String> {
    let scale = previous.area.sqrt().max(1.0);
    let search = crate::contour_math::search_rect(previous.center, scale, width, height);
    let candidates = find_contours_in(mask, search)?;
    let mut best: Option<(f32, ContourInfo)> = None;
    for candidate in candidates {
        let area_ratio = candidate.area / previous.area.max(1.0);
        if !(0.30..=3.0).contains(&area_ratio) {
            continue;
        }
        let jump = distance(candidate.center, previous.center);
        if jump > scale * 1.5 + 60.0 {
            continue;
        }
        if best
            .as_ref()
            .map_or(true, |(best_jump, _)| jump < *best_jump)
        {
            best = Some((jump, candidate));
        }
    }
    Ok(best.map(|(_, info)| info))
}

impl ShapeModel {
    pub(crate) fn build(track: &WindowTrack) -> Result<Self, String> {
        let reference_points = normalize(&track.reference, false);
        let reference_mask = rasterize(&reference_points)?;
        let mut masks = Vec::new();
        for (_, info) in &track.poses {
            let mut best: Option<(f32, Mat)> = None;
            for flip in [false, true] {
                let points = normalize(info, flip);
                let mask = rasterize(&points)?;
                let iou = mask_iou(&reference_mask, &mask)?;
                if best.as_ref().map_or(true, |(best_iou, _)| iou > *best_iou) {
                    best = Some((iou, mask));
                }
            }
            let Some((iou, mask)) = best else {
                continue;
            };
            if iou >= 0.70 {
                masks.push(mask);
            }
        }
        if masks.len() < 5 {
            return Err(tr("msg.too_few_contours"));
        }
        let mut counts = vec![0u16; (CANON_SIZE * CANON_SIZE) as usize];
        for mask in &masks {
            let bytes = mask
                .data_typed::<u8>()
                .map_err(|err| cv("read normalized mask", err))?;
            for (index, value) in bytes.iter().enumerate() {
                if *value > 0 {
                    counts[index] = counts[index].saturating_add(1);
                }
            }
        }
        let needed = (masks.len() as u16 * 6 + 9) / 10;
        let fused_bytes: Vec<u8> = counts
            .iter()
            .map(|count| if *count >= needed { 255 } else { 0 })
            .collect();
        let flat = Mat::from_slice(&fused_bytes)
            .map_err(|err| cv("blend mask", err))?;
        let mut fused = flat
            .reshape(1, CANON_SIZE)
            .map_err(|err| cv("blend mask reshape", err))?
            .clone_pointee();
        let kernel = imgproc::get_structuring_element(
            imgproc::MORPH_ELLIPSE,
            Size::new(3, 3),
            Point::new(-1, -1),
        )
        .map_err(|err| cv("morphology kernel", err))?;
        let mut cleaned = Mat::default();
        imgproc::morphology_ex(
            &fused,
            &mut cleaned,
            imgproc::MORPH_CLOSE,
            &kernel,
            Point::new(-1, -1),
            1,
            core::BORDER_CONSTANT,
            Scalar::all(0.0),
        )
        .map_err(|err| cv("blend mask close", err))?;
        fused = cleaned;

        let mut contours = Vector::<Mat>::new();
        imgproc::find_contours(
            &fused,
            &mut contours,
            imgproc::RETR_EXTERNAL,
            imgproc::CHAIN_APPROX_NONE,
            Point::new(0, 0),
        )
        .map_err(|err| cv("find fixed contour", err))?;
        let mut best: Option<(f64, Mat)> = None;
        for contour in contours.iter() {
            let area = imgproc::contour_area(&contour, false)
                .map_err(|err| cv("fixed contour area", err))?;
            if area < 20.0 {
                continue;
            }
            if best.as_ref().map_or(true, |(best_area, _)| area > *best_area) {
                best = Some((area, contour));
            }
        }
        let Some((_, contour)) = best else {
            return Err(tr("msg.no_valid_contour"));
        };
        let points = contour
            .data_typed::<Point>()
            .map_err(|err| cv("read fixed contour", err))?
            .iter()
            .map(|point| Point2f::new(point.x as f32, point.y as f32))
            .collect::<Vec<_>>();
        if points.len() < 3 {
            return Err(tr("msg.too_few_contour_points"));
        }
        let normalized = points
            .iter()
            .map(|point| {
                [
                    (point.x - CANON_CENTER) / CANON_RADIUS,
                    (point.y - CANON_CENTER) / CANON_RADIUS,
                ]
            })
            .collect();
        let reference_scale = track.reference.area.sqrt().max(1.0);
        Ok(Self {
            canonical: points,
            normalized,
            reference_scale,
        })
    }

    pub(crate) fn normalized_contour(&self) -> &[[f32; 2]] {
        &self.normalized
    }

    pub(crate) fn match_candidates(
        &self,
        mask: &Mat,
        search: core::Rect,
        predicted: (f32, f32),
        last: ShapePose,
    ) -> Result<Option<ShapePose>, String> {
        let candidates = find_contours_in(mask, search)?;
        let mut best_candidate: Option<(f32, usize, Vec<ShapePose>)> = None;
        for (index, info) in candidates.iter().enumerate() {
            let area_ratio = info.area / last.area.max(1.0);
            if !(0.35..=2.8).contains(&area_ratio) {
                continue;
            }
            let jump = distance(info.center, predicted);
            if jump > last.scale * 1.5 + 80.0 {
                continue;
            }
            let peaks = self.branch_peaks(info)?;
            let best_iou = peaks
                .iter()
                .map(|pose| pose.iou)
                .fold(0.0f32, f32::max);
            if best_iou < MIN_SHAPE_IOU {
                continue;
            }
            if best_candidate
                .as_ref()
                .map_or(true, |(candidate_iou, _, _)| best_iou > *candidate_iou)
            {
                best_candidate = Some((best_iou, index, peaks));
            }
        }
        let Some((best_iou, index, peaks)) = best_candidate else {
            return Ok(None);
        };
        let threshold = (best_iou * PEAK_RELATIVE_THRESHOLD).max(MIN_SHAPE_IOU);
        let mut selected: Option<ShapePose> = None;
        let mut best_score = f32::NEG_INFINITY;
        for peak in peaks.iter().filter(|peak| peak.iou >= threshold) {
            let delta = angular_delta(peak.angle, last.angle);
            let cost = CONTINUITY_COST * 0.5 * (1.0 - delta.cos());
            let score = peak.iou - cost;
            if score > best_score {
                best_score = score;
                selected = Some(*peak);
            }
        }
        let Some(selected) = selected else {
            return Ok(None);
        };
        let info = &candidates[index];
        self.refine_peak(info, selected).map(Some)
    }

    /// Returns the best pose for each of the two 180°-equivalent branches.
    /// The continuity cost is applied only to these real peaks, never to the
    /// angle optimization itself.
    fn branch_peaks(&self, info: &ContourInfo) -> Result<Vec<ShapePose>, String> {
        let base_scale = info.area.sqrt().max(1.0);
        let mut peaks = Vec::new();
        for flip in [0.0, std::f32::consts::PI] {
            let base_angle = info.angle + flip;
            let mut best: Option<ShapePose> = None;
            for delta in [-6.0f32, -3.0, 0.0, 3.0, 6.0] {
                let angle = base_angle + delta.to_radians();
                for factor in [0.94f32, 0.97, 1.0, 1.03, 1.06] {
                    let scale = base_scale * factor;
                    let iou = self.iou_for_pose(info, angle, scale)?;
                    if best.as_ref().map_or(true, |pose| iou > pose.iou) {
                        best = Some(ShapePose {
                            center: info.center,
                            angle,
                            scale,
                            area: info.area,
                            iou,
                        });
                    }
                }
            }
            if let Some(peak) = best {
                peaks.push(peak);
            }
        }
        Ok(peaks)
    }

    fn refine_peak(
        &self,
        info: &ContourInfo,
        peak: ShapePose,
    ) -> Result<ShapePose, String> {
        let mut best = peak;
        let step = 0.5f32.to_radians();
        let half = 2.5f32.to_radians();
        let mut delta = -half;
        while delta <= half + 1e-6 {
            let angle = peak.angle + delta;
            for factor in [0.97f32, 0.985, 1.0, 1.015, 1.03] {
                let scale = peak.scale * factor;
                let iou = self.iou_for_pose(info, angle, scale)?;
                if iou > best.iou {
                    best = ShapePose {
                        center: peak.center,
                        angle,
                        scale,
                        area: peak.area,
                        iou,
                    };
                }
            }
            delta += step;
        }
        Ok(best)
    }

    fn iou_for_pose(
        &self,
        candidate: &ContourInfo,
        angle: f32,
        scale: f32,
    ) -> Result<f32, String> {
        let (sin, cos) = angle.sin_cos();
        let factor = scale / CANON_RADIUS;
        let transformed: Vec<Point2f> = self
            .canonical
            .iter()
            .map(|point| {
                let x = (point.x - CANON_CENTER) * factor;
                let y = (point.y - CANON_CENTER) * factor;
                Point2f::new(
                    candidate.center.0 + cos * x - sin * y,
                    candidate.center.1 + sin * x + cos * y,
                )
            })
            .collect();
        let min_x = transformed
            .iter()
            .map(|point| point.x)
            .fold(candidate.bbox.x as f32, f32::min);
        let min_y = transformed
            .iter()
            .map(|point| point.y)
            .fold(candidate.bbox.y as f32, f32::min);
        let max_x = transformed
            .iter()
            .map(|point| point.x)
            .fold((candidate.bbox.x + candidate.bbox.width) as f32, f32::max);
        let max_y = transformed
            .iter()
            .map(|point| point.y)
            .fold((candidate.bbox.y + candidate.bbox.height) as f32, f32::max);
        let x0 = (min_x.floor() as i32 - 4).max(0);
        let y0 = (min_y.floor() as i32 - 4).max(0);
        let x1 = ((max_x.ceil() as i32 + 4).max(0)).min(4096);
        let y1 = ((max_y.ceil() as i32 + 4).max(0)).min(4096);
        if x1 - x0 < 3 || y1 - y0 < 3 {
            return Ok(0.0);
        }
        let local = core::Rect::new(x0, y0, x1 - x0, y1 - y0);
        let mut candidate_mask = Mat::new_rows_cols_with_default(
            local.height,
            local.width,
            core::CV_8UC1,
            Scalar::all(0.0),
        )
        .map_err(|err| cv("candidate mask", err))?;
        let candidate_poly: Vector<Point> = candidate
            .points
            .iter()
            .map(|point| {
                Point::new(
                    point.x.round() as i32 - local.x,
                    point.y.round() as i32 - local.y,
                )
            })
            .collect();
        let candidate_polygons = Vector::<Vector<Point>>::from_iter([candidate_poly]);
        imgproc::fill_poly(
            &mut candidate_mask,
            &candidate_polygons,
            Scalar::all(255.0),
            imgproc::LINE_8,
            0,
            Point::new(0, 0),
        )
        .map_err(|err| cv("fill candidate contour", err))?;

        let mut model_mask = Mat::new_rows_cols_with_default(
            local.height,
            local.width,
            core::CV_8UC1,
            Scalar::all(0.0),
        )
        .map_err(|err| cv("model mask", err))?;
        let model_poly: Vector<Point> = transformed
            .iter()
            .map(|point| {
                Point::new(
                    point.x.round() as i32 - local.x,
                    point.y.round() as i32 - local.y,
                )
            })
            .collect();
        let model_polygons = Vector::<Vector<Point>>::from_iter([model_poly]);
        imgproc::fill_poly(
            &mut model_mask,
            &model_polygons,
            Scalar::all(255.0),
            imgproc::LINE_8,
            0,
            Point::new(0, 0),
        )
        .map_err(|err| cv("fill model contour", err))?;

        let mut intersection = Mat::default();
        core::bitwise_and(
            &candidate_mask,
            &model_mask,
            &mut intersection,
            &core::no_array(),
        )
        .map_err(|err| cv("mask intersection", err))?;
        let mut union = Mat::default();
        core::bitwise_or(&candidate_mask, &model_mask, &mut union, &core::no_array())
            .map_err(|err| cv("mask union", err))?;
        let intersection = core::count_non_zero(&intersection)
            .map_err(|err| cv("count mask intersection", err))?;
        let union =
            core::count_non_zero(&union).map_err(|err| cv("count mask union", err))?;
        Ok(if union > 0 {
            intersection as f32 / union as f32
        } else {
            0.0
        })
    }
}

pub(crate) fn normalized_window_contour(info: &ContourInfo, angle: f32) -> Vec<[f32; 2]> {
    let scale = info.area.sqrt().max(1.0);
    let (sin, cos) = (-angle).sin_cos();
    info.points
        .iter()
        .map(|point| {
            let x = point.x - info.center.0;
            let y = point.y - info.center.1;
            [
                (cos * x - sin * y) / scale,
                (sin * x + cos * y) / scale,
            ]
        })
        .collect()
}

fn normalize(info: &ContourInfo, flip: bool) -> Vec<Point2f> {
    let scale = info.area.sqrt().max(1e-3);
    let angle = info.angle + if flip { std::f32::consts::PI } else { 0.0 };
    let (sin, cos) = (-angle).sin_cos();
    info.points
        .iter()
        .map(|point| {
            let x = point.x - info.center.0;
            let y = point.y - info.center.1;
            Point2f::new(
                (cos * x - sin * y) / scale * CANON_RADIUS + CANON_CENTER,
                (sin * x + cos * y) / scale * CANON_RADIUS + CANON_CENTER,
            )
        })
        .collect()
}

fn rasterize(points: &[Point2f]) -> Result<Mat, String> {
    let mut mask = Mat::new_rows_cols_with_default(
        CANON_SIZE,
        CANON_SIZE,
        core::CV_8UC1,
        Scalar::all(0.0),
    )
    .map_err(|err| cv("normalize mask", err))?;
    let polygon: Vector<Point> = points
        .iter()
        .map(|point| Point::new(point.x.round() as i32, point.y.round() as i32))
        .collect();
    let polygons = Vector::<Vector<Point>>::from_iter([polygon]);
    imgproc::fill_poly(
        &mut mask,
        &polygons,
        Scalar::all(255.0),
        imgproc::LINE_8,
        0,
        Point::new(0, 0),
    )
    .map_err(|err| cv("fill normalized contour", err))?;
    Ok(mask)
}

#[cfg(test)]
mod tests {
    use super::angular_delta;
    use std::f32::consts::{PI, TAU};

    #[test]
    fn angular_delta_wraps_to_pi() {
        assert!(angular_delta(0.0, 0.0).abs() < 1e-6);
        assert!((angular_delta(PI, 0.0) - PI).abs() < 1e-6);
        assert!((angular_delta(-PI, 0.0) + PI).abs() < 1e-6);
        assert!(angular_delta(TAU, 0.0).abs() < 1e-6);
        assert!((angular_delta(-TAU - 0.25, 0.0) + 0.25).abs() < 1e-6);
    }
}

fn mask_iou(a: &Mat, b: &Mat) -> Result<f32, String> {
    let mut intersection = Mat::default();
    core::bitwise_and(a, b, &mut intersection, &core::no_array())
        .map_err(|err| cv("mask intersection", err))?;
    let mut union = Mat::default();
    core::bitwise_or(a, b, &mut union, &core::no_array())
        .map_err(|err| cv("mask union", err))?;
    let intersection =
        core::count_non_zero(&intersection).map_err(|err| cv("count intersection", err))?;
    let union = core::count_non_zero(&union).map_err(|err| cv("count union", err))?;
    Ok(if union > 0 {
        intersection as f32 / union as f32
    } else {
        0.0
    })
}
