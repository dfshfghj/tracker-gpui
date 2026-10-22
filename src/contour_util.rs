//! Contour pose extraction and template matching for the silhouette tracker.

use opencv::core::{self, Mat, Point, Point2f, Scalar, Vector};
use opencv::imgproc;
use opencv::prelude::*;

#[derive(Clone)]
pub(crate) struct ContourInfo {
    pub points: Vec<Point2f>,
    pub center: (f32, f32),
    pub angle: f32,
    pub area: f32,
    pub bbox: core::Rect,
}

pub(crate) fn find_contours_in(
    mask: &Mat,
    rect: core::Rect,
) -> Result<Vec<ContourInfo>, String> {
    if rect.width < 4 || rect.height < 4 {
        return Ok(Vec::new());
    }
    let crop = mask
        .roi(rect)
        .map_err(|err| format!("轮廓 ROI 裁剪失败: {err}"))?;
    let mut contours = Vector::<Mat>::new();
    imgproc::find_contours(
        &crop,
        &mut contours,
        imgproc::RETR_EXTERNAL,
        imgproc::CHAIN_APPROX_SIMPLE,
        Point::new(0, 0),
    )
    .map_err(|err| format!("查找轮廓失败: {err}"))?;
    let max_area = rect.width as f64 * rect.height as f64 * 0.9;
    let mut result = Vec::new();
    for contour in contours.iter() {
        let area = imgproc::contour_area(&contour, false)
            .map_err(|err| format!("轮廓面积失败: {err}"))?;
        if area < 20.0 || area > max_area {
            continue;
        }
        let moments = imgproc::moments(&contour, false)
            .map_err(|err| format!("轮廓矩失败: {err}"))?;
        if moments.m00 <= 1e-6 {
            continue;
        }
        let center = (
            rect.x as f32 + (moments.m10 / moments.m00) as f32,
            rect.y as f32 + (moments.m01 / moments.m00) as f32,
        );
        let angle = 0.5 * (2.0 * moments.mu11).atan2(moments.mu20 - moments.mu02) as f32;
        let contour_points = contour
            .data_typed::<Point>()
            .map_err(|err| format!("轮廓点读取失败: {err}"))?;
        let points = contour_points
            .iter()
            .map(|point| {
                Point2f::new(
                    rect.x as f32 + point.x as f32,
                    rect.y as f32 + point.y as f32,
                )
            })
            .collect();
        let local_bbox = imgproc::bounding_rect(&contour)
            .map_err(|err| format!("轮廓包围盒失败: {err}"))?;
        let bbox = core::Rect::new(
            local_bbox.x + rect.x,
            local_bbox.y + rect.y,
            local_bbox.width,
            local_bbox.height,
        );
        result.push(ContourInfo {
            points,
            center,
            angle,
            area: area as f32,
            bbox,
        });
    }
    Ok(result)
}

pub(crate) struct Template {
    pub patch: Mat,
    pub mask: Mat,
    pub center_offset: Point2f,
}

pub(crate) fn make_template(gray: &Mat, info: &ContourInfo) -> Result<Template, String> {
    let margin = 2;
    let x = (info.bbox.x - margin).max(0);
    let y = (info.bbox.y - margin).max(0);
    let right = (info.bbox.x + info.bbox.width + margin).min(gray.cols());
    let bottom = (info.bbox.y + info.bbox.height + margin).min(gray.rows());
    let bbox = core::Rect::new(x, y, (right - x).max(2), (bottom - y).max(2));
    let patch = gray
        .roi(bbox)
        .map_err(|err| format!("模板裁剪失败: {err}"))?
        .clone_pointee();
    let mut mask =
        Mat::new_rows_cols_with_default(bbox.height, bbox.width, core::CV_8UC1, Scalar::all(0.0))
            .map_err(|err| format!("模板掩膜失败: {err}"))?;
    let points: Vector<Point> = info
        .points
        .iter()
        .map(|point| Point::new(point.x as i32 - bbox.x, point.y as i32 - bbox.y))
        .collect();
    let polygons = Vector::<Vector<Point>>::from_iter([points]);
    imgproc::fill_poly(
        &mut mask,
        &polygons,
        Scalar::all(255.0),
        imgproc::LINE_8,
        0,
        Point::new(0, 0),
    )
    .map_err(|err| format!("填充模板失败: {err}"))?;
    Ok(Template {
        patch,
        mask,
        center_offset: Point2f::new(info.center.0 - bbox.x as f32, info.center.1 - bbox.y as f32),
    })
}

pub(crate) fn match_template_in(
    gray: &Mat,
    search: core::Rect,
    template: &Template,
) -> Result<Option<((f32, f32), f32)>, String> {
    if template.patch.cols() >= search.width || template.patch.rows() >= search.height {
        return Ok(None);
    }
    let image = gray
        .roi(search)
        .map_err(|err| format!("模板搜索裁剪失败: {err}"))?;
    let mut result = Mat::default();
    imgproc::match_template(
        &image,
        &template.patch,
        &mut result,
        imgproc::TM_CCORR_NORMED,
        &template.mask,
    )
    .map_err(|err| format!("模板匹配失败: {err}"))?;
    let mut max_val = 0.0;
    let mut max_loc = Point::new(0, 0);
    core::min_max_loc(
        &result,
        None,
        Some(&mut max_val),
        None,
        Some(&mut max_loc),
        &core::no_array(),
    )
    .map_err(|err| format!("模板峰值失败: {err}"))?;
    if !max_val.is_finite() || max_val < 0.45 {
        return Ok(None);
    }
    Ok(Some((
        (
            search.x as f32 + max_loc.x as f32 + template.center_offset.x,
            search.y as f32 + max_loc.y as f32 + template.center_offset.y,
        ),
        max_val as f32,
    )))
}
