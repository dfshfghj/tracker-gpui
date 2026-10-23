//! Background MP4 encoding; no dependency on an ffmpeg executable or GPU.
use crate::{
    export_overlay::{Renderer, Scene},
    video::{self, FrameData, FrameSource},
};
use ffmpeg_the_third as ffmpeg;
use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

#[derive(Clone, Debug)]
pub(crate) struct Options {
    pub first: usize,
    pub last: usize,
    pub scale: f32,
    pub fps: Option<f64>,
    pub bitrate: usize,
    pub audio: bool,
}
#[derive(Clone, Debug)]
pub(crate) struct Sample {
    pub frame: usize,
    pub pts: i64,
    pub duration: i64,
}
pub(crate) const TIME_BASE: ffmpeg::Rational = ffmpeg::Rational(1, 60000);
impl Options {
    pub fn dimensions(&self, source: &FrameSource) -> (u32, u32) {
        let m = source.meta();
        let even = |v: u32| (((v as f32 * self.scale).round() as u32) / 2 * 2).max(2);
        (even(m.width), even(m.height))
    }
    pub fn validate(&self, source: &FrameSource) -> Result<(), String> {
        let m = source.meta();
        if self.first > self.last
            || self.last >= m.frames
            || m.frame_times.len() != m.frames
            || !self.scale.is_finite()
            || self.scale <= 0.
            || self.scale > 1.
            || self.bitrate < 100000
            || self.bitrate > 100000000
        {
            return Err("invalid export parameters".into());
        }
        if self
            .fps
            .is_some_and(|fps| !fps.is_finite() || fps < 1. || fps > 240.)
        {
            return Err("invalid frame rate".into());
        }
        Ok(())
    }
    pub fn samples(&self, source: &FrameSource) -> Result<Vec<Sample>, String> {
        self.validate(source)?;
        let m = source.meta();
        let start = m.frame_times[self.first];
        let finish = m
            .frame_times
            .get(self.last + 1)
            .copied()
            .unwrap_or(m.duration);
        let ticks = |t: f64| (t * 60000.).round() as i64;
        let mut times = Vec::new();
        if let Some(fps) = self.fps {
            if !fps.is_finite() || fps < 1. || fps > 240. {
                return Err("invalid frame rate".into());
            }
            let count = ((finish - start) * fps - 1e-8).ceil().max(1.) as usize;
            for i in 0..count {
                let time = start + i as f64 / fps;
                times.push((
                    m.frame_at_or_before(time).clamp(self.first, self.last),
                    ticks(time - start),
                ));
            }
        } else {
            for n in self.first..=self.last {
                times.push((n, ticks(m.frame_times[n] - start)));
            }
        }
        if times.windows(2).any(|w| w[0].1 >= w[1].1) {
            return Err("source timestamps are not strictly increasing".into());
        }
        let end = ticks(finish - start);
        Ok(times
            .iter()
            .enumerate()
            .map(|(i, (frame, pts))| Sample {
                frame: *frame,
                pts: *pts,
                duration: times
                    .get(i + 1)
                    .map(|x| x.1)
                    .unwrap_or(end)
                    .saturating_sub(*pts)
                    .max(1),
            })
            .collect())
    }
}
pub(crate) struct Progress {
    pub done: AtomicUsize,
    pub total: AtomicUsize,
    pub cancel: AtomicBool,
}
impl Default for Progress {
    fn default() -> Self {
        Self {
            done: AtomicUsize::new(0),
            total: AtomicUsize::new(1),
            cancel: AtomicBool::new(false),
        }
    }
}
struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
static NEXT: AtomicUsize = AtomicUsize::new(1);
fn temporary(path: &Path) -> Result<Temporary, String> {
    let directory = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    loop {
        let temp = directory.join(format!(
            ".tracker-export-{}-{}.mp4",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
        {
            Ok(_) => return Ok(Temporary(temp)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        }
    }
}
fn publish(temp: &Path, dest: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::ffi::OsStrExt;
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn MoveFileExW(from: *const u16, to: *const u16, flags: u32) -> i32;
        }
        let from: Vec<_> = temp.as_os_str().encode_wide().chain(Some(0)).collect();
        let to: Vec<_> = dest.as_os_str().encode_wide().chain(Some(0)).collect();
        if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), 1 | 8) } == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::fs::rename(temp, dest).map_err(|e| e.to_string())
    }
}
fn check_cancel(progress: &Progress) -> Result<(), String> {
    if progress.cancel.load(Ordering::Relaxed) {
        Err("cancelled".into())
    } else {
        Ok(())
    }
}

pub(crate) fn export(
    source: &FrameSource,
    scene: &Scene,
    options: &Options,
    path: &Path,
    progress: &Progress,
) -> Result<(), String> {
    video::init();
    if std::fs::canonicalize(path)
        .ok()
        .zip(std::fs::canonicalize(&source.meta().path).ok())
        .is_some_and(|(a, b)| a == b)
    {
        return Err("cannot overwrite the source video".into());
    }
    let samples = options.samples(source)?;
    progress.total.store(samples.len(), Ordering::Relaxed);
    let codec = ffmpeg::encoder::find(ffmpeg::codec::Id::MPEG4)
        .ok_or("FFmpeg MPEG-4 encoder is unavailable")?;
    let temp = temporary(path)?;
    let mut output =
        ffmpeg::format::output_as(&temp.0, "mp4").map_err(|e| format!("MP4 output: {e}"))?;
    let (width, height) = options.dimensions(source);
    let mut encoder = ffmpeg::codec::context::Context::new_with_codec(codec)
        .encoder()
        .video()
        .map_err(|e| e.to_string())?;
    encoder.set_width(width);
    encoder.set_height(height);
    encoder.set_format(ffmpeg::format::Pixel::YUV420P);
    encoder.set_time_base(TIME_BASE);
    encoder.set_frame_rate(Some(ffmpeg::Rational(
        ((options.fps.unwrap_or(source.meta().fps as f64)) * 1000.).round() as i32,
        1000,
    )));
    encoder.set_bit_rate(options.bitrate);
    encoder.set_gop(60);
    encoder.set_max_b_frames(0);
    if output
        .format()
        .flags()
        .contains(ffmpeg::format::flag::Flags::GLOBAL_HEADER)
    {
        encoder.set_flags(ffmpeg::codec::flag::Flags::GLOBAL_HEADER);
    }
    let mut encoder = encoder
        .open_as(codec)
        .map_err(|e| format!("open MPEG-4 encoder: {e}"))?;
    {
        let mut stream = output.add_stream(codec).map_err(|e| e.to_string())?;
        stream.set_time_base(TIME_BASE);
        stream.set_parameters(ffmpeg::codec::Parameters::from(&encoder));
    }
    let mut audio = if options.audio {
        Some(Audio::new(source, &mut output)?)
    } else {
        None
    };
    output.write_header().map_err(|e| {
        format!("MP4 header (try disabling original audio if its codec is unsupported): {e}")
    })?;
    let out_tb = output
        .stream(0)
        .ok_or("missing output video stream")?
        .time_base();
    let mut scaler = ffmpeg::software::scaling::context::Context::get(
        ffmpeg::format::Pixel::BGRA,
        width,
        height,
        ffmpeg::format::Pixel::YUV420P,
        width,
        height,
        ffmpeg::software::scaling::flag::Flags::BILINEAR,
    )
    .map_err(|e| e.to_string())?;
    let renderer = Renderer::new();
    let mut reader = source.reader()?;
    let mut rendered: Option<FrameData> = None;
    for (index, sample) in samples.iter().enumerate() {
        check_cancel(progress)?;
        if rendered
            .as_ref()
            .is_none_or(|frame| frame.index != sample.frame)
        {
            rendered = Some(renderer.frame(scene, &reader.read(sample.frame)?, (width, height))?);
        }
        let rgb = raw_frame(rendered.as_ref().unwrap());
        let mut yuv = ffmpeg::util::frame::Video::empty();
        scaler.run(&rgb, &mut yuv).map_err(|e| e.to_string())?;
        yuv.set_pts(Some(sample.pts));
        encoder
            .send_frame(&yuv)
            .map_err(|e| format!("encode: {e}"))?;
        drain(&mut encoder, &mut output, out_tb, &samples)?;
        progress.done.store(index + 1, Ordering::Relaxed);
    }
    encoder.send_eof().map_err(|e| e.to_string())?;
    drain(&mut encoder, &mut output, out_tb, &samples)?;
    if let Some(audio) = audio.as_mut() {
        audio.copy(source, options, &samples, &mut output, progress)?;
    }
    check_cancel(progress)?;
    output
        .write_trailer()
        .map_err(|e| format!("finish MP4: {e}"))?;
    drop(output);
    check_cancel(progress)?;
    publish(&temp.0, path)?;
    Ok(())
}
fn raw_frame(frame: &FrameData) -> ffmpeg::util::frame::Video {
    let mut raw =
        ffmpeg::util::frame::Video::new(ffmpeg::format::Pixel::BGRA, frame.width, frame.height);
    let stride = raw.stride(0);
    let width = frame.width as usize * 4;
    for (y, row) in frame.bgra.chunks_exact(width).enumerate() {
        raw.data_mut(0)[y * stride..y * stride + width].copy_from_slice(row);
    }
    raw
}
fn drain(
    encoder: &mut ffmpeg::codec::encoder::video::Encoder,
    output: &mut ffmpeg::format::context::Output,
    tb: ffmpeg::Rational,
    samples: &[Sample],
) -> Result<(), String> {
    loop {
        let mut packet = ffmpeg::Packet::empty();
        match encoder.receive_packet(&mut packet) {
            Ok(()) => {
                if let Some(pts) = packet.pts() {
                    if let Ok(i) = samples.binary_search_by_key(&pts, |s| s.pts) {
                        packet.set_duration(samples[i].duration);
                    }
                }
                packet.set_stream(0);
                packet.rescale_ts(TIME_BASE, tb);
                packet.set_position(-1);
                packet
                    .write_interleaved(output)
                    .map_err(|e| format!("write video: {e}"))?;
            }
            Err(ffmpeg::Error::Eof) => return Ok(()),
            Err(ffmpeg::Error::Other { errno }) if errno == libc::EAGAIN => return Ok(()),
            Err(e) => return Err(format!("encoded packet: {e}")),
        }
    }
}
struct Audio {
    input: ffmpeg::format::context::Input,
    mapping: Vec<Option<usize>>,
}
impl Audio {
    fn new(
        source: &FrameSource,
        output: &mut ffmpeg::format::context::Output,
    ) -> Result<Self, String> {
        let input = ffmpeg::format::input(&source.meta().path).map_err(|e| e.to_string())?;
        let mut mapping = vec![None; input.nb_streams() as usize];
        for stream in input
            .streams()
            .filter(|s| s.parameters().medium() == ffmpeg::media::Type::Audio)
        {
            let mut out = output
                .add_stream(ffmpeg::encoder::find(ffmpeg::codec::Id::None))
                .map_err(|e| e.to_string())?;
            out.set_parameters(stream.parameters());
            // Drop the source container's codec tag; the MP4 muxer chooses its own.
            unsafe {
                (*out.parameters_mut().as_mut_ptr()).codec_tag = 0;
            }
            out.set_time_base(stream.time_base());
            mapping[stream.index()] = Some(out.index());
        }
        Ok(Self { input, mapping })
    }
    fn copy(
        &mut self,
        source: &FrameSource,
        options: &Options,
        samples: &[Sample],
        output: &mut ffmpeg::format::context::Output,
        progress: &Progress,
    ) -> Result<(), String> {
        if self.mapping.iter().all(Option::is_none) {
            return Ok(());
        }
        let start = source.start_time() + source.meta().frame_times[options.first];
        let duration = samples
            .last()
            .map(|s| (s.pts + s.duration) as f64 / 60000.)
            .unwrap_or(0.);
        // Stream-copy packets; no audio decoder/encoder or resampling dependency.
        for packet in self.input.packets() {
            let (stream, mut packet) = packet.map_err(|e| format!("read audio: {e}"))?;
            check_cancel(progress)?;
            let Some(index) = self.mapping[stream.index()] else {
                continue;
            };
            let tb = stream.time_base();
            let Some(pts) = packet.pts().or(packet.dts()) else {
                continue;
            };
            let time = pts as f64 * f64::from(tb);
            if time < start || time >= start + duration {
                continue;
            }
            let offset = (start / f64::from(tb)).round() as i64;
            packet.set_pts(packet.pts().map(|p| p - offset));
            packet.set_dts(packet.dts().map(|p| p - offset));
            packet.set_stream(index);
            packet.set_position(-1);
            packet.rescale_ts(
                tb,
                output
                    .stream(index)
                    .ok_or("missing audio stream")?
                    .time_base(),
            );
            packet
                .write_interleaved(output)
                .map_err(|e| format!("copy audio (try disabling original audio): {e}"))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{track::Footprint, DocState, Track};

    #[test]
    fn export_roundtrip_preserves_timing_objects_audio_and_existing_files() {
        let directory =
            std::env::temp_dir().join(format!("tracker-video-export-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let source_path = directory.join("source.mp4");
        std::fs::write(
            &source_path,
            include_bytes!("../tests/fixtures/export-source.mp4"),
        )
        .unwrap();
        let stream = video::VideoStream::open(source_path.clone(), 0).unwrap();
        let source = stream.frame_source();
        assert_eq!(source.meta().frames, 12);
        let mut doc = DocState::placeholder();
        doc.trail_visible = false;
        doc.labels_visible = false;
        let mut track = Track::point_mass(1, "visible", 0xff0000);
        track.footprint = Footprint::Crosshair;
        for frame in 0..12 {
            track.set_point(frame, 30. + frame as f32, 30.);
        }
        let mut hidden = Track::point_mass(2, "hidden", 0x00ff00);
        hidden.visible = false;
        hidden.set_point(2, 75., 40.);
        doc.tracks = vec![track, hidden];
        let scene = Scene::capture(&doc);
        assert_eq!(scene.tracks.len(), 1);
        let options = Options {
            first: 2,
            last: 7,
            scale: 0.5,
            fps: Some(24.),
            bitrate: 2_000_000,
            audio: true,
        };
        let samples = options.samples(&source).unwrap();
        assert_eq!(samples.len(), 12);
        assert_eq!(samples.first().unwrap().frame, 2);
        assert_eq!(samples.last().unwrap().frame, 7);
        assert_eq!(samples[0].pts, 0);
        assert_eq!(
            samples.last().unwrap().pts + samples.last().unwrap().duration,
            30000
        );
        let path = directory.join("result.mp4");
        std::fs::write(&path, b"previous file").unwrap();
        export(&source, &scene, &options, &path, &Progress::default()).unwrap();
        let exported = video::VideoStream::open(path.clone(), 0).unwrap();
        assert_eq!(
            (
                exported.meta.width,
                exported.meta.height,
                exported.meta.frames
            ),
            (48, 32, 12)
        );
        assert!((exported.meta.frame_times[11] - 11. / 24.).abs() < 0.001);
        let image = exported.frame_source().reader().unwrap().read(0).unwrap();
        let red = image
            .bgra
            .chunks_exact(4)
            .filter(|p| p[2] > 60 && p[2] > p[1] * 2 && p[2] > p[0] * 2)
            .count();
        assert!(red > 5, "visible object was not encoded");
        let audio_packets = |path: &Path| {
            let mut input = ffmpeg::format::input(path).unwrap();
            input
                .packets()
                .filter_map(Result::ok)
                .filter(|(stream, _)| stream.parameters().medium() == ffmpeg::media::Type::Audio)
                .map(|(_, packet)| packet.data().unwrap().to_vec())
                .collect::<Vec<_>>()
        };
        let original = audio_packets(&source_path);
        let copied = audio_packets(&path);
        assert!(!copied.is_empty(), "original audio was dropped");
        assert!(
            copied.iter().all(|p| original.contains(p)),
            "audio was not stream copied"
        );
        drop(exported);

        let original_options = Options {
            fps: None,
            audio: false,
            ..options.clone()
        };
        let original_samples = original_options.samples(&source).unwrap();
        assert_eq!(original_samples.len(), 6);
        assert_eq!(original_samples[1].pts, 5000);
        let silent_path = directory.join("silent.mp4");
        export(
            &source,
            &scene,
            &original_options,
            &silent_path,
            &Progress::default(),
        )
        .unwrap();
        assert!(audio_packets(&silent_path).is_empty());

        let before = std::fs::read(&path).unwrap();
        let cancelled = Progress::default();
        cancelled.cancel.store(true, Ordering::Relaxed);
        assert_eq!(
            export(&source, &scene, &options, &path, &cancelled).unwrap_err(),
            "cancelled"
        );
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert!(export(
            &source,
            &scene,
            &options,
            &source_path,
            &Progress::default()
        )
        .is_err());
        assert!(export(
            &source,
            &scene,
            &Options {
                last: 12,
                ..options
            },
            &path,
            &Progress::default()
        )
        .is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert!(std::fs::read_dir(&directory).unwrap().all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".tracker-export-")));
        drop(stream);
        std::fs::remove_file(source_path).unwrap();
        std::fs::remove_file(path).unwrap();
        std::fs::remove_file(silent_path).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn original_frame_rate_preserves_variable_timestamps_and_final_duration() {
        let directory =
            std::env::temp_dir().join(format!("tracker-export-vfr-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let source_path = directory.join("vfr.mp4");
        let output_path = directory.join("output.mp4");
        std::fs::write(
            &source_path,
            include_bytes!("../tests/fixtures/export-vfr.mp4"),
        )
        .unwrap();
        let stream = video::VideoStream::open(source_path.clone(), 0).unwrap();
        let source = stream.frame_source();
        assert!((source.meta().frame_times[7] - source.meta().frame_times[6]) > 0.15);
        let options = Options {
            first: 0,
            last: source.meta().frames - 1,
            scale: 1.,
            fps: None,
            bitrate: 2_000_000,
            audio: false,
        };
        export(
            &source,
            &Scene::capture(&DocState::placeholder()),
            &options,
            &output_path,
            &Progress::default(),
        )
        .unwrap();
        let output = video::VideoStream::open(output_path.clone(), 0).unwrap();
        assert_eq!(output.meta.frames, source.meta().frames);
        for (original, exported) in source
            .meta()
            .frame_times
            .iter()
            .zip(&output.meta.frame_times)
        {
            assert!((original - exported).abs() < 0.0001);
        }
        assert!((source.meta().duration - output.meta.duration).abs() < 0.0001);
        drop(output);
        drop(stream);
        std::fs::remove_file(source_path).unwrap();
        std::fs::remove_file(output_path).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }
}
