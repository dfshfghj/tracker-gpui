use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use crate::*;

impl DocState {
    pub(crate) fn placeholder() -> Self {
        Self {
            id: 0,
            path: PathBuf::new(),
            title: "未打开视频".into(),
            frame: 0,
            playing: false,
            play_start: None,
            play_frame: 0,
            tracks: Vec::new(),
            active: 0,
            next_track_id: 1,
            collapsed: HashSet::new(),
            plot: PlotState::default(),
            coords: ImageCoordSystem::default(),
            calibration: Calibration::default(),
            active_stick: 0,
            selection: Selection::None,
            axes_visible: false,
            trail_visible: true,
            trail_length: 15,
            labels_visible: true,
            trace_visible: false,
            table_columns: [true, true, true, false, false],
            table_selection: HashSet::new(),
            scale: 1.0,
            tx: 0.0,
            ty: 0.0,
            fitted: false,
            user_adjusted: false,
            sx: 0.0,
            sy: 0.0,
            sw: 1.0,
            sh: 1.0,
            created: Instant::now(),
            draw_ms: 0.0,
            fps: 0.0,
            frames_counted: 0,
            fps_t: Instant::now(),
            drawn_points: 0,
            frame_count: 0,
            img_w: 0.0,
            img_h: 0.0,
            video: None,
            frame_image: None,
            retired: VecDeque::new(),
            to_drop: Vec::new(),
            last_requested: None,
            ready: None,
            loading: None,
            error: None,
            status: None,
            pending_restore: None,
            history: UndoStack::new(EditSnapshot {
                tracks: Vec::new(),
                active: 0,
                coords: ImageCoordSystem::default(),
                calibration: Calibration::default(),
            }),
            tracking: None,
            tracker_overlay: None,
            tracker_open: false,
        }
    }

    pub(crate) fn loading(id: u64, path: PathBuf, name: String, progress: Arc<AtomicU32>) -> Self {
        let mut doc = Self::placeholder();
        doc.id = id;
        doc.path = path;
        doc.title = name.clone();
        doc.loading = Some(LoadingVideo { name, progress });
        doc
    }

    pub(crate) fn is_placeholder(&self) -> bool {
        self.id == 0 && self.path.as_os_str().is_empty()
    }

    pub(crate) fn active_track(&self) -> Option<&Track> {
        self.tracks.get(self.active)
    }

    pub(crate) fn active_track_mut(&mut self) -> Option<&mut Track> {
        let active = self.active;
        self.tracks.get_mut(active)
    }

    pub(crate) fn delete_track_by_id(&mut self, id: u64) -> bool {
        let Some(index) = self.tracks.iter().position(|track| track.id == id) else {
            return false;
        };
        self.tracks.remove(index);
        self.active = self.active.min(self.tracks.len().saturating_sub(1));
        self.selection = Selection::None;
        self.sync_plot_for_active();
        self.commit_edit();
        true
    }

    pub(crate) fn delete_selected_object(&mut self) -> bool {
        match self.selection {
            Selection::Stick(index) => {
                self.calibration.remove_stick(index);
                self.active_stick = self
                    .active_stick
                    .min(self.calibration.stick_count().saturating_sub(1));
                self.selection = Selection::None;
                self.commit_edit();
                true
            }
            Selection::Track(id) => self.delete_track_by_id(id),
            Selection::Axes | Selection::None => false,
        }
    }

    pub(crate) fn sync_plot_for_active(&mut self) {
        let active = self.active;
        if let Some(track) = self.tracks.get(active) {
            if track.is_point_mass() {
                self.plot.ensure_for_track(track);
            }
        } else {
            self.plot = PlotState::default();
        }
    }

    pub(crate) fn table_tsv(&self) -> String {
        let Some(track) = self.active_track().filter(|track| track.is_point_mass()) else {
            return String::new();
        };
        let unit = self.calibration.unit.clone();
        let decimals = self.calibration.decimals;
        let mut headers = Vec::new();
        if self.table_columns[0] {
            headers.push("t (s)".to_string());
        }
        if self.table_columns[1] {
            headers.push(format!("x ({unit})"));
        }
        if self.table_columns[2] {
            headers.push(format!("y ({unit})"));
        }
        if self.table_columns[3] {
            headers.push(format!("vx ({unit}/s)"));
        }
        if self.table_columns[4] {
            headers.push(format!("vy ({unit}/s)"));
        }
        if headers.is_empty() {
            return String::new();
        }
        let selected = if self.table_selection.is_empty() {
            None
        } else {
            Some(&self.table_selection)
        };
        let frame_times = self
            .video
            .as_ref()
            .map(|video| video.meta.frame_times.as_slice())
            .unwrap_or(&[]);
        let mut text = headers.join("	")
            + "
";
        for (frame, image_x, image_y) in track.points() {
            if selected.map_or(false, |frames| !frames.contains(&frame)) {
                continue;
            }
            let (world_x, world_y) = track
                .world_position(frame, &self.coords)
                .unwrap_or_else(|| self.to_world(image_x, image_y));
            let velocity = track.world_velocity(frame, &self.coords, frame_times);
            let mut values = Vec::new();
            if self.table_columns[0] {
                values.push(format!("{:.3}", self.frame_time(frame)));
            }
            if self.table_columns[1] {
                values.push(format!("{:.*}", decimals, world_x));
            }
            if self.table_columns[2] {
                values.push(format!("{:.*}", decimals, world_y));
            }
            if self.table_columns[3] {
                values.push(
                    velocity
                        .filter(|value| value.0.is_finite())
                        .map(|value| format!("{:.*}", decimals, value.0))
                        .unwrap_or_else(|| "-".to_string()),
                );
            }
            if self.table_columns[4] {
                values.push(
                    velocity
                        .filter(|value| value.1.is_finite())
                        .map(|value| format!("{:.*}", decimals, value.1))
                        .unwrap_or_else(|| "-".to_string()),
                );
            }
            text.push_str(&values.join("\t"));
            text.push('\n');
        }
        text
    }

    pub(crate) fn set_plot_count(&mut self, count: usize) {
        let active = self.active;
        if let Some(track) = self.tracks.get(active) {
            self.plot.set_count(count, track);
        }
    }

    pub(crate) fn toggle_plot_points(&mut self) {
        let show = !self.plot.configs.iter().all(|config| config.points);
        for config in &mut self.plot.configs {
            config.points = show;
        }
    }

    pub(crate) fn toggle_plot_lines(&mut self) {
        let show = !self.plot.configs.iter().all(|config| config.lines);
        for config in &mut self.plot.configs {
            config.lines = show;
        }
    }

    pub(crate) fn set_plot_var(&mut self, plot: usize, axis: PlotAxis, var: PlotVar) {
        if let Some(config) = self.plot.configs.get_mut(plot) {
            match axis {
                PlotAxis::X => config.x_var = var,
                PlotAxis::Y => config.y_var = var,
            }
            config.x_range = None;
            config.y_range = None;
        }
    }

    pub(crate) fn snapshot(&self) -> EditSnapshot {
        EditSnapshot {
            tracks: self.tracks.clone(),
            active: self.active,
            coords: self.coords.clone(),
            calibration: self.calibration.clone(),
        }
    }

    pub(crate) fn restore_snapshot(&mut self, snapshot: &EditSnapshot) {
        self.tracks = snapshot.tracks.clone();
        self.active = if self.tracks.is_empty() {
            0
        } else {
            snapshot.active.min(self.tracks.len() - 1)
        };
        self.coords = snapshot.coords.clone();
        self.calibration = snapshot.calibration.clone();
        self.active_stick = self
            .active_stick
            .min(self.calibration.stick_count().saturating_sub(1));
        self.selection = match self.selection {
            Selection::Stick(index) if index >= self.calibration.stick_count() => Selection::None,
            Selection::Track(id) if !self.tracks.iter().any(|track| track.id == id) => {
                Selection::None
            }
            other => other,
        };
    }

    pub(crate) fn commit_edit(&mut self) {
        if self.video.is_none() {
            return;
        }
        let snapshot = self.snapshot();
        self.history.record(snapshot);
    }

    pub(crate) fn apply_edit_command(&mut self, command: EditCommand) -> bool {
        self.apply_edit_command_impl(command, true)
    }

    pub(crate) fn apply_edit_command_transient(&mut self, command: EditCommand) -> bool {
        self.apply_edit_command_impl(command, false)
    }

    pub(crate) fn apply_edit_command_impl(&mut self, command: EditCommand, record: bool) -> bool {
        if self.video.is_none() {
            return false;
        }
        match &command {
            EditCommand::SetPoint { track, .. } => {
                let Some(track) = self.tracks.get(*track) else {
                    return false;
                };
                if track.locked || !track.is_point_mass() {
                    return false;
                }
            }
            EditCommand::DeletePoint { track, frame } => {
                let Some(track) = self.tracks.get(*track) else {
                    return false;
                };
                if track.locked || !track.has_point(*frame) {
                    return false;
                }
            }
            EditCommand::SetOrigin { .. } if self.coords.locked => return false,
            EditCommand::ResetCalibration if self.img_w <= 0.0 || self.img_h <= 0.0 => {
                return false;
            }
            _ => {}
        }

        match command {
            EditCommand::SetPoint { track, frame, x, y } => {
                self.tracks[track].set_point(frame, x, y);
                self.tracks[track].add_keyframe(frame);
            }
            EditCommand::DeletePoint { track, frame } => {
                self.tracks[track].delete_point(frame);
            }
            EditCommand::SetCalibration { a, b, length } => {
                let n = self.frame;
                self.calibration.set_point(n, 0, a[0], a[1]);
                self.calibration.set_point(n, 1, b[0], b[1]);
                self.calibration.set_length(n, length as f64);
                self.calibration.apply(n, &mut self.coords);
            }
            EditCommand::ResetCalibration => {
                self.calibration = Calibration::new(self.img_w, self.img_h);
                self.coords = ImageCoordSystem::default();
                self.calibration.apply(self.frame, &mut self.coords);
            }
            EditCommand::SetOrigin { frame, x, y } => {
                self.coords.set_origin(frame, [x as f64, y as f64]);
            }
        }
        if record {
            self.history.record(self.snapshot());
        }
        true
    }

    pub(crate) fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub(crate) fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    pub(crate) fn undo_edit(&mut self) -> bool {
        let Some(snapshot) = self.history.undo() else {
            return false;
        };
        self.restore_snapshot(&snapshot);
        true
    }

    pub(crate) fn redo_edit(&mut self) -> bool {
        let Some(snapshot) = self.history.redo() else {
            return false;
        };
        self.restore_snapshot(&snapshot);
        true
    }

    pub(crate) fn add_point_mass_track(&mut self) {
        if self.video.is_none() {
            return;
        }
        let id = self.next_track_id;
        self.next_track_id += 1;
        let color = PALETTE[(id as usize - 1) % PALETTE.len()];
        self.tracks
            .push(Track::point_mass(id, format!("Track {id}"), color));
        self.active = self.tracks.len() - 1;
        self.selection = Selection::Track(id);
        self.sync_plot_for_active();
        self.commit_edit();
    }

    pub(crate) fn delete_active_track(&mut self) {
        if self.video.is_none() || self.tracks.is_empty() {
            return;
        }
        self.tracks.remove(self.active);
        self.active = self.active.min(self.tracks.len().saturating_sub(1));
        self.selection = Selection::None;
        self.sync_plot_for_active();
        self.commit_edit();
    }

    pub(crate) fn ensure_point_mass_track(&mut self) -> usize {
        if self
            .active_track()
            .map(|track| track.is_point_mass())
            .unwrap_or(false)
        {
            return self.active;
        }
        if let Some(index) = self.tracks.iter().position(|track| track.is_point_mass()) {
            self.active = index;
            return index;
        }
        let id = self.next_track_id;
        self.next_track_id += 1;
        let color = PALETTE[(id as usize - 1) % PALETTE.len()];
        self.tracks
            .push(Track::point_mass(id, format!("Track {id}"), color));
        self.active = self.tracks.len() - 1;
        self.sync_plot_for_active();
        self.active
    }

    pub(crate) fn create_measurement_track(&mut self, kind: MeasurementKind) -> usize {
        let id = self.next_track_id;
        self.next_track_id += 1;
        let color = PALETTE[(id as usize - 1) % PALETTE.len()];
        self.tracks.push(Track::measurement(
            id,
            kind,
            format!("{} {}", kind.label(), id),
            color,
        ));
        self.active = self.tracks.len() - 1;
        self.selection = Selection::Track(id);
        self.sync_plot_for_active();
        self.active
    }

    pub(crate) fn ensure_measurement_track(&mut self, kind: MeasurementKind) -> usize {
        if self
            .active_track()
            .map(|track| track.measurement_kind() == Some(kind))
            .unwrap_or(false)
        {
            return self.active;
        }
        if let Some(index) = self
            .tracks
            .iter()
            .position(|track| track.measurement_kind() == Some(kind))
        {
            self.active = index;
            self.sync_plot_for_active();
            return index;
        }
        self.create_measurement_track(kind)
    }

    pub(crate) fn start_tracking(
        &mut self,
        settings: AutoTrackSettings,
        single_step: bool,
    ) -> Result<(), String> {
        if self.tracking.is_some() {
            return Err("自动跟踪已在运行".into());
        }
        if self.video.is_none() {
            return Err("未打开视频".into());
        }
        let Some(track) = self.active_track() else {
            return Err("没有可跟踪的轨道".into());
        };
        if !track.is_point_mass() {
            return Err("自动跟踪仅支持质点轨道".into());
        }
        if track.locked {
            return Err("当前轨道已锁定".into());
        }
        let Some((x, y)) = track.point_at(self.frame) else {
            return Err("请先在当前帧标记跟踪起点".into());
        };
        if self.frame >= self.frame_count.saturating_sub(1) {
            return Err("当前已是最后一帧".into());
        }
        let track_id = track.id;
        let step = track.step_size.max(1);
        let start_frame = self.frame;
        let end_frame = self.frame_count.saturating_sub(1);
        let source = self.video.as_ref().unwrap().frame_source();
        self.pause();
        let settings = settings.normalized();

        if let Some(track) = self.tracks.get_mut(self.active) {
            track.add_keyframe(start_frame);
            track.clear_points_after(start_frame, true);
        }
        self.commit_edit();

        let backend = Box::new(TemplateMatchBackend::new(settings.clone()));
        let backend_name = backend.name();
        let (tx, rx) = std::sync::mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let paused = Arc::new(AtomicBool::new(false));
        tracking::spawn_auto_track(
            AutoTrackRequest {
                source,
                backend,
                start_frame,
                end_frame,
                start_position: (x, y),
                step,
                search_policy: settings.search_policy,
                stop_policy: settings.stop_policy,
                single_step,
            },
            tx,
            stop.clone(),
            paused.clone(),
        )?;
        self.tracking = Some(TrackingRun {
            track_id,
            rx,
            stop,
            paused,
            paused_ui: false,
            fast: false,
            settings: settings.clone(),
            start_frame,
            last_frame: start_frame,
            last_confidence: f32::NAN,
            points: 0,
            search_frame: start_frame,
            search_center: Some((x, y)),
            last_match: Some((x, y)),
            scores: Vec::new(),
        });
        self.tracker_overlay = Some(TrackerOverlay {
            track_id,
            key_frame: start_frame,
            key_center: (x, y),
            mask_width: settings.mask_width,
            mask_height: settings.mask_height,
            shape: settings.shape,
            search_size: settings.search_size,
            search_center: (x, y),
            search_frame: start_frame,
            match_position: Some((x, y)),
        });
        self.status = Some(if single_step {
            format!("{backend_name} 搜索下一帧…")
        } else {
            format!("{backend_name} 自动跟踪中…")
        });
        Ok(())
    }

    pub(crate) fn stop_tracking(&mut self) -> bool {
        let Some(run) = self.tracking.take() else {
            return false;
        };
        run.stop.store(true, Ordering::Relaxed);
        if run.points > 0 {
            self.commit_edit();
        }
        if !self.tracker_open {
            self.tracker_overlay = None;
        }
        self.status = Some(format!(
            "自动跟踪已停止 · {} 点 · 到第 {} 帧",
            run.points, run.last_frame
        ));
        true
    }

    pub(crate) fn pause_tracking(&mut self) -> bool {
        let Some(run) = &mut self.tracking else {
            return false;
        };
        run.paused.store(true, Ordering::Relaxed);
        run.paused_ui = true;
        let frame = run.last_frame;
        self.frame = frame.min(self.frame_count.saturating_sub(1));
        self.last_requested = None;
        self.ready = None;
        self.status = Some(format!(
            "AutoTracker 已暂停 · 第 {} 帧 · {} 点",
            run.last_frame, run.points
        ));
        true
    }

    pub(crate) fn resume_tracking(&mut self) -> bool {
        let Some(run) = &mut self.tracking else {
            return false;
        };
        run.paused.store(false, Ordering::Relaxed);
        run.paused_ui = false;
        self.status = Some("AutoTracker 搜索中…".into());
        true
    }

    pub(crate) fn toggle_tracking_pause(&mut self) -> bool {
        if self
            .tracking
            .as_ref()
            .map(|run| run.paused_ui)
            .unwrap_or(false)
        {
            self.resume_tracking()
        } else {
            self.pause_tracking()
        }
    }

    pub(crate) fn poll_tracking(&mut self) -> bool {
        let Some(mut run) = self.tracking.take() else {
            return false;
        };
        let mut messages = Vec::new();
        let mut disconnected = false;
        loop {
            match run.rx.try_recv() {
                Ok(message) => messages.push(message),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    disconnected = true;
                    break;
                }
            }
        }

        let mut changed = false;
        let mut finish: Option<(AutoTrackEnd, usize, f32, String)> = None;
        for message in messages {
            match message {
                AutoTrackMessage::Point {
                    frame,
                    position,
                    hint,
                    confidence,
                } => {
                    let Some(track) = self
                        .tracks
                        .iter_mut()
                        .find(|track| track.id == run.track_id)
                    else {
                        finish = Some((
                            AutoTrackEnd::Stopped,
                            run.last_frame,
                            run.last_confidence,
                            "轨道已删除".into(),
                        ));
                        break;
                    };
                    if track.locked {
                        finish = Some((
                            AutoTrackEnd::Stopped,
                            run.last_frame,
                            run.last_confidence,
                            "轨道已锁定".into(),
                        ));
                        break;
                    }
                    track.set_point(frame, position.0, position.1);
                    run.last_frame = frame;
                    run.last_confidence = confidence;
                    run.search_frame = frame;
                    run.search_center = Some(hint);
                    run.last_match = Some(position);
                    run.scores.push((frame, confidence));
                    run.points += 1;
                    if let Some(overlay) = &mut self.tracker_overlay {
                        overlay.search_frame = frame;
                        overlay.search_center = hint;
                        overlay.match_position = Some(position);
                    }
                    if !run.fast && !self.playing {
                        self.frame = frame.min(self.frame_count.saturating_sub(1));
                        self.last_requested = None;
                        self.ready = None;
                    }
                    changed = true;
                }
                AutoTrackMessage::Miss {
                    frame,
                    position,
                    hint,
                    confidence,
                    quality,
                } => {
                    run.last_frame = frame;
                    run.last_confidence = confidence;
                    run.search_frame = frame;
                    run.search_center = Some(hint);
                    if quality != MatchQuality::Lost {
                        run.last_match = Some(position);
                    }
                    run.scores.push((frame, confidence));
                    if let Some(overlay) = &mut self.tracker_overlay {
                        overlay.search_frame = frame;
                        overlay.search_center = hint;
                        overlay.match_position =
                            (quality != MatchQuality::Lost).then_some(position);
                    }
                    if !run.fast && !self.playing {
                        self.frame = frame.min(self.frame_count.saturating_sub(1));
                        self.last_requested = None;
                        self.ready = None;
                    }
                    changed = true;
                }
                AutoTrackMessage::Done {
                    end,
                    frame,
                    confidence,
                    points,
                } => {
                    run.last_frame = frame;
                    run.last_confidence = confidence;
                    run.points = points.max(run.points);
                    finish = Some((end, frame, confidence, String::new()));
                    break;
                }
                AutoTrackMessage::Error(message) => {
                    finish = Some((
                        AutoTrackEnd::Stopped,
                        run.last_frame,
                        run.last_confidence,
                        message,
                    ));
                    break;
                }
            }
        }

        if finish.is_none() && disconnected {
            finish = Some((
                AutoTrackEnd::Stopped,
                run.last_frame,
                run.last_confidence,
                "跟踪线程已结束".into(),
            ));
        }
        if let Some((end, frame, confidence, message)) = finish {
            let points = run.points;
            let start_frame = run.start_frame;
            self.tracking = None;
            if run.fast {
                self.frame = frame.min(self.frame_count.saturating_sub(1));
                self.last_requested = None;
                self.ready = None;
            }
            if !self.tracker_open {
                self.tracker_overlay = None;
            }
            if points > 0 {
                self.commit_edit();
            }
            let mut status = format!(
                "{} · {} 点 · 第 {}→{} 帧",
                end.label(),
                points,
                start_frame,
                frame
            );
            if confidence.is_finite() {
                status.push_str(&format!(" · 置信度 {confidence:.2}"));
            }
            if !message.is_empty() {
                status.push_str(&format!(" · {message}"));
            }
            self.status = Some(status);
            changed = true;
        } else {
            self.tracking = Some(run);
        }
        changed
    }

    pub(crate) fn is_dirty(&self) -> bool {
        self.history.dirty()
    }

    pub(crate) fn mark_saved(&mut self) {
        self.history.mark_saved();
    }

    pub(crate) fn to_project_doc(&self) -> Option<DocProject> {
        if self.is_placeholder() || self.video.is_none() {
            return None;
        }
        Some(DocProject {
            path: self.path.clone(),
            title: self.title.clone(),
            frame: self.frame,
            tracks: self
                .tracks
                .iter()
                .map(|track| TrackProject {
                    name: track.name.clone(),
                    color: track.color,
                    visible: track.visible,
                    locked: track.locked,
                    step_size: track.step_size,
                    auto_advance: track.auto_advance,
                    footprint: track.footprint,
                    keyframes: track.keyframes().collect(),
                    points: track
                        .points()
                        .map(|(frame, x, y)| PointProject { frame, x, y })
                        .collect(),
                    measurement: track.measurement_steps().map(|steps| MeasurementProject {
                        kind: track
                            .measurement_kind()
                            .map(|kind| kind.tag().to_string())
                            .unwrap_or_default(),
                        steps: steps
                            .iter()
                            .map(|(frame, step)| {
                                MeasurementStepProject::from_handles(*frame, &step.handles)
                            })
                            .collect(),
                    }),
                })
                .collect(),
            active_track: self.active,
            coords: Some(self.coords.clone()),
            calibration: Some(self.calibration.clone()),
            cal_a: [0.0, 0.0],
            cal_b: [0.0, 0.0],
            cal_m: 1.0,
            scale: self.scale,
            tx: self.tx,
            ty: self.ty,
            user_adjusted: self.user_adjusted,
            axes_visible: self.axes_visible,
            trail_visible: self.trail_visible,
            trail_length: self.trail_length,
            labels_visible: self.labels_visible,
            trace_visible: self.trace_visible,
            plot: Some(self.plot.clone()),
        })
    }

    pub(crate) fn restore_project(&mut self, project: &DocProject) {
        self.path = project.path.clone();
        self.title = project.title.clone();
        self.tracks = project
            .tracks
            .iter()
            .enumerate()
            .map(|(index, track)| {
                let mut restored = match track
                    .measurement
                    .as_ref()
                    .and_then(|measurement| measurement.kind().map(|kind| (kind, measurement)))
                {
                    Some((kind, measurement)) => {
                        let mut restored = Track::measurement(
                            index as u64 + 1,
                            kind,
                            track.name.clone(),
                            track.color,
                        );
                        for step in &measurement.steps {
                            restored.set_measurement_step(
                                step.frame,
                                MeasurementStep {
                                    handles: step.handles.iter().map(|[x, y]| (*x, *y)).collect(),
                                },
                            );
                        }
                        restored
                    }
                    None => {
                        let mut restored =
                            Track::point_mass(index as u64 + 1, track.name.clone(), track.color);
                        for point in &track.points {
                            restored.set_point(point.frame, point.x, point.y);
                        }
                        restored
                    }
                };
                restored.visible = track.visible;
                restored.locked = track.locked;
                restored.step_size = track.step_size.max(1);
                restored.auto_advance = track.auto_advance;
                restored.footprint = track.footprint;
                restored
            })
            .collect();
        self.next_track_id = self.tracks.len() as u64 + 1;
        self.active = if self.tracks.is_empty() {
            0
        } else {
            project.active_track.min(self.tracks.len() - 1)
        };
        let mut calibration = project.calibration.clone();
        let mut coords = project.coords.clone();
        if coords.is_none() {
            let cal = calibration.clone().unwrap_or_else(|| {
                let mut cal = Calibration::new(self.img_w, self.img_h);
                let a = project.cal_a;
                let b = project.cal_b;
                if a[0] != 0.0 || a[1] != 0.0 || b[0] != 0.0 || b[1] != 0.0 {
                    cal.steps.insert(
                        0,
                        CalibrationStep {
                            a: [a[0] as f64, a[1] as f64],
                            b: [b[0] as f64, b[1] as f64],
                            world_a: [0.0, 0.0],
                            world_b: [project.cal_m.max(1e-6) as f64, 0.0],
                        },
                    );
                    cal.fixed_coordinates = true;
                }
                cal
            });
            let mut new_coords = ImageCoordSystem::default();
            cal.apply(0, &mut new_coords);
            calibration = Some(cal);
            coords = Some(new_coords);
        }
        if calibration.is_none() {
            calibration = Some(Calibration::new(self.img_w, self.img_h));
        }
        self.coords = coords.unwrap_or_default();
        self.calibration = calibration.unwrap();
        self.active_stick = 0;
        self.selection = Selection::None;
        self.scale = if project.scale > 0.0 {
            project.scale
        } else {
            1.0
        };
        self.tx = project.tx;
        self.ty = project.ty;
        self.user_adjusted = project.user_adjusted;
        self.axes_visible = project.axes_visible;
        self.trail_visible = project.trail_visible;
        self.trail_length = project.trail_length;
        self.labels_visible = project.labels_visible;
        self.trace_visible = project.trace_visible;
        self.plot = project.plot.clone().unwrap_or_default();
        self.sync_plot_for_active();
        self.frame = if self.frame_count == 0 {
            0
        } else {
            project.frame.min(self.frame_count - 1)
        };
        self.history.reset(self.snapshot());
        self.history.mark_saved();
        self.pending_restore = None;
    }
}

impl DocState {
    pub(crate) fn fit(&mut self) {
        if self.sw < 2.0 || self.sh < 2.0 || self.img_w <= 0.0 || self.img_h <= 0.0 {
            return;
        }
        let pad = 24.0;
        self.scale = ((self.sw - pad * 2.0) / self.img_w).min((self.sh - pad * 2.0) / self.img_h);
        self.tx = (self.sw - self.img_w * self.scale) / 2.0;
        self.ty = (self.sh - self.img_h * self.scale) / 2.0;
        self.fitted = true;
        self.user_adjusted = false;
    }

    pub(crate) fn to_screen(&self, x: f32, y: f32) -> (f32, f32) {
        (self.tx + x * self.scale, self.ty + y * self.scale)
    }

    pub(crate) fn to_img(&self, sx: f32, sy: f32) -> (f32, f32) {
        ((sx - self.tx) / self.scale, (sy - self.ty) / self.scale)
    }

    pub(crate) fn ppm(&self) -> f32 {
        self.coords.scale_x(self.frame) as f32
    }

    pub(crate) fn apply_calibration(&mut self) {
        let frame = self.frame;
        self.calibration.apply(frame, &mut self.coords);
        let coords = self.coords.clone();
        self.calibration.sync_sticks(frame, &coords, Some(0));
    }

    pub(crate) fn set_stick_length_value(
        &mut self,
        index: usize,
        length: f64,
    ) -> Result<(), String> {
        if !length.is_finite() || length <= 0.0 {
            return Err("长度必须大于 0".into());
        }
        let frame = self.frame;
        let Some((a, b, _)) = self.calibration.stick(frame, index) else {
            return Err("标尺不存在".into());
        };
        let dx = b[0] - a[0];
        let dy = b[1] - a[1];
        let pixel_length = (dx * dx + dy * dy).sqrt();
        if pixel_length <= 1e-9 {
            return Err("标尺长度为零，无法计算比例".into());
        }
        self.calibration.set_stick_length(frame, index, length);
        self.coords.fixed_scale = self.calibration.fixed_coordinates;
        self.coords.set_scale_forced(frame, pixel_length / length);
        let coords = self.coords.clone();
        self.calibration.sync_sticks(frame, &coords, Some(index));
        self.commit_edit();
        Ok(())
    }

    pub(crate) fn add_calibration_stick(&mut self) {
        if self.video.is_none() || self.coords.locked {
            return;
        }
        let frame = self.frame;
        let Some((a, b, length)) = self.calibration.stick(frame, 0) else {
            return;
        };
        let dx = b[0] - a[0];
        let dy = b[1] - a[1];
        let pixel_length = (dx * dx + dy * dy).sqrt().max(1.0);
        let (ux, uy) = (dx / pixel_length, dy / pixel_length);
        let mid = [
            (a[0] + b[0]) * 0.5,
            (a[1] + b[1]) * 0.5 + pixel_length * 0.6,
        ];
        let half = pixel_length * 0.5;
        self.calibration.add_extra_stick(
            [mid[0] - ux * half, mid[1] - uy * half],
            [mid[0] + ux * half, mid[1] + uy * half],
            length,
        );
        self.active_stick = self.calibration.stick_count() - 1;
        self.selection = Selection::Stick(self.active_stick);
        self.commit_edit();
    }

    pub(crate) fn to_world(&self, x: f32, y: f32) -> (f32, f32) {
        self.coords.image_to_world(self.frame, x, y)
    }

    pub(crate) fn count_frame(&mut self) {
        self.frames_counted += 1;
        let dt = self.fps_t.elapsed().as_secs_f32();
        if dt >= 0.5 {
            self.fps = self.frames_counted as f32 / dt;
            if std::env::var_os("PROBE_TRACE").is_some() {
                eprintln!("[ui] render fps {:.1} at frame {}", self.fps, self.frame);
            }
            self.frames_counted = 0;
            self.fps_t = Instant::now();
        }
    }

    pub(crate) fn tick(&mut self) {
        if !self.playing {
            return;
        }
        if self.video.is_none() {
            self.playing = false;
            return;
        }
        // Wait until the current frame is actually on screen. The playback
        // clock then decides whether the next frame is due; if decoding is
        // slow we advance one frame at a time instead of skipping or seeking.
        if self.frame_image.as_ref().map(|(i, _)| *i) != Some(self.frame) {
            return;
        }
        if self.frame >= self.frame_count - 1 {
            self.playing = false;
            self.play_start = None;
            return;
        }
        let Some(t0) = self.play_start else { return };
        let desired = {
            let Some(meta) = self.video.as_ref().map(|v| &v.meta) else {
                return;
            };
            let start = meta
                .frame_times
                .get(self.play_frame)
                .copied()
                .unwrap_or(0.0);
            meta.frame_at_or_before(start + t0.elapsed().as_secs_f64())
        };
        if desired > self.frame {
            let next = self.frame + 1;
            if let Some(ready) = self.ready.take() {
                if ready.index == next {
                    self.frame = next;
                    self.show(ready);
                    return;
                }
                self.ready = Some(ready);
            }
            self.frame = next;
        }
    }

    pub(crate) fn play(&mut self) {
        if self.frame_count == 0 {
            return;
        }
        if self.frame >= self.frame_count - 1 {
            self.frame = 0;
            self.ready = None;
        }
        self.playing = true;
        self.last_requested = None;
        self.play_frame = self.frame;
        self.play_start = Some(Instant::now());
    }

    pub(crate) fn pause(&mut self) {
        self.playing = false;
        self.play_start = None;
    }

    pub(crate) fn set_frame(&mut self, frame: usize) {
        self.frame = frame.min(self.frame_count.saturating_sub(1));
        self.last_requested = None;
        self.ready = None;
    }

    pub(crate) fn current_time(&self) -> f64 {
        match &self.video {
            Some(video) => video
                .meta
                .frame_times
                .get(self.frame)
                .copied()
                .unwrap_or(0.0),
            None => 0.0,
        }
    }

    pub(crate) fn total_time(&self) -> f64 {
        match &self.video {
            Some(video) => video.meta.frame_times.last().copied().unwrap_or(0.0),
            None => 0.0,
        }
    }

    pub(crate) fn frame_time(&self, frame: usize) -> f64 {
        self.video
            .as_ref()
            .and_then(|video| video.meta.frame_times.get(frame).copied())
            .unwrap_or(0.0)
    }

    pub(crate) fn seek_fraction(&mut self, fraction: f32) {
        if self.frame_count == 0 {
            return;
        }
        let last = self.frame_count.saturating_sub(1);
        let frame = (fraction.clamp(0.0, 1.0) * last as f32).round() as usize;
        self.pause();
        self.set_frame(frame);
    }

    pub(crate) fn progress_fraction(&self) -> f32 {
        if let Some(loading) = &self.loading {
            loading.progress.load(Ordering::Relaxed) as f32 / 100.0
        } else if self.frame_count > 1 {
            self.frame as f32 / (self.frame_count - 1) as f32
        } else {
            0.0
        }
    }

    pub(crate) fn show(&mut self, frame: FrameData) {
        let index = frame.index;
        let image = make_image(frame.bgra, frame.width, frame.height);
        if let Some((_, old)) = self.frame_image.replace((index, image)) {
            self.retire(old);
        }
        self.last_requested = None;
        if std::env::var_os("PROBE_TRACE").is_some() {
            eprintln!(
                "[ui] frame {} t={}ms",
                index,
                self.created.elapsed().as_millis()
            );
        }
    }

    pub(crate) fn retire(&mut self, image: Arc<RenderImage>) {
        self.retired.push_back(image);
        if self.retired.len() > 3 {
            if let Some(old) = self.retired.pop_front() {
                self.to_drop.push(old);
            }
        }
    }

    pub(crate) fn take_drops(&mut self) -> Vec<Arc<RenderImage>> {
        std::mem::take(&mut self.to_drop)
    }

    pub(crate) fn apply_video(&mut self, stream: VideoStream) {
        let meta = stream.meta.clone();
        self.img_w = meta.width as f32;
        self.img_h = meta.height as f32;
        self.frame_count = meta.frames;
        self.video = Some(Arc::new(stream));
        self.loading = None;
        if let Some((_, old)) = self.frame_image.take() {
            self.retire(old);
        }
        self.last_requested = None;
        self.ready = None;
        self.frame = 0;
        self.playing = false;
        self.play_start = None;
        self.tracks = vec![Track::point_mass(1, "Track 1", PALETTE[0])];
        self.active = 0;
        self.next_track_id = 2;
        self.plot = PlotState::for_track(&self.tracks[0]);
        self.calibration = Calibration::new(self.img_w, self.img_h);
        self.coords = ImageCoordSystem::default();
        self.active_stick = 0;
        self.selection = Selection::None;
        self.calibration.apply(0, &mut self.coords);
        self.fitted = false;
        self.user_adjusted = false;
        if self.sw > 2.0 && self.sh > 2.0 {
            self.fit();
        }
        self.fps_t = Instant::now();
        self.frames_counted = 0;
        self.history.reset(self.snapshot());
        self.history.mark_saved();
    }

    pub(crate) fn ensure_frame_image(&mut self) {
        let Some(stream) = self.video.as_ref().map(Arc::clone) else {
            self.frame_image = None;
            self.ready = None;
            return;
        };
        let mut received = Vec::new();
        while let Some(frame) = stream.try_recv() {
            received.push(frame);
        }

        if self.ready.as_ref().map(|f| f.index) != Some(self.frame + 1) {
            self.ready = None;
        }
        let already_displayed = self.frame_image.as_ref().map(|(i, _)| *i) == Some(self.frame);
        let current = if already_displayed {
            None
        } else {
            received
                .iter()
                .position(|f| f.index == self.frame)
                .map(|i| received.remove(i))
        };
        let next_index = self.frame + 1;
        let prefetched = if self.playing {
            received
                .iter()
                .position(|f| f.index == next_index)
                .map(|i| received.remove(i))
        } else {
            None
        };

        let displayed = current.is_some() || already_displayed;
        if !displayed {
            if self.last_requested != Some(self.frame) {
                stream.request(self.frame);
                self.last_requested = Some(self.frame);
            }
        } else if self.playing && self.frame < self.frame_count.saturating_sub(1) {
            let ready_next =
                prefetched.is_some() || self.ready.as_ref().map(|f| f.index) == Some(next_index);
            if !ready_next && self.last_requested != Some(next_index) {
                stream.request(next_index);
                self.last_requested = Some(next_index);
            }
        }

        if let Some(frame) = current {
            self.show(frame);
        }
        if let Some(frame) = prefetched {
            self.ready = Some(frame);
        }
    }

    pub(crate) fn hit_test(&self, sx: f32, sy: f32) -> Option<Drag> {
        let near = |a: (f32, f32), b: (f32, f32), r: f32| {
            ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt() <= r
        };
        let frame = self.frame;
        if self.axes_visible {
            let origin = self.coords.origin(frame);
            let origin_screen = self.to_screen(origin[0] as f32, origin[1] as f32);
            if near(origin_screen, (sx, sy), 10.0) {
                return Some(Drag::AxisOrigin);
            }
            let angle = self.coords.angle(frame);
            let handle = (
                origin_screen.0 + angle.cos() as f32 * 60.0,
                origin_screen.1 - angle.sin() as f32 * 60.0,
            );
            if near(handle, (sx, sy), 10.0) {
                return Some(Drag::AxisAngle);
            }
            let frame_coords = self.coords.frame(frame);
            let dirs = [
                [frame_coords.angle.cos(), -frame_coords.angle.sin()],
                [-frame_coords.angle.sin(), -frame_coords.angle.cos()],
            ];
            for dir in dirs {
                if let Some((a, b)) = clip_axis(
                    frame_coords.origin,
                    dir,
                    self.img_w as f64,
                    self.img_h as f64,
                ) {
                    let pa = self.to_screen(a[0] as f32, a[1] as f32);
                    let pb = self.to_screen(b[0] as f32, b[1] as f32);
                    if point_segment_distance(sx, sy, pa.0, pa.1, pb.0, pb.1) <= 6.0 {
                        return Some(Drag::AxisLine {
                            start: self.to_img(sx, sy),
                            origin: frame_coords.origin,
                        });
                    }
                }
            }
        }
        for stick in 0..self.calibration.stick_count() {
            if !self.calibration.stick_visible(stick) {
                continue;
            }
            let Some((a, b, _)) = self.calibration.stick(frame, stick) else {
                continue;
            };
            let ca = self.to_screen(a[0] as f32, a[1] as f32);
            let cb = self.to_screen(b[0] as f32, b[1] as f32);
            if near(ca, (sx, sy), 11.0) {
                return Some(Drag::CalStick { stick, end: 0 });
            }
            if near(cb, (sx, sy), 11.0) {
                return Some(Drag::CalStick { stick, end: 1 });
            }
            if point_segment_distance(sx, sy, ca.0, ca.1, cb.0, cb.1) <= 9.0 {
                return Some(Drag::StickMove {
                    stick,
                    start: self.to_img(sx, sy),
                    a,
                    b,
                });
            }
        }
        for track_index in (0..self.tracks.len()).rev() {
            let track = &self.tracks[track_index];
            if !track.visible {
                continue;
            }
            let handles = track.handles(frame);
            if handles.is_empty() {
                continue;
            }
            let screen: Vec<(f32, f32)> = handles
                .iter()
                .map(|(x, y)| self.to_screen(*x, *y))
                .collect();
            for (index, point) in screen.iter().enumerate() {
                if near(*point, (sx, sy), 10.0) {
                    return Some(Drag::Handle {
                        track: track_index,
                        frame,
                        index,
                        created: false,
                    });
                }
            }
            let body_hit = match track.measurement_kind() {
                Some(MeasurementKind::Tape) if screen.len() >= 2 => {
                    point_segment_distance(
                        sx,
                        sy,
                        screen[0].0,
                        screen[0].1,
                        screen[1].0,
                        screen[1].1,
                    ) <= 8.0
                }
                Some(MeasurementKind::Protractor) if screen.len() >= 3 => {
                    point_segment_distance(
                        sx,
                        sy,
                        screen[0].0,
                        screen[0].1,
                        screen[1].0,
                        screen[1].1,
                    ) <= 8.0
                        || point_segment_distance(
                            sx,
                            sy,
                            screen[0].0,
                            screen[0].1,
                            screen[2].0,
                            screen[2].1,
                        ) <= 8.0
                }
                Some(MeasurementKind::Circle) if screen.len() >= 3 => track
                    .circle_values(frame)
                    .map_or(false, |(center, radius)| {
                        let (cx, cy) = self.to_screen(center.0, center.1);
                        ((sx - cx).hypot(sy - cy) - radius * self.scale).abs() <= 8.0
                    }),
                _ => false,
            };
            if body_hit {
                return Some(Drag::TrackMove {
                    track: track_index,
                    start: self.to_img(sx, sy),
                    handles,
                });
            }
        }
        None
    }
}

impl State {
    pub(crate) fn new() -> Self {
        let prefs = Preferences::load();
        let theme_pref = std::env::var("PROBE_THEME").unwrap_or_else(|_| prefs.theme.clone());
        set_theme_mode(match theme_pref.as_str() {
            "light" => ThemeMode::Light,
            "dark" => ThemeMode::Dark,
            _ => ThemeMode::Auto,
        });
        let initial_dir = std::env::var("PROBE_DIR")
            .ok()
            .map(PathBuf::from)
            .or_else(|| prefs.last_workdir.clone());
        let mut st = Self {
            docs: vec![DocState::placeholder()],
            active_doc: 0,
            next_doc_id: 1,
            workdir: None,
            tree: Vec::new(),
            expanded: HashSet::new(),
            project_path: None,
            prefs,
            modal: None,
            cal_length_input: "1".to_string(),
            cal_unit_input: "m".to_string(),
            cal_decimals_input: "3".to_string(),
            cal_field: CalibrationField::Length,
            cal_error: None,
            menu_open: None,
            menu_x: 8.0,
            tool: Tool::Move,
            measurement_tool: Tool::Tape,
            tool_menu: None,
            tool_menu_pos: (0.0, 0.0),
            plot_var_menu: None,
            plot_var_pos: (0.0, 0.0),
            suppress_stage_click_until: None,
            shift_down: false,
            inline_edit: None,
            tab_scroll: ScrollHandle::new(),
            tracker_settings: AutoTrackSettings::default(),
            tracker_inputs: Default::default(),
            tracker_field: TrackerField::MaskWidth,
            tracker_error: None,
            tracker_fast: false,
            tracker_thumbs: None,
            tracker_thumbs_drop: Vec::new(),
            drag: None,
            progress_rect: (0.0, 0.0, 0.0, 0.0),
            scrubbing: false,
            left_open: true,
            right_open: true,
            right_view: RightView::Inspector,
            textures_to_drop: Vec::new(),
        };
        if let Some(dir) = initial_dir {
            if dir.is_dir() {
                st.set_workdir(dir);
            }
        }
        st
    }

    pub(crate) fn open_tracker_dialog(&mut self) {
        let settings = &self.tracker_settings;
        self.tracker_inputs = [
            settings.mask_width.to_string(),
            settings.mask_height.to_string(),
            settings.search_size.to_string(),
            format!("{:.0}", settings.evolve * 100.0),
            format!("{:.0}", settings.tether * 100.0),
            format!("{}", settings.good_match),
        ];
        self.tracker_field = TrackerField::MaskWidth;
        self.tracker_error = None;
        self.tracker_open = true;
        let active_id = self.active_track().map(|track| track.id);
        if self
            .tracker_overlay
            .as_ref()
            .map(|overlay| overlay.track_id)
            == active_id
            && self.tracker_overlay.is_some()
        {
            self.sync_tracker_overlay();
        } else {
            self.init_tracker_overlay();
        }
        self.pause();
    }

    pub(crate) fn close_tracker_dialog(&mut self) {
        self.modal = None;
        self.tracker_error = None;
        self.tracker_open = false;
        if self.tracking.is_none() {
            self.tracker_overlay = None;
        }
        if let Some(thumbs) = self.tracker_thumbs.take() {
            if let Some(image) = thumbs.template {
                self.tracker_thumbs_drop.push(image);
            }
            if let Some(image) = thumbs.matched {
                self.tracker_thumbs_drop.push(image);
            }
        }
    }

    pub(crate) fn init_tracker_overlay(&mut self) {
        let settings = self.tracker_settings.clone();
        let frame = self.frame;
        let Some(track) = self.active_track() else {
            self.tracker_overlay = None;
            return;
        };
        if !track.is_point_mass() {
            self.tracker_overlay = None;
            return;
        }
        let key_frame = track.nearest_keyframe(frame, true).unwrap_or(frame);
        let center = track
            .point_at(frame)
            .or_else(|| track.point_at(key_frame))
            .or_else(|| track.points().last().map(|(_, x, y)| (x, y)));
        let Some(center) = center else {
            self.tracker_overlay = None;
            return;
        };
        let key_center = track.point_at(key_frame).unwrap_or(center);
        self.tracker_overlay = Some(TrackerOverlay {
            track_id: track.id,
            key_frame,
            key_center,
            mask_width: settings.mask_width,
            mask_height: settings.mask_height,
            shape: settings.shape,
            search_size: settings.search_size,
            search_center: center,
            search_frame: frame,
            match_position: track.point_at(frame),
        });
    }

    pub(crate) fn sync_tracker_overlay(&mut self) {
        let settings = self.tracker_settings.clone();
        let Some(track_id) = self
            .tracker_overlay
            .as_ref()
            .map(|overlay| overlay.track_id)
        else {
            self.init_tracker_overlay();
            return;
        };
        let frame = self.frame;
        let key_frame = self
            .tracks
            .iter()
            .find(|track| track.id == track_id)
            .and_then(|track| track.nearest_keyframe(frame, true));
        let key_center = self
            .tracks
            .iter()
            .find(|track| track.id == track_id)
            .and_then(|track| {
                track
                    .nearest_keyframe(frame, true)
                    .and_then(|key_frame| track.point_at(key_frame))
            });
        let point = self
            .tracks
            .iter()
            .find(|track| track.id == track_id)
            .and_then(|track| {
                track.point_at(frame).or_else(|| {
                    track
                        .nearest_keyframe(frame, true)
                        .and_then(|f| track.point_at(f))
                })
            });
        let tracking_idle = self.tracking.is_none();
        let Some(overlay) = &mut self.tracker_overlay else {
            return;
        };
        overlay.mask_width = settings.mask_width;
        overlay.mask_height = settings.mask_height;
        overlay.shape = settings.shape;
        overlay.search_size = settings.search_size;
        if tracking_idle {
            if let Some(key_center) = key_center {
                overlay.key_center = key_center;
            }
            if let Some(key_frame) = key_frame {
                overlay.key_frame = key_frame;
            }
        }
        if settings.search_policy == SearchPolicy::Fixed {
            overlay.search_center = overlay.key_center;
            overlay.search_frame = frame;
        } else if let Some(point) = point {
            overlay.search_center = point;
            overlay.search_frame = frame;
        }
        overlay.match_position = point;
    }

    pub(crate) fn tracker_input_mut(&mut self, field: TrackerField) -> &mut String {
        &mut self.tracker_inputs[field.index()]
    }

    pub(crate) fn cycle_tracker_field(&mut self) {
        self.tracker_field = self.tracker_field.next();
    }

    pub(crate) fn apply_tracker_dialog(&mut self) -> bool {
        let parse_u32 = |text: &str, label: &str| -> Result<u32, String> {
            text.trim()
                .parse::<u32>()
                .map_err(|_| format!("{label}必须是整数"))
        };
        let parse_f32 = |text: &str, label: &str| -> Result<f32, String> {
            text.trim()
                .parse::<f32>()
                .map_err(|_| format!("{label}必须是数字"))
        };
        let result = (|| {
            let width = parse_u32(&self.tracker_inputs[0], "掩膜宽")?;
            let height = parse_u32(&self.tracker_inputs[1], "掩膜高")?;
            if !(3..=99).contains(&width) || !(3..=99).contains(&height) {
                return Err("掩膜宽/高必须在 3–99 像素之间".to_string());
            }
            let search = parse_u32(&self.tracker_inputs[2], "搜索范围")?;
            if !(3..=240).contains(&search) {
                return Err("搜索范围必须在 3–240 像素之间".to_string());
            }
            let evolve = parse_f32(&self.tracker_inputs[3], "演化百分比")?;
            let tether = parse_f32(&self.tracker_inputs[4], "束缚百分比")?;
            if !(0.0..=95.0).contains(&evolve) || !(0.0..=95.0).contains(&tether) {
                return Err("演化/束缚百分比必须在 0–95 之间".to_string());
            }
            let good = parse_f32(&self.tracker_inputs[5], "自动标记阈值")?;
            if !good.is_finite() || !(0.0..=10.0).contains(&good) {
                return Err("自动标记阈值必须在 0–10 之间".to_string());
            }
            let current = self.tracker_settings.clone();
            self.tracker_settings = AutoTrackSettings {
                mask_width: width,
                mask_height: height,
                search_size: search,
                good_match: good,
                possible_match: current.possible_match.min(good).max(0.0),
                evolve: evolve / 100.0,
                tether: tether / 100.0,
                shape: current.shape,
                search_policy: current.search_policy,
                stop_policy: current.stop_policy,
                one_d: current.one_d,
            }
            .normalized();
            self.sync_tracker_overlay();
            Ok(())
        })();
        match result {
            Ok(()) => {
                self.tracker_error = None;
                true
            }
            Err(message) => {
                self.tracker_error = Some(message);
                false
            }
        }
    }

    pub(crate) fn start_tracking_ui(&mut self) -> bool {
        let settings = self.tracker_settings.clone();
        let fast = self.tracker_fast;
        match self.start_tracking(settings, false) {
            Ok(()) => {
                if let Some(run) = &mut self.tracking {
                    run.fast = fast;
                }
                self.tracker_error = None;
                self.status = Some("AutoTracker 搜索中…".into());
                true
            }
            Err(err) => {
                self.tracker_error = Some(err);
                false
            }
        }
    }

    pub(crate) fn single_step_tracking_ui(&mut self) -> bool {
        if self.tracking.is_some() {
            self.stop_tracking();
        }
        let settings = self.tracker_settings.clone();
        let fast = self.tracker_fast;
        match self.start_tracking(settings, true) {
            Ok(()) => {
                if let Some(run) = &mut self.tracking {
                    run.fast = fast;
                }
                self.tracker_error = None;
                true
            }
            Err(err) => {
                self.tracker_error = Some(err);
                false
            }
        }
    }

    pub(crate) fn tracker_search_action(&mut self) {
        if self.tracking.is_some() {
            self.toggle_tracking_pause();
        } else if self.apply_tracker_dialog() {
            self.start_tracking_ui();
        }
    }

    pub(crate) fn tracker_step_back(&mut self) {
        if self.tracking.is_some() {
            self.stop_tracking();
        }
        let step = self
            .active_track()
            .map(|track| track.step_size.max(1))
            .unwrap_or(1);
        let next = self.frame.saturating_sub(step);
        self.set_frame(next);
        self.sync_tracker_overlay();
        self.status = Some(format!("AutoTracker：回到第 {next} 帧"));
    }

    pub(crate) fn tracker_show_keyframe(&mut self) {
        let Some(track) = self.active_track() else {
            return;
        };
        let Some(frame) = track
            .nearest_keyframe(self.frame, true)
            .or_else(|| track.keyframes().next())
        else {
            self.status = Some("当前轨道尚无关键帧".into());
            return;
        };
        self.set_frame(frame);
        self.sync_tracker_overlay();
        self.status = Some(format!("AutoTracker：关键帧 {frame}"));
    }

    pub(crate) fn tracker_delete_current(&mut self) {
        let frame = self.frame;
        let active = self.active;
        self.apply_edit_command(EditCommand::DeletePoint {
            track: active,
            frame,
        });
        if let Some(track) = self.tracks.get_mut(active) {
            track.remove_keyframe(frame);
        }
        self.status = Some(format!("已删除第 {frame} 帧标记"));
    }

    pub(crate) fn tracker_match_csv(&self) -> String {
        let mut text = String::from("frame,confidence\n");
        if let Some(run) = &self.tracking {
            for (frame, confidence) in &run.scores {
                text.push_str(&format!("{frame},{confidence:.3}\n"));
            }
        } else if let Some(track) = self.active_track() {
            for (frame, _, _) in track.points() {
                text.push_str(&format!("{frame},\n"));
            }
        }
        text
    }

    pub(crate) fn cycle_tracker_track(&mut self) {
        let point_tracks: Vec<usize> = self
            .tracks
            .iter()
            .enumerate()
            .filter(|(_, track)| track.is_point_mass())
            .map(|(index, _)| index)
            .collect();
        if point_tracks.is_empty() {
            return;
        }
        let next = point_tracks
            .iter()
            .position(|index| *index == self.active)
            .map(|position| point_tracks[(position + 1) % point_tracks.len()])
            .unwrap_or(point_tracks[0]);
        self.active = next;
        self.sync_plot_for_active();
        self.init_tracker_overlay();
    }

    pub(crate) fn tracker_thumbnails(
        &mut self,
    ) -> (Option<Arc<RenderImage>>, Option<Arc<RenderImage>>) {
        let Some(overlay) = self.tracker_overlay.clone() else {
            return (None, None);
        };
        let Some((frame_index, image)) = self
            .frame_image
            .as_ref()
            .map(|(index, image)| (*index, image.clone()))
        else {
            return (None, None);
        };
        let settings = self.tracker_settings.clone();
        let track_point = self
            .tracks
            .iter()
            .find(|track| track.id == overlay.track_id)
            .and_then(|track| track.point_at(self.frame));
        let template_center = overlay.key_center;
        let matched_center = if self.frame == overlay.search_frame {
            overlay.match_position.or(track_point)
        } else {
            track_point
        };
        let key = (
            frame_index,
            (template_center.0 * 10.0) as i64,
            (template_center.1 * 10.0) as i64,
            (matched_center.map(|p| p.0).unwrap_or(0.0) * 10.0) as i64,
            (matched_center.map(|p| p.1).unwrap_or(0.0) * 10.0) as i64,
            settings.mask_width,
            settings.mask_height,
        );
        if let Some(cache) = &self.tracker_thumbs {
            if cache.key == key {
                return (cache.template.clone(), cache.matched.clone());
            }
        }
        let width = self.img_w.max(1.0) as u32;
        let height = self.img_h.max(1.0) as u32;
        let template = crop_thumbnail(
            &image,
            width,
            height,
            template_center,
            settings.mask_width,
            settings.mask_height,
        );
        let matched = matched_center.and_then(|center| {
            crop_thumbnail(
                &image,
                width,
                height,
                center,
                settings.mask_width,
                settings.mask_height,
            )
        });
        if let Some(old) = self.tracker_thumbs.take() {
            if let Some(image) = old.template {
                self.tracker_thumbs_drop.push(image);
            }
            if let Some(image) = old.matched {
                self.tracker_thumbs_drop.push(image);
            }
        }
        self.tracker_thumbs = Some(TrackerThumbs {
            key,
            template: template.clone(),
            matched: matched.clone(),
        });
        (template, matched)
    }

    pub(crate) fn select_tool(&mut self, tool: Tool) {
        self.tool = tool;
        if tool.is_measurement_group() {
            self.measurement_tool = tool;
        }
    }

    pub(crate) fn begin_inline_edit(&mut self, target: InlineTarget, text: String) {
        self.inline_edit = Some(InlineEdit {
            target,
            text,
            error: None,
        });
    }

    pub(crate) fn apply_inline_edit(&mut self) -> bool {
        let Some(edit) = self.inline_edit.take() else {
            return false;
        };
        let result = match edit.target {
            InlineTarget::OriginX | InlineTarget::OriginY => {
                if self.coords.locked {
                    Err("坐标已锁定".to_string())
                } else {
                    edit.text
                        .trim()
                        .parse::<f64>()
                        .map_err(|_| "请输入数字".to_string())
                        .and_then(|value| {
                            if !value.is_finite() {
                                return Err("请输入有限数值".into());
                            }
                            let frame = self.frame;
                            let mut origin = self.coords.origin(frame);
                            if edit.target == InlineTarget::OriginX {
                                origin[0] = value;
                            } else {
                                origin[1] = value;
                            }
                            self.coords.set_origin(frame, origin);
                            self.commit_edit();
                            Ok(())
                        })
                }
            }
            InlineTarget::Angle => {
                if self.coords.locked {
                    Err("坐标已锁定".to_string())
                } else {
                    edit.text
                        .trim()
                        .parse::<f64>()
                        .map_err(|_| "角度必须是数字".to_string())
                        .and_then(|degrees| {
                            if !degrees.is_finite() {
                                return Err("角度必须是有限数值".into());
                            }
                            let frame = self.frame;
                            self.coords.set_angle(frame, degrees.to_radians());
                            self.commit_edit();
                            Ok(())
                        })
                }
            }
            InlineTarget::StickLength(index) => edit
                .text
                .trim()
                .parse::<f64>()
                .map_err(|_| "长度必须是数字".to_string())
                .and_then(|length| self.set_stick_length_value(index, length)),
        };
        match result {
            Ok(()) => {
                self.inline_edit = None;
                true
            }
            Err(message) => {
                self.inline_edit = Some(InlineEdit {
                    target: edit.target,
                    text: edit.text,
                    error: Some(message),
                });
                false
            }
        }
    }

    pub(crate) fn suppress_next_stage_click(&mut self) {
        self.suppress_stage_click_until = Some(Instant::now());
    }

    pub(crate) fn consume_suppressed_stage_click(&mut self) -> bool {
        let Some(when) = self.suppress_stage_click_until.take() else {
            return false;
        };
        when.elapsed() < std::time::Duration::from_millis(300)
    }

    pub(crate) fn apply_theme_mode(&mut self, mode: ThemeMode) {
        set_theme_mode(mode);
        self.prefs.theme = match mode {
            ThemeMode::Auto => "auto",
            ThemeMode::Light => "light",
            ThemeMode::Dark => "dark",
        }
        .to_string();
        self.prefs.save();
    }
}

impl State {
    pub(crate) fn set_workdir(&mut self, path: PathBuf) {
        self.tree = scan_videos(&path, 4);
        self.expanded.clear();
        for node in &self.tree {
            if node.is_dir {
                self.expanded.insert(node.path.clone());
            }
        }
        self.workdir = Some(path.clone());
        self.prefs.note_workdir(&path);
    }

    pub(crate) fn project_snapshot(&self) -> ProjectFile {
        let mut docs = Vec::new();
        let mut active = 0;
        let mut active_seen = false;
        for (index, doc) in self.docs.iter().enumerate() {
            if let Some(project_doc) = doc.to_project_doc() {
                if index == self.active_doc {
                    active = docs.len();
                    active_seen = true;
                }
                docs.push(project_doc);
            }
        }
        if !active_seen {
            active = 0;
        }
        ProjectFile::new(self.workdir.clone(), active, docs)
    }

    pub(crate) fn save_project_to(&mut self, path: &std::path::Path) -> Result<(), String> {
        self.project_snapshot().save(path)?;
        self.project_path = Some(path.to_path_buf());
        self.prefs.note_project(path);
        for doc in &mut self.docs {
            doc.mark_saved();
        }
        Ok(())
    }

    pub(crate) fn prepare_project_load(
        &mut self,
        project: ProjectFile,
        path: PathBuf,
    ) -> Vec<(u64, PathBuf, Arc<AtomicU32>)> {
        for doc in self.docs.drain(..) {
            if let Some((_, image)) = doc.frame_image {
                self.textures_to_drop.push(image);
            }
            self.textures_to_drop.extend(doc.retired);
            self.textures_to_drop.extend(doc.to_drop);
        }
        self.docs.clear();
        self.active_doc = 0;
        self.drag = None;
        self.menu_open = None;
        self.modal = None;

        if let Some(workdir) = project.workdir.clone() {
            if workdir.is_dir() {
                self.set_workdir(workdir);
            }
        }

        let mut loads = Vec::new();
        for project_doc in project.docs {
            let id = self.next_doc_id;
            self.next_doc_id += 1;
            let name = if project_doc.title.is_empty() {
                project_doc
                    .path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("video")
                    .to_string()
            } else {
                project_doc.title.clone()
            };
            let progress = Arc::new(AtomicU32::new(0));
            let mut doc = DocState::loading(id, project_doc.path.clone(), name, progress.clone());
            doc.pending_restore = Some(project_doc.clone());
            self.docs.push(doc);
            loads.push((id, project_doc.path, progress));
        }
        if self.docs.is_empty() {
            self.docs.push(DocState::placeholder());
        }
        self.active_doc = project.active_doc.min(self.docs.len().saturating_sub(1));
        self.project_path = Some(path.clone());
        self.prefs.note_project(&path);
        loads
    }

    pub(crate) fn new_project(&mut self) {
        for doc in self.docs.drain(..) {
            if let Some((_, image)) = doc.frame_image {
                self.textures_to_drop.push(image);
            }
            self.textures_to_drop.extend(doc.retired);
            self.textures_to_drop.extend(doc.to_drop);
        }
        self.docs.push(DocState::placeholder());
        self.active_doc = 0;
        self.project_path = None;
        self.drag = None;
        self.menu_open = None;
        self.modal = None;
    }

    pub(crate) fn has_dirty_docs(&self) -> bool {
        self.docs.iter().any(|doc| doc.is_dirty())
    }

    pub(crate) fn open_calibration_dialog(&mut self) {
        let length = self
            .calibration
            .stick(self.frame, self.active_stick)
            .map(|(_, _, length)| length)
            .unwrap_or_else(|| self.calibration.length(self.frame));
        let unit = self.calibration.unit.clone();
        let decimals = self.calibration.decimals;
        self.cal_length_input = format!("{length}");
        self.cal_unit_input = unit;
        self.cal_decimals_input = decimals.to_string();
        self.cal_field = CalibrationField::Length;
        self.cal_error = None;
        self.modal = Some(Modal::Calibration);
    }

    pub(crate) fn apply_calibration_dialog(&mut self) -> bool {
        if self.coords.locked {
            self.cal_error = Some("坐标已锁定".into());
            return false;
        }
        let Ok(length) = self.cal_length_input.trim().parse::<f64>() else {
            self.cal_error = Some("长度必须是数字".into());
            return false;
        };
        let Ok(decimals) = self.cal_decimals_input.trim().parse::<usize>() else {
            self.cal_error = Some("小数位数必须是整数".into());
            return false;
        };
        if !length.is_finite() || length <= 0.0 {
            self.cal_error = Some("长度必须大于 0".into());
            return false;
        }
        if decimals > 6 {
            self.cal_error = Some("小数位数最多 6".into());
            return false;
        }
        let unit = self.cal_unit_input.trim().to_string();
        let unit = if unit.is_empty() {
            "m".to_string()
        } else {
            unit
        };
        self.calibration.unit = unit;
        self.calibration.decimals = decimals;
        let active_stick = self.active_stick;
        if let Err(message) = self.set_stick_length_value(active_stick, length) {
            self.cal_error = Some(message);
            return false;
        }
        self.modal = None;
        self.cal_error = None;
        true
    }

    pub(crate) fn cycle_calibration_field(&mut self) {
        self.cal_field = match self.cal_field {
            CalibrationField::Length => CalibrationField::Unit,
            CalibrationField::Unit => CalibrationField::Decimals,
            CalibrationField::Decimals => CalibrationField::Length,
        };
    }

    pub(crate) fn begin_load(&mut self, path: PathBuf) -> Option<(u64, Arc<AtomicU32>)> {
        if let Some(index) = self.docs.iter().position(|doc| doc.path == path) {
            self.activate_doc(index);
            return None;
        }
        let id = self.next_doc_id;
        self.next_doc_id += 1;
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("video")
            .to_string();
        let progress = Arc::new(AtomicU32::new(0));
        let doc = DocState::loading(id, path, name, progress.clone());
        for doc in &mut self.docs {
            doc.pause();
        }
        if self.docs.len() == 1 && self.docs[0].is_placeholder() {
            self.docs[0] = doc;
            self.active_doc = 0;
        } else {
            self.docs.push(doc);
            self.active_doc = self.docs.len() - 1;
        }
        self.drag = None;
        Some((id, progress))
    }

    pub(crate) fn finish_load(&mut self, id: u64, result: Result<VideoStream, String>) {
        let Some(index) = self.docs.iter().position(|doc| doc.id == id) else {
            return;
        };
        self.docs[index].loading = None;
        let restore = self.docs[index].pending_restore.take();
        match result {
            Ok(stream) => self.docs[index].apply_video(stream),
            Err(err) => {
                eprintln!("load video failed: {err}");
                self.docs[index].error = Some(err);
            }
        }
        if let Some(project) = restore {
            self.docs[index].restore_project(&project);
        }
        if std::env::var("PROBE_PLAY").is_ok() && self.active_doc == index {
            self.docs[index].play();
        }
    }

    pub(crate) fn add_video_stream(&mut self, path: PathBuf, stream: VideoStream) {
        let Some((id, _)) = self.begin_load(path) else {
            return;
        };
        self.finish_load(id, Ok(stream));
    }

    pub(crate) fn close_doc(&mut self, index: usize) {
        if index >= self.docs.len() || self.docs[index].is_placeholder() {
            return;
        }
        let old_active = self.active_doc;
        let doc = self.docs.remove(index);
        if let Some((_, image)) = doc.frame_image {
            self.textures_to_drop.push(image);
        }
        self.textures_to_drop.extend(doc.retired);
        self.textures_to_drop.extend(doc.to_drop);

        if self.docs.is_empty() {
            self.docs.push(DocState::placeholder());
            self.active_doc = 0;
        } else if index < old_active {
            self.active_doc = old_active - 1;
        } else if index == old_active {
            self.active_doc = index.min(self.docs.len() - 1);
        } else {
            self.active_doc = old_active.min(self.docs.len() - 1);
        }
        self.drag = None;
    }

    pub(crate) fn activate_doc(&mut self, index: usize) {
        if index >= self.docs.len() || self.docs[index].is_placeholder() {
            return;
        }
        for doc in &mut self.docs {
            doc.pause();
        }
        self.active_doc = index;
        self.drag = None;
    }
}

fn crop_thumbnail(
    source: &Arc<RenderImage>,
    image_width: u32,
    image_height: u32,
    center: (f32, f32),
    mask_width: u32,
    mask_height: u32,
) -> Option<Arc<RenderImage>> {
    let bytes = source.as_bytes(0)?;
    if image_width == 0 || image_height == 0 {
        return None;
    }
    let expected = image_width as usize * image_height as usize * 4;
    if bytes.len() < expected {
        return None;
    }
    let crop_w = mask_width.clamp(3, 99) as i32;
    let crop_h = mask_height.clamp(3, 99) as i32;
    let max_left = image_width as i32 - crop_w;
    let max_top = image_height as i32 - crop_h;
    let left = ((center.0.round() as i32) - crop_w / 2).clamp(0, max_left.max(0));
    let top = ((center.1.round() as i32) - crop_h / 2).clamp(0, max_top.max(0));
    let zoom = 3i32;
    let out_w = crop_w * zoom;
    let out_h = crop_h * zoom;
    let mut out = vec![0u8; out_w as usize * out_h as usize * 4];
    for oy in 0..out_h {
        let sy = top + oy / zoom;
        for ox in 0..out_w {
            let sx = left + ox / zoom;
            let src = ((sy as usize * image_width as usize) + sx as usize) * 4;
            let dst = ((oy as usize * out_w as usize) + ox as usize) * 4;
            out[dst..dst + 4].copy_from_slice(&bytes[src..src + 4]);
        }
    }
    Some(make_image(out, out_w as u32, out_h as u32))
}
