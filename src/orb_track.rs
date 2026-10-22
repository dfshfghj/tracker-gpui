//! Markerless rigid-body tracking from a user-selected ROI.

use crate::i18n::tr;
use crate::i18n::cv;
use std::collections::BTreeMap;

use opencv::core::{self, Point2f};
use opencv::features2d::BFMatcher;

use crate::aruco_detect::gray_from_frame;
use crate::aruco_math::{estimate_pose_inliers, Pose};
use crate::orb_mask::roi_mask;
use crate::orb_match::{match_frame, reference_features};
use crate::rigid::{RigidPoint, RigidResult};
use crate::track::RigidMaskMode;
use crate::video::FrameSource;

pub(crate) fn detect_orb(
    source: &FrameSource,
    roi: (f32, f32, f32, f32),
    reference_frame: usize,
) -> Result<RigidResult, String> {
    let meta = source.meta().clone();
    if meta.frames == 0 {
        return Err(tr("msg.no_frames"));
    }
    let reference_frame = reference_frame.min(meta.frames - 1);
    let rect = normalized_rect(roi, meta.width, meta.height)?;
    let mut reader = source.reader()?;
    let reference = reader.read(reference_frame)?;
    let reference_gray = gray_from_frame(&reference)?;
    let reference = reference_features(&reference_gray, rect)?;
    if reference.keypoints.len() < 20 {
        return Err(tr("msg.too_few_features"));
    }
    let mut matcher = BFMatcher::new(core::NORM_HAMMING, false)
        .map_err(|err| cv("create feature matcher", err))?;
    let roi_center = Point2f::new(
        rect.x as f32 + rect.width as f32 * 0.5,
        rect.y as f32 + rect.height as f32 * 0.5,
    );
    let body_size = ((rect.width as f32) * (rect.height as f32)).sqrt();
    let mut last_pose = Pose {
        a: 1.0,
        b: 0.0,
        tx: 0.0,
        ty: 0.0,
        scale: 1.0,
        used: 0,
    };
    let mut previous_angle = Some(0.0f32);
    let mut points = vec![RigidPoint {
        frame: reference_frame,
        x: roi_center.x,
        y: roi_center.y,
        angle: 0.0,
        scale: body_size,
        marker_count: reference.keypoints.len().min(u8::MAX as usize) as u8,
    }];
    let mut detections = 1usize;
    let trace = std::env::var_os("PROBE_TRACE").is_some();

    for frame_index in reference_frame + 1..meta.frames {
        let frame = reader.read(frame_index)?;
        let gray = gray_from_frame(&frame)?;
        let mask = roi_mask(rect, &last_pose, meta.width as i32, meta.height as i32)?;
        let (source_points, target_points) = match_frame(&reference, &gray, &mask, &mut matcher)?;
        if trace && frame_index < reference_frame + 5 {
            eprintln!("[orb] f{} matches={}", frame_index, source_points.len());
        }
        if source_points.len() < 8 {
            continue;
        }
        let pose_result =
            estimate_pose_inliers(&source_points, &target_points, source_points.len());
        if trace && frame_index < reference_frame + 5 {
            match &pose_result {
                Some((pose, inliers)) => eprintln!(
                    "[orb] f{} inliers={} scale={:.3}",
                    frame_index, inliers, pose.scale
                ),
                None => eprintln!("[orb] f{} no pose", frame_index),
            }
        }
        let Some((pose, inliers)) = pose_result else {
            continue;
        };
        if inliers < 12 || inliers * 6 < source_points.len() {
            continue;
        }
        let current_angle = pose.b.atan2(pose.a);
        let last_angle = last_pose.b.atan2(last_pose.a);
        let angle_delta = (current_angle - last_angle).abs();
        let angle_delta = angle_delta.min(std::f32::consts::TAU - angle_delta);
        let scale_ratio = pose.scale / last_pose.scale.max(1e-4);
        let last_center = transform_point(&last_pose, roi_center);
        let current_center = transform_point(&pose, roi_center);
        let center_jump = ((current_center.0 - last_center.0).powi(2)
            + (current_center.1 - last_center.1).powi(2))
        .sqrt();
        if trace && frame_index < reference_frame + 5 {
            eprintln!(
                "[orb] f{} inliers={} scale={:.3} ratio={:.3} angle_delta={:.3} jump={:.1}",
                frame_index, inliers, pose.scale, scale_ratio, angle_delta, center_jump
            );
        }
        if !(0.6..=1.7).contains(&scale_ratio)
            || !(0.3..=3.0).contains(&pose.scale)
            || angle_delta > 0.7
            || center_jump > body_size * 1.5 + 80.0
        {
            continue;
        }
        last_pose = Pose {
            a: pose.a,
            b: pose.b,
            tx: pose.tx,
            ty: pose.ty,
            scale: pose.scale,
            used: pose.used,
        };
        let mut angle = current_angle;
        if let Some(previous) = previous_angle {
            while angle - previous > std::f32::consts::PI {
                angle -= std::f32::consts::TAU;
            }
            while angle - previous < -std::f32::consts::PI {
                angle += std::f32::consts::TAU;
            }
        }
        previous_angle = Some(angle);
        points.push(RigidPoint {
            frame: frame_index,
            x: current_center.0,
            y: current_center.1,
            angle,
            scale: body_size * pose.scale,
            marker_count: inliers.min(u8::MAX as usize) as u8,
        });
        detections += 1;
    }

    if points.len() < 2 {
        return Err(tr("msg.orb_lost"));
    }
    Ok(RigidResult {
        dictionary: "ORB".into(),
        markers: Vec::new(),
        points,
        detections,
        contour: Vec::new(),
        mask_mode: RigidMaskMode::FixedShape,
        step_contours: BTreeMap::new(),
    })
}

fn transform_point(pose: &Pose, point: Point2f) -> (f32, f32) {
    (
        pose.tx + pose.a * point.x - pose.b * point.y,
        pose.ty + pose.b * point.x + pose.a * point.y,
    )
}

fn normalized_rect(
    roi: (f32, f32, f32, f32),
    width: u32,
    height: u32,
) -> Result<core::Rect, String> {
    let x0 = roi.0.min(roi.0 + roi.2).max(0.0).min(width as f32);
    let y0 = roi.1.min(roi.1 + roi.3).max(0.0).min(height as f32);
    let x1 = roi.0.max(roi.0 + roi.2).max(0.0).min(width as f32);
    let y1 = roi.1.max(roi.1 + roi.3).max(0.0).min(height as f32);
    let w = (x1 - x0).round() as i32;
    let h = (y1 - y0).round() as i32;
    if w < 16 || h < 16 {
        return Err(tr("status.roi_too_small"));
    }
    Ok(core::Rect::new(x0 as i32, y0 as i32, w, h))
}
