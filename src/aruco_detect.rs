//! ArUco marker detection helpers.

use opencv::core::{Mat, Point2f, Vector};
use opencv::imgproc;
use opencv::objdetect::{
    get_predefined_dictionary, ArucoDetector, ArucoDetectorTraitConst, DetectorParameters,
    PredefinedDictionaryType, RefineParameters,
};
use opencv::prelude::*;

use crate::video::FrameData;

pub(crate) struct Detection {
    pub id: u32,
    pub corners: [Point2f; 4],
}

pub(crate) fn new_detector() -> Result<ArucoDetector, String> {
    let dictionary = get_predefined_dictionary(PredefinedDictionaryType::DICT_4X4_50)
        .map_err(|err| format!("加载 ArUco 字典失败: {err}"))?;
    let parameters =
        DetectorParameters::default().map_err(|err| format!("ArUco 参数失败: {err}"))?;
    let refine = RefineParameters::new(10.0, 3.0, true)
        .map_err(|err| format!("ArUco refine 参数失败: {err}"))?;
    ArucoDetector::new(&dictionary, &parameters, refine)
        .map_err(|err| format!("创建 ArUco 检测器失败: {err}"))
}

pub(crate) fn detect_frame(detector: &ArucoDetector, gray: &Mat) -> Result<Vec<Detection>, String> {
    let mut corners = Vector::<Vector<Point2f>>::new();
    let mut ids = Vector::<i32>::new();
    let mut rejected = Vector::<Vector<Point2f>>::new();
    detector
        .detect_markers(gray, &mut corners, &mut ids, &mut rejected)
        .map_err(|err| format!("ArUco 检测失败: {err}"))?;
    let mut out = Vec::new();
    for (id, quad) in ids.iter().zip(corners.iter()) {
        if quad.len() != 4 {
            continue;
        }
        let mut points = [Point2f::new(0.0, 0.0); 4];
        for (index, point) in quad.iter().enumerate() {
            points[index] = point;
        }
        out.push(Detection {
            id: u32::try_from(id).unwrap_or(0),
            corners: points,
        });
    }
    Ok(out)
}

pub(crate) fn gray_from_frame(data: &FrameData) -> Result<Mat, String> {
    let bgra = Mat::from_slice(&data.bgra).map_err(|err| format!("BGRA Mat 失败: {err}"))?;
    let bgra = bgra
        .reshape(4, data.height as i32)
        .map_err(|err| format!("BGRA reshape 失败: {err}"))?;
    let mut gray = Mat::default();
    imgproc::cvt_color_def(&bgra, &mut gray, imgproc::COLOR_BGRA2GRAY)
        .map_err(|err| format!("转灰度失败: {err}"))?;
    Ok(gray)
}
