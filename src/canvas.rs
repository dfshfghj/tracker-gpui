use gpui_kit::*;

use crate::track::Footprint;
use crate::*;

pub(crate) fn stroke_path(points: &[(f32, f32)], width: f32) -> Option<Path<Pixels>> {
    if points.len() < 2 {
        return None;
    }
    let mut b = PathBuilder::stroke(px(width));
    for (i, (x, y)) in points.iter().enumerate() {
        let p = point(px(*x), px(*y));
        if i == 0 {
            b.move_to(p);
        } else {
            b.line_to(p);
        }
    }
    b.build().ok()
}

pub(crate) fn circle_path(
    cx: f32,
    cy: f32,
    r: f32,
    fill: bool,
    width: f32,
) -> Option<Path<Pixels>> {
    let mut b = if fill {
        PathBuilder::fill()
    } else {
        PathBuilder::stroke(px(width))
    };
    let steps = 32;
    for i in 0..=steps {
        let a = i as f32 * std::f32::consts::TAU / steps as f32;
        let p = point(px(cx + r * a.cos()), px(cy + r * a.sin()));
        if i == 0 {
            b.move_to(p);
        } else {
            b.line_to(p);
        }
    }
    b.build().ok()
}

pub(crate) fn rect_path(
    cx: f32,
    cy: f32,
    size: f32,
    fill: bool,
    width: f32,
) -> Option<Path<Pixels>> {
    let h = size / 2.0;
    let pts = [
        (cx - h, cy - h),
        (cx + h, cy - h),
        (cx + h, cy + h),
        (cx - h, cy + h),
        (cx - h, cy - h),
    ];
    let mut b = if fill {
        PathBuilder::fill()
    } else {
        PathBuilder::stroke(px(width))
    };
    for (i, (x, y)) in pts.iter().enumerate() {
        let p = point(px(*x), px(*y));
        if i == 0 {
            b.move_to(p);
        } else {
            b.line_to(p);
        }
    }
    b.build().ok()
}

pub(crate) fn draw_scene(st: &State, window: &mut Window, cx: &mut App) -> usize {
    let ox = st.sx;
    let oy = st.sy;
    let s = st.scale;

    // stage background
    window.paint_quad(quad(
        Bounds {
            origin: point(px(ox), px(oy)),
            size: size(px(st.sw), px(st.sh)),
        },
        px(0.),
        c(0x070b12),
        px(0.),
        transparent_black(),
        Default::default(),
    ));

    // image area
    let ix = ox + st.tx;
    let iy = oy + st.ty;
    let iw = st.img_w;
    let ih = st.img_h;
    window.paint_quad(quad(
        Bounds {
            origin: point(px(ix), px(iy)),
            size: size(px(iw * s), px(ih * s)),
        },
        px(0.),
        c(0x0b1220),
        px(0.),
        transparent_black(),
        Default::default(),
    ));

    if let Some((_, image)) = st.frame_image.as_ref() {
        use std::sync::atomic::{AtomicBool, Ordering};
        static TRACED: AtomicBool = AtomicBool::new(false);
        let dest = Bounds {
            origin: point(px(ix), px(iy)),
            size: size(px(iw * s), px(ih * s)),
        };
        let result = window.paint_image(dest, dest, px(0.).into(), image.clone(), 0, false);
        if !TRACED.swap(true, Ordering::Relaxed) || result.is_err() {
            eprintln!("paint_image result: {result:?}");
        }
    }

    if st.axes_visible {
        draw_axes(st, window, cx, ox, oy);
    }

    // tracks: show a long trail by default, only the current step while marking (Shift)
    let mut drawn = 0usize;
    let current_frame = st.frame;
    for (track_index, track) in st.tracks.iter().enumerate() {
        if !track.visible {
            continue;
        }
        if track.measurement_kind().is_some() {
            if let Some(step) = track.measurement_step(st.frame) {
                draw_measurement(st, window, cx, track, ox, oy, track_index == st.active);
                drawn += step.handles.len();
            }
            continue;
        }
        if track.step_count() == 0 {
            continue;
        }
        let points: Vec<(usize, f32, f32)> = track.points().collect();
        let active = track_index == st.active;
        let color = rgb(track.color);
        if st.trace_visible && points.len() > 1 {
            let screen: Vec<(f32, f32)> = points
                .iter()
                .map(|(_, x, y)| {
                    let (x, y) = st.to_screen(*x, *y);
                    (ox + x, oy + y)
                })
                .collect();
            if let Some(path) = stroke_path(&screen, if active { 1.5 } else { 1.1 }) {
                window.paint_path(path, color.alpha(if active { 0.9 } else { 0.6 }));
            }
        }
        let marking = st.shift_down && active;
        let visible: Vec<(usize, f32, f32)> = if marking || !st.trail_visible {
            points
                .iter()
                .filter(|(frame, _, _)| *frame == current_frame)
                .copied()
                .collect()
        } else {
            let start_frame = if st.trail_length == 0 {
                0
            } else {
                current_frame.saturating_sub(st.trail_length.saturating_sub(1))
            };
            points
                .iter()
                .filter(|(frame, _, _)| *frame <= current_frame && *frame >= start_frame)
                .copied()
                .collect()
        };
        let highlight_suppressed = st.tracker_open
            && st
                .tracker_overlay
                .as_ref()
                .map(|overlay| overlay.track_id == track.id)
                .unwrap_or(false);
        for (frame, px, py) in &visible {
            let (sx, sy) = st.to_screen(*px, *py);
            let x = ox + sx;
            let y = oy + sy;
            draw_footprint(window, track.footprint, x, y, color, active);
            if st.labels_visible {
                plot_text(
                    window,
                    cx,
                    &format!("{frame}"),
                    x - 14.0,
                    y - 17.0,
                    10.0,
                    color,
                );
            }
            if *frame == current_frame && !highlight_suppressed {
                if let Some(path) = circle_path(x, y, 6.0, false, 2.0) {
                    window.paint_path(path, color);
                }
            }
        }
        if track.is_rigid_body() {
            draw_rigid_body(st, window, track, &visible, ox, oy, active);
        }
        drawn += visible.len();
    }

    // calibration sticks (all share the coordinate scale, each has its own length)
    for stick in 0..st.calibration.stick_count() {
        if !st.calibration.stick_visible(stick) {
            continue;
        }
        let Some((a, b, length)) = st.calibration.stick(st.frame, stick) else {
            continue;
        };
        let active = stick == st.active_stick;
        let color = if active { c(0xffd166) } else { c(0x22b8cf) };
        draw_ruler(
            &**st,
            window,
            cx,
            (a[0] as f32, a[1] as f32),
            (b[0] as f32, b[1] as f32),
            length,
            &st.calibration.unit,
            st.calibration.decimals,
            color,
            active,
        );
    }
    if matches!(st.tool, Tool::Origin | Tool::OriginOffset) && !st.axes_visible {
        draw_origin_marker(st, window, ox, oy);
    }

    draw_tracking_overlay(st, window, cx, ox, oy);

    if let Some(Drag::RigidRoi { start, current, .. }) = &st.drag {
        let (x0, y0) = st.to_screen(start.0.min(current.0), start.1.min(current.1));
        let (x1, y1) = st.to_screen(start.0.max(current.0), start.1.max(current.1));
        window.paint_quad(quad(
            Bounds {
                origin: point(px(ox + x0), px(oy + y0)),
                size: size(px((x1 - x0).max(1.0)), px((y1 - y0).max(1.0))),
            },
            px(0.),
            c(0xffd166).alpha(0.12),
            px(1.5),
            c(0xffd166),
            Default::default(),
        ));
    }

    // image border
    window.paint_quad(quad(
        Bounds {
            origin: point(px(ix), px(iy)),
            size: size(px(iw * s), px(ih * s)),
        },
        px(0.),
        transparent_black(),
        px(1.),
        c(0x78a0dc).alpha(0.28),
        Default::default(),
    ));

    drawn
}

fn draw_rigid_body(
    st: &State,
    window: &mut Window,
    track: &Track,
    visible: &[(usize, f32, f32)],
    ox: f32,
    oy: f32,
    active: bool,
) {
    let color = rgb(track.color);
    for (frame, px, py) in visible {
        let Some(step) = track.rigid_step(*frame) else {
            continue;
        };
        let (sx, sy) = st.to_screen(*px, *py);
        let x = ox + sx;
        let y = oy + sy;
        let side = (step.scale * st.scale).max(8.0);
        let half = side * 0.5;
        let angle = step.angle;
        let (sin, cos) = angle.sin_cos();
        let current = *frame == st.frame;
        let width = if active || current { 1.6 } else { 1.0 };
        if current {
            let contour = step
                .contour
                .as_ref()
                .filter(|contour| contour.len() > 2)
                .or_else(|| {
                    track
                        .rigid
                        .as_ref()
                        .map(|rigid| &rigid.contour)
                        .filter(|contour| contour.len() > 2)
                });
            let outline = contour
                .map(|contour| {
                    let mut points: Vec<(f32, f32)> = contour
                        .iter()
                        .map(|[lx, ly]| {
                            let (px, py) = (lx * side, ly * side);
                            (x + cos * px - sin * py, y + sin * px + cos * py)
                        })
                        .collect();
                    points.push(points[0]);
                    points
                })
                .unwrap_or_else(|| {
                    [
                        (-half, -half),
                        (half, -half),
                        (half, half),
                        (-half, half),
                        (-half, -half),
                    ]
                    .map(|(lx, ly)| (x + cos * lx - sin * ly, y + sin * lx + cos * ly))
                    .to_vec()
                });
            if let Some(path) = stroke_path(&outline, width) {
                window.paint_path(path, color.alpha(if active { 0.95 } else { 0.55 }));
            }
        }
        let editable = active && current && !track.locked;
        let axis_end = if editable {
            rigid_rotation_handle((x, y), angle, side)
        } else {
            (x + cos * side * 0.8, y + sin * side * 0.8)
        };
        if let Some(path) = stroke_path(&[(x, y), axis_end], width) {
            window.paint_path(path, color.alpha(if active { 0.95 } else { 0.55 }));
        }
        if editable {
            if let Some(path) = circle_path(axis_end.0, axis_end.1, 6.0, false, 2.0) {
                window.paint_path(path, c(0xffd166));
            }
            if let Some(path) = circle_path(axis_end.0, axis_end.1, 1.8, true, 1.0) {
                window.paint_path(path, c(0xffd166));
            }
        }
    }
}

pub(crate) fn measurement_summary(
    track: &Track,
    frame: usize,
    coords: &ImageCoordSystem,
    unit: &str,
    decimals: usize,
) -> Option<Vec<(&'static str, String)>> {
    let kind = track.measurement_kind()?;
    let handles = track.handles(frame);
    let mut rows = Vec::new();
    match kind {
        MeasurementKind::Tape => {
            let (pixel_length, _) = track.tape_values(frame)?;
            if handles.len() < 2 {
                return None;
            }
            let a = coords.image_to_world(frame, handles[0].0, handles[0].1);
            let b = coords.image_to_world(frame, handles[1].0, handles[1].1);
            let length = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
            let angle = (b.1 - a.1).atan2(b.0 - a.0).to_degrees();
            rows.push((
                "长度",
                format!("{:.*} {} ({pixel_length:.0} px)", decimals, length, unit),
            ));
            rows.push(("角度", format!("{angle:.1}°")));
        }
        MeasurementKind::Protractor => {
            if handles.len() < 3 {
                return None;
            }
            track.protractor_values(frame)?;
            let vertex = coords.image_to_world(frame, handles[0].0, handles[0].1);
            let arm1 = coords.image_to_world(frame, handles[1].0, handles[1].1);
            let arm2 = coords.image_to_world(frame, handles[2].0, handles[2].1);
            let v1 = (arm1.0 - vertex.0, arm1.1 - vertex.1);
            let v2 = (arm2.0 - vertex.0, arm2.1 - vertex.1);
            let length1 = (v1.0 * v1.0 + v1.1 * v1.1).sqrt();
            let length2 = (v2.0 * v2.0 + v2.1 * v2.1).sqrt();
            if length1 < 1e-6 || length2 < 1e-6 {
                return None;
            }
            let angle = ((v1.0 * v2.0 + v1.1 * v2.1) / (length1 * length2))
                .clamp(-1.0, 1.0)
                .acos()
                .to_degrees();
            rows.push(("角度", format!("{angle:.1}°")));
            rows.push((
                "臂长",
                format!(
                    "{:.*} / {:.*} {}",
                    decimals, length1, decimals, length2, unit
                ),
            ));
        }
        MeasurementKind::Circle => {
            let (center, pixel_radius) = track.circle_values(frame)?;
            if handles.is_empty() {
                return None;
            }
            let world_center = coords.image_to_world(frame, center.0, center.1);
            let world_edge = coords.image_to_world(frame, handles[0].0, handles[0].1);
            let radius = ((world_edge.0 - world_center.0).powi(2)
                + (world_edge.1 - world_center.1).powi(2))
            .sqrt();
            rows.push((
                "圆心",
                format!(
                    "({:.*}, {:.*}) {}",
                    decimals, world_center.0, decimals, world_center.1, unit
                ),
            ));
            rows.push((
                "半径",
                format!("{:.*} {} ({pixel_radius:.0} px)", decimals, radius, unit),
            ));
        }
    }
    Some(rows)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_ruler(
    st: &DocState,
    window: &mut Window,
    cx: &mut App,
    a: (f32, f32),
    b: (f32, f32),
    world_length: f64,
    unit: &str,
    decimals: usize,
    color: Rgba,
    active: bool,
) {
    let (sx0, sy0) = st.to_screen(a.0, a.1);
    let (sx1, sy1) = st.to_screen(b.0, b.1);
    let (x0, y0) = (st.sx + sx0, st.sy + sy0);
    let (x1, y1) = (st.sx + sx1, st.sy + sy1);
    let dx = x1 - x0;
    let dy = y1 - y0;
    let pixel_length = (dx * dx + dy * dy).sqrt();
    if pixel_length < 2.0 || world_length <= 1e-9 || !world_length.is_finite() {
        return;
    }
    let (ux, uy) = (dx / pixel_length, dy / pixel_length);
    let (nx, ny) = (-uy, ux);
    let alpha = if active { 1.0 } else { 0.7 };
    if let Some(path) = stroke_path(&[(x0, y0), (x1, y1)], 1.3) {
        window.paint_path(path, color.alpha(alpha));
    }
    // End caps instead of endpoint dots.
    for (ex, ey) in [(x0, y0), (x1, y1)] {
        if let Some(path) = stroke_path(
            &[
                (ex - nx * 7.0, ey - ny * 7.0),
                (ex + nx * 7.0, ey + ny * 7.0),
            ],
            1.3,
        ) {
            window.paint_path(path, color.alpha(alpha));
        }
    }

    let scale = pixel_length as f64 / world_length;
    let major = axis_step(scale);
    if major <= 0.0 || !major.is_finite() {
        return;
    }
    let minor = major / 5.0;
    let minor_pixels = minor * scale;
    let count = (world_length / minor).floor() as usize;
    let label_color = c(0xdbe6f5).alpha(if active { 0.95 } else { 0.75 });
    let mut last_label_x = f32::NEG_INFINITY;
    let mut last_label_y = f32::NEG_INFINITY;
    for i in 0..=count {
        let value = i as f64 * minor;
        if value > world_length + 1e-9 {
            break;
        }
        if (world_length - value) * scale < 4.0 {
            continue;
        }
        let fraction = (value / world_length) as f32;
        let px = x0 + dx * fraction;
        let py = y0 + dy * fraction;
        let is_major = i % 5 == 0;
        let tick = if is_major {
            10.0
        } else if minor_pixels >= 5.0 {
            4.0
        } else {
            continue;
        };
        if let Some(path) = stroke_path(&[(px, py), (px + nx * tick, py + ny * tick)], 1.0) {
            window.paint_path(path, color.alpha(alpha));
        }
        if is_major && (world_length - value) * scale >= 30.0 {
            let label = format!("{:.*} {}", decimals, value, unit);
            let lx = px + nx * (tick + 3.0) - 8.0;
            let ly = py + ny * (tick + 3.0) - 5.0;
            if (lx - last_label_x).abs() + (ly - last_label_y).abs() > 24.0 {
                plot_text(window, cx, &label, lx, ly, 9.0, label_color);
                last_label_x = lx;
                last_label_y = ly;
            }
        }
    }
    // Always label the far end.
    let end_label = format!("{:.*} {}", decimals, world_length, unit);
    plot_text(
        window,
        cx,
        &end_label,
        x1 + nx * 12.0 - 10.0,
        y1 + ny * 12.0 - 5.0,
        9.0,
        label_color,
    );
}

pub(crate) fn draw_measurement(
    st: &DocState,
    window: &mut Window,
    cx: &mut App,
    track: &Track,
    ox: f32,
    oy: f32,
    active: bool,
) {
    let handles = track.handles(st.frame);
    if handles.is_empty() {
        return;
    }
    let points: Vec<(f32, f32)> = handles
        .iter()
        .map(|(x, y)| {
            let (sx, sy) = st.to_screen(*x, *y);
            (ox + sx, oy + sy)
        })
        .collect();
    let color = rgb(track.color);
    let alpha = if active { 1.0 } else { 0.7 };
    let width = if active { 1.8 } else { 1.1 };

    match track.measurement_kind() {
        Some(MeasurementKind::Tape) if handles.len() >= 2 => {
            let wa = st
                .coords
                .image_to_world(st.frame, handles[0].0, handles[0].1);
            let wb = st
                .coords
                .image_to_world(st.frame, handles[1].0, handles[1].1);
            let length = ((wb.0 - wa.0).powi(2) + (wb.1 - wa.1).powi(2)).sqrt();
            draw_ruler(
                st,
                window,
                cx,
                handles[0],
                handles[1],
                length as f64,
                &st.calibration.unit,
                st.calibration.decimals,
                color,
                active,
            );
            return;
        }
        Some(MeasurementKind::Protractor) if points.len() >= 3 => {
            let (vertex, arm1, arm2) = (points[0], points[1], points[2]);
            if let Some(path) = stroke_path(&[vertex, arm1], width) {
                window.paint_path(path, color.alpha(alpha));
            }
            if let Some(path) = stroke_path(&[vertex, arm2], width) {
                window.paint_path(path, color.alpha(alpha));
            }
            let a1 = (arm1.1 - vertex.1).atan2(arm1.0 - vertex.0);
            let a2 = (arm2.1 - vertex.1).atan2(arm2.0 - vertex.0);
            let mut sweep = a2 - a1;
            while sweep > std::f32::consts::PI {
                sweep -= std::f32::consts::TAU;
            }
            while sweep < -std::f32::consts::PI {
                sweep += std::f32::consts::TAU;
            }
            let radius = ((arm1.0 - vertex.0).hypot(arm1.1 - vertex.1))
                .min((arm2.0 - vertex.0).hypot(arm2.1 - vertex.1))
                * 0.55;
            let arc: Vec<(f32, f32)> = (0..=24)
                .map(|i| {
                    let angle = a1 + sweep * i as f32 / 24.0;
                    (
                        vertex.0 + radius * angle.cos(),
                        vertex.1 + radius * angle.sin(),
                    )
                })
                .collect();
            if let Some(path) = stroke_path(&arc, 1.2) {
                window.paint_path(path, color.alpha(alpha));
            }
        }
        Some(MeasurementKind::Circle) if points.len() >= 3 => {
            if let Some((center, radius)) = track.circle_values(st.frame) {
                let (cx, cy) = st.to_screen(center.0, center.1);
                if let Some(path) = circle_path(ox + cx, oy + cy, radius * st.scale, false, width) {
                    window.paint_path(path, color.alpha(alpha));
                }
                if let Some(path) =
                    stroke_path(&[(ox + cx - 5.0, oy + cy), (ox + cx + 5.0, oy + cy)], 1.0)
                {
                    window.paint_path(path, color.alpha(alpha));
                }
                if let Some(path) =
                    stroke_path(&[(ox + cx, oy + cy - 5.0), (ox + cx, oy + cy + 5.0)], 1.0)
                {
                    window.paint_path(path, color.alpha(alpha));
                }
            }
        }
        _ => {}
    }

    for (index, point) in points.iter().enumerate() {
        let radius = if active { 4.5 } else { 3.2 };
        if let Some(path) = circle_path(point.0, point.1, radius, true, 0.0) {
            window.paint_path(path, c(0xffffff));
        }
        if let Some(path) = circle_path(
            point.0,
            point.1,
            radius,
            false,
            if index == 0 && active { 2.0 } else { 1.4 },
        ) {
            window.paint_path(path, color.alpha(alpha));
        }
    }
}

pub(crate) fn point_segment_distance(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let dx = bx - ax;
    let dy = by - ay;
    let len_sq = dx * dx + dy * dy;
    if len_sq <= 1e-9 {
        return ((px - ax).powi(2) + (py - ay).powi(2)).sqrt();
    }
    let t = (((px - ax) * dx + (py - ay) * dy) / len_sq).clamp(0.0, 1.0);
    let cx = ax + dx * t;
    let cy = ay + dy * t;
    ((px - cx).powi(2) + (py - cy).powi(2)).sqrt()
}

pub(crate) fn clip_axis(
    origin: [f64; 2],
    dir: [f64; 2],
    width: f64,
    height: f64,
) -> Option<([f64; 2], [f64; 2])> {
    let mut t_min = f64::NEG_INFINITY;
    let mut t_max = f64::INFINITY;
    for (position, delta, min, max) in [
        (origin[0], dir[0], 0.0, width),
        (origin[1], dir[1], 0.0, height),
    ] {
        if delta.abs() < 1e-9 {
            if position < min || position > max {
                return None;
            }
        } else {
            let t1 = (min - position) / delta;
            let t2 = (max - position) / delta;
            t_min = t_min.max(t1.min(t2));
            t_max = t_max.min(t1.max(t2));
        }
    }
    if t_min < t_max {
        Some((
            [origin[0] + dir[0] * t_min, origin[1] + dir[1] * t_min],
            [origin[0] + dir[0] * t_max, origin[1] + dir[1] * t_max],
        ))
    } else {
        None
    }
}

pub(crate) fn axis_step(scale: f64) -> f64 {
    if scale <= 0.0 || !scale.is_finite() {
        return 1.0;
    }
    let minimum = 80.0 / scale;
    let exponent = minimum.log10().floor();
    let base = 10f64.powf(exponent);
    for factor in [1.0, 2.0, 5.0, 10.0] {
        if base * factor >= minimum {
            return base * factor;
        }
    }
    base * 10.0
}

pub(crate) fn draw_origin_marker(st: &DocState, window: &mut Window, ox: f32, oy: f32) {
    let origin = st.coords.origin(st.frame);
    let (x, y) = st.to_screen(origin[0] as f32, origin[1] as f32);
    let (x, y) = (ox + x, oy + y);
    if let Some(path) = circle_path(x, y, 5.0, false, 1.6) {
        window.paint_path(path, c(0xffd166));
    }
    if let Some(path) = stroke_path(&[(x - 8.0, y), (x + 8.0, y)], 1.2) {
        window.paint_path(path, c(0xffd166));
    }
    if let Some(path) = stroke_path(&[(x, y - 8.0), (x, y + 8.0)], 1.2) {
        window.paint_path(path, c(0xffd166));
    }
}

pub(crate) fn draw_tracking_overlay(
    st: &State,
    window: &mut Window,
    _cx: &mut App,
    ox: f32,
    oy: f32,
) {
    let Some(overlay) = &st.tracker_overlay else {
        return;
    };
    let Some(track) = st.tracks.iter().find(|track| track.id == overlay.track_id) else {
        return;
    };
    let frame = st.frame;
    let color = rgb(track.color);
    let point = track.point_at(frame).or_else(|| {
        track
            .points()
            .filter(|(point_frame, _, _)| *point_frame <= frame)
            .last()
            .map(|(_, x, y)| (x, y))
    });
    let target = point.unwrap_or(overlay.key_center);
    let (target_x, target_y) = st.to_screen(target.0, target.1);
    let target_x = ox + target_x;
    let target_y = oy + target_y;
    let mask_width = (overlay.mask_width as f32 * st.scale).max(4.0);
    let mask_height = (overlay.mask_height as f32 * st.scale).max(4.0);
    let is_keyframe =
        track.keyframes().any(|keyframe| keyframe == frame) || overlay.key_frame == frame;
    let run = st.tracking.as_ref();
    let mut search_center = if let Some(run) = run {
        if run.search_frame == frame {
            run.search_center.unwrap_or(target)
        } else {
            target
        }
    } else if overlay.search_frame == frame {
        overlay.search_center
    } else {
        target
    };
    if st.tracker_settings.search_policy == SearchPolicy::Fixed {
        search_center = overlay.key_center;
    }
    let search_size = (run
        .map(|run| run.settings.search_size)
        .unwrap_or(overlay.search_size) as f32
        * st.scale)
        .max(6.0);
    let (search_x, search_y) = st.to_screen(search_center.0, search_center.1);
    let search_x = ox + search_x;
    let search_y = oy + search_y;
    let search_half = search_size * 0.5;
    dashed_rect(
        window,
        search_x - search_half,
        search_y - search_half,
        search_x + search_half,
        search_y + search_half,
        color,
    );
    draw_corner_square(
        window,
        search_x + search_half,
        search_y + search_half,
        color,
    );

    if is_keyframe {
        match overlay.shape {
            MaskShape::Ellipse => solid_ellipse(
                window,
                target_x,
                target_y,
                mask_width * 0.5,
                mask_height * 0.5,
                color,
            ),
            MaskShape::Rectangle => solid_rect(
                window,
                target_x - mask_width * 0.5,
                target_y - mask_height * 0.5,
                target_x + mask_width * 0.5,
                target_y + mask_height * 0.5,
                color,
            ),
        }
        draw_corner_square(
            window,
            target_x + mask_width * 0.5,
            target_y + mask_height * 0.5,
            color,
        );
        if let Some(path) = stroke_path(
            &[(target_x - 9.0, target_y), (target_x + 9.0, target_y)],
            2.0,
        ) {
            window.paint_path(path, color);
        }
        if let Some(path) = stroke_path(
            &[(target_x, target_y - 9.0), (target_x, target_y + 9.0)],
            2.0,
        ) {
            window.paint_path(path, color);
        }
    } else {
        let match_position = run
            .filter(|run| run.search_frame == frame)
            .and_then(|run| run.last_match)
            .or(point)
            .or(overlay.match_position);
        if let Some((match_x, match_y)) = match_position {
            let (match_x, match_y) = st.to_screen(match_x, match_y);
            let match_x = ox + match_x;
            let match_y = oy + match_y;
            match overlay.shape {
                MaskShape::Ellipse => dotted_ellipse(
                    window,
                    match_x,
                    match_y,
                    mask_width * 0.5,
                    mask_height * 0.5,
                    color,
                ),
                MaskShape::Rectangle => dotted_rect(
                    window,
                    match_x - mask_width * 0.5,
                    match_y - mask_height * 0.5,
                    match_x + mask_width * 0.5,
                    match_y + mask_height * 0.5,
                    color,
                ),
            }
        }
    }
}

fn draw_corner_square(window: &mut Window, x: f32, y: f32, color: Rgba) {
    if let Some(path) = rect_path(x, y, 5.0, true, 0.0) {
        window.paint_path(path, color);
    }
}

fn solid_segment(window: &mut Window, a: (f32, f32), b: (f32, f32), color: Rgba) {
    if let Some(path) = stroke_path(&[a, b], 1.5) {
        window.paint_path(path, color);
    }
}

fn solid_rect(window: &mut Window, left: f32, top: f32, right: f32, bottom: f32, color: Rgba) {
    solid_segment(window, (left, top), (right, top), color);
    solid_segment(window, (right, top), (right, bottom), color);
    solid_segment(window, (right, bottom), (left, bottom), color);
    solid_segment(window, (left, bottom), (left, top), color);
}

fn solid_ellipse(window: &mut Window, cx: f32, cy: f32, rx: f32, ry: f32, color: Rgba) {
    let steps = 48;
    for index in 0..steps {
        let a0 = index as f32 / steps as f32 * std::f32::consts::TAU;
        let a1 = (index + 1) as f32 / steps as f32 * std::f32::consts::TAU;
        let p0 = (cx + rx * a0.cos(), cy + ry * a0.sin());
        let p1 = (cx + rx * a1.cos(), cy + ry * a1.sin());
        solid_segment(window, p0, p1, color);
    }
}

fn dashed_segment(window: &mut Window, a: (f32, f32), b: (f32, f32), color: Rgba) {
    let dx = b.0 - a.0;
    let dy = b.1 - a.1;
    let length = dx.hypot(dy);
    if length < 0.5 {
        return;
    }
    let mut offset = 0.0;
    while offset < length {
        let start = offset / length;
        let end = ((offset + 2.0) / length).min(1.0);
        let p0 = (a.0 + dx * start, a.1 + dy * start);
        let p1 = (a.0 + dx * end, a.1 + dy * end);
        if let Some(path) = stroke_path(&[p0, p1], 1.4) {
            window.paint_path(path, color);
        }
        offset += 10.0;
    }
}

fn dashed_rect(window: &mut Window, left: f32, top: f32, right: f32, bottom: f32, color: Rgba) {
    dashed_segment(window, (left, top), (right, top), color);
    dashed_segment(window, (right, top), (right, bottom), color);
    dashed_segment(window, (right, bottom), (left, bottom), color);
    dashed_segment(window, (left, bottom), (left, top), color);
}

fn draw_footprint(
    window: &mut Window,
    footprint: Footprint,
    x: f32,
    y: f32,
    color: Rgba,
    _active: bool,
) {
    let width = 1.1;
    match footprint {
        Footprint::Diamond => {
            let points = [
                (x, y - 5.0),
                (x + 5.0, y),
                (x, y + 5.0),
                (x - 5.0, y),
                (x, y - 5.0),
            ];
            if let Some(path) = stroke_path(&points, width) {
                window.paint_path(path, color);
            }
        }
        Footprint::Triangle => {
            let points = [
                (x, y - 5.0),
                (x + 4.5, y + 4.0),
                (x - 4.5, y + 4.0),
                (x, y - 5.0),
            ];
            if let Some(path) = stroke_path(&points, width) {
                window.paint_path(path, color);
            }
        }
        Footprint::Square => {
            if let Some(path) = rect_path(x, y, 9.0, false, width) {
                window.paint_path(path, color);
            }
        }
        Footprint::Circle => {
            if let Some(path) = circle_path(x, y, 4.5, false, width) {
                window.paint_path(path, color);
            }
        }
        Footprint::Crosshair => {
            if let Some(path) = stroke_path(&[(x - 6.0, y), (x + 6.0, y)], width) {
                window.paint_path(path, color);
            }
            if let Some(path) = stroke_path(&[(x, y - 6.0), (x, y + 6.0)], width) {
                window.paint_path(path, color);
            }
        }
    }
}

fn dotted_segment(window: &mut Window, a: (f32, f32), b: (f32, f32), color: Rgba) {
    let dx = b.0 - a.0;
    let dy = b.1 - a.1;
    let length = dx.hypot(dy);
    if length < 0.5 {
        return;
    }
    let step = 5.0;
    let mut offset = 0.0;
    while offset < length {
        let start = offset / length;
        let end = ((offset + 2.0) / length).min(1.0);
        let p0 = (a.0 + dx * start, a.1 + dy * start);
        let p1 = (a.0 + dx * end, a.1 + dy * end);
        if let Some(path) = stroke_path(&[p0, p1], 1.2) {
            window.paint_path(path, color);
        }
        offset += step;
    }
}

fn dotted_rect(window: &mut Window, left: f32, top: f32, right: f32, bottom: f32, color: Rgba) {
    dotted_segment(window, (left, top), (right, top), color);
    dotted_segment(window, (right, top), (right, bottom), color);
    dotted_segment(window, (right, bottom), (left, bottom), color);
    dotted_segment(window, (left, bottom), (left, top), color);
}

fn dotted_ellipse(window: &mut Window, cx: f32, cy: f32, rx: f32, ry: f32, color: Rgba) {
    let steps = 64;
    for index in (0..steps).step_by(2) {
        let a0 = index as f32 / steps as f32 * std::f32::consts::TAU;
        let a1 = (index + 1) as f32 / steps as f32 * std::f32::consts::TAU;
        let p0 = (cx + rx * a0.cos(), cy + ry * a0.sin());
        let p1 = (cx + rx * a1.cos(), cy + ry * a1.sin());
        if let Some(path) = stroke_path(&[p0, p1], 1.3) {
            window.paint_path(path, color);
        }
    }
}

pub(crate) fn draw_axes(st: &DocState, window: &mut Window, cx: &mut App, ox: f32, oy: f32) {
    let coords = st.coords.frame(st.frame);
    let width = st.img_w as f64;
    let height = st.img_h as f64;
    if width <= 0.0 || height <= 0.0 {
        return;
    }
    let cos = coords.angle.cos();
    let sin = coords.angle.sin();
    let dirs = [[cos, -sin], [-sin, -cos]];
    let colors = [c(0x4dabf7).alpha(0.85), c(0x51cf66).alpha(0.85)];

    for axis in 0..2 {
        let Some((a, b)) = clip_axis(coords.origin, dirs[axis], width, height) else {
            continue;
        };
        let pa = st.to_screen(a[0] as f32, a[1] as f32);
        let pb = st.to_screen(b[0] as f32, b[1] as f32);
        let pa = (ox + pa.0, oy + pa.1);
        let pb = (ox + pb.0, oy + pb.1);
        if let Some(path) = stroke_path(&[pa, pb], 1.3) {
            window.paint_path(path, colors[axis]);
        }
        let positive = (b[0] - coords.origin[0]) * dirs[axis][0]
            + (b[1] - coords.origin[1]) * dirs[axis][1]
            > (a[0] - coords.origin[0]) * dirs[axis][0] + (a[1] - coords.origin[1]) * dirs[axis][1];
        let tip = if positive { pb } else { pa };
        let inward = if positive { pa } else { pb };
        let dx = tip.0 - inward.0;
        let dy = tip.1 - inward.1;
        let len = (dx * dx + dy * dy).sqrt().max(1.0);
        let ux = dx / len;
        let uy = dy / len;
        let head = 8.0;
        let spread = 0.5;
        for sign in [-1.0f32, 1.0] {
            let angle = spread * sign;
            let hx = (ux * angle.cos() - uy * angle.sin()) * head;
            let hy = (ux * angle.sin() + uy * angle.cos()) * head;
            if let Some(path) = stroke_path(&[tip, (tip.0 - hx, tip.1 - hy)], 1.3) {
                window.paint_path(path, colors[axis]);
            }
        }
    }

    let step_x = axis_step(coords.scale[0]);
    let step_y = axis_step(coords.scale[1]);
    let mut min_wx = f64::INFINITY;
    let mut max_wx = f64::NEG_INFINITY;
    let mut min_wy = f64::INFINITY;
    let mut max_wy = f64::NEG_INFINITY;
    for (x, y) in [(0.0, 0.0), (width, 0.0), (0.0, height), (width, height)] {
        let (wx, wy) = coords.image_to_world(x, y);
        min_wx = min_wx.min(wx);
        max_wx = max_wx.max(wx);
        min_wy = min_wy.min(wy);
        max_wy = max_wy.max(wy);
    }

    let tick_half = 4.0;
    let dir_y_screen = (dirs[1][0] as f32 * st.scale, dirs[1][1] as f32 * st.scale);
    let norm_y = (dir_y_screen.0.hypot(dir_y_screen.1)).max(1e-6);
    let tick_y = (
        dir_y_screen.0 / norm_y * tick_half,
        dir_y_screen.1 / norm_y * tick_half,
    );
    let dir_x_screen = (dirs[0][0] as f32 * st.scale, dirs[0][1] as f32 * st.scale);
    let norm_x = (dir_x_screen.0.hypot(dir_x_screen.1)).max(1e-6);
    let tick_x = (
        dir_x_screen.0 / norm_x * tick_half,
        dir_x_screen.1 / norm_x * tick_half,
    );

    let count_x = ((max_wx - min_wx) / step_x).ceil() as i64 + 2;
    if count_x <= 200 {
        let start = (min_wx / step_x).floor() as i64 - 1;
        let end = (max_wx / step_x).ceil() as i64 + 1;
        for k in start..=end {
            let (x, y) = coords.world_to_image(k as f64 * step_x, 0.0);
            if x >= 0.0 && y >= 0.0 && x <= width && y <= height {
                let p = st.to_screen(x as f32, y as f32);
                let p = (ox + p.0, oy + p.1);
                if let Some(path) = stroke_path(
                    &[
                        (p.0 - tick_y.0, p.1 - tick_y.1),
                        (p.0 + tick_y.0, p.1 + tick_y.1),
                    ],
                    1.0,
                ) {
                    window.paint_path(path, colors[0]);
                }
                let label = format_tick(k as f64 * step_x, step_x);
                plot_text(window, cx, &label, p.0 - 8.0, p.1 + 4.0, 9.0, colors[0]);
            }
        }
        let minor_step = step_x / 5.0;
        if minor_step * coords.scale[0] >= 5.0 {
            for k in start..=end {
                for j in 1..5 {
                    let value = k as f64 * step_x + j as f64 * minor_step;
                    let (x, y) = coords.world_to_image(value, 0.0);
                    if x >= 0.0 && y >= 0.0 && x <= width && y <= height {
                        let p = st.to_screen(x as f32, y as f32);
                        let p = (ox + p.0, oy + p.1);
                        if let Some(path) = stroke_path(
                            &[
                                (p.0 - tick_y.0 * 0.5, p.1 - tick_y.1 * 0.5),
                                (p.0 + tick_y.0 * 0.5, p.1 + tick_y.1 * 0.5),
                            ],
                            0.8,
                        ) {
                            window.paint_path(path, colors[0].alpha(0.6));
                        }
                    }
                }
            }
        }
    }

    let count_y = ((max_wy - min_wy) / step_y).ceil() as i64 + 2;
    if count_y <= 200 {
        let start = (min_wy / step_y).floor() as i64 - 1;
        let end = (max_wy / step_y).ceil() as i64 + 1;
        for k in start..=end {
            let (x, y) = coords.world_to_image(0.0, k as f64 * step_y);
            if x >= 0.0 && y >= 0.0 && x <= width && y <= height {
                let p = st.to_screen(x as f32, y as f32);
                let p = (ox + p.0, oy + p.1);
                if let Some(path) = stroke_path(
                    &[
                        (p.0 - tick_x.0, p.1 - tick_x.1),
                        (p.0 + tick_x.0, p.1 + tick_x.1),
                    ],
                    1.0,
                ) {
                    window.paint_path(path, colors[1]);
                }
                let label = format_tick(k as f64 * step_y, step_y);
                plot_text(window, cx, &label, p.0 - 32.0, p.1 - 5.0, 9.0, colors[1]);
            }
        }
        let minor_step = step_y / 5.0;
        if minor_step * coords.scale[1] >= 5.0 {
            for k in start..=end {
                for j in 1..5 {
                    let value = k as f64 * step_y + j as f64 * minor_step;
                    let (x, y) = coords.world_to_image(0.0, value);
                    if x >= 0.0 && y >= 0.0 && x <= width && y <= height {
                        let p = st.to_screen(x as f32, y as f32);
                        let p = (ox + p.0, oy + p.1);
                        if let Some(path) = stroke_path(
                            &[
                                (p.0 - tick_x.0 * 0.5, p.1 - tick_x.1 * 0.5),
                                (p.0 + tick_x.0 * 0.5, p.1 + tick_x.1 * 0.5),
                            ],
                            0.8,
                        ) {
                            window.paint_path(path, colors[1].alpha(0.6));
                        }
                    }
                }
            }
        }
    }

    draw_origin_marker(st, window, ox, oy);
    let origin = st.coords.origin(st.frame);
    let origin_screen = st.to_screen(origin[0] as f32, origin[1] as f32);
    let origin_screen = (ox + origin_screen.0, oy + origin_screen.1);
    let angle = coords.angle;
    let handle = (
        origin_screen.0 + angle.cos() as f32 * 60.0,
        origin_screen.1 - angle.sin() as f32 * 60.0,
    );
    if let Some(path) = stroke_path(&[origin_screen, handle], 1.0) {
        window.paint_path(path, colors[0].alpha(0.55));
    }
    if let Some(path) = rect_path(handle.0, handle.1, 8.0, true, 0.0) {
        window.paint_path(path, c(0xffffff));
    }
    if let Some(path) = rect_path(handle.0, handle.1, 8.0, false, 1.8) {
        window.paint_path(path, colors[0]);
    }
}
