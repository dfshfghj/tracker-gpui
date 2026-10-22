//! Motion segmentation backends shared by the rigid-body trackers.

use crate::i18n::tr;
use crate::i18n::cv;
use opencv::core::{self, Mat, Point, Scalar, Size};
use opencv::video::{self, BackgroundSubtractorTrait};
use opencv::imgproc;

use crate::background_build::build_background;
use crate::video::FrameSource;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SegmenterKind {
    Knn,
    Mog2,
    Median,
}

impl SegmenterKind {
    pub(crate) fn label(self) -> String {
        match self {
            Self::Knn => "KNN".to_string(),
            Self::Mog2 => "MOG2".to_string(),
            Self::Median => tr("msg.bg.median"),
        }
    }

    pub(crate) fn from_env() -> Self {
        match std::env::var("PROBE_MOTION_MODEL")
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "mog2" => Self::Mog2,
            "median" => Self::Median,
            _ => Self::Knn,
        }
    }
}

pub(crate) struct Segmenter {
    kind: SegmenterKind,
    knn: Option<core::Ptr<video::BackgroundSubtractorKNN>>,
    mog2: Option<core::Ptr<video::BackgroundSubtractorMOG2>>,
    background: Option<Mat>,
    close: Mat,
    open: Mat,
}

impl Segmenter {
    pub(crate) fn new(source: &FrameSource, kind: SegmenterKind) -> Result<Self, String> {
        let (knn, mog2, background) = match kind {
            SegmenterKind::Knn => (
                Some(
                    video::create_background_subtractor_knn(500, 400.0, false)
                        .map_err(|err| cv("create KNN background model", err))?,
                ),
                None,
                None,
            ),
            SegmenterKind::Mog2 => (
                None,
                Some(
                    video::create_background_subtractor_mog2(120, 16.0, false)
                        .map_err(|err| cv("create MOG2 background model", err))?,
                ),
                None,
            ),
            SegmenterKind::Median => (None, None, Some(build_background(source)?)),
        };
        let close = imgproc::get_structuring_element(
            imgproc::MORPH_ELLIPSE,
            Size::new(7, 7),
            Point::new(-1, -1),
        )
        .map_err(|err| cv("morphology kernel", err))?;
        let open = imgproc::get_structuring_element(
            imgproc::MORPH_ELLIPSE,
            Size::new(3, 3),
            Point::new(-1, -1),
        )
        .map_err(|err| cv("morphology kernel", err))?;
        Ok(Self {
            kind,
            knn,
            mog2,
            background,
            close,
            open,
        })
    }

    pub(crate) fn mask(&mut self, gray: &Mat) -> Result<Mat, String> {
        let mut raw = Mat::default();
        match self.kind {
            SegmenterKind::Knn => {
                let model = self
                    .knn
                    .as_mut()
                    .ok_or_else(|| tr("msg.knn_uninitialized"))?;
                BackgroundSubtractorTrait::apply_def(model, gray, &mut raw)
                    .map_err(|err| cv("KNN background subtraction", err))?;
            }
            SegmenterKind::Mog2 => {
                let model = self
                    .mog2
                    .as_mut()
                    .ok_or_else(|| tr("msg.mog2_uninitialized"))?;
                BackgroundSubtractorTrait::apply_def(model, gray, &mut raw)
                    .map_err(|err| cv("MOG2 background subtraction", err))?;
            }
            SegmenterKind::Median => {
                let background = self
                    .background
                    .as_ref()
                    .ok_or_else(|| tr("msg.median_uninitialized"))?;
                let mut diff = Mat::default();
                core::absdiff(gray, background, &mut diff)
                    .map_err(|err| cv("background subtraction", err))?;
                imgproc::threshold(&diff, &mut raw, 25.0, 255.0, imgproc::THRESH_BINARY)
                    .map_err(|err| cv("threshold", err))?;
            }
        }

        let mut binary = Mat::default();
        imgproc::threshold(&raw, &mut binary, 127.0, 255.0, imgproc::THRESH_BINARY)
            .map_err(|err| cv("binarize", err))?;
        let mut cleaned = Mat::default();
        imgproc::morphology_ex(
            &binary,
            &mut cleaned,
            imgproc::MORPH_CLOSE,
            &self.close,
            Point::new(-1, -1),
            2,
            core::BORDER_CONSTANT,
            Scalar::all(0.0),
        )
        .map_err(|err| cv("close", err))?;
        let mut filtered = Mat::default();
        imgproc::morphology_ex(
            &cleaned,
            &mut filtered,
            imgproc::MORPH_OPEN,
            &self.open,
            Point::new(-1, -1),
            1,
            core::BORDER_CONSTANT,
            Scalar::all(0.0),
        )
        .map_err(|err| cv("open", err))?;
        Ok(filtered)
    }
}
