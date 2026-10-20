//! FFmpeg-backed frame service, modelled on Tracker's XuggleVideo:
//! index packets once, then decode frames on demand (forward decode when
//! possible, keyframe seek otherwise) instead of letting a decoder race the UI.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::{Arc, Condvar, Mutex, Once};
use std::thread::{self, JoinHandle};
use std::time::Instant;

use ffmpeg::format::Pixel;
use ffmpeg::software::scaling::{context::Context as Sws, flag::Flags};
use ffmpeg::util::frame::Video as VideoFrame;
use ffmpeg_the_third as ffmpeg;

static INIT: Once = Once::new();

pub fn init() {
    INIT.call_once(|| {
        let _ = ffmpeg::init();
    });
}

#[derive(Clone)]
pub struct VideoMeta {
    pub path: PathBuf,
    pub fps: f32,
    pub width: u32,
    pub height: u32,
    pub frames: usize,
    /// Presentation time of each frame, in seconds, relative to frame 0.
    pub frame_times: Vec<f64>,
}

impl VideoMeta {
    pub fn frame_at_or_before(&self, time: f64) -> usize {
        match self
            .frame_times
            .binary_search_by(|t| t.partial_cmp(&time).unwrap_or(std::cmp::Ordering::Less))
        {
            Ok(i) => i,
            Err(0) => 0,
            Err(i) => i - 1,
        }
    }
}

#[derive(Clone)]
pub struct FrameData {
    pub index: usize,
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

struct Index {
    /// Presentation timestamps in stream time base, sorted.
    raw_pts: Vec<i64>,
    keyframes: Vec<bool>,
    pts_to_frame: HashMap<i64, usize>,
    tb_num: i32,
    tb_den: i32,
}

impl Index {
    fn seek_us(&self, frame: usize) -> i64 {
        let pts = self.raw_pts[frame];
        let us = pts as f64 * (self.tb_num as f64 / self.tb_den as f64) * 1_000_000.0;
        us.max(0.0) as i64
    }

    fn keyframe_for(&self, frame: usize) -> usize {
        (0..=frame).rev().find(|&i| self.keyframes[i]).unwrap_or(0)
    }

    fn frame_for_pts(&self, pts: Option<i64>) -> Option<usize> {
        let pts = pts?;
        if let Some(&frame) = self.pts_to_frame.get(&pts) {
            return Some(frame);
        }
        match self.raw_pts.binary_search(&pts) {
            Ok(i) => Some(i),
            Err(0) => Some(0),
            Err(i) => Some(i - 1),
        }
    }
}

struct RequestSlot {
    target: Mutex<Option<usize>>,
    ready: Condvar,
}

impl RequestSlot {
    fn new() -> Self {
        Self {
            target: Mutex::new(None),
            ready: Condvar::new(),
        }
    }

    fn set(&self, frame: usize) {
        *self.target.lock().unwrap() = Some(frame);
        self.ready.notify_all();
    }

    fn take(&self) -> Option<usize> {
        self.target.lock().unwrap().take()
    }

    fn wait(&self, stop: &AtomicBool) -> Option<usize> {
        let mut target = self.target.lock().unwrap();
        loop {
            if stop.load(Ordering::Relaxed) {
                return None;
            }
            if let Some(frame) = target.take() {
                return Some(frame);
            }
            target = self.ready.wait(target).unwrap();
        }
    }

    fn wake(&self) {
        self.ready.notify_all();
    }
}

pub struct VideoStream {
    pub meta: VideoMeta,
    index: Arc<Index>,
    request: Arc<RequestSlot>,
    rx: Receiver<FrameData>,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl VideoStream {
    pub fn open(path: PathBuf, _start: usize) -> Result<Self, String> {
        Self::open_with_progress(path, _start, None)
    }

    pub fn open_with_progress(
        path: PathBuf,
        _start: usize,
        progress: Option<Arc<AtomicU32>>,
    ) -> Result<Self, String> {
        init();
        let (meta, index) = build_index(&path, progress.as_deref())?;
        let index = Arc::new(index);
        let request = Arc::new(RequestSlot::new());
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, rx) = sync_channel(4);

        let handle = {
            let meta = meta.clone();
            let index = index.clone();
            let request = request.clone();
            let stop = stop.clone();
            thread::Builder::new()
                .name("ffmpeg-frames".into())
                .spawn(move || {
                    if let Err(err) = decode_loop(&meta, &index, &request, &tx, &stop) {
                        eprintln!("ffmpeg decode error: {err}");
                    }
                })
                .map_err(|e| format!("spawn decoder: {e}"))?
        };

        Ok(Self {
            meta,
            index,
            request,
            rx,
            stop,
            handle: Some(handle),
        })
    }

    /// Shareable source for background frame-by-frame reading (AutoTracker).
    pub fn frame_source(&self) -> FrameSource {
        FrameSource {
            meta: self.meta.clone(),
            index: self.index.clone(),
        }
    }

    pub fn request(&self, frame: usize) {
        if self.meta.frames == 0 {
            return;
        }
        self.request.set(frame.min(self.meta.frames - 1));
    }

    pub fn try_recv(&self) -> Option<FrameData> {
        self.rx.try_recv().ok()
    }
}

impl Drop for VideoStream {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.request.wake();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

#[derive(Clone)]
pub struct FrameSource {
    meta: VideoMeta,
    index: Arc<Index>,
}

impl FrameSource {
    /// Opens an independent decoder so background analysis never competes with
    /// the UI decode thread.
    pub fn reader(&self) -> Result<FrameReader, String> {
        init();
        let input = ffmpeg::format::input(&self.meta.path)
            .map_err(|e| format!("open reader {:?}: {e}", self.meta.path))?;
        let (stream_index, decoder) = {
            let stream = input
                .streams()
                .best(ffmpeg::media::Type::Video)
                .ok_or("no video stream")?;
            let mut context = ffmpeg::codec::context::Context::from_parameters(stream.parameters())
                .map_err(|e| format!("reader parameters: {e}"))?;
            context.set_threading(ffmpeg::codec::threading::Config::kind(
                ffmpeg::codec::threading::Type::Frame,
            ));
            let decoder = context
                .decoder()
                .video()
                .map_err(|e| format!("reader video decoder: {e}"))?;
            (stream.index(), decoder)
        };
        Ok(FrameReader {
            meta: self.meta.clone(),
            index: self.index.clone(),
            input,
            stream_index,
            decoder,
            scaler: None,
            last: None,
        })
    }
}

/// Blocking sequential frame reader used by background analysis.
pub struct FrameReader {
    meta: VideoMeta,
    index: Arc<Index>,
    input: ffmpeg::format::context::Input,
    stream_index: usize,
    decoder: ffmpeg::codec::decoder::Video,
    scaler: Option<Sws>,
    last: Option<usize>,
}

impl FrameReader {
    pub fn read(&mut self, frame: usize) -> Result<FrameData, String> {
        if self.meta.frames == 0 {
            return Err("video has no frames".into());
        }
        let target = frame.min(self.meta.frames - 1);
        let seek = match self.last {
            None => true,
            Some(last) => target < last || target > last + 30,
        };
        if seek {
            let keyframe = self.index.keyframe_for(target);
            self.input
                .seek(self.index.seek_us(keyframe), ..)
                .map_err(|e| format!("reader seek frame {target}: {e}"))?;
            self.decoder.flush();
            self.last = None;
        }

        let mut packets = self.input.packets();
        loop {
            loop {
                let mut picture = VideoFrame::empty();
                match self.decoder.receive_frame(&mut picture) {
                    Ok(()) => {
                        let Some(index) = self.index.frame_for_pts(picture.pts()) else {
                            continue;
                        };
                        self.last = Some(index);
                        if index == target {
                            return to_bgra(&picture, &mut self.scaler, index);
                        }
                        if index > target {
                            return Err(format!("reader skipped frame {target} (decoded {index})"));
                        }
                    }
                    Err(ffmpeg::Error::Other { errno }) if errno == libc::EAGAIN => break,
                    Err(ffmpeg::Error::Eof) => {
                        return Err(format!("reader reached end before frame {target}"));
                    }
                    Err(err) => return Err(format!("reader decode frame {target}: {err}")),
                }
            }

            match packets.next() {
                Some(Ok((stream, packet))) => {
                    if stream.index() != self.stream_index {
                        continue;
                    }
                    self.decoder
                        .send_packet(&packet)
                        .map_err(|e| format!("reader send near frame {target}: {e}"))?;
                }
                Some(Err(err)) => return Err(format!("reader read packet near {target}: {err}")),
                None => {
                    self.decoder.send_eof().ok();
                    loop {
                        let mut picture = VideoFrame::empty();
                        match self.decoder.receive_frame(&mut picture) {
                            Ok(()) => {
                                let Some(index) = self.index.frame_for_pts(picture.pts()) else {
                                    continue;
                                };
                                self.last = Some(index);
                                if index == target {
                                    return to_bgra(&mut picture, &mut self.scaler, index);
                                }
                            }
                            _ => break,
                        }
                    }
                    return Err(format!("reader frame {target} not found at end of stream"));
                }
            }
        }
    }
}

fn build_index(path: &Path, progress: Option<&AtomicU32>) -> Result<(VideoMeta, Index), String> {
    let mut input = ffmpeg::format::input(path).map_err(|e| format!("open {path:?}: {e}"))?;
    let (stream_index, time_base, rate, width, height, frames_hint) = {
        let stream = input
            .streams()
            .best(ffmpeg::media::Type::Video)
            .ok_or("no video stream")?;
        let decoder = ffmpeg::codec::context::Context::from_parameters(stream.parameters())
            .map_err(|e| format!("decoder parameters: {e}"))?
            .decoder()
            .video()
            .map_err(|e| format!("video decoder: {e}"))?;
        (
            stream.index(),
            stream.time_base(),
            stream.avg_frame_rate(),
            decoder.width(),
            decoder.height(),
            stream.frames(),
        )
    };
    let tb_f = time_base.numerator() as f64 / time_base.denominator().max(1) as f64;
    let duration_us = input.duration();

    let mut packets: Vec<(i64, bool)> = Vec::new();
    let mut last_pts = i64::MIN;
    for item in input.packets() {
        let (stream, packet) = item.map_err(|e| format!("read packet: {e}"))?;
        if stream.index() != stream_index {
            continue;
        }
        let pts = packet
            .pts()
            .or_else(|| packet.dts())
            .unwrap_or_else(|| last_pts.saturating_add(1));
        last_pts = pts;
        packets.push((pts, packet.is_key()));

        if let Some(progress) = progress {
            if packets.len() % 16 == 0 {
                let fraction = if frames_hint > 0 {
                    packets.len() as f32 / frames_hint as f32
                } else if duration_us > 0 {
                    (pts as f64 * tb_f * 1_000_000.0 / duration_us as f64) as f32
                } else {
                    0.0
                };
                progress.store((fraction.clamp(0.0, 1.0) * 100.0) as u32, Ordering::Relaxed);
            }
        }
    }
    if packets.is_empty() {
        return Err("no video packets".into());
    }
    if let Some(progress) = progress {
        progress.store(100, Ordering::Relaxed);
    }

    packets.sort_by_key(|(pts, _)| *pts);
    let mut frames: Vec<(i64, bool)> = Vec::with_capacity(packets.len());
    for (pts, key) in packets {
        if let Some(last) = frames.last_mut() {
            if last.0 == pts {
                last.1 |= key;
                continue;
            }
        }
        frames.push((pts, key));
    }

    let tb_num = time_base.numerator();
    let tb_den = time_base.denominator().max(1);
    let tb = tb_num as f64 / tb_den as f64;
    let first_pts = frames[0].0;
    let frame_times: Vec<f64> = frames
        .iter()
        .map(|(pts, _)| (*pts - first_pts) as f64 * tb)
        .collect();

    let avg = rate.numerator() as f32 / rate.denominator().max(1) as f32;
    let fps = if avg.is_finite() && avg > 0.0 {
        avg
    } else if frames.len() > 1 {
        let span = frame_times.last().copied().unwrap_or(0.0);
        if span > 0.0 {
            ((frames.len() - 1) as f64 / span) as f32
        } else {
            30.0
        }
    } else {
        30.0
    };

    let raw_pts: Vec<i64> = frames.iter().map(|(pts, _)| *pts).collect();
    let keyframes: Vec<bool> = frames.iter().map(|(_, key)| *key).collect();
    let pts_to_frame = raw_pts
        .iter()
        .enumerate()
        .map(|(i, pts)| (*pts, i))
        .collect();

    let meta = VideoMeta {
        path: path.to_path_buf(),
        fps,
        width,
        height,
        frames: raw_pts.len(),
        frame_times,
    };
    let index = Index {
        raw_pts,
        keyframes,
        pts_to_frame,
        tb_num,
        tb_den,
    };
    if std::env::var_os("PROBE_TRACE").is_some() {
        let keys = index.keyframes.iter().filter(|k| **k).count();
        eprintln!(
            "[index] {}x{} {:.3} fps, {} frames, {} keyframes, first {:.3}s last {:.3}s",
            meta.width,
            meta.height,
            meta.fps,
            meta.frames,
            keys,
            meta.frame_times.first().copied().unwrap_or(0.0),
            meta.frame_times.last().copied().unwrap_or(0.0)
        );
    }
    Ok((meta, index))
}

fn decode_loop(
    meta: &VideoMeta,
    index: &Index,
    request: &RequestSlot,
    tx: &SyncSender<FrameData>,
    stop: &AtomicBool,
) -> Result<(), String> {
    let mut input = ffmpeg::format::input(&meta.path).map_err(|e| format!("open decoder: {e}"))?;
    let (stream_index, mut decoder) = {
        let stream = input
            .streams()
            .best(ffmpeg::media::Type::Video)
            .ok_or("no video stream")?;
        let mut context = ffmpeg::codec::context::Context::from_parameters(stream.parameters())
            .map_err(|e| format!("decoder parameters: {e}"))?;
        context.set_threading(ffmpeg::codec::threading::Config::kind(
            ffmpeg::codec::threading::Type::Frame,
        ));
        let decoder = context
            .decoder()
            .video()
            .map_err(|e| format!("video decoder: {e}"))?;
        (stream.index(), decoder)
    };
    if std::env::var_os("PROBE_TRACE").is_some() {
        let threading = decoder.threading();
        eprintln!(
            "[dec] threading {:?} count {}",
            threading.kind, threading.count
        );
    }

    let mut scaler: Option<Sws> = None;
    let mut cache: VecDeque<FrameData> = VecDeque::new();
    let mut last_decoded: Option<usize> = None;
    let mut need_seek = true;
    let trace = std::env::var_os("PROBE_TRACE").is_some();
    let started = Instant::now();

    while let Some(target) = request.wait(stop) {
        let target = target.min(meta.frames - 1);
        if trace {
            eprintln!(
                "[dec] t={}ms target {}",
                started.elapsed().as_millis(),
                target
            );
        }

        if let Some(frame) = cache.iter().find(|f| f.index == target) {
            let _ = tx.try_send(frame.clone());
            if trace {
                eprintln!("[dec] cache hit {target}");
            }
            continue;
        }

        let seek = need_seek
            || match last_decoded {
                None => true,
                Some(last) => target < last || target > last + 30,
            };
        if seek {
            let keyframe = index.keyframe_for(target);
            if trace {
                eprintln!("[dec] seek keyframe {keyframe} for {target}");
            }
            input
                .seek(index.seek_us(keyframe), ..)
                .map_err(|e| format!("seek frame {target}: {e}"))?;
            decoder.flush();
            cache.clear();
            last_decoded = None;
        }
        need_seek = false;

        let mut wanted = target;
        let mut sent = false;
        let mut aborted = false;

        {
            let mut frames = input.packets();
            while let Some(item) = frames.next() {
                if stop.load(Ordering::Relaxed) {
                    return Ok(());
                }
                if let Some(new_target) = request.take() {
                    let new_target = new_target.min(meta.frames - 1);
                    if new_target != wanted {
                        if trace {
                            eprintln!("[dec] abort {wanted} -> {new_target}");
                        }
                        wanted = new_target;
                        aborted = true;
                        break;
                    }
                }

                let (stream, packet) =
                    item.map_err(|e| format!("read packet near frame {target}: {e}"))?;
                if stream.index() != stream_index {
                    continue;
                }
                let send_started = Instant::now();
                decoder
                    .send_packet(&packet)
                    .map_err(|e| format!("send packet near frame {target}: {e}"))?;

                loop {
                    let mut picture = VideoFrame::empty();
                    match decoder.receive_frame(&mut picture) {
                        Ok(()) => {
                            if let Some(frame_index) = index.frame_for_pts(picture.pts()) {
                                let convert_started = Instant::now();
                                let data = to_bgra(&picture, &mut scaler, frame_index)?;
                                if trace {
                                    eprintln!(
                                        "[dec] decoded {frame_index} decode {}ms convert {}ms",
                                        send_started.elapsed().as_millis(),
                                        convert_started.elapsed().as_millis()
                                    );
                                }
                                last_decoded = Some(frame_index);
                                cache.push_back(data.clone());
                                while cache.len() > 4 {
                                    cache.pop_front();
                                }
                                if frame_index == wanted {
                                    let _ = tx.try_send(data);
                                    sent = true;
                                    if trace {
                                        eprintln!(
                                            "[dec] sent {frame_index} t={}ms",
                                            started.elapsed().as_millis()
                                        );
                                    }
                                    break;
                                }
                            }
                        }
                        Err(ffmpeg::Error::Other { errno }) if errno == libc::EAGAIN => break,
                        Err(ffmpeg::Error::Eof) => break,
                        Err(err) => return Err(format!("decode near frame {target}: {err}")),
                    }
                }
                if sent || aborted {
                    break;
                }
            }
        }

        if sent || aborted {
            continue;
        }

        // End of stream: drain delayed frames, then wait for a new request.
        decoder.send_eof().ok();
        loop {
            let mut picture = VideoFrame::empty();
            match decoder.receive_frame(&mut picture) {
                Ok(()) => {
                    if let Some(frame_index) = index.frame_for_pts(picture.pts()) {
                        let data = to_bgra(&picture, &mut scaler, frame_index)?;
                        last_decoded = Some(frame_index);
                        cache.push_back(data.clone());
                        while cache.len() > 4 {
                            cache.pop_front();
                        }
                        if frame_index == wanted {
                            let _ = tx.try_send(data);
                            sent = true;
                            if trace {
                                eprintln!(
                                    "[dec] sent {frame_index} (eos) t={}ms",
                                    started.elapsed().as_millis()
                                );
                            }
                        }
                    }
                }
                _ => break,
            }
        }
        if !sent {
            eprintln!("ffmpeg: frame {wanted} not found (end of stream)");
            need_seek = true;
        }
    }
    Ok(())
}

fn to_bgra(
    picture: &VideoFrame,
    scaler: &mut Option<Sws>,
    frame_index: usize,
) -> Result<FrameData, String> {
    let width = picture.width();
    let height = picture.height();
    if scaler.is_none() {
        *scaler = Some(
            Sws::get(
                picture.format(),
                width,
                height,
                Pixel::BGRA,
                width,
                height,
                Flags::BILINEAR,
            )
            .map_err(|e| format!("swscale setup: {e}"))?,
        );
    }
    let mut bgra_frame = VideoFrame::empty();
    scaler
        .as_mut()
        .unwrap()
        .run(picture, &mut bgra_frame)
        .map_err(|e| format!("swscale: {e}"))?;

    let row = width as usize * 4;
    let stride = bgra_frame.stride(0);
    let data = bgra_frame.data(0);
    let mut bgra = Vec::with_capacity(row * height as usize);
    if stride == row {
        bgra.extend_from_slice(&data[..row * height as usize]);
    } else {
        for y in 0..height as usize {
            let start = y * stride;
            bgra.extend_from_slice(&data[start..start + row]);
        }
    }
    Ok(FrameData {
        index: frame_index,
        width,
        height,
        bgra,
    })
}
