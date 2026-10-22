//! Small geometry helpers for silhouette tracking.

use opencv::core;

pub(crate) fn search_rect(center: (f32, f32), scale: f32, width: u32, height: u32) -> core::Rect {
    let half = (scale * 1.5 + 60.0).round() as i32;
    let x = (center.0.round() as i32 - half).max(0);
    let y = (center.1.round() as i32 - half).max(0);
    let right = (center.0.round() as i32 + half).min(width as i32);
    let bottom = (center.1.round() as i32 + half).min(height as i32);
    core::Rect::new(x, y, (right - x).max(16), (bottom - y).max(16))
}

pub(crate) fn nearest_axis(candidate: f32, reference: f32) -> f32 {
    let mut angle = candidate;
    while angle - reference > std::f32::consts::FRAC_PI_2 {
        angle -= std::f32::consts::PI;
    }
    while angle - reference < -std::f32::consts::FRAC_PI_2 {
        angle += std::f32::consts::PI;
    }
    angle
}

pub(crate) fn unwrap_angle(angle: f32, previous: Option<f32>) -> f32 {
    let Some(previous) = previous else {
        return angle;
    };
    let mut angle = angle;
    while angle - previous > std::f32::consts::PI {
        angle -= std::f32::consts::TAU;
    }
    while angle - previous < -std::f32::consts::PI {
        angle += std::f32::consts::TAU;
    }
    angle
}

pub(crate) fn distance(a: (f32, f32), b: (f32, f32)) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

pub(crate) fn normalized_rect(
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
        return Err("框选区域太小".into());
    }
    Ok(core::Rect::new(x0 as i32, y0 as i32, w, h))
}
