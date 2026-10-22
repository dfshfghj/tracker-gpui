//! Layout and per-frame similarity pose for a rigid marker board.

use std::collections::BTreeMap;

use opencv::core::{Point2f, Vector};

use crate::aruco_detect::Detection;
use crate::aruco_math::{
    estimate_pose, inverse_similarity, map_quad, pose_from_corners, quad_area, Pose, MARKER_UNIT,
};

pub(crate) struct Layout {
    pub markers: BTreeMap<u32, [[f32; 2]; 4]>,
}

pub(crate) fn init_layout(detected: &[Detection]) -> Option<Layout> {
    let primary = detected.iter().max_by(|a, b| {
        quad_area(&a.corners)
            .partial_cmp(&quad_area(&b.corners))
            .unwrap_or(std::cmp::Ordering::Equal)
    })?;
    let pose = pose_from_corners(&primary.corners)?;
    let inverse = inverse_similarity(pose);
    let mut markers = BTreeMap::new();
    markers.insert(primary.id, MARKER_UNIT);
    for detection in detected {
        if detection.id != primary.id {
            markers.insert(detection.id, map_quad(&detection.corners, inverse));
        }
    }
    Some(Layout { markers })
}

pub(crate) fn fit_pose(detected: &[Detection], layout: &Layout) -> Option<Pose> {
    let mut source = Vector::<Point2f>::new();
    let mut target = Vector::<Point2f>::new();
    let mut used = 0usize;
    for detection in detected {
        let Some(corners) = layout.markers.get(&detection.id) else {
            continue;
        };
        for index in 0..4 {
            source.push(Point2f::new(corners[index][0], corners[index][1]));
            target.push(detection.corners[index]);
        }
        used += 1;
    }
    if used == 0 {
        return None;
    }
    estimate_pose(&source, &target, used)
}
