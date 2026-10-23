use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gpui_kit::*;

use crate::video::{self, VideoStream};
use crate::i18n::tr;
use rust_i18n::t;
use crate::*;

impl Probe {
    pub(crate) fn new(cx: &mut Context<Self>) -> Self {
        let state = State::new();
        let mut st = state;
        let load_project = std::env::var("PROBE_LOAD").ok();
        // Test hook: load a video without going through the file tree.
        if load_project.is_none() {
            if let Ok(path) = std::env::var("PROBE_VIDEO") {
                video::init();
                match VideoStream::open(PathBuf::from(&path), 0) {
                    Ok(stream) => st.add_video_stream(PathBuf::from(&path), stream),
                    Err(err) => eprintln!("PROBE_VIDEO failed: {err}"),
                }
            }
        }
        #[cfg(feature = "vision")]
        if std::env::var_os("PROBE_RIGID").is_some() {
            let source = st.video.as_ref().map(|video| video.frame_source());
            let Some(source) = source else {
                eprintln!("[script] rigid skipped: no video");
                std::process::exit(1);
            };
            match crate::rigid::detect_rigid(&source) {
                Ok(result) => {
                    let steps: Vec<_> = result
                        .points
                        .iter()
                        .map(|point| {
                            (
                                point.frame,
                                point.x,
                                point.y,
                                point.angle,
                                point.scale,
                                point.marker_count,
                            )
                        })
                        .collect();
                    let markers = result
                        .markers
                        .iter()
                        .map(|marker| RigidBodyMarker {
                            id: marker.id,
                            corners: marker.corners,
                        })
                        .collect();
                    let count = st.add_rigid_body(
                        result.dictionary.clone(),
                        markers,
                        Vec::new(),
                        &steps,
                        &tr("track_name.rigid_detect"),
                    );
                    eprintln!(
                        "[script] rigid points={count} markers={} detections={} dict={}",
                        result.markers.len(),
                        result.detections,
                        result.dictionary
                    );
                    for point in result.points.iter().take(3) {
                        eprintln!(
                            "[script] rigid f{} ({:.1},{:.1}) angle={:.2} scale={:.1} markers={}",
                            point.frame,
                            point.x,
                            point.y,
                            point.angle,
                            point.scale,
                            point.marker_count
                        );
                    }
                    if let Some(point) = result.points.last() {
                        eprintln!(
                            "[script] rigid last f{} angle={:.2} scale={:.1}",
                            point.frame, point.angle, point.scale
                        );
                    }
                    if let Some(path) = std::env::var_os("PROBE_SAVE") {
                        match st.save_project_to(std::path::Path::new(&path)) {
                            Ok(()) => eprintln!("[script] rigid saved"),
                            Err(err) => eprintln!("[script] rigid save failed: {err}"),
                        }
                    }
                    std::process::exit(0);
                }
                Err(err) => {
                    eprintln!("[script] rigid failed: {err}");
                    std::process::exit(1);
                }
            }
        }
        #[cfg(feature = "vision")]
        if std::env::var_os("PROBE_ORB").is_some() {
            let roi = std::env::var("PROBE_ORB_ROI")
                .ok()
                .and_then(|text| {
                    let values: Vec<f32> = text
                        .split(',')
                        .filter_map(|part| part.trim().parse().ok())
                        .collect();
                    (values.len() == 4).then(|| (values[0], values[1], values[2], values[3]))
                })
                .unwrap_or((80.0, 160.0, 480.0, 260.0));
            let frame = std::env::var("PROBE_ORB_FRAME")
                .ok()
                .and_then(|text| text.trim().parse().ok())
                .unwrap_or(st.frame);
            let source = st.video.as_ref().map(|video| video.frame_source());
            let Some(source) = source else {
                eprintln!("[script] orb skipped: no video");
                std::process::exit(1);
            };
            match crate::orb_track::detect_orb(&source, roi, frame) {
                Ok(result) => {
                    let steps: Vec<_> = result
                        .points
                        .iter()
                        .map(|point| {
                            (
                                point.frame,
                                point.x,
                                point.y,
                                point.angle,
                                point.scale,
                                point.marker_count,
                            )
                        })
                        .collect();
                    let count = st.add_rigid_body(
                        result.dictionary.clone(),
                        Vec::new(),
                        Vec::new(),
                        &steps,
                        &tr("track_name.markerless"),
                    );
                    eprintln!(
                        "[script] orb points={count} tracked={} roi={roi:?}",
                        result.detections
                    );
                    if let Some(point) = result.points.last() {
                        eprintln!(
                            "[script] orb last f{} ({:.1},{:.1}) angle={:.2} scale={:.1} inliers={}",
                            point.frame,
                            point.x,
                            point.y,
                            point.angle,
                            point.scale,
                            point.marker_count
                        );
                    }
                    std::process::exit(0);
                }
                Err(err) => {
                    eprintln!("[script] orb failed: {err}");
                    std::process::exit(1);
                }
            }
        }
        #[cfg(feature = "vision")]
        if std::env::var_os("PROBE_MOTION").is_some() {
            let roi = std::env::var("PROBE_MOTION_ROI")
                .ok()
                .and_then(|text| {
                    let values: Vec<f32> = text
                        .split(',')
                        .filter_map(|part| part.trim().parse().ok())
                        .collect();
                    (values.len() == 4).then(|| (values[0], values[1], values[2], values[3]))
                })
                .unwrap_or((180.0, 150.0, 300.0, 280.0));
            let frame = std::env::var("PROBE_MOTION_FRAME")
                .ok()
                .and_then(|text| text.trim().parse().ok())
                .unwrap_or(5);
            let source = st.video.as_ref().map(|video| video.frame_source());
            let Some(source) = source else {
                eprintln!("[script] motion skipped: no video");
                std::process::exit(1);
            };
            let options = crate::motion_contour::MotionOptions::from_env();
            match crate::motion_contour::detect_motion_contour(&source, roi, frame, options) {
                Ok(result) => {
                    let steps: Vec<_> = result
                        .points
                        .iter()
                        .map(|point| {
                            (
                                point.frame,
                                point.x,
                                point.y,
                                point.angle,
                                point.scale,
                                point.marker_count,
                            )
                        })
                        .collect();
                    let count = st.add_rigid_body_with(
                        "MOTION".into(),
                        Vec::new(),
                        result.contour.clone(),
                        &steps,
                        &tr("track_name.motion_contour"),
                        result.mask_mode,
                        &result.step_contours,
                    );
                    eprintln!(
                        "[script] motion points={count} tracked={} contour={} roi={roi:?} mode={:?}",
                        result.detections,
                        result.contour.len(),
                        result.mask_mode
                    );
                    if let Some(point) = result.points.last() {
                        eprintln!(
                            "[script] motion last f{} ({:.1},{:.1}) angle={:.2} scale={:.1} quality={}",
                            point.frame,
                            point.x,
                            point.y,
                            point.angle,
                            point.scale,
                            point.marker_count
                        );
                    }
                    if let Some(path) = std::env::var_os("PROBE_SAVE") {
                        match st.save_project_to(std::path::Path::new(&path)) {
                            Ok(()) => eprintln!("[script] motion saved"),
                            Err(err) => eprintln!("[script] motion save failed: {err}"),
                        }
                    }
                    std::process::exit(0);
                }
                Err(err) => {
                    eprintln!("[script] motion failed: {err}");
                    std::process::exit(1);
                }
            }
        }
        #[cfg(feature = "vision")]
        if std::env::var_os("PROBE_AUTODETECT").is_some() {
            let preview = std::env::var_os("PROBE_AUTODETECT_PREVIEW").map(|value| {
                if value.to_str() == Some("1") {
                    PathBuf::from("target/motion_preview.png")
                } else {
                    PathBuf::from(value)
                }
            });
            let source = st.video.as_ref().map(|video| video.frame_source());
            let Some(source) = source else {
                eprintln!("[script] autodetect skipped: no video");
                std::process::exit(1);
            };
            match crate::motion_detect::detect_motion(&source, preview.as_deref()) {
                Ok(result) => {
                    let points: Vec<_> = result
                        .points
                        .iter()
                        .map(|point| (point.frame, point.x, point.y))
                        .collect();
                    let count = st.add_auto_points(&points, &tr("track_name.auto_detect"));
                    eprintln!(
                        "[script] autodetect points={count} candidates={} score={:.1} path={:.1} bright={:.1} preview={:?}",
                        result.candidates,
                        result.score,
                        result.path_length,
                        result.mean_brightness,
                        result.preview
                    );
                    std::process::exit(0);
                }
                Err(err) => {
                    eprintln!("[script] autodetect failed: {err}");
                    std::process::exit(1);
                }
            }
        }
        if std::env::var("PROBE_PLAY").is_ok() {
            st.play();
        }
        if std::env::var_os("PROBE_RIGHT_PLOT").is_some() {
            st.right_view = RightView::Plot;
        }
        if let Ok(list) = std::env::var("PROBE_COLLAPSE") {
            if list.contains("left") {
                st.left_open = false;
            }
            if list.contains("right") {
                st.right_open = false;
            }
        }
        let state = Rc::new(RefCell::new(st));
        if let Some(path) = load_project {
            open_project_file(state.clone(), PathBuf::from(path), cx);
        } else if let Ok(paths) = std::env::var("PROBE_OPEN") {
            for path in paths.split(';').filter(|path| !path.trim().is_empty()) {
                open_video(state.clone(), PathBuf::from(path.trim()), cx);
            }
        }
        if let Ok(menu) = std::env::var("PROBE_MENU") {
            let menu = match menu.to_ascii_lowercase().as_str() {
                "file" => Some(MenuId::File),
                "edit" => Some(MenuId::Edit),
                "view" => Some(MenuId::View),
                "tracks" => Some(MenuId::Tracks),
                "coords" => Some(MenuId::Coords),
                "help" => Some(MenuId::Help),
                _ => None,
            };
            let mut st = state.borrow_mut();
            st.menu_open = menu;
            st.menu_x = 8.0;
        }
        if std::env::var("PROBE_CAL_DIALOG").is_ok() {
            state.borrow_mut().open_calibration_dialog();
        }
        if std::env::var("PROBE_TRACKER_DIALOG").is_ok() {
            state.borrow_mut().open_tracker_dialog();
        }
        if std::env::var_os("PROBE_TRACKER_UI").is_some() {
            let point = std::env::var("PROBE_TRACKER_POINT")
                .ok()
                .and_then(|text| {
                    let parts: Vec<f32> = text
                        .split(',')
                        .filter_map(|part| part.trim().parse().ok())
                        .collect();
                    (parts.len() == 2).then(|| (parts[0], parts[1]))
                })
                .unwrap_or((200.0, 180.0));
            let scripted = state.clone();
            cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(2500))
                    .await;
                let _ = this.update(cx, |this, cx| {
                    {
                        let mut st = scripted.borrow_mut();
                        if st.video.is_some() {
                            st.set_frame(0);
                            let track = st.ensure_point_mass_track();
                            st.apply_edit_command(EditCommand::SetPoint {
                                track,
                                frame: 0,
                                x: point.0,
                                y: point.1,
                            });
                            st.active = track;
                        }
                    }
                    this.open_tracker_window(cx);
                    {
                        let st = scripted.borrow();
                        eprintln!(
                            "[script] tracker ui open overlay={} mask={}x{} search={}",
                            st.tracker_overlay.is_some(),
                            st.tracker_settings.mask_width,
                            st.tracker_settings.mask_height,
                            st.tracker_settings.search_size
                        );
                    }
                    cx.notify();
                });
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(1500))
                    .await;
                let _ = this.update(cx, |_this, cx| {
                    let mut st = scripted.borrow_mut();
                    if let Some(track) = st.active_track_mut() {
                        track.step_size = 1;
                    }
                    if st.apply_tracker_dialog() {
                        st.start_tracking_ui();
                    }
                    eprintln!(
                        "[script] tracker search running={} frame={} status={}",
                        st.tracking.is_some(),
                        st.frame,
                        st.status.clone().unwrap_or_default()
                    );
                    cx.notify();
                });
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(600))
                    .await;
                let _ = this.update(cx, |_this, cx| {
                    let mut st = scripted.borrow_mut();
                    let paused = st.pause_tracking();
                    eprintln!(
                        "[script] tracker pause={} frame={} overlay={}",
                        paused,
                        st.frame,
                        st.tracker_overlay.is_some()
                    );
                    cx.notify();
                });
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(600))
                    .await;
                let _ = this.update(cx, |_this, cx| {
                    let mut st = scripted.borrow_mut();
                    let resumed = st.resume_tracking();
                    eprintln!("[script] tracker resume={}", resumed);
                    cx.notify();
                });
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(1200))
                    .await;
                let _ = this.update(cx, |_this, _cx| {
                    let mut st = scripted.borrow_mut();
                    let scores = st
                        .tracking
                        .as_ref()
                        .map(|run| run.scores.len())
                        .unwrap_or(0);
                    let stopped = st.stop_tracking();
                    let points = st
                        .active_track()
                        .map(|track| track.step_count())
                        .unwrap_or(0);
                    let keyframes = st
                        .active_track()
                        .map(|track| track.keyframes().count())
                        .unwrap_or(0);
                    eprintln!(
                        "[script] tracker stop={} points={} keyframes={} scores={} frame={}",
                        stopped, points, keyframes, scores, st.frame
                    );
                    std::process::exit(0);
                });
            })
            .detach();
        }
        if std::env::var_os("PROBE_TRACKS_MENU").is_some() {
            let scripted = state.clone();
            cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(1500))
                    .await;
                let _ = this.update(cx, |_this, cx| {
                    let mut st = scripted.borrow_mut();
                    if st.video.is_some() {
                        st.add_point_mass_track();
                        st.add_point_mass_track();
                        let measurement =
                            st.create_measurement_track(MeasurementKind::Tape);
                        st.active = measurement;
                        st.tool_menu = Some(ToolGroup::Tracks);
                        st.tool_menu_pos = (60.0, 140.0);
                        let point_tracks = st
                            .tracks
                            .iter()
                            .filter(|track| track.is_point_mass())
                            .count();
                        eprintln!(
                            "[script] tracks-menu point_tracks={point_tracks} active_measurement={}",
                            st.active == measurement
                        );
                    }
                    cx.notify();
                });
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(2000))
                    .await;
                let _ = this.update(cx, |_this, _cx| {
                    std::process::exit(0);
                });
            })
            .detach();
        }
        if std::env::var_os("PROBE_TRAIL").is_some() {
            let scripted = state.clone();
            cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(1500))
                    .await;
                let _ = this.update(cx, |_this, cx| {
                    let mut st = scripted.borrow_mut();
                    if st.video.is_some() {
                        st.set_frame(16);
                        let track = st.ensure_point_mass_track();
                        for frame in 1..=16usize {
                            let x = 180.0 + frame as f32 * 9.0;
                            let y = 120.0 + frame as f32 * 6.0;
                            st.tracks[track].set_point(frame, x, y);
                        }
                        st.tracks[track].add_keyframe(16);
                        st.active = track;
                        st.selection = Selection::None;
                        st.sync_table_selection();
                        let table = st.table_csv();
                        eprintln!(
                            "[script] trail points={} frame={} labels={} trail={}/{} table_header={:?} table_rows={}",
                            st.tracks[track].step_count(),
                            st.frame,
                            st.labels_visible,
                            st.trail_visible,
                            st.trail_length,
                            table.lines().next().unwrap_or_default(),
                            table.lines().count().saturating_sub(1)
                        );
                    }
                    cx.notify();
                });
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(2500))
                    .await;
                let _ = this.update(cx, |_this, _cx| {
                    std::process::exit(0);
                });
            })
            .detach();
        }
        if std::env::var("PROBE_CAL_TEST").is_ok() {
            let cal_state = state.clone();
            cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(2500))
                    .await;
                let _ = this.update(cx, |_this, cx| {
                    let mut st = cal_state.borrow_mut();
                    if st.video.is_some() {
                        st.cal_length_input = "0.5".into();
                        st.cal_unit_input = "cm".into();
                        st.cal_decimals_input = "2".into();
                        let ok = st.apply_calibration_dialog();
                        let frame = st.frame;
                        let origin = st.coords.origin(frame);
                        eprintln!(
                            "[script] cal apply={ok} scale={:.3} unit={} origin=({:.1},{:.1})",
                            st.coords.scale_x(frame),
                            st.calibration.unit,
                            origin[0],
                            origin[1]
                        );
                    }
                    cx.notify();
                });
            })
            .detach();
        }
        if let Ok(path) = std::env::var("PROBE_SAVE") {
            let save_state = state.clone();
            cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(4000))
                    .await;
                let _ = this.update(cx, |_this, cx| {
                    let mut st = save_state.borrow_mut();
                    match st.save_project_to(std::path::Path::new(&path)) {
                        Ok(()) => eprintln!("[script] saved project {path}"),
                        Err(err) => eprintln!("[script] save failed: {err}"),
                    }
                    cx.notify();
                });
            })
            .detach();
        }
        if std::env::var("PROBE_EDIT").is_ok() {
            let edit_state = state.clone();
            cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(2500))
                    .await;
                let _ = this.update(cx, |_this, cx| {
                    let mut st = edit_state.borrow_mut();
                    if st.video.is_some() {
                        let (x, y) = (st.img_w * 0.5, st.img_h * 0.5);
                        let active = st.active;
                        st.apply_edit_command(EditCommand::SetPoint {
                            track: active,
                            frame: 0,
                            x,
                            y,
                        });
                        eprintln!("[script] edit dirty={}", st.is_dirty());
                    }
                    cx.notify();
                });
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(500))
                    .await;
                let _ = this.update(cx, |_this, cx| {
                    let mut st = edit_state.borrow_mut();
                    if st.video.is_some() {
                        st.undo_edit();
                        eprintln!("[script] undo dirty={}", st.is_dirty());
                    }
                    cx.notify();
                });
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(500))
                    .await;
                let _ = this.update(cx, |_this, cx| {
                    let mut st = edit_state.borrow_mut();
                    if st.video.is_some() {
                        st.redo_edit();
                        eprintln!("[script] redo dirty={}", st.is_dirty());
                    }
                    cx.notify();
                });
            })
            .detach();
        }
        if std::env::var_os("PROBE_TABS").is_some() {
            let scripted = state.clone();
            cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(3500))
                    .await;
                let _ = this.update(cx, |_this, cx| {
                    let mut st = scripted.borrow_mut();
                    let count = st.docs.len();
                    for index in 0..count {
                        if !st.docs[index].is_placeholder() {
                            st.activate_doc(index);
                            eprintln!("[script] tabs activate {index}/{count}");
                        }
                    }
                    drop(st);
                    cx.notify();
                });
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(2000))
                    .await;
                let _ = this.update(cx, |_this, _cx| {
                    std::process::exit(0);
                });
            })
            .detach();
        }
        if std::env::var_os("PROBE_PLOT").is_some() {
            let scripted = state.clone();
            cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(2500))
                    .await;
                let _ = this.update(cx, |_this, cx| {
                    let mut st = scripted.borrow_mut();
                    if st.video.is_some() {
                        _this.analysis_maximized = Some(crate::analysis::Section::Plots);
                        st.right_view = RightView::Plot;
                        st.sync_plot_for_active();
                        eprintln!(
                            "[script] plot sidebar active={} track={:?} configs={}",
                            st.active,
                            st.active_track().map(|track| track.kind_label()),
                            st.plot.configs.len()
                        );
                    }
                    cx.notify();
                });
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(3000))
                    .await;
                let _ = this.update(cx, |_this, _cx| {
                    std::process::exit(0);
                });
            })
            .detach();
        }
        if std::env::var_os("PROBE_MEASURE").is_some() {
            let scripted = state.clone();
            cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(2000))
                    .await;
                let _ = this.update(cx, |_this, cx| {
                    let mut st = scripted.borrow_mut();
                    if st.video.is_some() {
                        st.set_frame(0);
                        let tape = st.create_measurement_track(MeasurementKind::Tape);
                        st.tracks[tape].set_measurement_step(
                            0,
                            MeasurementStep {
                                handles: vec![(100.0, 100.0), (300.0, 100.0)],
                            },
                        );
                        let protractor = st.create_measurement_track(MeasurementKind::Protractor);
                        st.tracks[protractor].set_measurement_step(
                            0,
                            MeasurementStep {
                                handles: vec![(320.0, 240.0), (400.0, 240.0), (320.0, 320.0)],
                            },
                        );
                        let circle = st.create_measurement_track(MeasurementKind::Circle);
                        st.tracks[circle].set_measurement_step(
                            0,
                            MeasurementStep {
                                handles: vec![(140.0, 320.0), (260.0, 320.0), (200.0, 260.0)],
                            },
                        );
                        st.axes_visible = true;
                        st.add_calibration_stick();
                        st.active_stick = 0;
                        st.active = tape;
                        st.sync_plot_for_active();
                        st.set_plot_count(2);
                        st.plot.linked_x = true;
                        if let Some(config) = st.plot.configs.get_mut(0) {
                            config.y_var = PlotVar::Angle;
                            config.points = false;
                        }
                        st.commit_edit();
                        eprintln!(
                            "[script] measure tracks={} tape={:?} protractor={:?} circle={:?}",
                            st.tracks.len(),
                            st.tracks[tape].tape_values(0),
                            st.tracks[protractor].protractor_values(0),
                            st.tracks[circle].circle_values(0)
                        );
                        if let Some(path) = std::env::var_os("PROBE_SAVE") {
                            match st.save_project_to(std::path::Path::new(&path)) {
                                Ok(()) => eprintln!("[script] measure saved"),
                                Err(err) => eprintln!("[script] measure save failed: {err}"),
                            }
                        }
                    }
                    cx.notify();
                });
                for selection in [Selection::Stick(1), Selection::Track(2), Selection::Axes] {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(900))
                        .await;
                    let _ = this.update(cx, |_this, cx| {
                        let mut st = scripted.borrow_mut();
                        st.selection = selection;
                        cx.notify();
                    });
                }
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(1200))
                    .await;
                let _ = this.update(cx, |_this, _cx| {
                    std::process::exit(0);
                });
            })
            .detach();
        }
        if let Ok(spec) = std::env::var("PROBE_AUTOTRACK") {
            let point: Vec<f32> = spec
                .split(',')
                .filter_map(|part| part.trim().parse().ok())
                .collect();
            if point.len() == 2 {
                let scripted = state.clone();
                cx.spawn(async move |this, cx| {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(2000))
                        .await;
                    let _ = this.update(cx, |_this, cx| {
                        let mut st = scripted.borrow_mut();
                        if st.video.is_some() {
                            st.set_frame(0);
                            let active = st.active;
                            st.apply_edit_command(EditCommand::SetPoint {
                                track: active,
                                frame: 0,
                                x: point[0],
                                y: point[1],
                            });
                            st.start_tracking_ui();
                            eprintln!(
                                "[script] autotrack start ({}, {}) running={}",
                                point[0],
                                point[1],
                                st.tracking.is_some()
                            );
                        }
                        cx.notify();
                    });
                    for _ in 0..80 {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(250))
                            .await;
                        let Ok((done, count, status, running)) = this.update(cx, |_this, cx| {
                            let st = scripted.borrow();
                            let count = st.tracks.first().map(|t| t.step_count()).unwrap_or(0);
                            eprintln!(
                                "[script] autotrack frame={} points={} running={}",
                                st.frame, count, st.tracking.is_some()
                            );
                            cx.notify();
                            (
                                st.tracking.is_none(),
                                count,
                                st.status.clone().unwrap_or_default(),
                                st.tracking.is_some(),
                            )
                        }) else {
                            break;
                        };
                        if done {
                            let st = scripted.borrow();
                            let track = st.tracks.first();
                            let last = track.and_then(|track| track.point_at(st.frame));
                            let samples: Vec<_> = [0usize, 10, 30, 60, 89]
                                .into_iter()
                                .filter_map(|frame| {
                                    track.and_then(|track| track.point_at(frame)).map(|point| {
                                        (frame, point.0.round(), point.1.round())
                                    })
                                })
                                .collect();
                            eprintln!(
                                "[script] autotrack final: {status} points={count} last={last:?} samples={samples:?}"
                            );
                            drop(st);
                            if let Some(path) = std::env::var_os("PROBE_SAVE") {
                                let mut st = scripted.borrow_mut();
                                match st.save_project_to(std::path::Path::new(&path)) {
                                    Ok(()) => eprintln!(
                                        "[script] autotrack saved {}",
                                        std::path::Path::new(&path).display()
                                    ),
                                    Err(err) => eprintln!("[script] autotrack save failed: {err}"),
                                }
                            }
                            if std::env::var_os("PROBE_AUTOTRACK_UNDO").is_some() {
                                let mut st = scripted.borrow_mut();
                                let undone = st.undo_edit();
                                let after = st
                                    .tracks
                                    .first()
                                    .map(|track| track.step_count())
                                    .unwrap_or(0);
                                eprintln!(
                                    "[script] autotrack undo={undone} points_after={after}"
                                );
                            }
                            std::process::exit(if count > 1 && !running { 0 } else { 1 });
                        }
                    }
                    std::process::exit(1);
                })
                .detach();
            }
        }
        if std::env::var_os("PROBE_INSPECT").is_some() {
            let inspected = state.clone();
            cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(2500))
                    .await;
                let _ = this.update(cx, |_this, _cx| {
                    let st = inspected.borrow();
                    eprintln!(
                        "[script] inspect title={} frame={} tracks={}",
                        st.title,
                        st.frame,
                        st.tracks.len()
                    );
                    let stick_lengths: Vec<f64> = (0..st.calibration.stick_count())
                        .filter_map(|index| {
                            st.calibration
                                .stick(st.frame, index)
                                .map(|(_, _, length)| length)
                        })
                        .collect();
                    eprintln!(
                        "[script] inspect sticks={} lengths={stick_lengths:?}",
                        st.calibration.stick_count()
                    );
                    eprintln!(
                        "[script] inspect plot configs={:?} linked={}",
                        st.plot
                            .configs
                            .iter()
                            .map(|config| (config.x_var, config.y_var))
                            .collect::<Vec<_>>(),
                        st.plot.linked_x
                    );
                    for track in &st.tracks {
                        let frames = track.frames();
                        let first = frames.first().and_then(|frame| track.point_at(*frame));
                        let last = frames.last().and_then(|frame| track.point_at(*frame));
                        eprintln!(
                            "[script] inspect track name={:?} visible={} locked={} step={} auto={} footprint={:?} points={} first={first:?} last={last:?} measurement={:?} handles={:?}",
                            track.name,
                            track.visible,
                            track.locked,
                            track.step_size,
                            track.auto_advance,
                            track.footprint,
                            track.step_count(),
                            track.measurement_kind(),
                            track.handles(st.frame)
                        );
                    }
                    std::process::exit(0);
                });
            })
            .detach();
        }
        if let Ok(script) = std::env::var("PROBE_SEEK") {
            let targets: Vec<usize> = script
                .split(',')
                .filter_map(|part| part.trim().parse().ok())
                .collect();
            if !targets.is_empty() {
                let scripted = state.clone();
                cx.spawn(async move |this, cx| {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(1200))
                        .await;
                    for target in targets {
                        let _ = this.update(cx, |_this, cx| {
                            let mut st = scripted.borrow_mut();
                            st.pause();
                            st.set_frame(target);
                            drop(st);
                            if std::env::var_os("PROBE_TRACE").is_some() {
                                eprintln!("[script] seek {target}");
                            }
                            cx.notify();
                        });
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(700))
                            .await;
                    }
                })
                .detach();
            }
        }
        #[cfg(feature = "vision")]
        if std::env::var_os("PROBE_AUTODETECT_UI").is_some() {
            cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(1500))
                    .await;
                let _ = this.update(cx, |this, cx| {
                    this.start_motion_detection(cx);
                });
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(2000))
                    .await;
                let _ = this.update(cx, |this, cx| {
                    let st = this.state.borrow();
                    let points = st
                        .active_track()
                        .map(|track| track.step_count())
                        .unwrap_or(0);
                    eprintln!(
                        "[script] autodetect ui points={points} running={} status={:?}",
                        st.auto_detect_running, st.status
                    );
                    drop(st);
                    cx.notify();
                    std::process::exit(0);
                });
            })
            .detach();
        }
        cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(std::time::Duration::from_secs(5))
                .await;
            if theme_mode() != ThemeMode::Auto {
                continue;
            }
            let light = cx
                .background_executor()
                .spawn(async move { system_prefers_light() })
                .await;
            if light != light_theme() {
                LIGHT_THEME.store(light, Ordering::Relaxed);
                let _ = this.update(cx, |_this, cx| {
                    crate::theme::sync_component_theme(cx);
                    cx.notify();
                });
            }
        })
        .detach();
        crate::theme::sync_component_theme(cx);
        Self::observe_analysis_owner(cx);
        Self {
            state,
            focus: cx.focus_handle(),
            table_focus: cx.focus_handle(),
            variable_editor: None,
            inspector: crate::inspector::Inspector::default(),
            analysis_role: None,
            analysis_windows: Rc::new(RefCell::new(crate::analysis::PaneWindows::default())),
            analysis_split: None,
            analysis_maximized: None,
            table_scroll: UniformListScrollHandle::new(),
            table_x_scroll: ScrollHandle::new(),
            plot_scroll: ScrollHandle::new(),
            plot_cache: crate::plot_view::PlotCache::default(),
            table_columns_open: false,
            plot_var_menu: None,
            plot_var_pos: (0., 0.),
            focused_once: false,
            dialog_only: false,
            main: None,
            close_hook: false,
            pending_tracker_window: std::env::var_os("PROBE_TRACKER_DIALOG").is_some(),
            tracker_window: None,
            pending_settings_window: std::env::var_os("PROBE_SETTINGS").is_some(),
            settings_window: None,
            panel_layout: None,
            main_hwnd: None,
            owner_hwnd: None,
            owner_applied: false,
        }
    }

    fn new_tracker_window(
        state: Rc<RefCell<State>>,
        main: Entity<Probe>,
        owner_hwnd: Option<isize>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            state,
            focus: cx.focus_handle(),
            table_focus: cx.focus_handle(),
            variable_editor: None,
            inspector: crate::inspector::Inspector::default(),
            analysis_role: None,
            analysis_windows: Rc::new(RefCell::new(crate::analysis::PaneWindows::default())),
            analysis_split: None,
            analysis_maximized: None,
            table_scroll: UniformListScrollHandle::new(),
            table_x_scroll: ScrollHandle::new(),
            plot_scroll: ScrollHandle::new(),
            plot_cache: crate::plot_view::PlotCache::default(),
            table_columns_open: false,
            plot_var_menu: None,
            plot_var_pos: (0., 0.),
            focused_once: false,
            dialog_only: true,
            main: Some(main),
            close_hook: false,
            pending_tracker_window: false,
            tracker_window: None,
            pending_settings_window: false,
            settings_window: None,
            panel_layout: None,
            main_hwnd: None,
            owner_hwnd,
            owner_applied: false,
        }
    }

    pub(crate) fn open_tracker_window(&mut self, cx: &mut Context<Self>) {
        self.state.borrow_mut().open_tracker_dialog();
        self.state.borrow_mut().rigid_roi_mode = false;
        self.state.borrow_mut().motion_roi_mode = false;
        if let Some(handle) = self.tracker_window {
            let focused = handle
                .update(cx, |_view, window, _cx| {
                    window.activate_window();
                })
                .is_ok();
            if focused {
                cx.notify();
                return;
            }
            self.tracker_window = None;
        }
        let state = self.state.clone();
        let main = cx.entity();
        let owner_hwnd = self.main_hwnd;
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: point(px(560.), px(90.)),
                size: size(px(780.), px(600.)),
            })),
            titlebar: Some(TitlebarOptions {
                title: Some("AutoTracker".into()),
                appears_transparent: false,
                traffic_light_position: None,
            }),
            focus: true,
            show: true,
            kind: if cfg!(target_os = "macos") {
                WindowKind::Floating
            } else {
                WindowKind::Normal
            },
            is_movable: true,
            is_resizable: true,
            ..Default::default()
        };
        match gpui_kit::open_window(options, cx, move |_window, cx| {
            cx.new(|cx| Probe::new_tracker_window(state.clone(), main.clone(), owner_hwnd, cx))
        }) {
            Ok((handle, _view)) => {
                self.tracker_window = Some(handle);
            }
            Err(err) => {
                self.state.borrow_mut().status = Some(t!("msg.tracker_window_failed", err = err).to_string());
            }
        }
        cx.notify();
    }

    pub(crate) fn open_settings_window(&mut self, cx: &mut Context<Self>) {
        if let Some(handle) = self.settings_window {
            let focused = handle
                .update(cx, |_view, window, _cx| {
                    window.activate_window();
                })
                .is_ok();
            if focused {
                cx.notify();
                return;
            }
            self.settings_window = None;
        }
        let state = self.state.clone();
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: point(px(200.), px(90.)),
                size: size(px(720.), px(520.)),
            })),
            titlebar: Some(TitlebarOptions {
                title: Some(tr("settings.title").into()),
                appears_transparent: false,
                traffic_light_position: None,
            }),
            focus: true,
            show: true,
            kind: WindowKind::Normal,
            is_movable: true,
            is_resizable: true,
            ..Default::default()
        };
        match gpui_kit::open_window(options, cx, move |window, cx| {
            cx.new(|cx| crate::settings::SettingsWindow::new(state.clone(), window, cx))
        }) {
            Ok((handle, _view)) => {
                self.settings_window = Some(handle);
            }
            Err(err) => {
                self.state.borrow_mut().status =
                    Some(t!("msg.settings_window_failed", err = err).to_string());
            }
        }
        cx.notify();
    }

    #[cfg(feature = "vision")]
    pub(crate) fn start_motion_detection(&mut self, cx: &mut Context<Self>) {
        let (source, doc_id) = {
            let mut st = self.state.borrow_mut();
            if st.auto_detect_running {
                return;
            }
            let Some(source) = st.video.as_ref().map(|video| video.frame_source()) else {
                st.status = Some(tr("msg.open_video_first"));
                drop(st);
                cx.notify();
                return;
            };
            let doc_id = st.docs[st.active_doc].id;
            st.auto_detect_running = true;
            st.rigid_roi_mode = false;
            st.motion_roi_mode = false;
            st.status = Some(tr("msg.auto_detect.running"));
            (source, doc_id)
        };
        cx.notify();
        let state = self.state.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { crate::motion_detect::detect_motion(&source, None) })
                .await;
            let _ = this.update(cx, |_this, cx| {
                let mut st = state.borrow_mut();
                st.auto_detect_running = false;
                if st.docs[st.active_doc].id != doc_id {
                    st.status = Some(tr("msg.auto_detect.discarded"));
                    cx.notify();
                    return;
                }
                match result {
                    Ok(result) => {
                        let points: Vec<_> = result
                            .points
                            .iter()
                            .map(|point| (point.frame, point.x, point.y))
                            .collect();
                        let count = st.add_auto_points(&points, &tr("track_name.auto_detect"));
                        st.status = Some(
                            t!(
                                "msg.auto_detect.done",
                                count = count,
                                candidates = result.candidates
                            )
                            .to_string(),
                        );
                    }
                    Err(err) => {
                        st.status = Some(t!("msg.auto_detect.failed", err = err).to_string());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    #[cfg(feature = "vision")]
    pub(crate) fn start_rigid_tracking(&mut self, cx: &mut Context<Self>) {
        let (source, doc_id) = {
            let mut st = self.state.borrow_mut();
            if st.auto_detect_running {
                return;
            }
            let Some(source) = st.video.as_ref().map(|video| video.frame_source()) else {
                st.status = Some(tr("msg.open_video_first"));
                drop(st);
                cx.notify();
                return;
            };
            let doc_id = st.docs[st.active_doc].id;
            st.auto_detect_running = true;
            st.rigid_roi_mode = false;
            st.motion_roi_mode = false;
            st.status = Some(tr("msg.aruco.running"));
            (source, doc_id)
        };
        cx.notify();
        let state = self.state.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { crate::rigid::detect_rigid(&source) })
                .await;
            let _ = this.update(cx, |_this, cx| {
                let mut st = state.borrow_mut();
                st.auto_detect_running = false;
                if st.docs[st.active_doc].id != doc_id {
                    st.status = Some(tr("msg.aruco.discarded"));
                    cx.notify();
                    return;
                }
                match result {
                    Ok(result) => {
                        let steps: Vec<_> = result
                            .points
                            .iter()
                            .map(|point| {
                                (
                                    point.frame,
                                    point.x,
                                    point.y,
                                    point.angle,
                                    point.scale,
                                    point.marker_count,
                                )
                            })
                            .collect();
                        let markers = result
                            .markers
                            .iter()
                            .map(|marker| RigidBodyMarker {
                                id: marker.id,
                                corners: marker.corners,
                            })
                            .collect();
                        let count = st.add_rigid_body(
                            result.dictionary.clone(),
                            markers,
                            Vec::new(),
                            &steps,
                            &tr("track_name.rigid_detect"),
                        );
                        st.status = Some(
                            t!(
                                "msg.aruco.done",
                                count = count,
                                markers = result.markers.len(),
                                dict = result.dictionary
                            )
                            .to_string(),
                        );
                    }
                    Err(err) => {
                        st.status = Some(t!("msg.aruco.failed", err = err).to_string());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    #[cfg(feature = "vision")]
    pub(crate) fn start_orb_tracking(
        &mut self,
        roi: (f32, f32, f32, f32),
        reference_frame: usize,
        cx: &mut Context<Self>,
    ) {
        let (source, doc_id) = {
            let mut st = self.state.borrow_mut();
            if st.auto_detect_running {
                return;
            }
            let Some(source) = st.video.as_ref().map(|video| video.frame_source()) else {
                st.status = Some(tr("msg.open_video_first"));
                drop(st);
                cx.notify();
                return;
            };
            let doc_id = st.docs[st.active_doc].id;
            st.rigid_roi_mode = false;
            st.motion_roi_mode = false;
            st.auto_detect_running = true;
            st.status = Some(tr("msg.orb.running"));
            (source, doc_id)
        };
        cx.notify();
        let state = self.state.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { crate::orb_track::detect_orb(&source, roi, reference_frame) })
                .await;
            let _ = this.update(cx, |_this, cx| {
                let mut st = state.borrow_mut();
                st.auto_detect_running = false;
                if st.docs[st.active_doc].id != doc_id {
                    st.status = Some(tr("msg.orb.discarded"));
                    cx.notify();
                    return;
                }
                match result {
                    Ok(result) => {
                        let steps: Vec<_> = result
                            .points
                            .iter()
                            .map(|point| {
                                (
                                    point.frame,
                                    point.x,
                                    point.y,
                                    point.angle,
                                    point.scale,
                                    point.marker_count,
                                )
                            })
                            .collect();
                        let count = st.add_rigid_body(
                            "ORB".into(),
                            Vec::new(),
                            Vec::new(),
                            &steps,
                            &tr("track_name.markerless"),
                        );
                        st.status = Some(
                            t!(
                                "msg.orb.done",
                                count = count,
                                frames = result.detections
                            )
                            .to_string(),
                        );
                    }
                    Err(err) => {
                        st.status = Some(t!("msg.orb.failed", err = err).to_string());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    #[cfg(feature = "vision")]
    pub(crate) fn start_motion_contour_tracking(
        &mut self,
        roi: (f32, f32, f32, f32),
        reference_frame: usize,
        cx: &mut Context<Self>,
    ) {
        let (source, doc_id, pure_mask) = {
            let mut st = self.state.borrow_mut();
            if st.auto_detect_running {
                return;
            }
            let Some(source) = st.video.as_ref().map(|video| video.frame_source()) else {
                st.status = Some(tr("msg.open_video_first"));
                drop(st);
                cx.notify();
                return;
            };
            let doc_id = st.docs[st.active_doc].id;
            let pure_mask = st.motion_pure_mask;
            st.rigid_roi_mode = false;
            st.motion_roi_mode = false;
            st.auto_detect_running = true;
            st.status = Some(if pure_mask {
                tr("msg.motion.running_pure")
            } else {
                tr("msg.motion.running_fixed")
            });
            (source, doc_id, pure_mask)
        };
        cx.notify();
        let state = self.state.clone();
        cx.spawn(async move |this, cx| {
            let options = crate::motion_contour::MotionOptions {
                pure_mask,
                ..Default::default()
            };
            let result = cx
                .background_executor()
                .spawn(async move {
                    crate::motion_contour::detect_motion_contour(
                        &source,
                        roi,
                        reference_frame,
                        options,
                    )
                })
                .await;
            let _ = this.update(cx, |_this, cx| {
                let mut st = state.borrow_mut();
                st.auto_detect_running = false;
                if st.docs[st.active_doc].id != doc_id {
                    st.status = Some(tr("msg.motion.discarded"));
                    cx.notify();
                    return;
                }
                match result {
                    Ok(result) => {
                        let steps: Vec<_> = result
                            .points
                            .iter()
                            .map(|point| {
                                (
                                    point.frame,
                                    point.x,
                                    point.y,
                                    point.angle,
                                    point.scale,
                                    point.marker_count,
                                )
                            })
                            .collect();
                        let label = if result.mask_mode == RigidMaskMode::PureMotion {
                            tr("track_name.motion_pure")
                        } else {
                            tr("track_name.motion_fixed")
                        };
                        let count = st.add_rigid_body_with(
                            "MOTION".into(),
                            Vec::new(),
                            result.contour.clone(),
                            &steps,
                            &label,
                            result.mask_mode,
                            &result.step_contours,
                        );
                        st.status = Some(
                            t!(
                                "msg.motion.done",
                                label = label,
                                count = count,
                                frames = result.detections
                            )
                            .to_string(),
                        );
                    }
                    Err(err) => {
                        st.status = Some(t!("msg.motion.failed", err = err).to_string());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

#[derive(Clone)]
pub(crate) enum SaveAction {
    None,
    CloseDoc(usize),
    NewProject,
    OpenProject(PathBuf),
}

pub(crate) fn spawn_open(
    state: Rc<RefCell<State>>,
    id: u64,
    path: PathBuf,
    progress: Arc<AtomicU32>,
    cx: &mut Context<Probe>,
) {
    cx.spawn(async move |this, cx| {
        let opened = cx
            .background_executor()
            .spawn(async move { VideoStream::open_with_progress(path, 0, Some(progress)) })
            .await;
        let _ = this.update(cx, |_this, cx| {
            state.borrow_mut().finish_load(id, opened);
            cx.notify();
        });
    })
    .detach();
}

pub(crate) fn open_video(state: Rc<RefCell<State>>, path: PathBuf, cx: &mut Context<Probe>) {
    let Some((id, progress)) = state.borrow_mut().begin_load(path.clone()) else {
        cx.notify();
        return;
    };
    cx.notify();
    spawn_open(state, id, path, progress, cx);
}

pub(crate) fn open_project_file(state: Rc<RefCell<State>>, path: PathBuf, cx: &mut Context<Probe>) {
    let project = match ProjectFile::load(&path) {
        Ok(project) => project,
        Err(err) => {
            state.borrow_mut().modal = Some(Modal::Error(err));
            cx.notify();
            return;
        }
    };
    let loads = state.borrow_mut().prepare_project_load(project, path);
    cx.notify();
    for (id, video_path, progress) in loads {
        spawn_open(state.clone(), id, video_path, progress, cx);
    }
}

pub(crate) fn perform_save_action(
    state: &Rc<RefCell<State>>,
    action: SaveAction,
    cx: &mut Context<Probe>,
) {
    match action {
        SaveAction::None => {}
        SaveAction::CloseDoc(index) => state.borrow_mut().close_doc(index),
        SaveAction::NewProject => state.borrow_mut().new_project(),
        SaveAction::OpenProject(path) => {
            open_project_file(state.clone(), path, cx);
            return;
        }
    }
    state.borrow_mut().modal = None;
    cx.notify();
}

pub(crate) fn request_save(state: Rc<RefCell<State>>, action: SaveAction, cx: &mut Context<Probe>) {
    request_save_impl(state, action, false, cx);
}

pub(crate) fn request_save_as(
    state: Rc<RefCell<State>>,
    action: SaveAction,
    cx: &mut Context<Probe>,
) {
    request_save_impl(state, action, true, cx);
}

pub(crate) fn request_save_impl(
    state: Rc<RefCell<State>>,
    action: SaveAction,
    force_as: bool,
    cx: &mut Context<Probe>,
) {
    if !force_as {
        // Release the read borrow before saving or executing the follow-up action.
        let project_path = state.borrow().project_path.clone();
        if let Some(path) = project_path {
            let result = state.borrow_mut().save_project_to(&path);
            match result {
                Ok(()) => perform_save_action(&state, action, cx),
                Err(err) => {
                    state.borrow_mut().modal = Some(Modal::Error(err));
                    cx.notify();
                }
            }
            return;
        }
    }

    let directory = state
        .borrow()
        .workdir
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let receiver = cx.prompt_for_new_path(&directory, Some("untitled.tgp"));
    cx.spawn(async move |this, cx| {
        if let Ok(Ok(Some(mut path))) = receiver.await {
            if path.extension().is_none() {
                path.set_extension("tgp");
            }
            let _ = this.update(cx, |_this, cx| {
                let result = state.borrow_mut().save_project_to(&path);
                match result {
                    Ok(()) => perform_save_action(&state, action.clone(), cx),
                    Err(err) => {
                        state.borrow_mut().modal = Some(Modal::Error(err));
                        cx.notify();
                    }
                }
            });
        }
    })
    .detach();
}

pub(crate) fn request_open_project_path(
    state: Rc<RefCell<State>>,
    path: PathBuf,
    cx: &mut Context<Probe>,
) {
    if state.borrow().has_dirty_docs() {
        state.borrow_mut().modal = Some(Modal::ConfirmOpenProject(path));
    } else {
        open_project_file(state, path, cx);
        return;
    }
    cx.notify();
}

pub(crate) fn request_open_project(state: Rc<RefCell<State>>, cx: &mut Context<Probe>) {
    let receiver = cx.prompt_for_paths(PathPromptOptions {
        files: true,
        directories: false,
        multiple: false,
        prompt: Some(tr("dialog.open_project_title").into()),
    });
    cx.spawn(async move |this, cx| {
        if let Ok(Ok(Some(paths))) = receiver.await {
            if let Some(path) = paths.into_iter().next() {
                let _ = this.update(cx, |_this, cx| {
                    request_open_project_path(state.clone(), path, cx);
                });
            }
        }
    })
    .detach();
}

pub(crate) fn request_choose_folder(state: Rc<RefCell<State>>, cx: &mut Context<Probe>) {
    let receiver = cx.prompt_for_paths(PathPromptOptions {
        files: false,
        directories: true,
        multiple: false,
        prompt: Some(tr("dialog.choose_folder_title").into()),
    });
    cx.spawn(async move |this, cx| {
        if let Ok(Ok(Some(paths))) = receiver.await {
            if let Some(path) = paths.into_iter().next() {
                let _ = this.update(cx, |_this, cx| {
                    state.borrow_mut().set_workdir(path);
                    cx.notify();
                });
            }
        }
    })
    .detach();
}

pub(crate) fn request_new_project(state: Rc<RefCell<State>>, cx: &mut Context<Probe>) {
    if state.borrow().has_dirty_docs() {
        state.borrow_mut().modal = Some(Modal::ConfirmNew);
    } else {
        state.borrow_mut().new_project();
    }
    cx.notify();
}

pub(crate) fn request_close_doc(state: Rc<RefCell<State>>, index: usize, cx: &mut Context<Probe>) {
    let dirty = state
        .borrow()
        .docs
        .get(index)
        .map(|doc| doc.is_dirty())
        .unwrap_or(false);
    if dirty {
        state.borrow_mut().modal = Some(Modal::CloseDirty { index });
    } else {
        state.borrow_mut().close_doc(index);
    }
    cx.notify();
}

#[cfg(target_os = "windows")]
pub(crate) fn native_hwnd(window: &Window) -> Option<isize> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let handle = HasWindowHandle::window_handle(window).ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(win32) => Some(win32.hwnd.get()),
        _ => None,
    }
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn native_hwnd(_window: &Window) -> Option<isize> {
    None
}

#[cfg(target_os = "windows")]
pub(crate) fn set_native_owner(hwnd: isize, owner: isize) {
    const GWLP_HWNDPARENT: i32 = -8;
    unsafe extern "system" {
        fn SetWindowLongPtrW(hwnd: isize, index: i32, value: isize) -> isize;
        fn GetWindow(hwnd: isize, command: u32) -> isize;
    }
    const GW_OWNER: u32 = 4;
    unsafe {
        SetWindowLongPtrW(hwnd, GWLP_HWNDPARENT, owner);
        if std::env::var_os("PROBE_OWNER_CHECK").is_some() {
            let actual = GetWindow(hwnd, GW_OWNER);
            eprintln!(
                "[script] tracker owner hwnd={hwnd} main={owner} actual={actual} ok={}",
                actual == owner
            );
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn set_native_owner(_hwnd: isize, _owner: isize) {}

#[cfg(test)]
mod save_tests {
    use super::*;
    use ::gpui;
    use core::prelude::v1::test;

    #[gpui::test]
    fn saving_opened_project_releases_borrows_before_save_and_follow_up(cx: &mut TestAppContext) {
        let directory = std::env::temp_dir().join(format!(
            "tracker-gpui-save-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("opened.tgp");
        ProjectFile::new(None, 0, vec![]).save(&path).unwrap();
        let mut state = State::with_preferences(Preferences::default());
        state.prepare_project_load(ProjectFile::load(&path).unwrap(), path.clone());
        let state = Rc::new(RefCell::new(state));
        let probe = cx.new(|cx| {
            Probe::analysis_instance(
                state.clone(),
                None,
                Rc::new(RefCell::new(crate::analysis::PaneWindows::default())),
                cx,
            )
        });
        let make_dirty = || {
            let mut state = state.borrow_mut();
            state.coords.base.origin[0] += 1.;
            let snapshot = state.snapshot();
            state.history.record(snapshot);
            assert!(state.has_dirty_docs());
        };

        make_dirty();
        probe.update(cx, |_, cx| request_save(state.clone(), SaveAction::None, cx));
        assert!(ProjectFile::load(&path).is_ok());
        assert_eq!(state.borrow().project_path.as_ref(), Some(&path));
        assert!(!state.borrow().has_dirty_docs());
        assert!(state.borrow().modal.is_none());

        // A write failure must report an error and preserve unsaved work.
        make_dirty();
        state.borrow_mut().project_path = Some(directory.clone());
        probe.update(cx, |_, cx| request_save(state.clone(), SaveAction::NewProject, cx));
        assert!(matches!(state.borrow().modal, Some(Modal::Error(_))));
        assert!(state.borrow().has_dirty_docs());
        assert_eq!(state.borrow().project_path.as_ref(), Some(&directory));

        state.borrow_mut().project_path = Some(path.clone());
        probe.update(cx, |_, cx| request_save(state.clone(), SaveAction::NewProject, cx));
        assert!(state.borrow().project_path.is_none());
        assert!(state.borrow().modal.is_none());
        assert!(!state.borrow().has_dirty_docs());
        assert!(ProjectFile::load(&path).is_ok());
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }
}
