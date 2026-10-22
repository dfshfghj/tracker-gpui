//! Search mask around the predicted rigid-body ROI.

use opencv::core::{self, Mat, Point, Scalar, Vector};
use opencv::imgproc;

use crate::aruco_math::Pose;

pub(crate) fn roi_mask(
    rect: core::Rect,
    pose: &Pose,
    width: i32,
    height: i32,
) -> Result<Mat, String> {
    let mut mask = Mat::new_rows_cols_with_default(height, width, core::CV_8UC1, Scalar::all(0.0))
        .map_err(|err| format!("创建搜索掩膜失败: {err}"))?;
    let corners = [
        (rect.x as f32, rect.y as f32),
        ((rect.x + rect.width) as f32, rect.y as f32),
        ((rect.x + rect.width) as f32, (rect.y + rect.height) as f32),
        (rect.x as f32, (rect.y + rect.height) as f32),
    ];
    let mut transformed = [(0.0f32, 0.0f32); 4];
    for (index, (x, y)) in corners.iter().enumerate() {
        transformed[index] = (
            pose.tx + pose.a * x - pose.b * y,
            pose.ty + pose.b * x + pose.a * y,
        );
    }
    let center = (
        transformed.iter().map(|point| point.0).sum::<f32>() / 4.0,
        transformed.iter().map(|point| point.1).sum::<f32>() / 4.0,
    );
    let points = transformed.map(|(x, y)| {
        Point::new(
            (center.0 + (x - center.0) * 1.3) as i32,
            (center.1 + (y - center.1) * 1.3) as i32,
        )
    });
    let polygon = Vector::<Point>::from_slice(&points);
    imgproc::fill_convex_poly(&mut mask, &polygon, Scalar::all(255.0), imgproc::LINE_8, 0)
        .map_err(|err| format!("绘制搜索掩膜失败: {err}"))?;
    Ok(mask)
}
