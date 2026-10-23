//! Point tracking backends and the background AutoTracker worker.
//!
//! The first backend mirrors OSP Tracker's TemplateMatcher: RGB squared
//! difference over an elliptical mask, peak height confidence, parabolic
//! subpixel fit, and evolve/tether template updating. The trait keeps the door
//! open for learned or classical replacement backends.

use crate::i18n::tr;
use crate::video::{FrameData, FrameSource};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread::{self, JoinHandle};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchQuality {
    Good,
    Possible,
    Lost,
}

#[derive(Clone, Copy, Debug)]
pub struct TrackResult {
    pub position: (f32, f32),
    pub confidence: f32,
    pub quality: MatchQuality,
}

#[derive(Clone, Copy, Debug)]
pub struct TrackHint {
    pub center: (f32, f32),
}

/// A replaceable tracking algorithm. Backends are moved to a worker thread.
pub trait TrackingBackend: Send {
    fn name(&self) -> String;

    /// Captures the feature template at the start frame.
    fn initialize(&mut self, frame: &FrameData, center: (f32, f32)) -> Result<(), String>;

    /// Finds the feature in the next frame near the hint.
    fn locate(&mut self, frame: &FrameData, hint: &TrackHint) -> TrackResult;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchPolicy {
    LookAhead,
    Follow,
    Fixed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopPolicy {
    StopOnPoor,
    StopOnNoMatch,
    Never,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaskShape {
    Ellipse,
    Rectangle,
}

#[derive(Clone, Debug)]
pub struct AutoTrackSettings {
    pub mask_width: u32,
    pub mask_height: u32,
    pub search_size: u32,
    pub good_match: f32,
    pub possible_match: f32,
    /// Fraction of the new patch blended into the template after a good match.
    pub evolve: f32,
    /// Fraction of the original template blended back in to limit drift.
    pub tether: f32,
    pub shape: MaskShape,
    pub search_policy: SearchPolicy,
    pub stop_policy: StopPolicy,
    /// Search a single row through the target (x-axis only).
    pub one_d: bool,
}

impl Default for AutoTrackSettings {
    fn default() -> Self {
        Self {
            mask_width: 18,
            mask_height: 18,
            search_size: 40,
            good_match: 4.0,
            possible_match: 1.0,
            evolve: 0.20,
            tether: 0.05,
            shape: MaskShape::Ellipse,
            search_policy: SearchPolicy::LookAhead,
            stop_policy: StopPolicy::StopOnPoor,
            one_d: false,
        }
    }
}

impl AutoTrackSettings {
    pub fn normalized(&self) -> Self {
        Self {
            mask_width: self.mask_width.clamp(3, 99),
            mask_height: self.mask_height.clamp(3, 99),
            search_size: self.search_size.clamp(3, 240),
            good_match: self.good_match.max(0.0),
            possible_match: self.possible_match.max(0.0),
            evolve: self.evolve.clamp(0.0, 0.95),
            tether: self.tether.clamp(0.0, 0.95),
            shape: self.shape,
            search_policy: self.search_policy,
            stop_policy: self.stop_policy,
            one_d: self.one_d,
        }
    }
}

struct Patch {
    width: usize,
    height: usize,
    /// Offset from the tracked point to the patch center. Nonzero near edges.
    anchor: (f32, f32),
    rgb: Vec<[f32; 3]>,
    mask: Vec<usize>,
}

impl Patch {
    fn build(
        frame: &FrameData,
        center: (f32, f32),
        width: u32,
        height: u32,
        shape: MaskShape,
    ) -> Result<Self, String> {
        let width = width.clamp(3, 99) as usize;
        let height = height.clamp(3, 99) as usize;
        let img_w = frame.width as usize;
        let img_h = frame.height as usize;
        if img_w < width || img_h < height {
            return Err(format!(
                "mask {width}×{height} is larger than the {}×{} frame",
                img_w, img_h
            ));
        }
        let half_w = (width / 2) as i32;
        let half_h = (height / 2) as i32;
        let cx = (center.0.round() as i32).clamp(half_w, img_w as i32 - 1 - half_w);
        let cy = (center.1.round() as i32).clamp(half_h, img_h as i32 - 1 - half_h);
        let left = cx - half_w;
        let top = cy - half_h;
        let bgra = &frame.bgra;
        let mut rgb = Vec::with_capacity(width * height);
        let mut mask = Vec::new();
        let radius_x = width as f32 / 2.0;
        let radius_y = height as f32 / 2.0;
        for y in 0..height {
            for x in 0..width {
                let px = left as usize + x;
                let py = top as usize + y;
                let p = (py * img_w + px) * 4;
                rgb.push([bgra[p + 2] as f32, bgra[p + 1] as f32, bgra[p] as f32]);
                let dx = (x as f32 + 0.5 - radius_x) / radius_x;
                let dy = (y as f32 + 0.5 - radius_y) / radius_y;
                let inside = match shape {
                    MaskShape::Ellipse => dx * dx + dy * dy <= 1.0,
                    MaskShape::Rectangle => true,
                };
                if inside {
                    mask.push(y * width + x);
                }
            }
        }
        if mask.len() < 4 {
            mask = (0..width * height).collect();
        }
        Ok(Self {
            width,
            height,
            anchor: (center.0 - cx as f32, center.1 - cy as f32),
            rgb,
            mask,
        })
    }

    fn diff_squared(&self, frame: &FrameData, left: i32, top: i32) -> Option<f64> {
        let img_w = frame.width as usize;
        let img_h = frame.height as usize;
        if left < 0
            || top < 0
            || left as usize + self.width > img_w
            || top as usize + self.height > img_h
        {
            return None;
        }
        let bgra = &frame.bgra;
        let mut sum = 0.0f64;
        for &index in &self.mask {
            let x = left as usize + index % self.width;
            let y = top as usize + index / self.width;
            let p = (y * img_w + x) * 4;
            let t = &self.rgb[index];
            let dr = t[0] - bgra[p + 2] as f32;
            let dg = t[1] - bgra[p + 1] as f32;
            let db = t[2] - bgra[p] as f32;
            sum += (dr * dr + dg * dg + db * db) as f64;
        }
        Some(sum / self.mask.len() as f64)
    }
}

/// OSP TemplateMatcher baseline.
pub struct TemplateMatchBackend {
    settings: AutoTrackSettings,
    template: Option<Patch>,
    original_rgb: Vec<[f32; 3]>,
}

impl TemplateMatchBackend {
    pub fn new(settings: AutoTrackSettings) -> Self {
        Self {
            settings: settings.normalized(),
            template: None,
            original_rgb: Vec::new(),
        }
    }

    fn classify(&self, confidence: f32) -> MatchQuality {
        if confidence.is_infinite() || confidence >= self.settings.good_match {
            MatchQuality::Good
        } else if confidence >= self.settings.possible_match {
            MatchQuality::Possible
        } else {
            MatchQuality::Lost
        }
    }

    fn lost(&self, hint: (f32, f32)) -> TrackResult {
        TrackResult {
            position: hint,
            confidence: f32::NAN,
            quality: MatchQuality::Lost,
        }
    }

    fn evolve(&mut self, frame: &FrameData, position: (f32, f32)) {
        let Ok(patch) = Patch::build(
            frame,
            position,
            self.settings.mask_width,
            self.settings.mask_height,
            self.settings.shape,
        ) else {
            return;
        };
        let Some(template) = &mut self.template else {
            return;
        };
        if patch.rgb.len() != template.rgb.len()
            || patch.width != template.width
            || patch.height != template.height
        {
            return;
        }
        let evolve = self.settings.evolve;
        let tether = self.settings.tether;
        for (index, value) in template.rgb.iter_mut().enumerate() {
            for channel in 0..3 {
                let blended = value[channel] * (1.0 - evolve) + patch.rgb[index][channel] * evolve;
                value[channel] =
                    blended * (1.0 - tether) + self.original_rgb[index][channel] * tether;
            }
        }
    }
}

fn quadratic_offset(minus: f64, zero: f64, plus: f64) -> (f64, f64) {
    let a = 0.5 * (plus + minus) - zero;
    let b = 0.5 * (plus - minus);
    if a > 1e-12 {
        let offset = (-b / (2.0 * a)).clamp(-0.5, 0.5);
        let width = (2.0 * zero.max(0.0) / a).sqrt();
        (offset, width)
    } else {
        (0.0, f64::NAN)
    }
}

impl TrackingBackend for TemplateMatchBackend {
    fn name(&self) -> String {
        tr("tracker.backend_osp")
    }

    fn initialize(&mut self, frame: &FrameData, center: (f32, f32)) -> Result<(), String> {
        let patch = Patch::build(
            frame,
            center,
            self.settings.mask_width,
            self.settings.mask_height,
            self.settings.shape,
        )?;
        self.original_rgb = patch.rgb.clone();
        self.template = Some(patch);
        Ok(())
    }

    fn locate(&mut self, frame: &FrameData, hint: &TrackHint) -> TrackResult {
        let Some(template) = &self.template else {
            return self.lost(hint.center);
        };
        let search = self.settings.search_size.max(3) as i32;
        let half = search / 2;
        let base_left =
            (hint.center.0 - template.anchor.0).round() as i32 - template.width as i32 / 2;
        let base_top =
            (hint.center.1 - template.anchor.1).round() as i32 - template.height as i32 / 2;

        let mut min_score = f64::INFINITY;
        let mut sum_score = 0.0f64;
        let mut count = 0usize;
        let mut best = (0i32, 0i32);
        let y_offsets = if self.settings.one_d {
            0..=0
        } else {
            -half..=half
        };
        for oy in y_offsets {
            for ox in -half..=half {
                let left = base_left + ox;
                let top = base_top + oy;
                let Some(score) = template.diff_squared(frame, left, top) else {
                    continue;
                };
                sum_score += score;
                count += 1;
                if score < min_score {
                    min_score = score;
                    best = (left, top);
                }
            }
        }
        if count == 0 || !min_score.is_finite() {
            return self.lost(hint.center);
        }

        let average = sum_score / count as f64;
        let mut confidence = if min_score < 1.0e-6 {
            if average < 1.0e-6 {
                0.0
            } else {
                f32::INFINITY
            }
        } else {
            (average / min_score - 1.0) as f32
        };

        let center_score = |dx: i32, dy: i32| {
            template
                .diff_squared(frame, best.0 + dx, best.1 + dy)
                .unwrap_or(min_score)
        };
        let (dx, width_x) = quadratic_offset(center_score(-1, 0), min_score, center_score(1, 0));
        let (dy, width_y) = quadratic_offset(center_score(0, -1), min_score, center_score(0, 1));
        let width = if width_x.is_finite() && width_y.is_finite() {
            (width_x + width_y) * 0.5
        } else {
            f64::NAN
        };
        if width.is_finite() && width > 1.0 {
            confidence /= width as f32;
        }

        let position = (
            best.0 as f32 + template.width as f32 / 2.0 + dx as f32 + template.anchor.0,
            if self.settings.one_d {
                hint.center.1
            } else {
                best.1 as f32 + template.height as f32 / 2.0 + dy as f32 + template.anchor.1
            },
        );
        let quality = self.classify(confidence);
        if quality == MatchQuality::Good {
            self.evolve(frame, position);
        }
        TrackResult {
            position: (
                position.0.clamp(0.0, frame.width.saturating_sub(1) as f32),
                position.1.clamp(0.0, frame.height.saturating_sub(1) as f32),
            ),
            confidence,
            quality,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutoTrackEnd {
    Finished,
    Lost,
    Stopped,
}

impl AutoTrackEnd {
    pub fn label(self) -> String {
        match self {
            Self::Finished => tr("tracker.end_finished"),
            Self::Lost => tr("tracker.end_lost"),
            Self::Stopped => tr("tracker.end_stopped"),
        }
    }
}

#[derive(Debug)]
pub enum AutoTrackMessage {
    Point {
        frame: usize,
        position: (f32, f32),
        hint: (f32, f32),
        confidence: f32,
    },
    Miss {
        frame: usize,
        position: (f32, f32),
        hint: (f32, f32),
        confidence: f32,
        quality: MatchQuality,
    },
    Done {
        end: AutoTrackEnd,
        frame: usize,
        confidence: f32,
        points: usize,
    },
    Error(String),
}

pub struct AutoTrackRequest {
    pub source: FrameSource,
    pub backend: Box<dyn TrackingBackend>,
    pub start_frame: usize,
    pub end_frame: usize,
    pub start_position: (f32, f32),
    pub step: usize,
    pub search_policy: SearchPolicy,
    pub stop_policy: StopPolicy,
    pub single_step: bool,
}

/// Runs the backend on a dedicated thread. Decoding uses FrameReader's own
/// FFmpeg context, so UI playback stays independent.
pub fn spawn_auto_track(
    request: AutoTrackRequest,
    tx: Sender<AutoTrackMessage>,
    stop: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
) -> Result<JoinHandle<()>, String> {
    thread::Builder::new()
        .name("autotracker".into())
        .spawn(move || {
            let result = run_auto_track(request, &tx, &stop, &paused);
            if let Err(message) = result {
                let _ = tx.send(AutoTrackMessage::Error(message));
            }
        })
        .map_err(|err| format!("spawn autotracker: {err}"))
}

fn run_auto_track(
    request: AutoTrackRequest,
    tx: &Sender<AutoTrackMessage>,
    stop: &AtomicBool,
    paused: &AtomicBool,
) -> Result<(), String> {
    let mut reader = request.source.reader()?;
    let mut backend = request.backend;
    let start = reader.read(request.start_frame)?;
    backend.initialize(&start, request.start_position)?;

    let end_frame = request.end_frame;
    let step = request.step.max(1);
    let mut history: VecDeque<(f32, f32)> = VecDeque::from([request.start_position]);
    let mut frame = request.start_frame;
    let mut points = 0usize;
    let trace = std::env::var_os("PROBE_TRACK_TRACE").is_some();

    loop {
        while paused.load(Ordering::Relaxed) && !stop.load(Ordering::Relaxed) {
            thread::sleep(std::time::Duration::from_millis(25));
        }
        if stop.load(Ordering::Relaxed) {
            let _ = tx.send(AutoTrackMessage::Done {
                end: AutoTrackEnd::Stopped,
                frame,
                confidence: f32::NAN,
                points,
            });
            return Ok(());
        }
        if frame >= end_frame {
            let _ = tx.send(AutoTrackMessage::Done {
                end: AutoTrackEnd::Finished,
                frame,
                confidence: f32::NAN,
                points,
            });
            return Ok(());
        }
        let next = (frame + step).min(end_frame);
        let data = reader.read(next)?;
        let predicted = match request.search_policy {
            SearchPolicy::LookAhead => predict(&history, data.width, data.height),
            SearchPolicy::Follow => {
                let last = history.back().copied().unwrap_or(request.start_position);
                (
                    last.0.clamp(0.0, data.width.saturating_sub(1) as f32),
                    last.1.clamp(0.0, data.height.saturating_sub(1) as f32),
                )
            }
            SearchPolicy::Fixed => {
                let fixed = request.start_position;
                (
                    fixed.0.clamp(0.0, data.width.saturating_sub(1) as f32),
                    fixed.1.clamp(0.0, data.height.saturating_sub(1) as f32),
                )
            }
        };
        let hint = TrackHint { center: predicted };
        let result = backend.locate(&data, &hint);
        if trace {
            eprintln!(
                "[track] frame {next} hint ({:.1},{:.1}) -> ({:.2},{:.2}) conf {:.2} {:?}",
                predicted.0,
                predicted.1,
                result.position.0,
                result.position.1,
                result.confidence,
                result.quality
            );
        }
        if result.quality == MatchQuality::Good {
            if tx
                .send(AutoTrackMessage::Point {
                    frame: next,
                    position: result.position,
                    hint: predicted,
                    confidence: result.confidence,
                })
                .is_err()
            {
                return Ok(());
            }
            points += 1;
            history.push_back(result.position);
            while history.len() > 4 {
                history.pop_front();
            }
            frame = next;
            if request.single_step {
                let _ = tx.send(AutoTrackMessage::Done {
                    end: AutoTrackEnd::Finished,
                    frame,
                    confidence: result.confidence,
                    points,
                });
                return Ok(());
            }
            continue;
        }

        if tx
            .send(AutoTrackMessage::Miss {
                frame: next,
                position: result.position,
                hint: predicted,
                confidence: result.confidence,
                quality: result.quality,
            })
            .is_err()
        {
            return Ok(());
        }
        if request.single_step
            || request.stop_policy == StopPolicy::StopOnPoor
            || (request.stop_policy == StopPolicy::StopOnNoMatch
                && result.quality == MatchQuality::Lost)
        {
            let _ = tx.send(AutoTrackMessage::Done {
                end: AutoTrackEnd::Lost,
                frame: next,
                confidence: result.confidence,
                points,
            });
            return Ok(());
        }
        if result.quality == MatchQuality::Possible {
            history.push_back(result.position);
            while history.len() > 4 {
                history.pop_front();
            }
        }
        frame = next;
    }
}

fn predict(history: &VecDeque<(f32, f32)>, width: u32, height: u32) -> (f32, f32) {
    let Some(&last) = history.back() else {
        return (0.0, 0.0);
    };
    let mut prediction = last;
    let len = history.len();
    if len >= 3 {
        let p0 = history[len - 3];
        let p1 = history[len - 2];
        let p2 = last;
        let accel = (
            3.0 * p2.0 - 3.0 * p1.0 + p0.0,
            3.0 * p2.1 - 3.0 * p1.1 + p0.1,
        );
        let velocity_jump = (accel.0 - p2.0, accel.1 - p2.1);
        let last_velocity = (p2.0 - p1.0, p2.1 - p1.1);
        let plausible = velocity_jump.0.abs() <= 2.0 * last_velocity.0.abs() + 8.0
            && velocity_jump.1.abs() <= 2.0 * last_velocity.1.abs() + 8.0;
        if plausible {
            prediction = accel;
        }
    }
    if len >= 2 && prediction == last {
        let previous = history[len - 2];
        prediction = (2.0 * last.0 - previous.0, 2.0 * last.1 - previous.1);
    }
    (
        prediction.0.clamp(0.0, width.saturating_sub(1) as f32),
        prediction.1.clamp(0.0, height.saturating_sub(1) as f32),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame_with_target(width: u32, height: u32, cx: i32, cy: i32) -> FrameData {
        let mut bgra = vec![50u8; (width * height * 4) as usize];
        for pixel in bgra.chunks_exact_mut(4) {
            pixel[3] = 255;
        }
        let mut plot = |x: i32, y: i32, value: u8| {
            if x >= 0 && y >= 0 && x < width as i32 && y < height as i32 {
                let p = ((y as u32 * width + x as u32) * 4) as usize;
                bgra[p] = value;
                bgra[p + 1] = value;
                bgra[p + 2] = value;
            }
        };
        for dy in -4i32..=4 {
            for dx in -4i32..=4 {
                if dx.abs() + dy.abs() <= 4 {
                    let value = if (dx + dy) % 2 == 0 { 240 } else { 120 };
                    plot(cx + dx, cy + dy, value);
                }
            }
        }
        plot(cx, cy, 255);
        FrameData {
            index: 0,
            width,
            height,
            bgra,
        }
    }

    fn hint(center: (f32, f32)) -> TrackHint {
        TrackHint { center }
    }

    #[test]
    fn template_backend_tracks_translation() {
        let first = frame_with_target(96, 64, 30, 25);
        let second = frame_with_target(96, 64, 36, 30);
        let mut backend = TemplateMatchBackend::new(AutoTrackSettings::default());
        backend.initialize(&first, (30.0, 25.0)).unwrap();
        let result = backend.locate(&second, &hint((33.0, 28.0)));
        assert_eq!(result.quality, MatchQuality::Good);
        assert!(
            (result.position.0 - 36.0).abs() < 0.6,
            "{:?}",
            result.position
        );
        assert!(
            (result.position.1 - 30.0).abs() < 0.6,
            "{:?}",
            result.position
        );
    }

    #[test]
    fn one_dimensional_search_keeps_vertical_position() {
        let first = frame_with_target(96, 64, 30, 25);
        let second = frame_with_target(96, 64, 38, 25);
        let settings = AutoTrackSettings {
            one_d: true,
            search_size: 64,
            ..AutoTrackSettings::default()
        };
        let mut backend = TemplateMatchBackend::new(settings);
        backend.initialize(&first, (30.0, 25.0)).unwrap();
        let result = backend.locate(&second, &hint((30.0, 25.0)));
        assert!(
            (result.position.1 - 25.0).abs() < 0.6,
            "{:?}",
            result.position
        );
        assert!(
            (result.position.0 - 38.0).abs() < 0.8,
            "{:?}",
            result.position
        );
    }

    #[test]
    fn template_backend_rejects_flat_frame() {
        let first = frame_with_target(64, 64, 32, 32);
        let mut backend = TemplateMatchBackend::new(AutoTrackSettings::default());
        backend.initialize(&first, (32.0, 32.0)).unwrap();
        let flat = FrameData {
            index: 1,
            width: 64,
            height: 64,
            bgra: vec![50u8; 64 * 64 * 4],
        };
        let result = backend.locate(&flat, &hint((32.0, 32.0)));
        assert_ne!(result.quality, MatchQuality::Good);
    }

    #[test]
    fn prediction_uses_acceleration_and_velocity() {
        let accelerating = VecDeque::from([(10.0, 10.0), (20.0, 10.0), (32.0, 10.0)]);
        let predicted = predict(&accelerating, 100, 100);
        assert!((predicted.0 - 46.0).abs() < 1e-3);

        let constant = VecDeque::from([(20.0, 10.0), (30.0, 10.0)]);
        let predicted = predict(&constant, 100, 100);
        assert!((predicted.0 - 40.0).abs() < 1e-3);
    }
}
