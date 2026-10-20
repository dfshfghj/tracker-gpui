//! Tracker GPUI: a GPUI rewrite of the Tracker video workspace,
//! implemented with gpui-kit. Video is decoded by FFmpeg through
//! `video.rs` (packet index + on-demand frame requests), with a synthetic
//! bouncing ball available as a fallback.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::cell::RefCell;
use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::Arc;
use std::time::Instant;

mod video;
use video::{FrameData, VideoStream};

use gpui_kit::*;
use serde::{Deserialize, Serialize};

mod coords;
mod history;
mod plot;
mod prefs;
mod project;
mod track;
mod tracking;

mod app;
mod canvas;
mod icons;
mod menus;
mod modals;
mod plot_view;
mod render;
mod state;
mod theme;
mod tracker_modal;
mod widgets;
mod workspace;

use coords::{Calibration, CalibrationStep, ImageCoordSystem};
use history::{EditSnapshot, UndoStack};
use plot::{PlotConfig, PlotPoint, PlotSeries, PlotState, PlotVar};
use prefs::Preferences;
use track::{MeasurementKind, MeasurementStep, Track};
use tracking::{
    AutoTrackEnd, AutoTrackMessage, AutoTrackRequest, AutoTrackSettings, MaskShape, MatchQuality,
    SearchPolicy, StopPolicy, TemplateMatchBackend, TrackingBackend,
};

use app::*;
use canvas::*;
use icons::*;
use menus::*;
use modals::*;
use plot_view::*;
use project::{
    DocProject, MeasurementProject, MeasurementStepProject, PointProject, ProjectFile, TrackProject,
};
use theme::*;
use widgets::*;
use workspace::*;

pub(crate) const PALETTE: [u32; 8] = [
    0xff6b6b, 0x4dabf7, 0x51cf66, 0xffd43b, 0xb197fc, 0xff922b, 0x22b8cf, 0xf783ac,
];

pub(crate) const VIDEO_EXTS: &[&str] = &[
    "mp4", "mov", "m4v", "avi", "mkv", "webm", "wmv", "flv", "mpg", "mpeg", "ts", "m2ts", "ogv",
    "3gp",
];

#[derive(Clone, Copy, PartialEq)]
enum Tool {
    Move,
    Origin,
    OriginOffset,
    Tape,
    Protractor,
    Circle,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ToolGroup {
    Calibration,
    Measurement,
    Tracks,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PlotAxis {
    X,
    Y,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Selection {
    None,
    Axes,
    Stick(usize),
    Track(u64),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum InlineTarget {
    OriginX,
    OriginY,
    Angle,
    StickLength(usize),
}

struct InlineEdit {
    target: InlineTarget,
    text: String,
    error: Option<String>,
}

impl Tool {
    fn is_calibration_group(self) -> bool {
        matches!(self, Tool::Origin | Tool::OriginOffset)
    }

    fn is_measurement_group(self) -> bool {
        matches!(self, Tool::Tape | Tool::Protractor | Tool::Circle)
    }

    fn label(self) -> &'static str {
        match self {
            Tool::Move => "移动/选择",
            Tool::Origin => "设定坐标原点",
            Tool::OriginOffset => "原点偏移",
            Tool::Tape => "测量尺",
            Tool::Protractor => "量角器",
            Tool::Circle => "圆拟合",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Tool::Move => ICON_MOVE,
            Tool::Origin => ICON_ORIGIN,
            Tool::OriginOffset => ICON_ORIGIN_OFFSET,
            Tool::Tape => ICON_TAPE,
            Tool::Protractor => ICON_PROTRACTOR,
            Tool::Circle => ICON_CIRCLE_FIT,
        }
    }
}

#[derive(Clone, Copy)]
enum Cmd {
    First,
    Back10,
    Back1,
    Play,
    Fwd1,
    Fwd10,
    Last,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MenuId {
    File,
    Edit,
    View,
    Tracks,
    Coords,
    Help,
}

#[derive(Clone)]
enum Modal {
    CloseDirty { index: usize },
    ConfirmNew,
    ConfirmOpenProject(PathBuf),
    Calibration,
    Operations,
    About,
    Error(String),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CalibrationField {
    Length,
    Unit,
    Decimals,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum SideSection {
    Properties,
    Tracks,
    Object,
    Data,
    Plot,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RightView {
    Inspector,
    Plot,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TrackerField {
    MaskWidth,
    MaskHeight,
    Search,
    Evolve,
    Tether,
    AutoMark,
}

impl TrackerField {
    fn next(self) -> Self {
        match self {
            Self::MaskWidth => Self::MaskHeight,
            Self::MaskHeight => Self::Search,
            Self::Search => Self::Evolve,
            Self::Evolve => Self::Tether,
            Self::Tether => Self::AutoMark,
            Self::AutoMark => Self::MaskWidth,
        }
    }

    fn index(self) -> usize {
        match self {
            Self::MaskWidth => 0,
            Self::MaskHeight => 1,
            Self::Search => 2,
            Self::Evolve => 3,
            Self::Tether => 4,
            Self::AutoMark => 5,
        }
    }
}

struct TrackingRun {
    track_id: u64,
    rx: Receiver<AutoTrackMessage>,
    stop: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    paused_ui: bool,
    fast: bool,
    settings: AutoTrackSettings,
    start_frame: usize,
    last_frame: usize,
    last_confidence: f32,
    points: usize,
    search_frame: usize,
    search_center: Option<(f32, f32)>,
    last_match: Option<(f32, f32)>,
    scores: Vec<(usize, f32)>,
}

#[derive(Clone)]
struct TrackerOverlay {
    track_id: u64,
    key_frame: usize,
    key_center: (f32, f32),
    mask_width: u32,
    mask_height: u32,
    shape: MaskShape,
    search_size: u32,
    search_center: (f32, f32),
    search_frame: usize,
    match_position: Option<(f32, f32)>,
}

struct TrackerThumbs {
    key: (usize, i64, i64, i64, i64, u32, u32),
    template: Option<Arc<RenderImage>>,
    matched: Option<Arc<RenderImage>>,
}

impl Drop for TrackingRun {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum EditCommand {
    SetPoint {
        track: usize,
        frame: usize,
        x: f32,
        y: f32,
    },
    DeletePoint {
        track: usize,
        frame: usize,
    },
    SetCalibration {
        a: [f32; 2],
        b: [f32; 2],
        length: f32,
    },
    ResetCalibration,
    SetOrigin {
        frame: usize,
        x: f32,
        y: f32,
    },
}

#[derive(Clone)]
enum Drag {
    Handle {
        track: usize,
        frame: usize,
        index: usize,
        created: bool,
    },
    Origin {
        frame: usize,
    },
    CalStick {
        stick: usize,
        end: usize,
    },
    StickMove {
        stick: usize,
        start: (f32, f32),
        a: [f64; 2],
        b: [f64; 2],
    },
    TrackMove {
        track: usize,
        start: (f32, f32),
        handles: Vec<(f32, f32)>,
    },
    AxisOrigin,
    AxisAngle,
    AxisLine {
        start: (f32, f32),
        origin: [f64; 2],
    },
}

struct LoadingVideo {
    name: String,
    progress: Arc<AtomicU32>,
}

struct DocState {
    id: u64,
    path: PathBuf,
    title: String,
    frame: usize,
    playing: bool,
    play_start: Option<Instant>,
    play_frame: usize,

    tracks: Vec<Track>,
    active: usize,
    next_track_id: u64,
    collapsed: HashSet<SideSection>,
    plot: PlotState,

    coords: ImageCoordSystem,
    calibration: Calibration,
    active_stick: usize,
    selection: Selection,
    axes_visible: bool,
    trail_visible: bool,
    trail_length: usize,
    labels_visible: bool,
    trace_visible: bool,
    table_columns: [bool; 5],
    table_selection: HashSet<usize>,

    scale: f32,
    tx: f32,
    ty: f32,
    fitted: bool,
    user_adjusted: bool,

    // stage bounds in window coordinates
    sx: f32,
    sy: f32,
    sw: f32,
    sh: f32,

    created: Instant,
    draw_ms: f32,
    fps: f32,
    frames_counted: u32,
    fps_t: Instant,
    drawn_points: usize,
    frame_count: usize,
    img_w: f32,
    img_h: f32,
    video: Option<Arc<VideoStream>>,
    frame_image: Option<(usize, Arc<RenderImage>)>,
    retired: VecDeque<Arc<RenderImage>>,
    to_drop: Vec<Arc<RenderImage>>,
    last_requested: Option<usize>,
    ready: Option<FrameData>,
    loading: Option<LoadingVideo>,
    error: Option<String>,
    status: Option<String>,
    pending_restore: Option<DocProject>,
    history: UndoStack,
    tracking: Option<TrackingRun>,
    tracker_overlay: Option<TrackerOverlay>,
    tracker_open: bool,
}

#[derive(Clone)]
struct FileNode {
    name: String,
    path: PathBuf,
    is_dir: bool,
    children: Vec<FileNode>,
}

struct State {
    docs: Vec<DocState>,
    active_doc: usize,
    next_doc_id: u64,
    workdir: Option<PathBuf>,
    tree: Vec<FileNode>,
    expanded: HashSet<PathBuf>,
    project_path: Option<PathBuf>,
    prefs: Preferences,
    modal: Option<Modal>,
    cal_length_input: String,
    cal_unit_input: String,
    cal_decimals_input: String,
    cal_field: CalibrationField,
    cal_error: Option<String>,
    menu_open: Option<MenuId>,
    menu_x: f32,
    tool: Tool,
    measurement_tool: Tool,
    tool_menu: Option<ToolGroup>,
    tool_menu_pos: (f32, f32),
    plot_var_menu: Option<(usize, PlotAxis)>,
    plot_var_pos: (f32, f32),
    suppress_stage_click_until: Option<Instant>,
    shift_down: bool,
    inline_edit: Option<InlineEdit>,
    tab_scroll: ScrollHandle,
    tracker_settings: AutoTrackSettings,
    tracker_inputs: [String; 6],
    tracker_field: TrackerField,
    tracker_error: Option<String>,
    tracker_fast: bool,
    tracker_thumbs: Option<TrackerThumbs>,
    tracker_thumbs_drop: Vec<Arc<RenderImage>>,
    drag: Option<Drag>,
    progress_rect: (f32, f32, f32, f32),
    scrubbing: bool,
    left_open: bool,
    right_open: bool,
    right_view: RightView,
    textures_to_drop: Vec<Arc<RenderImage>>,
}

impl std::ops::Deref for State {
    type Target = DocState;

    fn deref(&self) -> &DocState {
        &self.docs[self.active_doc.min(self.docs.len() - 1)]
    }
}

impl std::ops::DerefMut for State {
    fn deref_mut(&mut self) -> &mut DocState {
        let index = self.active_doc.min(self.docs.len() - 1);
        &mut self.docs[index]
    }
}

fn make_image(data: Vec<u8>, w: u32, h: u32) -> Arc<RenderImage> {
    let frame = image::Frame::new(
        image::RgbaImage::from_raw(w, h, data).expect("frame buffer size mismatch"),
    );
    Arc::new(RenderImage::new(smallvec::smallvec![frame]))
}

// ---------------------------------------------------------------- views

struct Probe {
    state: Rc<RefCell<State>>,
    focus: FocusHandle,
    focused_once: bool,
    dialog_only: bool,
    main: Option<Entity<Probe>>,
    close_hook: bool,
    pending_tracker_window: bool,
    tracker_window: Option<AnyWindowHandle>,
    main_hwnd: Option<isize>,
    owner_hwnd: Option<isize>,
    owner_applied: bool,
}

fn main() {
    let (window_width, window_height) = std::env::var("PROBE_WINDOW_SIZE")
        .ok()
        .and_then(|value| {
            let (width, height) = value.split_once('x')?;
            Some((
                width.trim().parse::<f32>().ok()?,
                height.trim().parse::<f32>().ok()?,
            ))
        })
        .unwrap_or((1360.0, 820.0));
    gpui_kit::application().run(move |cx| {
        gpui_kit::init(cx);
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: point(px(10.), px(10.)),
                    size: size(px(window_width), px(window_height)),
                })),
                focus: true,
                ..Default::default()
            },
            cx,
            |_window, cx| cx.new(Probe::new),
        )
        .expect("failed to open window");
        cx.activate(true);
    });
}

#[cfg(test)]
mod tab_scroll_tests {
    use crate::workspace::clamp_tab_offset;

    #[test]
    fn clamp_tab_offset_handles_positive_max_offset() {
        assert_eq!(clamp_tab_offset(0.0, 40.0, 156.0), -40.0);
        assert_eq!(clamp_tab_offset(-150.0, 40.0, 156.0), -156.0);
        assert_eq!(clamp_tab_offset(-20.0, -40.0, 156.0), 0.0);
    }

    #[test]
    fn clamp_tab_offset_never_inverts_bounds() {
        assert_eq!(clamp_tab_offset(0.0, 30.0, 0.0), 0.0);
        assert_eq!(clamp_tab_offset(0.0, 30.0, -10.0), 0.0);
        assert_eq!(clamp_tab_offset(0.0, 30.0, f32::NAN), 0.0);
        assert_eq!(clamp_tab_offset(f32::NAN, 30.0, 100.0), 0.0);
    }
}
