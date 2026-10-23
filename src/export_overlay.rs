//! Image-space output scene, independent of viewport zoom and editing handles.
//! Preview and encoder consume the same composited pixels.
use crate::{
    coords::{Calibration, ImageCoordSystem},
    track::{Footprint, MeasurementKind, Track},
    video::FrameData,
    DocState,
};
use std::fmt::Write;

#[derive(Clone)]
pub(crate) struct Scene {
    pub tracks: Vec<Track>,
    pub coords: ImageCoordSystem,
    pub calibration: Calibration,
    pub axes: bool,
    pub trail: bool,
    pub trail_length: usize,
    pub labels: bool,
    pub trace: bool,
}
impl Scene {
    pub fn capture(doc: &DocState) -> Self {
        Self {
            tracks: doc.tracks.iter().filter(|t| t.visible).cloned().collect(),
            coords: doc.coords.clone(),
            calibration: doc.calibration.clone(),
            axes: doc.axes_visible,
            trail: doc.trail_visible,
            trail_length: doc.trail_length,
            labels: doc.labels_visible,
            trace: doc.trace_visible,
        }
    }
    fn svg(&self, frame: usize, width: u32, height: u32, out: (u32, u32)) -> String {
        let mut svg = Svg(format!(
            "<svg xmlns='http://www.w3.org/2000/svg' width='{}' height='{}' viewBox='0 0 {} {}'>",
            out.0, out.1, width, height
        ));
        if self.axes {
            let coords = self.coords.frame(frame);
            let (sin, cos) = coords.angle.sin_cos();
            for (axis, dir, color) in [(0, [cos, -sin], 0x4dabf7), (1, [-sin, -cos], 0x51cf66)] {
                if let Some((a, b)) =
                    crate::canvas::clip_axis(coords.origin, dir, width as f64, height as f64)
                {
                    let a = (a[0] as f32, a[1] as f32);
                    let b = (b[0] as f32, b[1] as f32);
                    svg.line(&[a, b], color, 1.3);
                    let tip = if (b.0 - coords.origin[0] as f32) * dir[0] as f32
                        + (b.1 - coords.origin[1] as f32) * dir[1] as f32
                        > 0.
                    {
                        b
                    } else {
                        a
                    };
                    let (dx, dy) = (dir[0] as f32, dir[1] as f32);
                    svg.line(
                        &[
                            (tip.0 - dx * 8. - dy * 4., tip.1 - dy * 8. + dx * 4.),
                            tip,
                            (tip.0 - dx * 8. + dy * 4., tip.1 - dy * 8. - dx * 4.),
                        ],
                        color,
                        1.3,
                    );
                    let scale = coords.scale[axis];
                    let step = crate::canvas::axis_step(scale);
                    let range = [
                        (0., 0.),
                        (width as f64, 0.),
                        (0., height as f64),
                        (width as f64, height as f64),
                    ]
                    .map(|(x, y)| {
                        let w = coords.image_to_world(x, y);
                        if axis == 0 {
                            w.0
                        } else {
                            w.1
                        }
                    });
                    let min = range.into_iter().fold(f64::INFINITY, f64::min);
                    let max = range.into_iter().fold(f64::NEG_INFINITY, f64::max);
                    if step.is_finite() && step > 0. && (max - min) / step < 200. {
                        for k in (min / step).floor() as i64..=(max / step).ceil() as i64 {
                            let (x, y) = if axis == 0 {
                                coords.world_to_image(k as f64 * step, 0.)
                            } else {
                                coords.world_to_image(0., k as f64 * step)
                            };
                            if x < 0. || y < 0. || x > width as f64 || y > height as f64 {
                                continue;
                            }
                            let (x, y) = (x as f32, y as f32);
                            svg.line(
                                &[(x - dy * 4., y + dx * 4.), (x + dy * 4., y - dx * 4.)],
                                color,
                                1.,
                            );
                            svg.text(
                                (x + 5., y + 12.),
                                &crate::plot_view::format_tick(k as f64 * step, step),
                                color,
                            );
                        }
                    }
                }
            }
        }
        for track in self.tracks.iter().filter(|t| t.visible) {
            let color = track.color;
            if let Some(kind) = track.measurement_kind() {
                let points = track.handles(frame);
                match kind {
                    MeasurementKind::Tape if points.len() >= 2 => {
                        let a = self.coords.image_to_world(frame, points[0].0, points[0].1);
                        let b = self.coords.image_to_world(frame, points[1].0, points[1].1);
                        svg.ruler(
                            points[0],
                            points[1],
                            ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt() as f64,
                            &self.calibration.unit,
                            self.calibration.decimals,
                            color,
                        );
                    }
                    MeasurementKind::Protractor if points.len() >= 3 => {
                        svg.line(&[points[1], points[0], points[2]], color, 1.5);
                        let v = points[0];
                        let a = (points[1].1 - v.1).atan2(points[1].0 - v.0);
                        let b = (points[2].1 - v.1).atan2(points[2].0 - v.0);
                        let sweep = (b - a + std::f32::consts::PI)
                            .rem_euclid(std::f32::consts::TAU)
                            - std::f32::consts::PI;
                        let r = (points[1].0 - v.0)
                            .hypot(points[1].1 - v.1)
                            .min((points[2].0 - v.0).hypot(points[2].1 - v.1))
                            * 0.55;
                        let arc: Vec<_> = (0..=24)
                            .map(|i| {
                                let a = a + sweep * i as f32 / 24.;
                                (v.0 + r * a.cos(), v.1 + r * a.sin())
                            })
                            .collect();
                        svg.line(&arc, color, 1.2);
                    }
                    MeasurementKind::Circle => {
                        if let Some((c, r)) = track.circle_values(frame) {
                            svg.circle(c, r, color, false);
                        }
                    }
                    _ => {}
                }
                for p in &points {
                    svg.circle(*p, 3., color, false);
                }
                if let (Some(p), Some(rows)) = (
                    points.first(),
                    crate::canvas::measurement_summary(
                        track,
                        frame,
                        &self.coords,
                        &self.calibration.unit,
                        self.calibration.decimals,
                    ),
                ) {
                    // The first numerical result is also visible for angle/circle measurements.
                    if kind != MeasurementKind::Tape {
                        if let Some((name, value)) = rows.get(if kind == MeasurementKind::Circle {
                            1
                        } else {
                            0
                        }) {
                            svg.text((p.0 + 8., p.1 - 10.), &format!("{name}: {value}"), color);
                        }
                    }
                }
                continue;
            }
            if self.trace {
                svg.line(
                    &track.points().map(|(_, x, y)| (x, y)).collect::<Vec<_>>(),
                    color,
                    1.2,
                );
            }
            let first = if !self.trail {
                frame
            } else if self.trail_length == 0 {
                0
            } else {
                frame.saturating_sub(self.trail_length - 1)
            };
            for (n, x, y) in track.points_in(first..=frame) {
                match track.footprint {
                    Footprint::Diamond => svg.line(
                        &[
                            (x, y - 5.),
                            (x + 5., y),
                            (x, y + 5.),
                            (x - 5., y),
                            (x, y - 5.),
                        ],
                        color,
                        1.2,
                    ),
                    Footprint::Square => svg.line(
                        &[
                            (x - 4.5, y - 4.5),
                            (x + 4.5, y - 4.5),
                            (x + 4.5, y + 4.5),
                            (x - 4.5, y + 4.5),
                            (x - 4.5, y - 4.5),
                        ],
                        color,
                        1.2,
                    ),
                    Footprint::Triangle => svg.line(
                        &[
                            (x, y - 5.),
                            (x + 4.5, y + 4.),
                            (x - 4.5, y + 4.),
                            (x, y - 5.),
                        ],
                        color,
                        1.2,
                    ),
                    Footprint::Circle => svg.circle((x, y), 4.5, color, false),
                    Footprint::Crosshair => {
                        svg.line(&[(x - 6., y), (x + 6., y)], color, 1.2);
                        svg.line(&[(x, y - 6.), (x, y + 6.)], color, 1.2);
                    }
                }
                if n == frame {
                    svg.circle((x, y), 6., color, false);
                }
                if self.labels {
                    svg.text((x - 14., y - 8.), &n.to_string(), color);
                }
                if let Some(step) = track.rigid_step(n) {
                    let side = step.scale.max(8.);
                    let (sin, cos) = step.angle.sin_cos();
                    if n == frame {
                        let fallback = vec![[-0.5, -0.5], [0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]];
                        let contour = step
                            .contour
                            .as_ref()
                            .filter(|c| c.len() > 2)
                            .or_else(|| {
                                track
                                    .rigid
                                    .as_ref()
                                    .map(|r| &r.contour)
                                    .filter(|c| c.len() > 2)
                            })
                            .unwrap_or(&fallback);
                        let mut path: Vec<_> = contour
                            .iter()
                            .map(|[a, b]| {
                                (
                                    x + side * (cos * a - sin * b),
                                    y + side * (sin * a + cos * b),
                                )
                            })
                            .collect();
                        path.push(path[0]);
                        svg.line(&path, color, 1.6);
                    }
                    svg.line(
                        &[(x, y), (x + cos * side * 0.8, y + sin * side * 0.8)],
                        color,
                        1.6,
                    );
                }
            }
        }
        for index in 0..self.calibration.stick_count() {
            if !self.calibration.stick_visible(index) {
                continue;
            }
            if let Some((a, b, length)) = self.calibration.stick(frame, index) {
                svg.ruler(
                    (a[0] as f32, a[1] as f32),
                    (b[0] as f32, b[1] as f32),
                    length,
                    &self.calibration.unit,
                    self.calibration.decimals,
                    0xffd166,
                );
            }
        }
        svg.0.push_str("</svg>");
        svg.0
    }
}
struct Svg(String);
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace(char::from(34), "&quot;")
        .replace(char::from(39), "&apos;")
}

impl Svg {
    fn line(&mut self, points: &[(f32, f32)], color: u32, width: f32) {
        if points.len() < 2 || points.iter().any(|(x, y)| !x.is_finite() || !y.is_finite()) {
            return;
        }
        let _ = write!(
            self.0,
            "<polyline fill='none' stroke='#{color:06x}' stroke-width='{width}' points='"
        );
        for (x, y) in points {
            let _ = write!(self.0, "{x},{y} ");
        }
        self.0.push_str("'/>");
    }
    fn circle(&mut self, p: (f32, f32), r: f32, color: u32, fill: bool) {
        if !p.0.is_finite() || !p.1.is_finite() || !r.is_finite() || r <= 0. {
            return;
        }
        let _ = write!(
            self.0,
            "<circle cx='{}' cy='{}' r='{r}' stroke='#{color:06x}' fill='{}' stroke-width='1.3'/>",
            p.0,
            p.1,
            if fill {
                format!("#{color:06x}")
            } else {
                "none".into()
            }
        );
    }
    fn text(&mut self, p: (f32, f32), text: &str, color: u32) {
        if p.0.is_finite() && p.1.is_finite() {
            let _=write!(self.0,"<text x='{}' y='{}' fill='#{color:06x}' font-family='sans-serif' font-size='10'>{}</text>",p.0,p.1,escape(text));
        }
    }
    fn ruler(
        &mut self,
        a: (f32, f32),
        b: (f32, f32),
        length: f64,
        unit: &str,
        decimals: usize,
        color: u32,
    ) {
        let dx = b.0 - a.0;
        let dy = b.1 - a.1;
        let pixels = dx.hypot(dy);
        if pixels < 2. || length <= 0. || !length.is_finite() {
            return;
        }
        let (nx, ny) = (-dy / pixels, dx / pixels);
        self.line(&[a, b], color, 1.3);
        for p in [a, b] {
            self.line(
                &[
                    (p.0 - nx * 7., p.1 - ny * 7.),
                    (p.0 + nx * 7., p.1 + ny * 7.),
                ],
                color,
                1.3,
            );
        }
        let minor = crate::canvas::axis_step(pixels as f64 / length) / 5.;
        let count = (length / minor).floor().min(1000.) as usize;
        for i in 0..=count {
            let fraction = (i as f64 * minor / length) as f32;
            let p = (a.0 + dx * fraction, a.1 + dy * fraction);
            let tick = if i % 5 == 0 { 10. } else { 4. };
            self.line(&[p, (p.0 + nx * tick, p.1 + ny * tick)], color, 1.);
        }
        self.text(
            (b.0 + nx * 12., b.1 + ny * 12.),
            &format!("{:.*} {unit}", decimals.min(8), length),
            color,
        );
    }
}
pub(crate) struct Renderer {
    options: resvg::usvg::Options<'static>,
}
impl Renderer {
    pub fn new() -> Self {
        let mut options = resvg::usvg::Options::default();
        static FONTS: std::sync::OnceLock<std::sync::Arc<resvg::usvg::fontdb::Database>> =
            std::sync::OnceLock::new();
        options.fontdb = FONTS
            .get_or_init(|| {
                let mut db = resvg::usvg::fontdb::Database::new();
                db.load_system_fonts();
                std::sync::Arc::new(db)
            })
            .clone();
        Self { options }
    }
    pub fn frame(
        &self,
        scene: &Scene,
        frame: &FrameData,
        output: (u32, u32),
    ) -> Result<FrameData, String> {
        let tree = resvg::usvg::Tree::from_str(
            &scene.svg(frame.index, frame.width, frame.height, output),
            &self.options,
        )
        .map_err(|e| format!("render objects: {e}"))?;
        let mut overlay =
            resvg::tiny_skia::Pixmap::new(output.0, output.1).ok_or("invalid export dimensions")?;
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::identity(),
            &mut overlay.as_mut(),
        );
        let image = image::RgbaImage::from_raw(frame.width, frame.height, frame.bgra.clone())
            .ok_or("invalid video frame")?;
        let mut bgra = if output == (frame.width, frame.height) {
            image.into_raw()
        } else {
            image::imageops::resize(
                &image,
                output.0,
                output.1,
                image::imageops::FilterType::Triangle,
            )
            .into_raw()
        };
        for (pixel, overlay) in bgra.chunks_exact_mut(4).zip(overlay.data().chunks_exact(4)) {
            let alpha = overlay[3] as u32;
            for (dst, src) in [(0, 2), (1, 1), (2, 0)] {
                pixel[dst] = (overlay[src] as u32 + (pixel[dst] as u32 * (255 - alpha) + 127) / 255)
                    .min(255) as u8;
            }
            pixel[3] = 255;
        }
        Ok(FrameData {
            index: frame.index,
            width: output.0,
            height: output.1,
            bgra,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::track::MeasurementStep;

    #[test]
    fn all_object_types_render_in_image_space_and_respect_visibility() {
        let renderer = Renderer::new();
        let frame = FrameData {
            index: 0,
            width: 300,
            height: 200,
            bgra: [0, 0, 0, 255].repeat(300 * 200),
        };
        for kind in 0..7 {
            let mut doc = DocState::placeholder();
            doc.calibration.primary_visible = false;
            doc.labels_visible = false;
            match kind {
                0 => {
                    doc.axes_visible = true;
                    doc.coords.base.origin = [80., 80.];
                }
                1 => {
                    doc.calibration = Calibration::new(300., 200.);
                    doc.calibration.unit = "m<&米".into();
                }
                2..=4 => {
                    let measurement = [
                        MeasurementKind::Tape,
                        MeasurementKind::Protractor,
                        MeasurementKind::Circle,
                    ][kind - 2];
                    let mut track = Track::measurement(1, measurement, "measure", 0xff0000);
                    track.set_measurement_step(
                        0,
                        MeasurementStep {
                            handles: vec![(50., 60.), (70., 40.), (90., 60.)],
                        },
                    );
                    doc.tracks.push(track);
                }
                5 => {
                    let mut track = Track::point_mass(1, "point", 0xff0000);
                    track.set_point(0, 60., 60.);
                    doc.tracks.push(track);
                }
                _ => {
                    let mut track = Track::rigid_body(1, "rigid", 0xff0000, "MOTION", vec![]);
                    track.set_point(0, 80., 80.);
                    track.set_rigid_step(0, 80., 80., 0.3, 40., 1);
                    doc.tracks.push(track);
                }
            }
            let mut scene = Scene::capture(&doc);
            let rendered = renderer.frame(&scene, &frame, (300, 200)).unwrap();
            let painted = rendered
                .bgra
                .chunks_exact(4)
                .filter(|p| p[0] > 5 || p[1] > 5 || p[2] > 5)
                .count();
            assert!(painted > 20, "missing object kind {kind}");
            scene.axes = false;
            scene.calibration.primary_visible = false;
            for track in &mut scene.tracks {
                track.visible = false;
            }
            let hidden = renderer.frame(&scene, &frame, (300, 200)).unwrap();
            assert_eq!(hidden.bgra, frame.bgra, "hidden object kind {kind} leaked");
        }
        assert_eq!(escape("<&>'\""), "&lt;&amp;&gt;&apos;&quot;");
    }
}
