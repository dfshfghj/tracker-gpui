//! Classical moving-object detection: background subtraction plus frame
//! differencing, contour filtering, and nearest-neighbour track linking.

use crate::i18n::tr;
use crate::i18n::cv;
use std::path::{Path, PathBuf};

use opencv::core::{self, Mat, Point, Scalar, Size, Vector};
use opencv::prelude::*;
use opencv::video::BackgroundSubtractorTrait;
use opencv::{imgcodecs, imgproc, video};

use crate::video::{FrameData, FrameSource, VideoMeta};

const MAX_MISSED: usize = 12;
const MIN_POINTS: usize = 4;

#[derive(Clone, Copy, Debug)]
pub(crate) struct MotionPoint {
    pub frame: usize,
    pub x: f32,
    pub y: f32,
}

pub(crate) struct MotionResult {
    pub points: Vec<MotionPoint>,
    pub candidates: usize,
    pub score: f64,
    pub path_length: f64,
    pub mean_brightness: f64,
    pub preview: Option<PathBuf>,
}

struct Detection {
    x: f64,
    y: f64,
    area: f64,
    brightness: f64,
}

struct Candidate {
    id: usize,
    points: Vec<MotionPoint>,
    last_x: f64,
    last_y: f64,
    vx: f64,
    vy: f64,
    last_frame: usize,
    missed: usize,
    area_sum: f64,
    brightness_sum: f64,
}

struct Scored {
    id: usize,
    points: Vec<MotionPoint>,
    path_length: f64,
    mean_area: f64,
    mean_brightness: f64,
    score: f64,
}

pub(crate) fn detect_motion(
    source: &FrameSource,
    preview_path: Option<&Path>,
) -> Result<MotionResult, String> {
    let meta = source.meta().clone();
    if meta.frames == 0 {
        return Err(tr("msg.no_frames"));
    }
    let mut reader = source.reader()?;
    let scale = (960.0 / meta.width as f64).min(1.0);
    let proc_w = ((meta.width as f64 * scale).round() as i32).max(1);
    let proc_h = ((meta.height as f64 * scale).round() as i32).max(1);
    let mut subtractor = video::create_background_subtractor_mog2(30, 16.0, true)
        .map_err(|err| cv("create background model", err))?;
    let mut previous: Option<Mat> = None;
    let mut candidates: Vec<Candidate> = Vec::new();
    let mut next_id = 1usize;
    let trace = std::env::var_os("PROBE_TRACE").is_some();
    let mut detection_count = 0usize;

    for frame_index in 0..meta.frames {
        let frame = reader.read(frame_index)?;
        let mut gray = gray_from_frame(&frame)?;
        if scale < 0.999 {
            let mut small = Mat::default();
            imgproc::resize(
                &gray,
                &mut small,
                Size::new(proc_w, proc_h),
                0.0,
                0.0,
                imgproc::INTER_AREA,
            )
            .map_err(|err| cv("scale frame", err))?;
            gray = small;
        }

        let mut mask = Mat::default();
        BackgroundSubtractorTrait::apply(&mut subtractor, &gray, &mut mask, 0.05)
            .map_err(|err| cv("background subtraction", err))?;
        let mut binary = Mat::default();
        imgproc::threshold(&mask, &mut binary, 200.0, 255.0, imgproc::THRESH_BINARY)
            .map_err(|err| cv("threshold", err))?;

        if let Some(previous) = previous.as_ref() {
            let mut diff = Mat::default();
            core::absdiff(&gray, previous, &mut diff).map_err(|err| cv("frame difference", err))?;
            let mut diff_mask = Mat::default();
            imgproc::threshold(&diff, &mut diff_mask, 18.0, 255.0, imgproc::THRESH_BINARY)
                .map_err(|err| cv("frame-diff threshold", err))?;
            let mut combined = Mat::default();
            core::bitwise_or(&binary, &diff_mask, &mut combined, &core::no_array())
                .map_err(|err| cv("merge masks", err))?;
            binary = combined;
        }
        previous = Some(gray.clone());

        let mut cleaned = Mat::default();
        let close = imgproc::get_structuring_element(
            imgproc::MORPH_ELLIPSE,
            Size::new(5, 5),
            Point::new(-1, -1),
        )
        .map_err(|err| cv("morphology kernel", err))?;
        imgproc::morphology_ex(
            &binary,
            &mut cleaned,
            imgproc::MORPH_CLOSE,
            &close,
            Point::new(-1, -1),
            2,
            core::BORDER_CONSTANT,
            Scalar::all(0.0),
        )
        .map_err(|err| cv("close", err))?;
        let open = imgproc::get_structuring_element(
            imgproc::MORPH_ELLIPSE,
            Size::new(3, 3),
            Point::new(-1, -1),
        )
        .map_err(|err| cv("morphology kernel", err))?;
        let mut filtered = Mat::default();
        imgproc::morphology_ex(
            &cleaned,
            &mut filtered,
            imgproc::MORPH_OPEN,
            &open,
            Point::new(-1, -1),
            1,
            core::BORDER_CONSTANT,
            Scalar::all(0.0),
        )
        .map_err(|err| cv("open", err))?;

        let mut contours = Vector::<Mat>::new();
        imgproc::find_contours(
            &filtered,
            &mut contours,
            imgproc::RETR_EXTERNAL,
            imgproc::CHAIN_APPROX_SIMPLE,
            Point::new(0, 0),
        )
        .map_err(|err| cv("find contours", err))?;

        let frame_area = (proc_w * proc_h) as f64;
        let mut detections = Vec::new();
        for contour in contours.iter() {
            let area = imgproc::contour_area(&contour, false)
                .map_err(|err| cv("contour area", err))?;
            if area < 6.0 || area > frame_area * 0.08 {
                continue;
            }
            let rect =
                imgproc::bounding_rect(&contour).map_err(|err| cv("contour bounding rect", err))?;
            if rect.width < 2 || rect.height < 2 {
                continue;
            }
            let fill = area / (rect.width as f64 * rect.height as f64).max(1.0);
            if fill < 0.12 {
                continue;
            }
            let aspect = rect.width as f64 / rect.height as f64;
            if !(0.15..=6.5).contains(&aspect) {
                continue;
            }
            let moments =
                imgproc::moments(&contour, false).map_err(|err| cv("contour moments", err))?;
            let (cx, cy) = if moments.m00 > 1e-6 {
                (moments.m10 / moments.m00, moments.m01 / moments.m00)
            } else {
                (
                    rect.x as f64 + rect.width as f64 * 0.5,
                    rect.y as f64 + rect.height as f64 * 0.5,
                )
            };
            detections.push(Detection {
                x: cx / scale,
                y: cy / scale,
                area: area / (scale * scale),
                brightness: rect_mean(&gray, rect),
            });
        }
        detection_count += detections.len();

        let gate = (meta.width as f64 * 0.18).clamp(24.0, 260.0);
        let mut claimed = vec![false; detections.len()];
        let mut matched_tracks = vec![false; candidates.len()];
        let mut pairs = Vec::new();
        for (track_index, candidate) in candidates.iter().enumerate() {
            if candidate.missed > MAX_MISSED {
                continue;
            }
            let dt = frame_index.saturating_sub(candidate.last_frame).max(1) as f64;
            let predicted_x = candidate.last_x + candidate.vx * dt;
            let predicted_y = candidate.last_y + candidate.vy * dt;
            for (detection_index, detection) in detections.iter().enumerate() {
                let dx = detection.x - predicted_x;
                let dy = detection.y - predicted_y;
                let distance = (dx * dx + dy * dy).sqrt();
                if distance <= gate {
                    pairs.push((distance, track_index, detection_index));
                }
            }
        }
        pairs.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (_, track_index, detection_index) in pairs {
            if matched_tracks[track_index] || claimed[detection_index] {
                continue;
            }
            let detection = &detections[detection_index];
            let candidate = &mut candidates[track_index];
            let dt = frame_index.saturating_sub(candidate.last_frame).max(1) as f64;
            let vx = (detection.x - candidate.last_x) / dt;
            let vy = (detection.y - candidate.last_y) / dt;
            candidate.vx = candidate.vx * 0.6 + vx * 0.4;
            candidate.vy = candidate.vy * 0.6 + vy * 0.4;
            candidate.last_x = detection.x;
            candidate.last_y = detection.y;
            candidate.last_frame = frame_index;
            candidate.missed = 0;
            candidate.area_sum += detection.area;
            candidate.brightness_sum += detection.brightness;
            candidate.points.push(MotionPoint {
                frame: frame_index,
                x: detection.x as f32,
                y: detection.y as f32,
            });
            matched_tracks[track_index] = true;
            claimed[detection_index] = true;
        }
        for (track_index, candidate) in candidates.iter_mut().enumerate() {
            if !matched_tracks[track_index] && candidate.missed <= MAX_MISSED {
                candidate.missed += 1;
            }
        }
        for (detection_index, detection) in detections.iter().enumerate() {
            if claimed[detection_index] {
                continue;
            }
            let id = next_id;
            next_id += 1;
            candidates.push(Candidate {
                id,
                points: vec![MotionPoint {
                    frame: frame_index,
                    x: detection.x as f32,
                    y: detection.y as f32,
                }],
                last_x: detection.x,
                last_y: detection.y,
                vx: 0.0,
                vy: 0.0,
                last_frame: frame_index,
                missed: 0,
                area_sum: detection.area,
                brightness_sum: detection.brightness,
            });
        }
    }

    let mut scored: Vec<Scored> = candidates
        .into_iter()
        .filter(|candidate| candidate.points.len() >= MIN_POINTS)
        .map(|candidate| score_candidate(candidate, &meta))
        .collect();
    scored.sort_by(|a, b| b.score.total_cmp(&a.score));
    if trace {
        for candidate in &scored {
            eprintln!(
                "[autodetect] candidate {} points={} path={:.1} area={:.1} bright={:.1} score={:.1}",
                candidate.id,
                candidate.points.len(),
                candidate.path_length,
                candidate.mean_area,
                candidate.mean_brightness,
                candidate.score
            );
        }
    }
    let candidate_count = scored.len();
    if std::env::var_os("PROBE_TRACE").is_some() {
        eprintln!(
            "[autodetect] {} frames, {} detections, {} candidates",
            meta.frames, detection_count, candidate_count
        );
    }
    let best = scored
        .into_iter()
        .next()
        .ok_or_else(|| tr("msg.no_stable_target"))?;

    let preview = if let Some(path) = preview_path {
        save_preview(&mut reader, &best.points, path)?;
        Some(path.to_path_buf())
    } else {
        None
    };

    Ok(MotionResult {
        candidates: candidate_count,
        points: best.points,
        score: best.score,
        path_length: best.path_length,
        mean_brightness: best.mean_brightness,
        preview,
    })
}

fn score_candidate(candidate: Candidate, meta: &VideoMeta) -> Scored {
    let count = candidate.points.len() as f64;
    let path_length: f64 = candidate
        .points
        .windows(2)
        .map(|pair| {
            let dx = (pair[1].x - pair[0].x) as f64;
            let dy = (pair[1].y - pair[0].y) as f64;
            (dx * dx + dy * dy).sqrt()
        })
        .sum();
    let mean_area = candidate.area_sum / count;
    let mean_brightness = candidate.brightness_sum / count;
    let center_x = meta.width as f64 * 0.5;
    let center_y = meta.height as f64 * 0.5;
    let radius =
        ((meta.width as f64 * meta.width as f64 + meta.height as f64 * meta.height as f64).sqrt())
            * 0.5;
    let mean_distance = candidate
        .points
        .iter()
        .map(|point| {
            let dx = point.x as f64 - center_x;
            let dy = point.y as f64 - center_y;
            (dx * dx + dy * dy).sqrt()
        })
        .sum::<f64>()
        / count
        / radius.max(1.0);
    let center_factor = (1.0 - mean_distance * 0.65).clamp(0.25, 1.0);
    let area_factor = if mean_area > 0.0 {
        ((meta.width as f64 * meta.height as f64 * 0.0008 / mean_area).sqrt()).clamp(0.5, 1.6)
    } else {
        1.0
    };
    let score = path_length * (0.3 + mean_brightness / 255.0) * center_factor * area_factor;
    Scored {
        id: candidate.id,
        points: candidate.points,
        path_length,
        mean_area,
        mean_brightness,
        score,
    }
}

fn gray_from_frame(data: &FrameData) -> Result<Mat, String> {
    let bgra = Mat::from_slice(&data.bgra).map_err(|err| cv("BGRA Mat", err))?;
    let bgra = bgra
        .reshape(4, data.height as i32)
        .map_err(|err| cv("BGRA reshape", err))?;
    let mut gray = Mat::default();
    imgproc::cvt_color_def(&bgra, &mut gray, imgproc::COLOR_BGRA2GRAY)
        .map_err(|err| cv("convert to gray", err))?;
    Ok(gray)
}

fn bgr_from_frame(data: &FrameData) -> Result<Mat, String> {
    let bgra = Mat::from_slice(&data.bgra).map_err(|err| cv("BGRA Mat", err))?;
    let bgra = bgra
        .reshape(4, data.height as i32)
        .map_err(|err| cv("BGRA reshape", err))?;
    let mut bgr = Mat::default();
    imgproc::cvt_color_def(&bgra, &mut bgr, imgproc::COLOR_BGRA2BGR)
        .map_err(|err| cv("convert to BGR", err))?;
    Ok(bgr)
}

fn rect_mean(image: &Mat, rect: core::Rect) -> f64 {
    if image.rows() == 0 || image.cols() == 0 {
        return 0.0;
    }
    let x = rect.x.clamp(0, image.cols() - 1);
    let y = rect.y.clamp(0, image.rows() - 1);
    let width = rect.width.clamp(1, image.cols() - x);
    let height = rect.height.clamp(1, image.rows() - y);
    let Ok(roi) = image.roi(core::Rect::new(x, y, width, height)) else {
        return 0.0;
    };
    let no = core::no_array();
    core::mean(&roi, &no).map(|mean| mean[0]).unwrap_or(0.0)
}

fn save_preview(
    reader: &mut crate::video::FrameReader,
    points: &[MotionPoint],
    path: &Path,
) -> Result<(), String> {
    let Some(anchor) = points.get(points.len() / 2).copied() else {
        return Ok(());
    };
    let frame = reader.read(anchor.frame)?;
    let mut image = bgr_from_frame(&frame)?;
    let green = Scalar::new(0.0, 220.0, 0.0, 0.0);
    let red = Scalar::new(0.0, 0.0, 255.0, 0.0);
    for pair in points.windows(2) {
        let a = Point::new(pair[0].x as i32, pair[0].y as i32);
        let b = Point::new(pair[1].x as i32, pair[1].y as i32);
        imgproc::line(&mut image, a, b, green, 1, imgproc::LINE_AA, 0)
            .map_err(|err| cv("draw trail", err))?;
    }
    for point in points {
        imgproc::circle(
            &mut image,
            Point::new(point.x as i32, point.y as i32),
            3,
            green,
            1,
            imgproc::LINE_AA,
            0,
        )
        .map_err(|err| cv("draw detections", err))?;
    }
    imgproc::circle(
        &mut image,
        Point::new(anchor.x as i32, anchor.y as i32),
        8,
        red,
        2,
        imgproc::LINE_AA,
        0,
    )
    .map_err(|err| cv("draw target", err))?;
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).ok();
        }
    }
    imgcodecs::imwrite(path, &image, &Vector::<i32>::new())
        .map_err(|err| cv("save preview", err))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn links_opencv() {
        use opencv::prelude::MatTraitConst;
        assert_eq!(opencv::core::Mat::default().rows(), 0);
    }
}
