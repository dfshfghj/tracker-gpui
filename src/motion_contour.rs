//! Motion-silhouette rigid-body tracking.
//!
//! Fixed-shape mode learns a canonical mask from a short window around the
//! reference frame and tracks it by pose-normalized IoU. Pure-mask mode uses
//! the per-frame motion contour directly and falls back to a grayscale
//! template when the motion mask disappears.

use crate::i18n::tr;
use std::collections::BTreeMap;

use crate::aruco_detect::gray_from_frame;
use crate::contour_math::{nearest_axis, normalized_rect, unwrap_angle};
use crate::motion_segment::{Segmenter, SegmenterKind};
use crate::motion_tracker::{TrackSample, TrackState};
use crate::rigid::{RigidPoint, RigidResult};
use crate::shape_model::{
    collect_reference, collect_window_poses, normalized_window_contour, ShapeModel, ShapePose,
};
use crate::track::RigidMaskMode;
use crate::video::FrameSource;

const DEFAULT_SHAPE_FRAMES: usize = 50;
const MODEL_WARMUP: usize = 120;
const MAX_LOST: usize = 10;

#[derive(Clone, Copy)]
pub(crate) struct MotionOptions {
    pub pure_mask: bool,
    pub shape_frames: usize,
}

impl Default for MotionOptions {
    fn default() -> Self {
        Self {
            pure_mask: false,
            shape_frames: DEFAULT_SHAPE_FRAMES,
        }
    }
}

impl MotionOptions {
    pub(crate) fn from_env() -> Self {
        let pure_mask = std::env::var_os("PROBE_MOTION_PURE").is_some();
        let shape_frames = std::env::var("PROBE_MOTION_SHAPE_FRAMES")
            .ok()
            .and_then(|text| text.trim().parse().ok())
            .unwrap_or(DEFAULT_SHAPE_FRAMES);
        Self {
            pure_mask,
            shape_frames,
        }
    }
}

pub(crate) fn detect_motion_contour(
    source: &FrameSource,
    roi: (f32, f32, f32, f32),
    reference_frame: usize,
    options: MotionOptions,
) -> Result<RigidResult, String> {
    let meta = source.meta().clone();
    if meta.frames == 0 {
        return Err(tr("msg.no_frames"));
    }
    let rect = normalized_rect(roi, meta.width, meta.height)?;
    let reference_frame = reference_frame.min(meta.frames - 1);
    let roi_center = (
        rect.x as f32 + rect.width as f32 * 0.5,
        rect.y as f32 + rect.height as f32 * 0.5,
    );
    let kind = SegmenterKind::from_env();
    let trace = std::env::var_os("PROBE_TRACE").is_some();
    if trace {
        eprintln!(
            "[motion] model={} mode={} reference={} shape_frames={}",
            kind.label(),
            if options.pure_mask { "pure" } else { "fixed" },
            reference_frame,
            options.shape_frames
        );
    }
    if options.pure_mask {
        detect_pure(
            source,
            &meta,
            rect,
            roi_center,
            reference_frame,
            kind,
            trace,
        )
    } else {
        detect_fixed(
            source,
            &meta,
            rect,
            roi_center,
            reference_frame,
            options.shape_frames,
            kind,
            trace,
        )
    }
}

fn detect_fixed(
    source: &FrameSource,
    meta: &crate::video::VideoMeta,
    rect: opencv::core::Rect,
    roi_center: (f32, f32),
    reference_frame: usize,
    shape_frames: usize,
    kind: SegmenterKind,
    trace: bool,
) -> Result<RigidResult, String> {
    let half = (shape_frames.clamp(7, 201) / 2).max(3);
    let window_start = reference_frame.saturating_sub(half);
    let window_end = (reference_frame + half).min(meta.frames - 1);
    let warmup = if kind == SegmenterKind::Median {
        0
    } else {
        MODEL_WARMUP
    };
    let pass_start = window_start.saturating_sub(warmup);
    let mut reader = source.reader()?;
    let mut segmenter = Segmenter::new(source, kind)?;
    let mut window_masks = Vec::with_capacity(window_end - window_start + 1);
    for frame_index in pass_start..=window_end {
        let frame = reader.read(frame_index)?;
        let gray = gray_from_frame(&frame)?;
        let mask = segmenter.mask(&gray)?;
        if frame_index >= window_start {
            window_masks.push((frame_index, mask));
        }
    }
    let window = collect_window_poses(
        &window_masks,
        reference_frame,
        rect,
        roi_center,
        meta.width,
        meta.height,
    )?;
    let shape = ShapeModel::build(&window)?;
    let normalized_contour = shape.normalized_contour().to_vec();

    let mut points = Vec::new();
    let mut last_pose: Option<ShapePose> = None;
    let mut last_angle: Option<f32> = None;
    for (frame_index, info) in &window.poses {
        let angle = unwrap_angle(
            nearest_axis(info.angle, last_angle.unwrap_or(info.angle)),
            last_angle,
        );
        last_angle = Some(angle);
        let scale = info.area.sqrt().max(1.0);
        let pose = ShapePose {
            center: info.center,
            angle,
            scale,
            area: info.area,
            iou: 1.0,
        };
        points.push(RigidPoint {
            frame: *frame_index,
            x: info.center.0,
            y: info.center.1,
            angle,
            scale,
            marker_count: 100,
        });
        last_pose = Some(pose);
    }
    let Some(pose) = last_pose else {
        return Err(tr("msg.no_pose_in_window"));
    };
    let velocity = estimate_velocity(&points);
    if trace {
        eprintln!(
            "[motion] fixed window {}-{} masks={} poses={} shape_area={}",
            window_start,
            window_end,
            window_masks.len(),
            points.len(),
            shape.reference_scale
        );
    }
    drop(window_masks);

    let mut tracker = TrackState::new_fixed(shape, pose, velocity, last_angle);
    for frame_index in (window_end + 1)..meta.frames {
        let frame = reader.read(frame_index)?;
        let gray = gray_from_frame(&frame)?;
        let mask = segmenter.mask(&gray)?;
        if let Some(TrackSample { point, .. }) =
            tracker.step(&mask, &gray, frame_index, meta.width, meta.height)?
        {
            points.push(point);
        }
        if tracker.lost_count() > MAX_LOST {
            if trace {
                eprintln!("[motion] fixed stopped at frame {frame_index}");
            }
            break;
        }
    }
    if points.len() < 2 {
        return Err(tr("msg.motion_contour_lost"));
    }
    if trace {
        eprintln!(
            "[motion] fixed points={} last={}",
            points.len(),
            points.last().map(|point| point.frame).unwrap_or(0)
        );
    }
    let detections = points.len();
    Ok(RigidResult {
        dictionary: "MOTION".into(),
        markers: Vec::new(),
        points,
        detections,
        contour: normalized_contour,
        mask_mode: RigidMaskMode::FixedShape,
        step_contours: BTreeMap::new(),
    })
}

fn detect_pure(
    source: &FrameSource,
    meta: &crate::video::VideoMeta,
    rect: opencv::core::Rect,
    roi_center: (f32, f32),
    reference_frame: usize,
    kind: SegmenterKind,
    trace: bool,
) -> Result<RigidResult, String> {
    let warmup = if kind == SegmenterKind::Median {
        0
    } else {
        MODEL_WARMUP
    };
    let pass_start = reference_frame.saturating_sub(warmup);
    let mut reader = source.reader()?;
    let mut segmenter = Segmenter::new(source, kind)?;
    let mut reference_gray = None;
    let mut reference_mask = None;
    for frame_index in pass_start..=reference_frame {
        let frame = reader.read(frame_index)?;
        let gray = gray_from_frame(&frame)?;
        let mask = segmenter.mask(&gray)?;
        if frame_index == reference_frame {
            reference_gray = Some(gray);
            reference_mask = Some(mask);
        }
    }
    let (Some(reference_gray), Some(reference_mask)) = (reference_gray, reference_mask) else {
        return Err(tr("msg.reference_unreadable"));
    };
    let reference = collect_reference(&reference_mask, rect, roi_center)?
        .ok_or_else(|| tr("msg.no_contour_near_reference"))?;
    let scale = reference.area.sqrt().max(1.0);
    let pose = ShapePose {
        center: reference.center,
        angle: reference.angle,
        scale,
        area: reference.area,
        iou: 1.0,
    };
    let reference_contour = normalized_window_contour(&reference, reference.angle);
    let mut points = vec![RigidPoint {
        frame: reference_frame,
        x: reference.center.0,
        y: reference.center.1,
        angle: reference.angle,
        scale,
        marker_count: 100,
    }];
    let mut contours = BTreeMap::new();
    contours.insert(reference_frame, reference_contour.clone());
    if trace {
        eprintln!(
            "[motion] pure reference={} area={:.0} scale={:.1}",
            reference_frame, reference.area, scale
        );
    }
    let mut tracker = TrackState::new_pure(
        &reference,
        &reference_gray,
        pose,
        (0.0, 0.0),
        Some(reference.angle),
    );
    for frame_index in (reference_frame + 1)..meta.frames {
        let frame = reader.read(frame_index)?;
        let gray = gray_from_frame(&frame)?;
        let mask = segmenter.mask(&gray)?;
        if let Some(sample) = tracker.step(&mask, &gray, frame_index, meta.width, meta.height)? {
            if let Some(contour) = &sample.contour {
                contours.insert(frame_index, contour.clone());
            }
            points.push(sample.point);
        }
        if tracker.lost_count() > MAX_LOST {
            if trace {
                eprintln!("[motion] pure stopped at frame {frame_index}");
            }
            break;
        }
    }
    if points.len() < 2 {
        return Err(tr("msg.motion_contour_lost"));
    }
    if trace {
        eprintln!(
            "[motion] pure points={} last={}",
            points.len(),
            points.last().map(|point| point.frame).unwrap_or(0)
        );
    }
    let detections = points.len();
    Ok(RigidResult {
        dictionary: "MOTION".into(),
        markers: Vec::new(),
        points,
        detections,
        contour: reference_contour,
        mask_mode: RigidMaskMode::PureMotion,
        step_contours: contours,
    })
}

fn estimate_velocity(points: &[RigidPoint]) -> (f32, f32) {
    if points.len() < 2 {
        return (0.0, 0.0);
    }
    let take = points.len().min(6);
    let first = &points[points.len() - take];
    let last = &points[points.len() - 1];
    let dt = (last.frame - first.frame).max(1) as f32;
    ((last.x - first.x) / dt, (last.y - first.y) / dt)
}
