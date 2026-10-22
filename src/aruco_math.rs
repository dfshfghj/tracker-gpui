//! Similarity transform helpers for ArUco corners.

use opencv::calib3d::{estimate_affine_partial_2d, RANSAC};
use opencv::core::{count_non_zero, Mat, Point2f, Vector};
use opencv::prelude::*;

pub(crate) const MARKER_UNIT: [[f32; 2]; 4] = [[-0.5, -0.5], [0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]];

pub(crate) struct Pose {
    pub a: f32,
    pub b: f32,
    pub tx: f32,
    pub ty: f32,
    pub scale: f32,
    pub used: usize,
}

pub(crate) fn pose_from_corners(corners: &[Point2f; 4]) -> Option<Pose> {
    let mut source = Vector::<Point2f>::new();
    let mut target = Vector::<Point2f>::new();
    for index in 0..4 {
        source.push(Point2f::new(MARKER_UNIT[index][0], MARKER_UNIT[index][1]));
        target.push(corners[index]);
    }
    estimate_pose(&source, &target, 1)
}

pub(crate) fn estimate_pose(
    source: &Vector<Point2f>,
    target: &Vector<Point2f>,
    used: usize,
) -> Option<Pose> {
    estimate_pose_inliers(source, target, used).map(|(pose, _)| pose)
}

pub(crate) fn estimate_pose_inliers(
    source: &Vector<Point2f>,
    target: &Vector<Point2f>,
    used: usize,
) -> Option<(Pose, usize)> {
    let mut inliers = Mat::default();
    let matrix =
        estimate_affine_partial_2d(source, target, &mut inliers, RANSAC, 3.0, 2000, 0.99, 10)
            .ok()?;
    if matrix.rows() != 2 || matrix.cols() != 3 {
        return None;
    }
    let a = *matrix.at_2d::<f64>(0, 0).ok()? as f32;
    let b = *matrix.at_2d::<f64>(1, 0).ok()? as f32;
    let tx = *matrix.at_2d::<f64>(0, 2).ok()? as f32;
    let ty = *matrix.at_2d::<f64>(1, 2).ok()? as f32;
    let scale = (a * a + b * b).sqrt();
    if !a.is_finite()
        || !b.is_finite()
        || !tx.is_finite()
        || !ty.is_finite()
        || !scale.is_finite()
        || scale < 1e-4
    {
        return None;
    }
    let inlier_count = count_non_zero(&inliers).ok()? as usize;
    Some((
        Pose {
            a,
            b,
            tx,
            ty,
            scale,
            used,
        },
        inlier_count,
    ))
}

pub(crate) fn map_quad(corners: &[Point2f; 4], inverse: (f32, f32, f32, f32)) -> [[f32; 2]; 4] {
    let (a, b, tx, ty) = inverse;
    corners.map(|point| {
        [
            a * point.x + b * point.y + tx,
            -b * point.x + a * point.y + ty,
        ]
    })
}

pub(crate) fn inverse_similarity(pose: Pose) -> (f32, f32, f32, f32) {
    let divisor = (pose.a * pose.a + pose.b * pose.b).max(1e-12);
    let a = pose.a / divisor;
    let b = pose.b / divisor;
    let tx = -(a * pose.tx + b * pose.ty);
    let ty = -(-b * pose.tx + a * pose.ty);
    (a, b, tx, ty)
}

pub(crate) fn quad_area(corners: &[Point2f; 4]) -> f32 {
    let mut area = 0.0;
    for index in 0..4 {
        let a = corners[index];
        let b = corners[(index + 1) % 4];
        area += a.x * b.y - b.x * a.y;
    }
    area.abs() * 0.5
}
