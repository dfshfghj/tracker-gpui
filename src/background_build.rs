//! Build a static background by temporal median for motion segmentation.

use opencv::core::Mat;
use opencv::prelude::*;

use crate::aruco_detect::gray_from_frame;
use crate::video::FrameSource;

pub(crate) fn build_background(source: &FrameSource) -> Result<Mat, String> {
    let meta = source.meta();
    if meta.frames == 0 {
        return Err("视频没有可用帧".into());
    }
    let sample_count = 21.min(meta.frames).max(1);
    let mut reader = source.reader()?;
    let mut samples: Vec<Vec<u8>> = Vec::with_capacity(sample_count);
    for index in 0..sample_count {
        let frame = if sample_count == 1 {
            0
        } else {
            index * (meta.frames - 1) / (sample_count - 1)
        };
        let data = reader.read(frame)?;
        let gray = gray_from_frame(&data)?;
        let bytes = gray
            .data_typed::<u8>()
            .map_err(|err| format!("背景采样失败: {err}"))?;
        samples.push(bytes.to_vec());
    }
    let pixels = (meta.width * meta.height) as usize;
    let mut background = vec![0u8; pixels];
    let mut values = [0u8; 21];
    for pixel in 0..pixels {
        for (index, sample) in samples.iter().enumerate() {
            values[index] = sample[pixel];
        }
        values[..samples.len()].sort_unstable();
        background[pixel] = values[samples.len() / 2];
    }
    let flat = Mat::from_slice(background.as_slice())
        .map_err(|err| format!("背景 Mat 失败: {err}"))?;
    let background = flat
        .reshape(1, meta.height as i32)
        .map_err(|err| format!("背景 reshape 失败: {err}"))?
        .clone_pointee();
    Ok(background)
}
