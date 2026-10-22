//! ArUco based 2D rigid-body tracking.

use std::collections::BTreeMap;

use crate::aruco_detect::{detect_frame, gray_from_frame, new_detector};
use crate::aruco_pose::{fit_pose, init_layout, Layout};
use crate::track::RigidMaskMode;
use crate::video::FrameSource;

const DICTIONARY_NAME: &str = "DICT_4X4_50";

#[derive(Clone, Copy, Debug)]
pub(crate) struct RigidPoint {
    pub frame: usize,
    pub x: f32,
    pub y: f32,
    pub angle: f32,
    pub scale: f32,
    pub marker_count: u8,
}

#[derive(Clone, Debug)]
pub(crate) struct RigidMarker {
    pub id: u32,
    pub corners: [[f32; 2]; 4],
}

pub(crate) struct RigidResult {
    pub dictionary: String,
    pub markers: Vec<RigidMarker>,
    pub points: Vec<RigidPoint>,
    pub detections: usize,
    pub contour: Vec<[f32; 2]>,
    pub mask_mode: RigidMaskMode,
    pub step_contours: BTreeMap<usize, Vec<[f32; 2]>>,
}

pub(crate) fn detect_rigid(source: &FrameSource) -> Result<RigidResult, String> {
    let meta = source.meta().clone();
    if meta.frames == 0 {
        return Err("视频没有可用帧".into());
    }
    let detector = new_detector()?;
    let mut reader = source.reader()?;
    let mut layout: Option<Layout> = None;
    let mut points = Vec::new();
    let mut detections = 0usize;
    let mut previous_angle: Option<f32> = None;

    for frame_index in 0..meta.frames {
        let frame = reader.read(frame_index)?;
        let gray = gray_from_frame(&frame)?;
        let detected = detect_frame(&detector, &gray)?;
        detections += detected.len();
        if detected.is_empty() {
            continue;
        }
        if layout.is_none() {
            layout = init_layout(&detected);
        }
        let Some(layout) = layout.as_ref() else {
            continue;
        };
        let Some(pose) = fit_pose(&detected, layout) else {
            continue;
        };
        let mut angle = pose.b.atan2(pose.a);
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
            x: pose.tx,
            y: pose.ty,
            angle,
            scale: pose.scale,
            marker_count: pose.used.min(u8::MAX as usize) as u8,
        });
    }

    let Some(layout) = layout else {
        return Err("未检测到 ArUco 标记".into());
    };
    if points.len() < 2 {
        return Err("刚体轨迹点太少".into());
    }
    Ok(RigidResult {
        dictionary: DICTIONARY_NAME.to_string(),
        markers: layout
            .markers
            .iter()
            .map(|(id, corners)| RigidMarker {
                id: *id,
                corners: *corners,
            })
            .collect(),
        points,
        detections,
        contour: Vec::new(),
        mask_mode: RigidMaskMode::FixedShape,
        step_contours: BTreeMap::new(),
    })
}
