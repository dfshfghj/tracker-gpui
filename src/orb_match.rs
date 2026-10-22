//! ORB reference extraction and masked frame-to-reference matching.

use opencv::core::{self, DMatch, KeyPoint, KeyPointTraitConst, Mat, Point2f, Vector};
use opencv::features2d::{BFMatcher, Feature2DTrait, ORB};
use opencv::prelude::*;

pub(crate) struct ReferenceFeatures {
    pub keypoints: Vector<KeyPoint>,
    pub descriptors: Mat,
    pub origin: (f32, f32),
}

pub(crate) fn reference_features(
    gray: &Mat,
    rect: core::Rect,
) -> Result<ReferenceFeatures, String> {
    let roi = gray
        .roi(rect)
        .map_err(|err| format!("ROI 裁剪失败: {err}"))?;
    let mut orb = ORB::create_def().map_err(|err| format!("创建 ORB 失败: {err}"))?;
    let mut keypoints = Vector::<KeyPoint>::new();
    let mut descriptors = Mat::default();
    orb.detect_and_compute_def(&roi, &core::no_array(), &mut keypoints, &mut descriptors)
        .map_err(|err| format!("ROI 特征提取失败: {err}"))?;
    Ok(ReferenceFeatures {
        keypoints,
        descriptors,
        origin: (rect.x as f32, rect.y as f32),
    })
}

pub(crate) fn match_frame(
    reference: &ReferenceFeatures,
    gray: &Mat,
    mask: &Mat,
    matcher: &mut BFMatcher,
) -> Result<(Vector<Point2f>, Vector<Point2f>), String> {
    let mut orb = ORB::create_def().map_err(|err| format!("创建 ORB 失败: {err}"))?;
    let mut keypoints = Vector::<KeyPoint>::new();
    let mut descriptors = Mat::default();
    orb.detect_and_compute_def(gray, mask, &mut keypoints, &mut descriptors)
        .map_err(|err| format!("提取帧特征失败: {err}"))?;
    let mut source = Vector::<Point2f>::new();
    let mut target = Vector::<Point2f>::new();
    if keypoints.len() < 8 || descriptors.empty() {
        return Ok((source, target));
    }
    let mut matches = Vector::<Vector<DMatch>>::new();
    matcher
        .knn_train_match_def(&reference.descriptors, &descriptors, &mut matches, 2)
        .map_err(|err| format!("特征匹配失败: {err}"))?;
    for pair in matches.iter() {
        let mut iter = pair.iter();
        let (Some(first), Some(second)) = (iter.next(), iter.next()) else {
            continue;
        };
        if first.distance >= second.distance * 0.75 {
            continue;
        }
        let Ok(reference_keypoint) = reference.keypoints.get(first.query_idx as usize) else {
            continue;
        };
        let Ok(keypoint) = keypoints.get(second.train_idx as usize) else {
            continue;
        };
        let point = reference_keypoint.pt();
        source.push(Point2f::new(
            point.x + reference.origin.0,
            point.y + reference.origin.1,
        ));
        target.push(keypoint.pt());
    }
    Ok((source, target))
}
